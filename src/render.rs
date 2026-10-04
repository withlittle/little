//! Turn a raw Markdown file (with optional YAML frontmatter) into
//! frontmatter data, an HTML fragment, and an in-page table of contents. A
//! notebook's cells are made into the same tree, in `notebook`, and rendered
//! here as Markdown's is.

use anyhow::{Context, Result};
use comrak::nodes::{AstNode, NodeValue};
use comrak::options::Plugins;
use comrak::{Anchorizer, Arena, Options};
use serde_yaml::Mapping;
use std::borrow::Cow;
use std::mem;
use std::ops::RangeInclusive;

use crate::config::{Markdown, Math, SiteUrl};
use crate::{highlight, math};

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

/// The result of rendering one document: a Markdown file, or a notebook.
pub struct Rendered {
    /// The frontmatter's settings: none when a page has no frontmatter.
    pub frontmatter: Mapping,
    /// Rendered HTML body.
    pub html: String,
    /// In-page table of contents built from the body's headings.
    pub toc: Vec<TocItem>,
    /// Text of the first `<h1>`, used as a title fallback.
    pub first_h1: Option<String>,
    /// The body's links and images, in the order they come.
    pub links: Vec<WrittenLink>,
    /// The formulas in the body that couldn't be written as MathML, so are
    /// written as TeX, in the order they come.
    pub unrendered_math: Vec<math::Unrendered>,
}

/// A link or image in a document, so a broken one can be shown as it's
/// written, not as little rewrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenLink {
    /// As the document has it, like `install.md#usage`.
    pub written: String,
    /// Where it leads in the HTML, as a browser reads it there, like
    /// `/my-project/guide/install#usage`.
    pub href: String,
}

/// Parse a document, written with the extensions to CommonMark in
/// `markdown`, into its frontmatter, rendered HTML, with its math written and
/// its code highlighted as `markdown` chooses, and outline, which lists its
/// headings at the levels in `toc_depths` (the config's `toc_min_depth` to
/// `toc_max_depth`).
/// Links and images relative to the document, like `images/logo.png`, lead
/// from `folder`, the URL of the folder it's in, like `/guide/`; those from
/// the root of the site get the path of the site's `url`. A link to a page's
/// file, like `install.md`, leads to the page.
pub fn parse_document(
    raw: &str,
    folder: &str,
    toc_depths: &RangeInclusive<u8>,
    url: &SiteUrl,
    markdown: &Markdown,
) -> Result<Rendered> {
    let (frontmatter_src, body) = split_frontmatter(raw);
    let frontmatter = parse_frontmatter(&frontmatter_src)?;
    let options = render_options(markdown);
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, &body, &options);
    render_tree(
        root,
        frontmatter,
        folder,
        toc_depths,
        url,
        markdown,
        &options,
    )
}

/// The settings in frontmatter, as `split_frontmatter` finds it.
pub(crate) fn parse_frontmatter(frontmatter: &str) -> Result<Mapping> {
    // Frontmatter is settings, like a `little.yml`, as each one becomes a key
    // of `page`. Anything else, like a list or plain text, is an error rather
    // than silently dropped. Empty or comment-only frontmatter has none.
    Ok(serde_yaml::from_str::<Option<Mapping>>(frontmatter)
        .context("parsing frontmatter")?
        .unwrap_or_default())
}

/// Render a document parsed with `options`, as `parse_document` does, from
/// its tree in comrak's arena: its links led within the site, its outline,
/// and its HTML.
pub(crate) fn render_tree<'a>(
    root: &'a AstNode<'a>,
    frontmatter: Mapping,
    folder: &str,
    toc_depths: &RangeInclusive<u8>,
    url: &SiteUrl,
    markdown: &Markdown,
    options: &Options,
) -> Result<Rendered> {
    let links = link_within_site(root, folder, url, options);
    let (toc, first_h1) = outline(root, toc_depths);

    let mut plugins = Plugins::default();
    plugins.render.codefence_syntax_highlighter = highlight::highlighter(markdown.highlight);
    let mut html = String::new();
    let unrendered_math = match markdown.math {
        Some(Math::MathML) => math::format_document(root, options, &plugins, &mut html),
        _ => comrak::html::format_document_with_plugins(root, options, &mut html, &plugins)
            .map(|()| Vec::new()),
    }
    .context("rendering markdown to HTML")?;

    Ok(Rendered {
        frontmatter,
        html,
        toc,
        first_h1,
        links,
        unrendered_math,
    })
}

