//! Turn a raw Markdown file (with optional YAML frontmatter) into
//! frontmatter data, an HTML fragment, and an in-page table of contents.

use anyhow::{Context, Result};
use comrak::nodes::{AstNode, NodeValue};
use comrak::{Anchorizer, Arena, Options};
use serde::Serialize;
use serde_yaml::Value;

/// TOC entries are collected for headings between these levels (inclusive).
/// `<h1>` is the page title; `h4`+ are too granular for a page outline.
const TOC_MIN_DEPTH: u8 = 2;
const TOC_MAX_DEPTH: u8 = 3;

/// One entry in a page's in-page table of contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TocItem {
    /// Human-readable heading text.
    pub title: String,
    /// Fragment link to the heading, e.g. `#getting-started`.
    pub anchor: String,
    /// Heading level (2 or 3).
    pub depth: u8,
}

/// The result of rendering one Markdown document.
pub struct Rendered {
    /// Parsed YAML frontmatter (or `Value::Null` when absent).
    pub frontmatter: Value,
    /// Rendered HTML body.
    pub html: String,
    /// In-page table of contents built from the body's headings.
    pub toc: Vec<TocItem>,
    /// Text of the first `<h1>`, used as a title fallback.
    pub first_h1: Option<String>,
}

/// Parse a document into its frontmatter, rendered HTML, and outline.
pub fn parse_document(raw: &str) -> Result<Rendered> {
    let (frontmatter_src, body) = split_frontmatter(raw);

    let frontmatter = if frontmatter_src.trim().is_empty() {
        Value::Null
    } else {
        serde_yaml::from_str(&frontmatter_src).context("parsing frontmatter")?
    };

    let options = render_options();
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, &body, &options);

    let (toc, first_h1) = outline(root);

    let mut html = String::new();
    comrak::html::format_document(root, &options, &mut html)
        .context("rendering markdown to HTML")?;

    Ok(Rendered {
        frontmatter,
        html,
        toc,
        first_h1,
    })
}

/// comrak options shared by rendering and outline building. Heading IDs are
/// enabled (empty prefix) so the anchors the TOC links to actually exist in the
/// HTML; both sides slugify with comrak's `Anchorizer`, so they stay in sync.
fn render_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.header_id_prefix = Some(String::new());
    options
}

/// Walk the parsed document once to build the TOC and find the first `<h1>`.
///
/// Every heading is fed to the same `Anchorizer` in document order so the slugs
/// match the `id`s comrak emits into the HTML (its formatter anchorizes every
/// heading the same way), including the `-1`, `-2` … suffixes added to
/// duplicates.
fn outline<'a>(root: &'a AstNode<'a>) -> (Vec<TocItem>, Option<String>) {
    let mut anchorizer = Anchorizer::new();
    let mut toc = Vec::new();
    let mut first_h1 = None;

    for node in root.descendants() {
        let level = match &node.data.borrow().value {
            NodeValue::Heading(heading) => heading.level,
            _ => continue,
        };

        let text = heading_text(node);
        let slug = anchorizer.anchorize(&text);

        if level == 1 && first_h1.is_none() {
            first_h1 = Some(text.clone());
        }

        if (TOC_MIN_DEPTH..=TOC_MAX_DEPTH).contains(&level) {
            toc.push(TocItem {
                title: text,
                anchor: format!("#{slug}"),
                depth: level,
            });
        }
    }

    (toc, first_h1)
}

/// Collect the plain-text content of a heading (text + inline code, descending
/// through emphasis/links/etc.).
fn heading_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    collect_text(node, &mut out);
    out
}

fn collect_text<'a>(node: &'a AstNode<'a>, out: &mut String) {
    for child in node.children() {
        match &child.data.borrow().value {
            NodeValue::Text(text) => out.push_str(text),
            NodeValue::Code(code) => out.push_str(&code.literal),
            _ => collect_text(child, out),
        }
    }
}

/// Split a `---` fenced YAML frontmatter block from the body.
fn split_frontmatter(raw: &str) -> (String, String) {
    let first_line = raw.lines().next().map(str::trim);
    if first_line != Some("---") {
        return (String::new(), raw.to_string());
    }

    let mut frontmatter = String::new();
    let mut body = String::new();
    let mut in_frontmatter = true;
    let mut closed = false;

    for (i, line) in raw.lines().enumerate() {
        if i == 0 {
            continue; // skip the opening `---`
        }
        if in_frontmatter && line.trim() == "---" {
            in_frontmatter = false;
            closed = true;
            continue;
        }
        if in_frontmatter {
            frontmatter.push_str(line);
            frontmatter.push('\n');
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }

    if closed {
        (frontmatter, body)
    } else {
        (String::new(), raw.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(md: &str) -> Rendered {
        parse_document(md).expect("document renders")
    }

    #[test]
    fn splits_frontmatter_from_body() {
        let r = render("---\ntitle: Hi\n---\n# Head\n\nBody text\n");
        assert_eq!(
            r.frontmatter.get("title").and_then(Value::as_str),
            Some("Hi")
        );
        assert!(r.html.contains("Body text"));
        assert!(!r.html.contains("title: Hi"));
    }

    #[test]
    fn missing_frontmatter_is_null() {
        let r = render("# Head\n");
        assert!(r.frontmatter.is_null());
    }

    #[test]
    fn unclosed_frontmatter_is_treated_as_body() {
        let r = render("---\nnot really frontmatter\n# Head\n");
        assert!(r.frontmatter.is_null());
        assert!(r.html.contains("not really frontmatter"));
    }

    #[test]
    fn toc_collects_only_h2_and_h3() {
        let md = "# Title\n\n## Getting Started\n\n### Install\n\n#### Too Deep\n\n## Usage\n";
        let r = render(md);
        let shape: Vec<(u8, &str)> = r.toc.iter().map(|t| (t.depth, t.anchor.as_str())).collect();
        assert_eq!(
            shape,
            vec![(2, "#getting-started"), (3, "#install"), (2, "#usage")]
        );
    }

    #[test]
    fn toc_anchor_matches_emitted_html_id() {
        let r = render("## Hello, World!\n");
        assert_eq!(r.toc[0].anchor, "#hello-world");
        assert!(r.html.contains("id=\"hello-world\""));
    }

    #[test]
    fn duplicate_headings_get_unique_anchors() {
        let r = render("## Setup\n\n## Setup\n");
        assert_eq!(r.toc[0].anchor, "#setup");
        assert_eq!(r.toc[1].anchor, "#setup-1");
        assert!(r.html.contains("id=\"setup\""));
        assert!(r.html.contains("id=\"setup-1\""));
    }

    #[test]
    fn heading_text_flattens_inline_markup() {
        let r = render("## Use `little` **today**\n");
        assert_eq!(r.toc[0].title, "Use little today");
        assert_eq!(r.toc[0].anchor, "#use-little-today");
    }

    #[test]
    fn first_h1_is_captured() {
        let r = render("# The Real Title\n\n## Sub\n");
        assert_eq!(r.first_h1.as_deref(), Some("The Real Title"));
    }

    #[test]
    fn first_h1_is_none_without_h1() {
        let r = render("## Only A Subheading\n");
        assert_eq!(r.first_h1, None);
    }
}
