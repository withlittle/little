//! A little plugin that builds a page from each Jupyter notebook, an `.ipynb`
//! file, as it was saved, outputs and all: its cells, in order, each code
//! cell followed by its outputs, become the parts of its page, which little
//! writes as it writes Markdown.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::sync::LazyLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use percent_encoding::percent_decode_str;
use pulldown_cmark::{Event, Options, Parser, Tag};
use regex_lite::Regex;
use serde::Deserialize;
use serde::de::{self, Deserializer};
use serde_json::Value;

/// A part of a notebook's page, as little's `formats` hook gives it back.
#[derive(Debug, PartialEq)]
enum Part {
    /// Markdown, which little reads on its own.
    Markdown(String),
    /// HTML, which little writes as it is.
    Html(String),
    /// Code, which little writes as a code block in its language.
    Code { language: String, text: String },
}

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

/// Check the plugin's settings, `json`, as little gives them to it: it has
/// none, but the `read` every plugin has.
fn check_settings(json: &str) -> Result<(), String> {
    let settings: serde_json::Map<String, Value> = serde_json::from_str(json)
        .map_err(|_| format!("its settings must be a mapping, not {json}"))?;
    match settings.keys().find(|name| *name != "read") {
        Some(name) => Err(format!(
            "it has no setting `{name}`, only `read`, which it doesn't need: little gives it each \
             notebook"
        )),
        None => Ok(()),
    }
}

/// The parts of the page of the notebook `text`: its frontmatter, at the
/// top of the first, then its cells, in order, each code cell followed by
/// its outputs. HTML and SVG are shown only if `markdown`, the site's
/// `markdown` settings, as JSON, let raw HTML in.
fn read(text: &str, markdown: &str) -> Result<Vec<Part>, String> {
    let markdown: Value = serde_json::from_str(markdown)
        .map_err(|e| format!("reading the `markdown` settings: {e}"))?;
    let Notebook {
        nbformat,
        metadata,
        mut cells,
    } = serde_json::from_str(text).map_err(|e| format!("reading the notebook: {e}"))?;
    if nbformat != 4 {
        return Err(format!(
            "it's in version {nbformat} of Jupyter's notebook format, and the plugin reads \
             version 4: open it in Jupyter, and save it, to upgrade it"
        ));
    }
    let mut page = Page {
        parts: vec![Part::Markdown(take_frontmatter(&mut cells))],
        language: metadata.language(),
        raw_html: markdown["raw_html"] == true,
    };
    for cell in cells {
        page.add_cell(cell);
    }
    Ok(page.parts)
}

/// Take the frontmatter from a raw or Markdown cell at the top of the
/// notebook, as Quarto and MyST write it. A raw cell is only frontmatter; a
/// Markdown cell's Markdown goes on after it. With any other cell at the
/// top, it's empty, so a page's frontmatter is only ever there.
fn take_frontmatter(cells: &mut [Cell]) -> String {
    match cells.first_mut() {
        Some(Cell::Raw { source }) => frontmatter(source).to_string(),
        Some(Cell::Markdown { source, .. }) => {
            let frontmatter = frontmatter(source).to_string();
            source.drain(..frontmatter.len());
            frontmatter
        }
        _ => String::new(),
    }
}

/// The frontmatter at the top of `text`, as little finds a page's: a `---`
/// line, the YAML after it, and the next `---` line. Without one, or with
/// one that isn't closed, it's empty.
fn frontmatter(text: &str) -> &str {
    let mut end = 0;
    for (n, line) in text.split_inclusive('\n').enumerate() {
        end += line.len();
        let fence = line.trim() == "---";
        if n == 0 && !fence {
            break;
        }
        if n > 0 && fence {
            return &text[..end];
        }
    }
    ""
}

/// A notebook's page, which its cells are added to, in order.
struct Page {
    parts: Vec<Part>,
    /// The language of the notebook's code.
    language: String,
    /// Whether the site lets raw HTML into its Markdown.
    raw_html: bool,
}

