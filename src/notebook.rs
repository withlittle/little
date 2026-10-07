//! Turn a Jupyter notebook, an `.ipynb` file, into a page, as a Markdown
//! file is turned into one: its cells, in order, each code cell followed by
//! its outputs, become comrak's tree of a Markdown document, which `render`
//! writes as it writes Markdown's.

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use comrak::nodes::{AstNode, NodeCodeBlock, NodeHtmlBlock, NodeLink, NodeValue};
use comrak::{Arena, Options};
use percent_encoding::percent_decode_str;
use regex::Regex;
use serde::Deserialize;
use serde::de::{self, Deserializer};
use serde_json::Value;
use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::sync::LazyLock;
use time::OffsetDateTime;

use crate::config::{Markdown, SiteUrl};
use crate::mermaid;
use crate::plugin::PagePlugins;
use crate::render::{self, Rendered};

/// The types of image, but SVG, that an output can be shown as, in the order
/// Jupyter prefers them. A notebook saves each in base64.
const IMAGES: [&str; 3] = ["image/png", "image/jpeg", "image/gif"];

/// The escape codes a terminal reads, like those that colour a traceback:
/// a CSI sequence, like `\x1b[31m`, an OSC sequence, like a link's, or any
/// other, like `\x1b(B`.
static ESCAPE_CODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07\x1b]*(?:\x07|\x1b\\)|[ -/]*[0-~])").unwrap()
});

/// A notebook, in version 4 of Jupyter's format, with only what its page
/// shows. It's read as leniently as Jupyter reads it: anything else, like a
/// cell's `id`, or a result's `execution_count`, is passed over, and what
/// isn't there is taken as empty.
#[derive(Deserialize)]
struct Notebook {
    nbformat: u64,
    #[serde(default)]
    metadata: Metadata,
    #[serde(default)]
    cells: Vec<Cell>,
}

/// What a notebook says of the language its code is in.
#[derive(Default, Deserialize)]
struct Metadata {
    language_info: Option<LanguageInfo>,
    kernelspec: Option<KernelSpec>,
}

#[derive(Deserialize)]
struct LanguageInfo {
    name: Option<String>,
}

#[derive(Deserialize)]
struct KernelSpec {
    language: Option<String>,
}

impl Metadata {
    /// The language the notebook's code is in, as its kernel says, if it
    /// says.
    fn language(self) -> String {
        (self.language_info.and_then(|info| info.name))
            .or_else(|| self.kernelspec.and_then(|kernel| kernel.language))
            .unwrap_or_default()
    }
}

#[derive(Deserialize)]
#[serde(tag = "cell_type", rename_all = "lowercase")]
enum Cell {
    Markdown {
        #[serde(default, deserialize_with = "text")]
        source: String,
        /// Images pasted into the cell, by name.
        #[serde(default)]
        attachments: BTreeMap<String, Bundle>,
    },
    Code {
        #[serde(default, deserialize_with = "text")]
        source: String,
        #[serde(default)]
        outputs: Vec<Output>,
    },
    Raw {
        #[serde(default, deserialize_with = "text")]
        source: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "output_type", rename_all = "snake_case")]
enum Output {
    /// Text the code wrote, to `stdout` or `stderr`.
    Stream {
        #[serde(default)]
        name: String,
        #[serde(default, deserialize_with = "text")]
        text: String,
    },
    /// What the code showed, or the value of its last line, in as many
    /// types as it could.
    #[serde(rename = "display_data", alias = "execute_result")]
    Display {
        #[serde(default)]
        data: Bundle,
    },
    /// An error the code raised.
    Error {
        #[serde(default)]
        ename: String,
        #[serde(default)]
        evalue: String,
        #[serde(default)]
        traceback: Vec<String>,
    },
}

/// Data in each type it's in, like `text/plain` and `image/png`, as text.
/// Data that isn't text, like JSON's, is passed over.
#[derive(Default, Deserialize)]
#[serde(from = "BTreeMap<String, Value>")]
struct Bundle(BTreeMap<String, String>);

impl From<BTreeMap<String, Value>> for Bundle {
    fn from(data: BTreeMap<String, Value>) -> Self {
        let text = (data.into_iter()).filter_map(|(mime, data)| Some((mime, text_of(data)?)));
        Self(text.collect())
    }
}

/// A cell's source, or a stream's text, as `text_of` reads it.
fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    text_of(Value::deserialize(deserializer)?)
        .ok_or_else(|| de::Error::custom("expected text: a string, or a list of strings"))
}