/// comrak's options for a page: the extensions the site chose, and heading
/// IDs (with an empty prefix), so the anchors the TOC links to actually exist
/// in the HTML; both sides slugify with comrak's `Anchorizer`, so they stay in
/// sync.
pub(crate) fn render_options(markdown: &Markdown) -> Options<'static> {
    let mut options = Options::default();
    let extension = &mut options.extension;
    extension.table = markdown.tables;
    extension.strikethrough = markdown.strikethrough;
    extension.autolink = markdown.autolinks;
    extension.tasklist = markdown.task_lists;
    extension.footnotes = markdown.footnotes;
    extension.alerts = markdown.alerts;
    extension.shortcodes = markdown.emoji;
    // GitHub's math: `$…$`, `$$…$$`, `` $`…`$ ``, and ```` ```math ````.
    extension.math_dollars = markdown.math.is_some();
    extension.math_code = markdown.math.is_some();
    extension.superscript = markdown.superscript;
    extension.subscript = markdown.subscript;
    extension.highlight = markdown.mark;
    extension.insert = markdown.insert;
    extension.description_lists = markdown.description_lists;
    extension.inline_footnotes = markdown.inline_footnotes;
    extension.block_directive = markdown.fenced_divs;
    extension.multiline_block_quotes = markdown.multiline_blockquotes;
    extension.cjk_friendly_emphasis = markdown.cjk_friendly_emphasis;
    extension.header_id_prefix = Some(String::new());
    options.parse.smart = markdown.smart_punctuation;
    options.render.hardbreaks = markdown.hard_breaks;
    options.render.r#unsafe = markdown.raw_html;
    options
}

/// Point each link and image where it leads in the built site. One relative
/// to the document leads from its `folder`, as where Markdown is read as
/// files, like on GitHub, whatever URL its page is served at: with or without
/// a `/` at the end, as hosts differ. A link to a page's file, Markdown or a
/// notebook, leads to the page, as it leads to the file on GitHub. Then one
/// from the root of the site gets the site's path, so an author can write
/// `/guide/` wherever the site is published. Links in raw HTML are left as
/// they're written, for the browser to follow.
///
/// Returns each link and image as it's written, and as it's read from the
/// HTML.
fn link_within_site<'a>(
    root: &'a AstNode<'a>,
    folder: &str,
    url: &SiteUrl,
    options: &Options,
) -> Vec<WrittenLink> {
    let mut links = Vec::new();
    for node in root.descendants() {
        let mut data = node.data.borrow_mut();
        let (link, is_image) = match &mut data.value {
            NodeValue::Link(link) => (link, false),
            NodeValue::Image(link) => (link, true),
            _ => continue,
        };
        let mut within = from_the_folder(folder, &link.url);
        // An image can't be a page.
        if !is_image && let Some(page) = page_of(&within) {
            within = Cow::Owned(page);
        }
        let href = url.link(&within).into_owned();
        links.push(WrittenLink {
            href: as_read(&href, options),
            written: mem::replace(&mut link.url, href),
        });
    }
    links
}

