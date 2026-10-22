//! Turn the parts a plugin's `formats` hook reads a file into, like a
//! Jupyter notebook, into a page, as a Markdown file is turned into one:
//! its parts, in order, become comrak's tree of a Markdown document, which
//! `render` writes as it writes Markdown's.

use anyhow::{Context, Result};
use comrak::nodes::{AstNode, NodeCodeBlock, NodeHtmlBlock, NodeValue};
use comrak::{Arena, Options};
use std::ops::RangeInclusive;
use time::OffsetDateTime;

use crate::config::{self, Markdown, SiteUrl};
use crate::mermaid;
use crate::plugin::{Code, PagePlugins, Part};
use crate::render::{self, Rendered};

/// Parse `parts`, as `render::parse_document` parses Markdown: into the
/// frontmatter at the top of the first, if it's Markdown, rendered HTML,
/// and outline, with its links and images led from `folder`, the URL of the
/// folder its file is in, the code blocks in its Markdown that `plugins`
/// render rendered, its diagrams drawn, as of `now`, and its code
/// highlighted, as `markdown` says.
pub fn parse_parts(
    mut parts: Vec<Part>,
    folder: &str,
    toc_depths: &RangeInclusive<u8>,
    url: &SiteUrl,
    markdown: &Markdown,
    now: OffsetDateTime,
    plugins: &mut PagePlugins<'_>,
) -> Result<Rendered> {
    let frontmatter = match parts.first_mut() {
        Some(Part::Markdown(first)) => {
            let (frontmatter, body) = render::split_frontmatter(first);
            *first = body;
            frontmatter
        }
        _ => String::new(),
    };
    let frontmatter = config::parse_config(&frontmatter).context("parsing frontmatter")?;

    let options = render::render_options(markdown);
    let arena = Arena::new();
    let mut page = Page {
        arena: &arena,
        root: arena.alloc(NodeValue::Document.into()),
        options: &options,
        footnotes: 0,
        inline_footnotes: 0,
        markdown_blocks: Vec::new(),
    };
    for part in parts {
        page.add_part(part);
    }
    let markdown_blocks = std::mem::take(&mut page.markdown_blocks);
    let root = page.finish();
    // A block's or a diagram's line would be its part's, not its file's.
    let unrendered_blocks = plugins.render(&arena, markdown_blocks, &options, None)?;
    let unrendered_diagrams = mermaid::draw(root, markdown, now, None);
    let rendered = render::render_tree(root, folder, toc_depths, url, markdown, &options, plugins)?;
    Ok(Rendered {
        frontmatter,
        unrendered_diagrams,
        unrendered_blocks,
        ..rendered
    })
}

/// The tree of a page, which its parts are added to, in order.
struct Page<'a> {
    arena: &'a Arena<'a>,
    root: &'a AstNode<'a>,
    options: &'a Options<'a>,
    /// How many footnotes the page has so far.
    footnotes: u32,
    /// How many names inline footnotes, `^[like this]`, have taken so far.
    inline_footnotes: u32,
    /// The code blocks in its Markdown, for plugins: not its code parts.
    markdown_blocks: Vec<&'a AstNode<'a>>,
}

impl<'a> Page<'a> {
    fn add_part(&mut self, part: Part) {
        match part {
            Part::Markdown(markdown) => self.add_markdown(&markdown),
            // Whole, as raw HTML in Markdown would end at a blank line, as
            // in the `<style>` pandas writes before a table.
            Part::Html(html) => self.add(NodeValue::HtmlBlock(NodeHtmlBlock {
                block_type: 6,
                literal: format!("{}\n", html.trim_end()),
            })),
            Part::Code(Code { language, mut text }) => {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                self.add(NodeValue::CodeBlock(Box::new(NodeCodeBlock {
                    fenced: true,
                    fence_char: b'`',
                    fence_length: 3,
                    info: language,
                    literal: text,
                    closed: true,
                    ..NodeCodeBlock::default()
                })));
            }
        }
    }