/// Text, which Jupyter saves as one string, or as a list of lines, each
/// with its `\n`.
fn text_of(value: Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text),
        Value::Array(lines) => lines.iter().map(Value::as_str).collect(),
        _ => None,
    }
}

/// Parse a notebook, `raw`, as `render::parse_document` parses Markdown:
/// into its frontmatter, rendered HTML, and outline, with its links and
/// images led from `folder`, the URL of the folder it's in, the code blocks
/// in its Markdown that `plugins` render rendered, its diagrams drawn, as of
/// `now`, and its code highlighted, as `markdown` says.
pub fn parse_notebook(
    raw: &str,
    folder: &str,
    toc_depths: &RangeInclusive<u8>,
    url: &SiteUrl,
    markdown: &Markdown,
    now: OffsetDateTime,
    plugins: &mut PagePlugins<'_>,
) -> Result<Rendered> {
    let Notebook {
        nbformat,
        metadata,
        mut cells,
    } = serde_json::from_str(raw).context("reading the notebook")?;
    if nbformat != 4 {
        bail!(
            "it's in version {nbformat} of Jupyter's notebook format, and little reads \
             version 4: open it in Jupyter, and save it, to upgrade it"
        );
    }
    let frontmatter = render::parse_frontmatter(&take_frontmatter(&mut cells))?;

    let options = render::render_options(markdown);
    let arena = Arena::new();
    let mut page = Page {
        arena: &arena,
        root: arena.alloc(NodeValue::Document.into()),
        options: &options,
        language: metadata.language(),
        footnotes: 0,
        inline_footnotes: 0,
        markdown_blocks: Vec::new(),
    };
    for cell in cells {
        page.add_cell(cell);
    }
    let markdown_blocks = std::mem::take(&mut page.markdown_blocks);
    let root = page.finish();
    // A block's or a diagram's line would be its cell's, not the notebook
    // file's.
    let unrendered_blocks = plugins.render(&arena, markdown_blocks, &options, None)?;
    let unrendered_diagrams = mermaid::draw(root, markdown, now, None);
    let rendered = render::render_tree(
        root,
        frontmatter,
        folder,
        toc_depths,
        url,
        markdown,
        &options,
    )?;
    Ok(Rendered {
        unrendered_diagrams,
        unrendered_blocks,
        ..rendered
    })
}

/// Take the frontmatter from a raw or Markdown cell at the top of the
/// notebook, as Quarto and MyST write it. A raw cell is only frontmatter; a
/// Markdown cell's Markdown goes on after it.
fn take_frontmatter(cells: &mut [Cell]) -> String {
    match cells.first_mut() {
        Some(Cell::Raw { source }) => render::split_frontmatter(source).0,
        Some(Cell::Markdown { source, .. }) => {
            let (frontmatter, body) = render::split_frontmatter(source);
            *source = body;
            frontmatter
        }
        _ => String::new(),
    }
}

/// The tree of a notebook's page, which its cells are added to, in order.
struct Page<'a> {
    arena: &'a Arena<'a>,
    root: &'a AstNode<'a>,
    options: &'a Options<'a>,
    /// The language of the notebook's code.
    language: String,
    /// How many footnotes the page has so far.
    footnotes: u32,
    /// How many names inline footnotes, `^[like this]`, have taken so far.
    inline_footnotes: u32,
    /// The code blocks in its Markdown, for plugins: not its code cells, nor
    /// what they wrote.
    markdown_blocks: Vec<&'a AstNode<'a>>,
}

impl<'a> Page<'a> {
    fn add_cell(&mut self, cell: Cell) {
        match cell {
            Cell::Markdown {
                source,
                attachments,
            } => self.add_markdown(&source, &attachments),
            Cell::Code { source, outputs } => {
                if !source.trim().is_empty() {
                    self.add_code(&self.language, &source);
                }
                self.add_outputs(outputs);
            }
            // A raw cell is for converting the notebook to another format,
            // like LaTeX, not for its page.
            Cell::Raw { .. } => {}
        }
    }