/// The page built from the Markdown file or notebook at `url`, from the root
/// of the site, which a link to the file leads to, as on GitHub:
/// `/guide/install` for `/guide/install.md`, or `/guide/install.ipynb`, and
/// the folder's page, `/guide/`, for `/guide/README.md`. The `?query` and
/// `#fragment` follow as they are. `None` for any other URL, like
/// `/logo.png`, or `https://…/README.md`.
fn page_of(url: &str) -> Option<String> {
    if !url.starts_with('/') || url.starts_with("//") {
        return None;
    }
    let (path, rest) = url.split_at(url.find(['?', '#']).unwrap_or(url.len()));
    let (folder, file) = path.rsplit_once('/')?;
    // Only a `.md` or `.ipynb` file is a page: not `.MD`, nor `.md` alone,
    // which is hidden.
    let name = [".md", ".ipynb"]
        .iter()
        .find_map(|extension| file.strip_suffix(extension))
        .filter(|name| !name.is_empty())?;
    // As in the build, a `README` in any case is its folder's page.
    let page = if name.eq_ignore_ascii_case("readme") {
        ""
    } else {
        name
    };
    Some(format!("{folder}/{page}{rest}"))
}

/// `href` as a browser, or a link checker, reads it from the HTML: escaped
/// as comrak writes it, with the `&` and `'` it writes as entities read back.
fn as_read(href: &str, options: &Options) -> String {
    let mut escaped = String::new();
    // Writing to a `String` can't fail.
    let _ = comrak::html::escape_href(&mut escaped, href, options.parse.relaxed_autolinks);
    // `&#x27;` is read back first, or a URL with `&#x27;` in it, written
    // `&amp;#x27;`, would be read with a `'`.
    escaped.replace("&#x27;", "'").replace("&amp;", "&")
}

/// `url`, written in a document in the folder at `folder`, like `/guide/`,
/// from the root of the site: `images/logo.png` is `/guide/images/logo.png`,
/// and `../logo.png` is `/logo.png`, as `..` can't leave the site. Any other
/// URL, like `https://…`, `/…`, `#…`, or `?…`, is left as it is.
fn from_the_folder<'a>(folder: &str, url: &'a str) -> Cow<'a, str> {
    if url.is_empty() || url.starts_with(['/', '#', '?']) || has_scheme(url) {
        return Cow::Borrowed(url);
    }
    // The `?query` and `#fragment` follow the path as they are.
    let (path, rest) = url.split_at(url.find(['?', '#']).unwrap_or(url.len()));
    let mut segments: Vec<&str> = folder.split('/').filter(|s| !s.is_empty()).collect();
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        match part {
            "." => {}
            ".." => {
                segments.pop();
            }
            _ => segments.push(part),
        }
        // A path that ends at a folder, like `..`, ends with its `/`.
        if parts.peek().is_none() && matches!(part, "." | "..") {
            segments.push("");
        }
    }
    Cow::Owned(format!("/{}{rest}", segments.join("/")))
}

/// Whether `url` starts with a scheme, like `https:` or `mailto:`.
fn has_scheme(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

/// Walk the parsed document once to build the TOC from the headings at the
/// levels in `toc_depths`, and find the first `<h1>`.
///
/// Every heading is fed to the same `Anchorizer` in document order, with the
/// text comrak gives it its `id` from, so the slugs match the `id`s comrak
/// emits into the HTML (its formatter anchorizes every heading the same way),
/// including the `-1`, `-2` … suffixes added to duplicates.
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
        let slug = anchorizer.anchorize(&node.collect_text());

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

/// A heading's text, as a reader sees it: its text and code, its math as
/// it's written, and its emoji, with a space for each line break.
fn heading_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut text = String::new();
    for node in node.descendants() {
        match &node.data.borrow().value {
            NodeValue::Text(literal) => text.push_str(literal),
            NodeValue::Code(code) => text.push_str(&code.literal),
            NodeValue::Math(math) => text.push_str(&math.literal),
            NodeValue::ShortCode(emoji) => text.push_str(&emoji.emoji),
            NodeValue::LineBreak | NodeValue::SoftBreak => text.push(' '),
            _ => {}
        }
    }
    text
}