    /// Add Markdown, read on its own, so a code block one part leaves open
    /// doesn't swallow the next.
    fn add_markdown(&mut self, markdown: &str) {
        let document = comrak::parse_document(self.arena, markdown, self.options);
        let mut footnotes = 0;
        let mut inline_footnotes = 0;
        for node in document.descendants() {
            match &mut node.data.borrow_mut().value {
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

    /// Name an inline footnote on from those in the parts before, as comrak
    /// names those in each part `__inline_1`, `__inline_2`, and so on, and
    /// give its number in its part: 0 for a footnote that isn't inline.
    fn rename_inline_footnote(&self, name: &mut String) -> u32 {
        let Some(n) = (name.strip_prefix("__inline_")).and_then(|n| n.parse::<u32>().ok()) else {
            return 0;
        };
        *name = format!("__inline_{}", n + self.inline_footnotes);
        n
    }

    /// Add a block to the end of the page.
    fn add(&self, block: NodeValue) {
        self.root.append(self.arena.alloc(block.into()));
    }

    /// The page's tree, with each part's footnotes moved to its end, where
    /// comrak writes them in one list.
    fn finish(self) -> &'a AstNode<'a> {
        let footnotes: Vec<_> = (self.root.children())
            .filter(|node| matches!(node.data.borrow().value, NodeValue::FootnoteDefinition(_)))
            .collect();
        self.root.extend(footnotes);
        self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// little's defaults, from `src/little.yml`: every heading but the title.
    const TOC_DEPTHS: RangeInclusive<u8> = 2..=6;

    fn parse(parts: Vec<Part>, markdown: &Markdown) -> Rendered {
        parse_parts(
            parts,
            "/guide/",
            &TOC_DEPTHS,
            &SiteUrl::default(),
            markdown,
            OffsetDateTime::UNIX_EPOCH,
            &mut PagePlugins::none(),
        )
        .unwrap()
    }

    /// The page of `parts`, without highlighting, so its HTML is plain to
    /// compare.
    fn html(parts: Vec<Part>) -> String {
        let markdown = Markdown {
            highlight: None,
            ..Markdown::default()
        };
        parse(parts, &markdown).html
    }

    fn markdown(markdown: &str) -> Part {
        Part::Markdown(markdown.to_string())
    }

    fn code(language: &str, text: &str) -> Part {
        Part::Code(Code {
            language: language.to_string(),
            text: text.to_string(),
        })
    }

    #[test]
    fn parts_are_a_page() {
        // As pandas shows a table, with a blank line that would end raw HTML
        // in Markdown.
        let table = "<div>\n<style scoped>\n    .dataframe { color: red; }\n\n</style>\n\
                     <table class=\"dataframe\"><tr><td>1</td></tr></table>\n</div>";
        let parts = vec![
            markdown("What the data says."),
            code("python", "print(\"Hello\")\nx = 6 * 7"),
            code("output", "Hello\n"),
            Part::Html(format!("{table}\n\n")),
            code("output", ""),
        ];
        assert_eq!(
            html(parts),
            format!(
                "<p>What the data says.</p>\n\
                 <pre><code class=\"language-python\">print(&quot;Hello&quot;)\nx = 6 * 7\n\
                 </code></pre>\n\
                 <pre><code class=\"language-output\">Hello\n</code></pre>\n\
                 {table}\n\
                 <pre><code class=\"language-output\"></code></pre>\n"
            )
        );
        assert_eq!(html(Vec::new()), "");
    }

    #[test]
    fn the_first_h1_the_headings_and_the_links_are_the_pages() {
        let parts = vec![
            markdown("# Analysis"),
            markdown("## Loading the data\n\nFrom [the data](data.csv)."),
            code("python", "1"),
            markdown("## Results"),
        ];
        let rendered = parse(parts, &Markdown::default());
        assert_eq!(rendered.first_h1.as_deref(), Some("Analysis"));
        let toc: Vec<_> = rendered.toc.iter().map(|item| &item.anchor).collect();
        assert_eq!(toc, ["#loading-the-data", "#results"]);
        let links: Vec<_> = (rendered.links.iter())
            .map(|link| (link.written.as_str(), link.href.as_str()))
            .collect();
        assert_eq!(links, [("data.csv", "/guide/data.csv")]);
    }

    #[test]
    fn each_markdown_part_is_read_on_its_own() {
        // A code block left open ends with its part.
        let parts = vec![markdown("```python\nprint(1)"), markdown("After")];
        assert_eq!(
            html(parts),
            "<pre><code class=\"language-python\">print(1)</code></pre>\n<p>After</p>\n"
        );
    }

    #[test]
    fn frontmatter_is_at_the_top_of_the_first_part() {
        let title = |parts: Vec<Part>| {
            let rendered = parse(parts, &Markdown::default());
            let title = rendered
                .frontmatter
                .get("title")
                .and_then(|title| title.as_str());
            (title.map(str::to_string), rendered.html)
        };
        let (frontmatter, html) = title(vec![markdown("---\ntitle: Analysis\n---\nBody")]);
        assert_eq!(frontmatter.as_deref(), Some("Analysis"));
        assert_eq!(html, "<p>Body</p>\n");

        // Anywhere else, it's Markdown.
        let (frontmatter, html) = title(vec![
            code("python", "1"),
            markdown("---\ntitle: Analysis\n---"),
        ]);
        assert_eq!(frontmatter, None);
        assert!(html.contains("<hr />\n<h2 id=\"title-analysis\">title: Analysis"));
        let (frontmatter, _) = title(vec![markdown("Body"), markdown("---\ntitle: A\n---")]);
        assert_eq!(frontmatter, None);
    }

    #[test]
    fn diagrams_and_math_are_drawn_through_the_page() {
        let flowchart = "flowchart LR\n  A --> B\n";
        let parts = vec![
            markdown(&format!("```mermaid\n{flowchart}```")),
            markdown("```mermaid\nflowchat LR\n```"),
            code("mermaid", flowchart),
            code("math", "x^2"),
        ];
        let r = parse(parts, &Markdown::default());
        assert!(r.html.starts_with("<svg id=\"mermaid-1\" "), "{}", r.html);
        let code = "<code class=\"language-mermaid\">flowchat LR\n</code>";
        assert!(r.html.contains(code), "{}", r.html);
        assert!(r.html.contains("<svg id=\"mermaid-3\" "), "{}", r.html);
        assert!(r.html.contains("<math display=\"block\">"), "{}", r.html);
        // Without a line, which would be its part's.
        let unrendered = mermaid::Unrendered {
            line: None,
            reason: "No diagram type detected".to_string(),
        };
        assert_eq!(r.unrendered_diagrams, [unrendered]);
    }

    #[test]
    fn code_is_highlighted_in_its_language() {
        let html = parse(vec![code("R", "x <- 1")], &Markdown::default()).html;
        assert!(
            html.starts_with("<pre class=\"highlight\"><code class=\"language-R\">"),
            "{html}"
        );
        let html = parse(vec![code("", "x <- 1")], &Markdown::default()).html;
        assert!(
            html.starts_with("<pre class=\"highlight\"><code>x"),
            "{html}"
        );
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
        let parts = vec![
            markdown("One.[^a]\n\n[^a]: First."),
            markdown("Two.[^b] Three.[^c]\n\n[^c]: Third.\n\n[^b]: Second."),
            markdown("The end."),
        ];
        assert_eq!(
            html(parts),
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
        // comrak names those in each part from `__inline_1`.
        let options = Markdown {
            inline_footnotes: true,
            highlight: None,
            ..Markdown::default()
        };
        let parts = vec![markdown("One.^[First.]"), markdown("Two.^[Second.]")];
        assert_eq!(
            parse(parts, &options).html,
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
        let parts = vec![markdown("One.^[ ]^[First.]"), markdown("Two.^[Second.]")];
        let html = parse(parts, &options).html;
        let names: Vec<_> = (html.split("<li id=\"").skip(1))
            .map(|rest| rest.split('"').next().unwrap())
            .collect();
        assert_eq!(names.len(), 2, "{html}");
        assert_ne!(names[0], names[1], "{html}");
    }
}
