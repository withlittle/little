//! Turn a raw Markdown file (with optional YAML frontmatter) into
//! frontmatter data, an HTML fragment, and an in-page table of contents.

use anyhow::{Context, Result};
use comrak::nodes::{AstNode, NodeValue};
use comrak::{Anchorizer, Arena, Options};
use serde_yaml::Mapping;
use std::borrow::Cow;
use std::ops::RangeInclusive;

use crate::config::SiteUrl;

/// One heading in a page's in-page table of contents. They're listed in the
/// order they come, and the build nests them like `site.nav`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocItem {
    /// Human-readable heading text.
    pub title: String,
    /// Fragment link to the heading, e.g. `#getting-started`.
    pub anchor: String,
    /// Heading level, from 1 for `#` to 6.
    pub depth: u8,
}

/// The result of rendering one Markdown document.
pub struct Rendered {
    /// The frontmatter's settings: none when a page has no frontmatter.
    pub frontmatter: Mapping,
    /// Rendered HTML body.
    pub html: String,
    /// In-page table of contents built from the body's headings.
    pub toc: Vec<TocItem>,
    /// Text of the first `<h1>`, used as a title fallback.
    pub first_h1: Option<String>,
}

/// Parse a document into its frontmatter, rendered HTML, and outline, which
/// lists its headings at the levels in `toc_depths` (the config's
/// `toc_min_depth` to `toc_max_depth`). Links and images from the root of the
/// site, like `/guide/`, get the path of the site's `url`.
pub fn parse_document(
    raw: &str,
    toc_depths: &RangeInclusive<u8>,
    url: &SiteUrl,
) -> Result<Rendered> {
    let (frontmatter_src, body) = split_frontmatter(raw);

    // Frontmatter is settings, like a `little.yml`, as each one becomes a key
    // of `page`. Anything else, like a list or plain text, is an error rather
    // than silently dropped. Empty or comment-only frontmatter has none.
    let frontmatter = serde_yaml::from_str::<Option<Mapping>>(&frontmatter_src)
        .context("parsing frontmatter")?
        .unwrap_or_default();

    let options = render_options();
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, &body, &options);

    link_within_site(root, url);
    let (toc, first_h1) = outline(root, toc_depths);

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

/// Put the site's path before each link and image from the root of the site,
/// so an author can write `/guide/` wherever the site is published. Raw HTML
/// isn't rendered, so its links needn't be found.
fn link_within_site<'a>(root: &'a AstNode<'a>, url: &SiteUrl) {
    for node in root.descendants() {
        if let NodeValue::Link(link) | NodeValue::Image(link) = &mut node.data.borrow_mut().value
            && let Cow::Owned(within) = url.link(&link.url)
        {
            link.url = within;
        }
    }
}

/// Walk the parsed document once to build the TOC from the headings at the
/// levels in `toc_depths`, and find the first `<h1>`.
///
/// Every heading is fed to the same `Anchorizer` in document order so the slugs
/// match the `id`s comrak emits into the HTML (its formatter anchorizes every
/// heading the same way), including the `-1`, `-2` … suffixes added to
/// duplicates.
fn outline<'a>(
    root: &'a AstNode<'a>,
    toc_depths: &RangeInclusive<u8>,
) -> (Vec<TocItem>, Option<String>) {
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

        if toc_depths.contains(&level) {
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
    use serde_yaml::Value;

    /// little's defaults, from `src/little.yml`: every heading but the title.
    const TOC_DEPTHS: RangeInclusive<u8> = 2..=6;

    fn render(md: &str) -> Rendered {
        parse_document(md, &TOC_DEPTHS, &SiteUrl::default()).expect("document renders")
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
    fn missing_or_empty_frontmatter_has_no_settings() {
        for md in [
            "# Head\n",
            "---\n---\n# Head\n",
            "---\n# nothing to set yet\n---\n# Head\n",
        ] {
            assert!(render(md).frontmatter.is_empty(), "{md}");
        }
    }

    #[test]
    fn frontmatter_holds_settings() {
        // Each setting becomes a key of `page`: a list, or plain text between
        // the `---` lines, can't.
        for md in [
            "---\n- title: Hi\n---\n# Head\n",
            "---\nJust some words\n---\n# Head\n",
        ] {
            let message = format!(
                "{:#}",
                parse_document(md, &TOC_DEPTHS, &SiteUrl::default())
                    .err()
                    .unwrap()
            );
            assert!(message.starts_with("parsing frontmatter: "), "{message}");
            assert!(message.contains("expected a YAML mapping"), "{message}");
        }
    }

    #[test]
    fn unclosed_frontmatter_is_treated_as_body() {
        let r = render("---\nnot really frontmatter\n# Head\n");
        assert!(r.frontmatter.is_empty());
        assert!(r.html.contains("not really frontmatter"));
    }

    #[test]
    fn toc_collects_the_headings_at_its_depths() {
        let md = "# Title\n\n## Getting Started\n\n### Install\n\n#### From Source\n\n\
                  ##### On Linux\n\n###### With Nix\n\n# Another Title\n\n## Usage\n";
        let toc = |depths| {
            let r = parse_document(md, &depths, &SiteUrl::default()).unwrap();
            let items = r.toc.iter().map(|t| format!("{} {}", t.depth, t.anchor));
            items.collect::<Vec<_>>()
        };

        assert_eq!(
            toc(TOC_DEPTHS),
            [
                "2 #getting-started",
                "3 #install",
                "4 #from-source",
                "5 #on-linux",
                "6 #with-nix",
                "2 #usage",
            ]
        );
        assert_eq!(toc(2..=3), ["2 #getting-started", "3 #install", "2 #usage"]);
        assert_eq!(
            toc(1..=2),
            [
                "1 #title",
                "2 #getting-started",
                "1 #another-title",
                "2 #usage",
            ]
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

    #[test]
    fn links_from_the_root_get_the_sites_path() {
        let url = SiteUrl {
            origin: "https://withlittle.github.io".into(),
            path: "/little".into(),
        };
        let md = "[Install](/guide/install) ![Logo](/logo.png) [Guide][guide]\n\n\
                  [Usage](usage) [Top](#top) [little](https://example.com/)\n\n\
                  [guide]: /guide/\n";
        let html = parse_document(md, &TOC_DEPTHS, &url).unwrap().html;
        for within in [
            r#"<a href="/little/guide/install">Install</a>"#,
            r#"<img src="/little/logo.png" alt="Logo" />"#,
            r#"<a href="/little/guide/">Guide</a>"#,
        ] {
            assert!(html.contains(within), "{within} in {html}");
        }

        // A link from the page, or to another site, is left as it is.
        for other in [
            r#"<a href="usage">Usage</a>"#,
            r##"<a href="#top">Top</a>"##,
            r#"<a href="https://example.com/">little</a>"#,
        ] {
            assert!(html.contains(other), "{other} in {html}");
        }

        // As is every link of a site at the root of its domain.
        let html = render(md).html;
        assert!(html.contains(r#"<a href="/guide/install">"#), "{html}");
    }
}