/// Split a `---` fenced YAML frontmatter block from the body.
pub(crate) fn split_frontmatter(raw: &str) -> (String, String) {
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
    use crate::config::Highlight;
    use serde_yaml::Value;

    /// little's defaults, from `src/little.yml`: every heading but the title.
    const TOC_DEPTHS: RangeInclusive<u8> = 2..=6;

    fn render(md: &str) -> Rendered {
        parse_document(
            md,
            "/",
            &TOC_DEPTHS,
            &SiteUrl::default(),
            &Markdown::default(),
        )
        .expect("document renders")
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
                parse_document(
                    md,
                    "/",
                    &TOC_DEPTHS,
                    &SiteUrl::default(),
                    &Markdown::default()
                )
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
            let r = parse_document(md, "/", &depths, &SiteUrl::default(), &Markdown::default())
                .unwrap();
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
    fn headings_with_math_emoji_or_two_lines_link_from_the_toc() {
        let md = "## The $x^2$ term\n\n## Done :tada:\n\nTwo\nlines\n---\n";
        for math in [Some(Math::MathML), Some(Math::Tex)] {
            let markdown = Markdown {
                math,
                ..Markdown::default()
            };
            let r = parse_document(md, "/", &TOC_DEPTHS, &SiteUrl::default(), &markdown).unwrap();
            // A heading's math is TeX in the outline, which is text.
            let titles: Vec<_> = r.toc.iter().map(|item| item.title.as_str()).collect();
            assert_eq!(titles, ["The x^2 term", "Done 🎉", "Two lines"]);
            // Each links to its heading's id, as comrak writes it.
            for item in &r.toc {
                let id = format!("id=\"{}\"", &item.anchor[1..]);
                assert!(r.html.contains(&id), "{id} in {}", r.html);
            }
        }
    }

    #[test]
    fn each_extension_turns_on_and_off() {
        // Each setting, some Markdown, and a piece of the HTML it's rendered
        // to only with the setting on.
        type Setting = fn(&mut Markdown) -> &mut bool;
        let cases: [(Setting, &str, &str); 19] = [
            (|m| &mut m.tables, "| a |\n| - |\n| b |\n", "<table>"),
            (|m| &mut m.strikethrough, "~~a~~", "<del>a</del>"),
            (|m| &mut m.autolinks, "www.example.com", "<a href="),
            (|m| &mut m.task_lists, "- [x] a", "type=\"checkbox\""),
            (|m| &mut m.footnotes, "a[^1]\n\n[^1]: b", "data-footnotes"),
            (|m| &mut m.alerts, "> [!NOTE]\n> a", "markdown-alert-note"),
            (|m| &mut m.emoji, ":tada:", "🎉"),
            (|m| &mut m.raw_html, "<kbd>a</kbd>", "<kbd>a</kbd>"),
            (|m| &mut m.smart_punctuation, "\"a\" -- b", "“a” – b"),
            (|m| &mut m.superscript, "a^2^", "<sup>2</sup>"),
            (|m| &mut m.subscript, "H~2~O", "<sub>2</sub>"),
            (|m| &mut m.mark, "==a==", "<mark>a</mark>"),
            (|m| &mut m.insert, "++a++", "<ins>a</ins>"),
            (|m| &mut m.description_lists, "a\n: b", "<dl>"),
            (|m| &mut m.inline_footnotes, "a^[b]", "data-footnotes"),
            (
                |m| &mut m.fenced_divs,
                ":::note\na\n:::",
                "<div class=\"note\">",
            ),
            (
                |m| &mut m.multiline_blockquotes,
                ">>>\na\n\nb\n>>>",
                "<blockquote>\n<p>a</p>\n<p>b",
            ),
            (|m| &mut m.hard_breaks, "a\nb", "a<br />"),
            (
                |m| &mut m.cjk_friendly_emphasis,
                "**これは重要。**だ",
                "<strong>",
            ),
        ];
        for (setting, md, html) in cases {
            for on in [true, false] {
                let mut markdown = Markdown::default();
                *setting(&mut markdown) = on;
                let rendered =
                    parse_document(md, "/", &TOC_DEPTHS, &SiteUrl::default(), &markdown).unwrap();
                assert_eq!(
                    rendered.html.contains(html),
                    on,
                    "{md:?}: {}",
                    rendered.html
                );
            }
        }
    }

    #[test]
    fn alerts_are_githubs_and_can_have_a_title() {
        let r = render("> [!TIP]\n> Run it.\n\n> [!warning] Before you upgrade\n> Back up.\n");
        let tip = "<div class=\"markdown-alert markdown-alert-tip\">\n\
                   <p class=\"markdown-alert-title\">Tip</p>\n<p>Run it.</p>\n</div>";
        assert!(r.html.contains(tip), "{}", r.html);
        let warning = "<div class=\"markdown-alert markdown-alert-warning\">\n\
                       <p class=\"markdown-alert-title\">Before you upgrade</p>";
        assert!(r.html.contains(warning), "{}", r.html);
    }

    #[test]
    fn code_is_highlighted_with_the_chosen_classes() {
        let md = "```rust\nfn main() {}\n```\n";
        let html = |highlight| {
            let markdown = Markdown {
                highlight,
                ..Markdown::default()
            };
            parse_document(md, "/", &TOC_DEPTHS, &SiteUrl::default(), &markdown)
                .unwrap()
                .html
        };
        assert_eq!(
            html(Some(Highlight::Pygments)),
            "<pre class=\"highlight\"><code class=\"language-rust\"><span class=\"kd\">fn</span> \
             <span class=\"nf\">main</span>() {}\n</code></pre>\n"
        );
        assert_eq!(
            html(Some(Highlight::GitHub)),
            "<pre class=\"highlight\"><code class=\"language-rust\"><span class=\"pl-k\">fn</span> \
             <span class=\"pl-en\">main</span>() {}\n</code></pre>\n"
        );
        // `false` leaves code as comrak writes it, for a theme's script.
        assert_eq!(
            html(None),
            "<pre><code class=\"language-rust\">fn main() {}\n</code></pre>\n"
        );
    }

    #[test]
    fn code_without_a_language_little_knows_is_plain() {
        for (md, code) in [
            (
                "```mermaid\nA-->B\n```\n",
                "<code class=\"language-mermaid\">",
            ),
            ("```\nA-->B\n```\n", "<code>"),
            ("    A-->B\n", "<code>"),
        ] {
            let expected = format!("<pre class=\"highlight\">{code}A--&gt;B\n</code></pre>\n");
            assert_eq!(render(md).html, expected, "{md:?}");
        }
    }

    #[test]
    fn math_is_written_as_the_site_chooses() {
        let html = |math| {
            let markdown = Markdown {
                math,
                ..Markdown::default()
            };
            let md = "$a*b + c*d$ costs $5";
            parse_document(md, "/", &TOC_DEPTHS, &SiteUrl::default(), &markdown)
                .unwrap()
                .html
        };
        let mathml = "<math><mi>a</mi><mo lspace=\"0.2222em\" rspace=\"0.2222em\">∗</mo>\
                      <mi>b</mi><mo>+</mo><mi>c</mi>\
                      <mo lspace=\"0.2222em\" rspace=\"0.2222em\">∗</mo><mi>d</mi></math>";
        assert_eq!(
            html(Some(Math::MathML)),
            format!("<p>{mathml} costs $5</p>\n")
        );
        assert_eq!(
            html(Some(Math::Tex)),
            "<p><span data-math-style=\"inline\">a*b + c*d</span> costs $5</p>\n"
        );
        // Off, `$` is a dollar, and `*` emphasis, as anywhere.
        assert_eq!(html(None), "<p>$a<em>b + c</em>d$ costs $5</p>\n");
    }

    #[test]
    fn math_blocks_arent_highlighted() {
        let md = "```math\nx^2\n```\n";
        let html = |math| {
            let markdown = Markdown {
                math,
                ..Markdown::default()
            };
            parse_document(md, "/", &TOC_DEPTHS, &SiteUrl::default(), &markdown)
                .unwrap()
                .html
        };
        assert_eq!(
            html(Some(Math::MathML)),
            "<math display=\"block\"><msup><mi>x</mi><mn>2</mn></msup></math>\n"
        );
        // A theme's script finds it by its `data-math-style`, as comrak marks
        // a block in `math` even with math off.
        for math in [Some(Math::Tex), None] {
            assert_eq!(
                html(math),
                "<pre><code class=\"language-math\" data-math-style=\"display\">x^2\n</code></pre>\n"
            );
        }
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
        let html = parse_document(md, "/guide/", &TOC_DEPTHS, &url, &Markdown::default())
            .unwrap()
            .html;
        for within in [
            r#"<a href="/little/guide/install">Install</a>"#,
            r#"<img src="/little/logo.png" alt="Logo" />"#,
            r#"<a href="/little/guide/">Guide</a>"#,
            // From the page's folder, then from the root.
            r#"<a href="/little/guide/usage">Usage</a>"#,
        ] {
            assert!(html.contains(within), "{within} in {html}");
        }

        // A link within the page, or to another site, is left as it is.
        for other in [
            r##"<a href="#top">Top</a>"##,
            r#"<a href="https://example.com/">little</a>"#,
        ] {
            assert!(html.contains(other), "{other} in {html}");
        }

        // As is every link of a site at the root of its domain.
        let html = render(md).html;
        assert!(html.contains(r#"<a href="/guide/install">"#), "{html}");
    }

    #[test]
    fn relative_links_lead_from_the_documents_folder() {
        // Like on GitHub, whatever URL the page is served at.
        for (url, from_guide) in [
            ("images/logo.png", "/guide/images/logo.png"),
            ("logo.png", "/guide/logo.png"),
            ("./logo.png", "/guide/logo.png"),
            ("../assets/logo.png", "/assets/logo.png"),
            ("install", "/guide/install"),
            ("install/", "/guide/install/"),
            ("a/../b//c", "/guide/b//c"),
            ("logo.png?v=2#top", "/guide/logo.png?v=2#top"),
            // `.` and `..` at the end lead to a folder, which ends with `/`.
            (".", "/guide/"),
            ("./", "/guide/"),
            ("..", "/"),
            // `..` can't leave the site.
            ("../../../logo.png", "/logo.png"),
            // Anything else is left as it is.
            ("", ""),
            ("/logo.png", "/logo.png"),
            ("//example.com/logo.png", "//example.com/logo.png"),
            ("#top", "#top"),
            ("?v=2", "?v=2"),
            ("https://example.com/", "https://example.com/"),
            ("mailto:me@example.com", "mailto:me@example.com"),
            ("data:image/png;base64,AAAA", "data:image/png;base64,AAAA"),
            // A `:` after a `/` isn't a scheme's.
            ("images/a:b.png", "/guide/images/a:b.png"),
        ] {
            assert_eq!(from_the_folder("/guide/", url), from_guide, "{url}");
        }
        assert_eq!(from_the_folder("/", "images/logo.png"), "/images/logo.png");
        assert_eq!(from_the_folder("/", "../logo.png"), "/logo.png");
        assert_eq!(from_the_folder("/a/b/", "../../c"), "/c");

        // In Markdown, in a link or an image, written however Markdown
        // allows: a space is escaped as the page is written.
        let md = "[Logo](logo.png) ![Logo](<images/the logo.png> \"Logo\") \
                  ![Logo][logo]\n\n[logo]: ../logo.png\n";
        let html = parse_document(
            md,
            "/guide/",
            &TOC_DEPTHS,
            &SiteUrl::default(),
            &Markdown::default(),
        )
        .unwrap()
        .html;
        for resolved in [
            r#"<a href="/guide/logo.png">Logo</a>"#,
            r#"<img src="/guide/images/the%20logo.png" alt="Logo" title="Logo" />"#,
            r#"<img src="/logo.png" alt="Logo" />"#,
        ] {
            assert!(html.contains(resolved), "{resolved} in {html}");
        }
    }

    #[test]
    fn links_to_a_pages_file_lead_to_the_page() {
        // As the build makes a page of each `.md` file, and each notebook.
        for (url, page) in [
            ("/guide/install.md", Some("/guide/install")),
            ("/guide/install.md#usage", Some("/guide/install#usage")),
            (
                "/guide/install.md?v=2#usage",
                Some("/guide/install?v=2#usage"),
            ),
            ("/guide/README.md", Some("/guide/")),
            ("/guide/readme.md#usage", Some("/guide/#usage")),
            ("/README.md", Some("/")),
            ("/notes.md.md", Some("/notes.md")),
            ("/guide/analysis.ipynb", Some("/guide/analysis")),
            (
                "/guide/analysis.ipynb#results",
                Some("/guide/analysis#results"),
            ),
            ("/guide/README.ipynb", Some("/guide/")),
            // Anything else is left as it is.
            ("/guide/README.MD", None),
            ("/guide/install.markdown", None),
            ("/guide/.md", None),
            ("/guide/.ipynb", None),
            ("/guide/analysis.IPYNB", None),
            ("/guide/install.md/", None),
            ("/guide/", None),
            ("/logo.png", None),
            ("/guide?file=install.md", None),
            ("//example.com/README.md", None),
            ("https://example.com/README.md", None),
            ("#install.md", None),
        ] {
            assert_eq!(page_of(url).as_deref(), page, "{url}");
        }

        // In Markdown, from the document's folder, and with the site's path,
        // in a link, but not an image.
        let url = SiteUrl {
            origin: "https://withlittle.github.io".into(),
            path: "/little".into(),
        };
        let md = "[Install](install.md#from-source) [Home](../README.md) \
                  [Guide](./README.md) [Usage][usage] ![Diagram](diagram.md)\n\n\
                  [usage]: /guide/usage.md\n";
        let html = parse_document(md, "/guide/", &TOC_DEPTHS, &url, &Markdown::default())
            .unwrap()
            .html;
        for within in [
            r#"<a href="/little/guide/install#from-source">Install</a>"#,
            r#"<a href="/little/">Home</a>"#,
            r#"<a href="/little/guide/">Guide</a>"#,
            r#"<a href="/little/guide/usage">Usage</a>"#,
            r#"<img src="/little/guide/diagram.md" alt="Diagram" />"#,
        ] {
            assert!(html.contains(within), "{within} in {html}");
        }
    }

    #[test]
    fn keeps_each_link_as_its_written() {
        let url = SiteUrl {
            origin: "https://withlittle.github.io".into(),
            path: "/little".into(),
        };
        let md = "[Install](install.md#from-source) ![Logo](<images/the logo.png>) \
                  [Top](#top) [Search](https://example.com/?q=a&b='c') [Usage][usage]\n\n\
                  [usage]: /guide/usage\n";
        let rendered =
            parse_document(md, "/guide/", &TOC_DEPTHS, &url, &Markdown::default()).unwrap();
        let links: Vec<_> = rendered
            .links
            .iter()
            .map(|link| (link.written.as_str(), link.href.as_str()))
            .collect();
        assert_eq!(
            links,
            [
                (
                    "install.md#from-source",
                    "/little/guide/install#from-source"
                ),
                ("images/the logo.png", "/little/guide/images/the%20logo.png"),
                ("#top", "#top"),
                (
                    "https://example.com/?q=a&b='c'",
                    "https://example.com/?q=a&b='c'"
                ),
                ("/guide/usage", "/little/guide/usage"),
            ]
        );
        // Each as a browser reads it from the HTML.
        let search = r#"<a href="https://example.com/?q=a&amp;b=&#x27;c&#x27;">"#;
        assert!(rendered.html.contains(search), "{}", rendered.html);
    }
}