impl Page {
    fn add_cell(&mut self, cell: Cell) {
        match cell {
            // Read on its own, as Jupyter shows each cell, so a code block
            // one leaves open doesn't swallow the next.
            Cell::Markdown {
                source,
                attachments,
            } => (self.parts).push(Part::Markdown(with_attachments(source, &attachments))),
            Cell::Code { source, outputs } => {
                if !source.trim().is_empty() {
                    self.parts.push(code(&self.language, &source));
                }
                self.add_outputs(outputs);
            }
            // A raw cell is for converting the notebook to another format,
            // like LaTeX, not for its page.
            Cell::Raw { .. } => {}
        }
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
        let html = data.get("text/html").filter(|_| self.raw_html);
        let svg = data.get("image/svg+xml").filter(|_| self.raw_html);
        let image = IMAGES
            .iter()
            .find_map(|&mime| Some((mime, data.get(mime)?)));
        if let Some(html) = html {
            self.parts.push(Part::Html(html.clone()));
        } else if let Some(markdown) = data.get("text/markdown") {
            self.parts.push(Part::Markdown(markdown.clone()));
        } else if let Some(svg) = svg {
            self.add_image(&data_url("image/svg+xml", &BASE64.encode(svg)));
        } else if let Some((mime, image)) = image {
            self.add_image(&data_url(mime, image));
        } else if let Some(text) = data.get("text/plain") {
            self.add_text_output(text);
        }
    }

    /// Add text a code cell wrote, or showed, as a code block in `output`.
    fn add_text_output(&mut self, text: &str) {
        let text = as_shown(text);
        if !text.trim().is_empty() {
            self.parts.push(code("output", &text));
        }
    }

    /// Add an image, in a paragraph of its own. Its alt text is empty, as a
    /// notebook doesn't say what an image shows.
    fn add_image(&mut self, url: &str) {
        self.parts.push(Part::Markdown(format!("![]({url})")));
    }
}

/// Code in `language`, without the space at its end.
fn code(language: &str, text: &str) -> Part {
    Part::Code {
        language: language.to_string(),
        text: text.trim_end().to_string(),
    }
}

/// A Markdown cell's `markdown`, with each image attached to it, like
/// `![Chart](attachment:chart.png)`, shown from its `data:` URL.
fn with_attachments(markdown: String, attachments: &BTreeMap<String, Bundle>) -> String {
    if attachments.is_empty() {
        return markdown;
    }
    // With footnotes, which would otherwise be read as links' definitions.
    let mut urls: Vec<_> = Parser::new_ext(&markdown, Options::ENABLE_FOOTNOTES)
        .filter_map(|event| match event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                let data = attached(&dest_url, attachments)?;
                Some((dest_url.into_string(), data))
            }
            _ => None,
        })
        .collect();
    // The longest first, so `attachment:a.png` doesn't change
    // `attachment:a.png.png`.
    urls.sort_by_key(|(url, _)| Reverse(url.len()));
    (urls.into_iter()).fold(markdown, |markdown, (url, data)| {
        markdown.replace(&url, &data)
    })
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

#[cfg(target_arch = "wasm32")]
mod plugin {
    wit_bindgen::generate!({
        path: "../../wit/plugin.wit",
        inline: "
            package little:jupyter;

            world jupyter {
                export little:plugin/setup@0.2.1;
                export little:plugin/formats@0.2.1;
            }
        ",
        world: "little:jupyter/jupyter",
        generate_all,
    });

    use exports::little::plugin::formats::{self, Code, Part};
    use exports::little::plugin::setup;

    struct Plugin;

    impl setup::Guest for Plugin {
        fn setup(settings: String) -> Result<(), String> {
            super::check_settings(&settings)
        }
    }

    impl formats::Guest for Plugin {
        fn extensions() -> Vec<String> {
            vec!["ipynb".to_string()]
        }

        fn read(_: String, text: String, markdown: String) -> Result<Vec<Part>, String> {
            let parts = super::read(&text, &markdown)?.into_iter();
            Ok(parts
                .map(|part| match part {
                    super::Part::Markdown(markdown) => Part::Markdown(markdown),
                    super::Part::Html(html) => Part::Html(html),
                    super::Part::Code { language, text } => Part::Code(Code { language, text }),
                })
                .collect())
        }
    }

    export!(Plugin);
}

#[cfg(test)]
mod tests;