    /// Add Markdown, read on its own, as Jupyter shows each cell, so a code
    /// block one leaves open doesn't swallow the next. An image attached to
    /// it is shown from its `data:` URL.
    fn add_markdown(&mut self, markdown: &str, attachments: &BTreeMap<String, Bundle>) {
        let document = comrak::parse_document(self.arena, markdown, self.options);
        let mut footnotes = 0;
        let mut inline_footnotes = 0;
        for node in document.descendants() {
            match &mut node.data.borrow_mut().value {
                NodeValue::Image(image) => {
                    if let Some(url) = attached(&image.url, attachments) {
                        image.url = url;
                    }
                }
                // Numbered on from the footnotes before, as the page lists
                // them all at its end.
                NodeValue::FootnoteReference(reference) => {
                    reference.ix += self.footnotes;
                    let n = self.rename_inline_footnote(&mut reference.name);
                    inline_footnotes = inline_footnotes.max(n);
                }
                NodeValue::FootnoteDefinition(definition) => {
                    footnotes += 1;
                    let n = self.rename_inline_footnote(&mut definition.name);
                    inline_footnotes = inline_footnotes.max(n);
                }
                NodeValue::CodeBlock(_) => self.markdown_blocks.push(node),
                _ => {}
            }
        }
        self.footnotes += footnotes;
        self.inline_footnotes += inline_footnotes;
        self.root.extend(document.children().collect::<Vec<_>>());
    }

    /// Name an inline footnote on from those in the cells before, as comrak
    /// names those in each cell `__inline_1`, `__inline_2`, and so on, and
    /// give its number in its cell: 0 for a footnote that isn't inline.
    fn rename_inline_footnote(&self, name: &mut String) -> u32 {
        let Some(n) = (name.strip_prefix("__inline_")).and_then(|n| n.parse::<u32>().ok()) else {
            return 0;
        };
        *name = format!("__inline_{}", n + self.inline_footnotes);
        n
    }

    /// Add a code block of `code` in `language`.
    fn add_code(&self, language: &str, code: &str) {
        self.add(NodeValue::CodeBlock(Box::new(NodeCodeBlock {
            fenced: true,
            fence_char: b'`',
            fence_length: 3,
            info: language.to_string(),
            literal: format!("{}\n", code.trim_end()),
            closed: true,
            ..NodeCodeBlock::default()
        })));
    }

    /// Add a code cell's outputs, in order.
    fn add_outputs(&mut self, outputs: Vec<Output>) {
        let mut outputs = outputs.into_iter().peekable();
        while let Some(output) = outputs.next() {
            match output {
                Output::Stream { name, mut text } => {
                    // What's written to a stream can be saved in parts.
                    let same_stream = |output: &Output| matches!(output, Output::Stream { name: next, .. } if *next == name);
                    while let Some(Output::Stream { text: more, .. }) = outputs.next_if(same_stream)
                    {
                        text.push_str(&more);
                    }
                    self.add_text_output(&text);
                }
                Output::Display { data } => self.add_display(data),
                Output::Error {
                    ename,
                    evalue,
                    traceback,
                } => {
                    if traceback.is_empty() {
                        self.add_text_output(&format!("{ename}: {evalue}"));
                    } else {
                        self.add_text_output(&traceback.join("\n"));
                    }
                }
            }
        }
    }

    /// Add what a code cell showed, as the first of its types that a page
    /// can show, in the order Jupyter prefers them. HTML and SVG are shown
    /// only when raw HTML in Markdown is: comrak leaves out the `data:` URL
    /// of SVG, which can hold script, unless it is.
    fn add_display(&mut self, Bundle(data): Bundle) {
        let raw_html = self.options.render.r#unsafe;
        let html = data.get("text/html").filter(|_| raw_html);
        let svg = data.get("image/svg+xml").filter(|_| raw_html);
        let image = IMAGES
            .iter()
            .find_map(|&mime| Some((mime, data.get(mime)?)));
        if let Some(html) = html {
            self.add(NodeValue::HtmlBlock(NodeHtmlBlock {
                block_type: 6,
                literal: format!("{}\n", html.trim_end()),
            }));
        } else if let Some(markdown) = data.get("text/markdown") {
            self.add_markdown(markdown, &BTreeMap::new());
        } else if let Some(svg) = svg {
            self.add_image(data_url("image/svg+xml", &BASE64.encode(svg)));
        } else if let Some((mime, image)) = image {
            self.add_image(data_url(mime, image));
        } else if let Some(text) = data.get("text/plain") {
            self.add_text_output(text);
        }
    }

    /// Add text a code cell wrote, or showed, as a code block in `output`.
    fn add_text_output(&self, text: &str) {
        let text = as_shown(text);
        if !text.trim().is_empty() {
            self.add_code("output", &text);
        }
    }

    /// Add an image, in a paragraph of its own. Its alt text is empty, as a
    /// notebook doesn't say what an image shows.
    fn add_image(&self, url: String) {
        let link = NodeLink {
            url,
            title: String::new(),
        };
        let image = self.arena.alloc(NodeValue::Image(Box::new(link)).into());
        self.add(NodeValue::Paragraph).append(image);
    }

    /// Add a block to the end of the page.
    fn add(&self, block: NodeValue) -> &'a AstNode<'a> {
        let node = self.arena.alloc(block.into());
        self.root.append(node);
        node
    }

    /// The page's tree, with each cell's footnotes moved to its end, where
    /// comrak writes them in one list.
    fn finish(self) -> &'a AstNode<'a> {
        let footnotes: Vec<_> = (self.root.children())
            .filter(|node| matches!(node.data.borrow().value, NodeValue::FootnoteDefinition(_)))
            .collect();
        self.root.extend(footnotes);
        self.root
    }
}

/// The `data:` URL of the image attached to a cell that `url` leads to,
/// like `attachment:chart.png`, by its name as it's written, or as Jupyter
/// writes it, percent-encoded. `None` for any other URL.
fn attached(url: &str, attachments: &BTreeMap<String, Bundle>) -> Option<String> {
    let name = url.strip_prefix("attachment:")?;
    let Bundle(image) = (attachments.get(name))
        .or_else(|| attachments.get(&*percent_decode_str(name).decode_utf8().ok()?))?;
    let (mime, data) = image.first_key_value()?;
    Some(data_url(mime, data))
}

/// The `data:` URL of data in base64, which a notebook can save in lines.
fn data_url(mime: &str, base64: &str) -> String {
    let mut url = format!("data:{mime};base64,");
    url.extend(base64.chars().filter(|c| !c.is_ascii_whitespace()));
    url
}

/// Text as a terminal shows it: without the escape codes that colour it,
/// and with each line as it was drawn last, as a progress bar draws over its
/// line after a `\r`.
fn as_shown(text: &str) -> String {
    let text = ESCAPE_CODE.replace_all(text, "");
    let mut shown = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (line, end) = line
            .strip_suffix('\n')
            .map_or((line, ""), |line| (line, "\n"));
        // A `\r` that ends a line, as in `\r\n`, draws over nothing.
        let line = line.trim_end_matches('\r');
        shown.push_str(line.rsplit('\r').next().unwrap_or_default());
        shown.push_str(end);
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// little's defaults, from `src/little.yml`: every heading but the title.
    const TOC_DEPTHS: RangeInclusive<u8> = 2..=6;

    /// A notebook of `cells`, with Python's metadata, as Jupyter saves it.
    fn notebook(cells: Value) -> String {
        json!({
            "cells": cells,
            "metadata": {
                "kernelspec": {
                    "display_name": "Python 3",
                    "language": "python",
                    "name": "python3"
                },
                "language_info": { "name": "python", "version": "3.12.4" }
            },
            "nbformat": 4,
            "nbformat_minor": 5
        })
        .to_string()
    }

    fn parse(raw: &str, markdown: &Markdown) -> Result<Rendered> {
        parse_notebook(
            raw,
            "/",
            &TOC_DEPTHS,
            &SiteUrl::default(),
            markdown,
            OffsetDateTime::UNIX_EPOCH,
            &mut PagePlugins::none(),
        )
    }

    /// The page of a notebook of `cells`, without highlighting, so its HTML
    /// is plain to compare.
    fn html(cells: Value) -> String {
        let markdown = Markdown {
            highlight: None,
            ..Markdown::default()
        };
        parse(&notebook(cells), &markdown).unwrap().html
    }

    fn markdown(source: &str) -> Value {
        json!({ "cell_type": "markdown", "id": "a1", "metadata": {}, "source": source })
    }

    fn code(source: &str, outputs: Value) -> Value {
        json!({
            "cell_type": "code",
            "execution_count": 1,
            "id": "b2",
            "metadata": {},
            "outputs": outputs,
            "source": source
        })
    }

    fn stream(name: &str, text: &str) -> Value {
        json!({ "output_type": "stream", "name": name, "text": text })
    }

    fn display(data: Value) -> Value {
        json!({ "output_type": "display_data", "data": data, "metadata": {} })
    }

    #[test]
    fn cells_and_outputs_are_a_page() {
        let cells = json!([
            markdown("What the data says."),
            code(
                "print(\"Hello\")\nx = 6 * 7\nx\n",
                json!([
                    stream("stdout", "Hello\n"),
                    {
                        "output_type": "execute_result",
                        "data": { "text/plain": "42" },
                        "execution_count": 1,
                        "metadata": {}
                    }
                ])
            ),
        ]);
        assert_eq!(
            html(cells),
            "<p>What the data says.</p>\n\
             <pre><code class=\"language-python\">print(&quot;Hello&quot;)\nx = 6 * 7\nx\n</code></pre>\n\
             <pre><code class=\"language-output\">Hello\n</code></pre>\n\
             <pre><code class=\"language-output\">42\n</code></pre>\n"
        );
    }

    #[test]
    fn text_is_a_string_or_a_list_of_lines() {
        let lines = json!({
            "cell_type": "markdown",
            "metadata": {},
            "source": ["# Analysis\n", "\n", "What the data says."]
        });
        assert_eq!(
            html(json!([lines])),
            html(json!([markdown("# Analysis\n\nWhat the data says.")]))
        );
    }

    #[test]
    fn the_first_h1_and_the_headings_are_the_pages() {
        let cells = json!([
            markdown("# Analysis"),
            markdown("## Loading the data"),
            code("1", json!([])),
            markdown("## Results"),
        ]);
        let rendered = parse(&notebook(cells), &Markdown::default()).unwrap();
        assert_eq!(rendered.first_h1.as_deref(), Some("Analysis"));
        let toc: Vec<_> = rendered.toc.iter().map(|item| &item.anchor).collect();
        assert_eq!(toc, ["#loading-the-data", "#results"]);
    }

    #[test]
    fn each_markdown_cell_is_read_on_its_own() {
        // As Jupyter shows each cell: a code block left open ends with its
        // cell.
        let cells = json!([markdown("```python\nprint(1)"), markdown("After")]);
        assert_eq!(
            html(cells),
            "<pre><code class=\"language-python\">print(1)</code></pre>\n<p>After</p>\n"
        );
    }

    #[test]
    fn diagrams_are_drawn_through_the_notebook() {
        let flowchart = markdown("```mermaid\nflowchart LR\n  A --> B\n```");
        let cells = json!([
            flowchart,
            markdown("```mermaid\nflowchat LR\n```"),
            flowchart
        ]);
        let r = parse(&notebook(cells), &Markdown::default()).unwrap();
        assert!(r.html.starts_with("<svg id=\"mermaid-1\" "), "{}", r.html);
        let code = "<code class=\"language-mermaid\">flowchat LR\n</code>";
        assert!(r.html.contains(code), "{}", r.html);
        assert!(r.html.contains("<svg id=\"mermaid-3\" "), "{}", r.html);
        // Without a line, which would be its cell's.
        let unrendered = mermaid::Unrendered {
            line: None,
            reason: "No diagram type detected".to_string(),
        };
        assert_eq!(r.unrendered_diagrams, [unrendered]);
    }

    #[test]
    fn code_is_in_the_notebooks_language() {
        let page = |metadata: Value| {
            let raw = json!({
                "cells": [code("x <- 1", json!([]))],
                "metadata": metadata,
                "nbformat": 4,
                "nbformat_minor": 2
            });
            parse(&raw.to_string(), &Markdown::default()).unwrap().html
        };
        let r = "<pre class=\"highlight\"><code class=\"language-R\">";
        // As its language says, or else its kernel.
        let language_info =
            json!({ "language_info": { "name": "R" }, "kernelspec": { "language": "python" } });
        assert!(page(language_info).starts_with(r));
        assert!(page(json!({ "kernelspec": { "language": "R" } })).starts_with(r));
        assert!(page(json!({})).starts_with("<pre class=\"highlight\"><code>x"));
    }

    #[test]
    fn an_empty_code_cell_is_left_out() {
        let cells = json!([code("\n  \n", json!([stream("stdout", "Hi\n")]))]);
        assert_eq!(
            html(cells),
            "<pre><code class=\"language-output\">Hi\n</code></pre>\n"
        );
    }

    #[test]
    fn a_stream_is_shown_as_a_terminal_shows_it() {
        let cells = json!([code(
            "train()",
            json!([
                stream("stdout", "Epoch 1\n"),
                // A progress bar, drawn over its line, and in colour.
                stream("stdout", "10%\r50%\r100%\r\n\x1b[32mDone\x1b[0m\r\n"),
                stream("stderr", "\x1b[1;31mWarning\x1b[0m: slow\n"),
                stream("stdout", "\n"),
            ])
        )]);
        assert_eq!(
            html(cells),
            "<pre><code class=\"language-python\">train()\n</code></pre>\n\
             <pre><code class=\"language-output\">Epoch 1\n100%\nDone\n</code></pre>\n\
             <pre><code class=\"language-output\">Warning: slow\n</code></pre>\n"
        );
    }

    #[test]
    fn an_error_shows_its_traceback() {
        let error = |traceback: Value| {
            let error = json!({
                "output_type": "error",
                "ename": "ZeroDivisionError",
                "evalue": "division by zero",
                "traceback": traceback
            });
            html(json!([code("1 / 0", json!([error]))]))
        };
        let traceback = json!([
            "\x1b[0;31m-----------\x1b[0m",
            "\x1b[0;31mZeroDivisionError\x1b[0m: division by zero"
        ]);
        let shown = "<pre><code class=\"language-output\">-----------\n\
                     ZeroDivisionError: division by zero\n</code></pre>\n";
        assert!(error(traceback).ends_with(shown));
        assert!(error(json!([])).ends_with(shown.replace("-----------\n", "").as_str()));
    }

    #[test]
    fn html_is_shown_when_markdown_lets_raw_html_in() {
        // As pandas shows a table, with a blank line that would end raw HTML
        // in Markdown.
        let table = "<div>\n<style scoped>\n    .dataframe { color: red; }\n\n</style>\n\
                     <table class=\"dataframe\"><tr><td>1</td></tr></table>\n</div>";
        let cells = json!([code(
            "df",
            json!([display(
                json!({ "text/html": table, "text/plain": "   a\n0  1" })
            )])
        )]);
        assert!(html(cells.clone()).ends_with(&format!("</code></pre>\n{table}\n")));

        let markdown = Markdown {
            raw_html: false,
            highlight: None,
            ..Markdown::default()
        };
        let html = parse(&notebook(cells), &markdown).unwrap().html;
        assert!(
            html.ends_with("<pre><code class=\"language-output\">   a\n0  1\n</code></pre>\n"),
            "{html}"
        );
    }

    #[test]
    fn markdown_is_shown_as_markdown() {
        let cells = json!([code(
            "Markdown(\"**Bold**\")",
            json!([display(
                json!({ "text/markdown": "**Bold**", "text/plain": "<Markdown>" })
            )])
        )]);
        assert!(html(cells).ends_with("<p><strong>Bold</strong></p>\n"));
    }

    #[test]
    fn an_image_is_in_the_page() {
        let image = |data: Value| html(json!([code("plot()", json!([display(data)]))]));
        let png = "<p><img src=\"data:image/png;base64,iVBORw0KGgo=\" alt=\"\" /></p>\n";
        // As Jupyter saves it, with a line's end, with text for a terminal.
        let data = json!({ "image/png": "iVBORw0K\nGgo=\n", "text/plain": "<Figure>" });
        assert!(image(data).ends_with(png));
        let data = json!({ "image/jpeg": "/9j/4AAQ", "image/png": "iVBORw0KGgo=" });
        assert!(image(data).ends_with(png));
        let data = json!({ "image/gif": "R0lGODlh" });
        assert!(
            image(data).ends_with("<img src=\"data:image/gif;base64,R0lGODlh\" alt=\"\" /></p>\n")
        );
    }

    #[test]
    fn svg_is_shown_when_markdown_lets_raw_html_in() {
        let image = |markdown: &Markdown, data: Value| {
            let cells = json!([code("plot()", json!([display(data)]))]);
            parse(&notebook(cells), markdown).unwrap().html
        };
        // SVG is saved as text, and preferred.
        let markdown = Markdown {
            highlight: None,
            ..Markdown::default()
        };
        let data = json!({ "image/svg+xml": ["<svg>", "</svg>"], "image/png": "iVBORw0KGgo=" });
        let svg = format!(
            "<img src=\"data:image/svg+xml;base64,{}\"",
            BASE64.encode("<svg></svg>")
        );
        assert!(image(&markdown, data.clone()).contains(&svg));

        let markdown = Markdown {
            raw_html: false,
            ..markdown
        };
        let png = "<p><img src=\"data:image/png;base64,iVBORw0KGgo=\" alt=\"\" /></p>\n";
        assert!(image(&markdown, data).ends_with(png));
        let data = json!({ "image/svg+xml": "<svg></svg>", "text/plain": "<Figure>" });
        assert!(
            image(&markdown, data)
                .ends_with("<pre><code class=\"language-output\">&lt;Figure&gt;\n</code></pre>\n")
        );
    }

    #[test]
    fn what_a_page_cant_show_falls_back_to_text() {
        let shown = |data: Value| html(json!([code("show()", json!([display(data)]))]));
        let data = json!({
            "application/javascript": "alert(1)",
            "application/vnd.jupyter.widget-view+json": { "model_id": "1a" },
            "text/latex": "$x^2$",
            "text/plain": "x**2"
        });
        assert!(
            shown(data).ends_with("<pre><code class=\"language-output\">x**2\n</code></pre>\n")
        );
        // And without text, there's nothing to show.
        let data = json!({ "application/vnd.jupyter.widget-view+json": { "model_id": "1a" } });
        assert_eq!(
            shown(data),
            "<pre><code class=\"language-python\">show()\n</code></pre>\n"
        );
    }

    #[test]
    fn an_attached_image_is_in_the_page() {
        let cell = json!({
            "cell_type": "markdown",
            "metadata": {},
            "source": "![Chart](attachment:chart.png) ![Map](attachment:the%20map.png) \
                       ![Lost](attachment:lost.png) ![Logo](logo.png)",
            "attachments": {
                "chart.png": { "image/png": "iVBORw0KGgo=\n" },
                "the map.png": { "image/jpeg": "/9j/4AAQ" }
            }
        });
        assert_eq!(
            html(json!([cell])),
            "<p><img src=\"data:image/png;base64,iVBORw0KGgo=\" alt=\"Chart\" /> \
             <img src=\"data:image/jpeg;base64,/9j/4AAQ\" alt=\"Map\" /> \
             <img src=\"attachment:lost.png\" alt=\"Lost\" /> \
             <img src=\"/logo.png\" alt=\"Logo\" /></p>\n"
        );
    }

    #[test]
    fn frontmatter_is_in_a_raw_or_markdown_cell_at_the_top() {
        let title = |cells: Value| {
            let rendered = parse(&notebook(cells), &Markdown::default()).unwrap();
            let title = rendered
                .frontmatter
                .get("title")
                .and_then(|title| title.as_str());
            (title.map(str::to_string), rendered.html)
        };
        let raw = |source: &str| json!({ "cell_type": "raw", "metadata": {}, "source": source });

        // A raw cell is only frontmatter, as Quarto writes it.
        let (frontmatter, html) = title(json!([
            raw("---\ntitle: Analysis\n---\nFor LaTeX"),
            markdown("Body")
        ]));
        assert_eq!(frontmatter.as_deref(), Some("Analysis"));
        assert_eq!(html, "<p>Body</p>\n");

        // A Markdown cell goes on after it, as MyST writes it.
        let (frontmatter, html) = title(json!([markdown("---\ntitle: Analysis\n---\nBody")]));
        assert_eq!(frontmatter.as_deref(), Some("Analysis"));
        assert_eq!(html, "<p>Body</p>\n");

        // Anywhere else, it's Markdown, and a raw cell is left out.
        let (frontmatter, html) = title(json!([
            code("1", json!([])),
            markdown("---\ntitle: Analysis\n---"),
            raw("---\ntitle: Raw\n---"),
        ]));
        assert_eq!(frontmatter, None);
        assert!(html.contains("<hr />\n<h2 id=\"title-analysis\">title: Analysis"));
        assert!(!html.contains("Raw"), "{html}");
    }

    /// The HTML comrak writes for the `n`th footnote's reference, to the
    /// footnote `name`.
    fn footnote_reference(name: &str, n: u8) -> String {
        format!(
            "<sup class=\"footnote-ref\"><a href=\"#fn-{name}\" id=\"fnref-{name}\" \
             data-footnote-ref>{n}</a></sup>"
        )
    }

    /// The HTML comrak writes for the `n`th footnote, `name`, of `text`.
    fn footnote(name: &str, n: u8, text: &str) -> String {
        format!(
            "<li id=\"fn-{name}\">\n<p>{text} <a href=\"#fnref-{name}\" \
             class=\"footnote-backref\" data-footnote-backref \
             data-footnote-backref-idx=\"{n}\" aria-label=\"Back to reference {n}\">↩</a></p>\n\
             </li>\n"
        )
    }

    #[test]
    fn footnotes_are_numbered_through_the_page_and_listed_at_its_end() {
        let cells = json!([
            markdown("One.[^a]\n\n[^a]: First."),
            markdown("Two.[^b] Three.[^c]\n\n[^c]: Third.\n\n[^b]: Second."),
            markdown("The end."),
        ]);
        assert_eq!(
            html(cells),
            format!(
                "<p>One.{}</p>\n<p>Two.{} Three.{}</p>\n<p>The end.</p>\n\
                 <section class=\"footnotes\" data-footnotes>\n<ol>\n{}{}{}</ol>\n</section>\n",
                footnote_reference("a", 1),
                footnote_reference("b", 2),
                footnote_reference("c", 3),
                footnote("a", 1, "First."),
                footnote("b", 2, "Second."),
                footnote("c", 3, "Third."),
            )
        );
    }

    #[test]
    fn inline_footnotes_are_named_through_the_page() {
        // comrak names those in each cell from `__inline_1`.
        let options = Markdown {
            inline_footnotes: true,
            highlight: None,
            ..Markdown::default()
        };
        let cells = json!([markdown("One.^[First.]"), markdown("Two.^[Second.]")]);
        assert_eq!(
            parse(&notebook(cells), &options).unwrap().html,
            format!(
                "<p>One.{}</p>\n<p>Two.{}</p>\n\
                 <section class=\"footnotes\" data-footnotes>\n<ol>\n{}{}</ol>\n</section>\n",
                footnote_reference("__inline_1", 1),
                footnote_reference("__inline_2", 2),
                footnote("__inline_1", 1, "First."),
                footnote("__inline_2", 2, "Second."),
            )
        );

        // A footnote of only space is left as text, but still takes a name.
        let cells = json!([markdown("One.^[ ]^[First.]"), markdown("Two.^[Second.]")]);
        let html = parse(&notebook(cells), &options).unwrap().html;
        let names: Vec<_> = (html.split("<li id=\"").skip(1))
            .map(|rest| rest.split('"').next().unwrap())
            .collect();
        assert_eq!(names.len(), 2, "{html}");
        assert_ne!(names[0], names[1], "{html}");
    }

    #[test]
    fn a_notebook_is_read_as_leniently_as_jupyter_reads_it() {
        // As `nbstripout --keep-output` saves a result, older notebooks save
        // cells without `id`s, and other tools leave out what's empty.
        let raw = json!({
            "cells": [
                { "cell_type": "markdown", "source": "# Analysis", "id": "a.b" },
                { "cell_type": "markdown", "source": "", "id": "a.b" },
                { "cell_type": "code", "source": "x" },
                {
                    "cell_type": "code",
                    "source": "x",
                    "outputs": [
                        { "output_type": "execute_result", "data": { "text/plain": "1" }, "execution_count": null },
                        { "output_type": "display_data", "data": { "application/json": { "a": 1 } } },
                        { "output_type": "stream", "text": "Hi" }
                    ],
                    "execution_count": null
                }
            ],
            "nbformat": 4,
            "nbformat_minor": 6
        });
        let html = parse(&raw.to_string(), &Markdown::default()).unwrap().html;
        assert!(
            html.contains("<code class=\"language-output\">1\n</code>"),
            "{html}"
        );
        assert!(
            html.contains("<code class=\"language-output\">Hi\n</code>"),
            "{html}"
        );
    }

    #[test]
    fn a_file_that_isnt_a_notebook_little_reads_is_an_error() {
        let error = |raw: &str| format!("{:#}", parse(raw, &Markdown::default()).err().unwrap());
        assert_eq!(
            error(r#"{ "nbformat": 3, "nbformat_minor": 0, "metadata": {}, "worksheets": [] }"#),
            "it's in version 3 of Jupyter's notebook format, and little reads version 4: \
             open it in Jupyter, and save it, to upgrade it"
        );
        assert!(error("# Analysis").starts_with("reading the notebook: expected value at line 1"));
        let error = error(r#"{ "nbformat": 4, "cells": [{ "cell_type": "heading" }] }"#);
        assert!(error.contains("unknown variant `heading`"), "{error}");
    }

    #[test]
    fn output_isnt_highlighted() {
        let cells = json!([code("1", json!([stream("stdout", "def main(): pass\n")]))]);
        let html = parse(&notebook(cells), &Markdown::default()).unwrap().html;
        assert!(html.ends_with(
            "<pre class=\"highlight\"><code class=\"language-output\">def main(): pass\n</code></pre>\n"
        ), "{html}");
    }

    #[test]
    fn the_terminal_shows_text_as_it_was_drawn_last() {
        for (text, shown) in [
            ("plain\n", "plain\n"),
            ("a\r\nb\r\n", "a\nb\n"),
            ("1/3\r2/3\r3/3", "3/3"),
            ("1/3\r3/3\r\r\n", "3/3\n"),
            ("\x1b[1m\x1b[31mred\x1b[0m", "red"),
            ("\x1b]8;;https://example.com\x07link\x1b]8;;\x07", "link"),
            ("\x1b(B\x1b[mreset", "reset"),
        ] {
            assert_eq!(as_shown(text), shown, "{text:?}");
        }
    }
}
