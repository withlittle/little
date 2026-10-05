//! Draw each Mermaid diagram in a page as SVG, with merman, before comrak
//! writes the page: a ```` ```mermaid ```` block becomes an `<svg>` with the
//! id `mermaid-1`, `mermaid-2`, and so on, in the order they come in the
//! page, which its own styles are scoped to.

use chrono::NaiveDate;
use comrak::nodes::{AstNode, NodeValue};
use merman::MermaidConfig;
use merman::render::{HeadlessError, HeadlessRenderer, RootBackgroundPostprocessor, SvgPipeline};
use serde_json::json;
use time::OffsetDateTime;

use crate::config::{Markdown, Mermaid};

/// A diagram merman can't draw, which is written as code instead, as with
/// `markdown.mermaid: false`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unrendered {
    /// The line of its file its block starts on, if its file is Markdown: a
    /// notebook's lines are its cells'.
    pub line: Option<usize>,
    /// Why, as merman says, on one line, like
    /// `Diagram parse error (flowchart-v2): …`.
    pub reason: String,
}

/// Draw each Mermaid block in the document at `root` as SVG, if `markdown`
/// says to, with `now` as Mermaid's today. A block's line in its file is
/// `lines_before` after its line in the document, for the frontmatter comrak
/// didn't read, or `None` for a notebook's. Returns each diagram that
/// couldn't be drawn.
pub fn draw<'a>(
    root: &'a AstNode<'a>,
    markdown: &Markdown,
    now: OffsetDateTime,
    lines_before: Option<usize>,
) -> Vec<Unrendered> {
    let mut unrendered = Vec::new();
    if markdown.mermaid != Some(Mermaid::Svg) {
        return unrendered;
    }
    // Made only for a page with a diagram in it, as it takes a moment.
    let mut renderer = None;
    let mut diagrams = 0;
    for node in root.descendants() {
        let mut data = node.data.borrow_mut();
        let NodeValue::CodeBlock(block) = &data.value else {
            continue;
        };
        if !is_mermaid(&block.info) {
            continue;
        }
        // One that can't be drawn keeps its number, so those after it keep
        // their ids while its author fixes it.
        diagrams += 1;
        let renderer = renderer.get_or_insert_with(|| renderer_at(now));
        renderer.svg.diagram_id = Some(format!("mermaid-{diagrams}"));
        match svg(renderer, &block.literal) {
            Ok(svg) => data.value = NodeValue::Raw(format!("{}\n", svg.trim_end())),
            Err(reason) => unrendered.push(Unrendered {
                line: lines_before.map(|before| before + data.sourcepos.start.line),
                reason,
            }),
        }
    }
    unrendered
}

/// Whether a block is Mermaid by its language, the first word after its
/// fence, up to a comma, in any case, as for highlighting.
fn is_mermaid(info: &str) -> bool {
    let word = info.split_ascii_whitespace().next().unwrap_or_default();
    (word.split(',').next()).is_some_and(|lang| lang.eq_ignore_ascii_case("mermaid"))
}

/// A renderer that draws a diagram the same in each build: with Mermaid's
/// random seeds fixed, and its today `now`, in UTC. A gantt chart is drawn
/// 800 pixels wide, as little can't measure the page, and each diagram on a
/// transparent background, so it's on the theme's.
fn renderer_at(now: OffsetDateTime) -> HeadlessRenderer {
    let config = json!({
        "handDrawnSeed": 1,
        "gitGraph": { "seed": 1 },
        "gantt": { "useWidth": 800 },
    });
    let today = NaiveDate::from_ymd_opt(now.year(), u8::from(now.month()).into(), now.day().into());
    let mut renderer = HeadlessRenderer::new()
        .with_strict_parsing()
        .with_site_config(MermaidConfig::from_value(config))
        .with_fixed_today(today)
        .with_fixed_local_offset_minutes(Some(0))
        .with_svg_pipeline(
            SvgPipeline::parity()
                .with_postprocessor(RootBackgroundPostprocessor::new("transparent")),
        );
    renderer.svg.now_ms_override = i64::try_from(now.unix_timestamp_nanos() / 1_000_000).ok();
    renderer
}

/// `diagram` drawn as SVG, or why merman can't draw it, on one line.
fn svg(renderer: &HeadlessRenderer, diagram: &str) -> Result<String, String> {
    let reason = match renderer.render_svg_sync(diagram) {
        Ok(Some(svg)) => return Ok(svg),
        // merman's message for this goes on to repeat the whole diagram.
        Ok(None) | Err(HeadlessError::Parse(merman::Error::DetectType(_))) => {
            "No diagram type detected".to_string()
        }
        Err(error) => error.to_string(),
    };
    Err(reason.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SiteUrl;
    use crate::render::{Rendered, parse_document};

    const FLOWCHART: &str = "```mermaid\nflowchart LR\n  A --> B\n```\n";

    /// Render `md` with `markdown`, as if built at the start of 2026.
    fn render_with(md: &str, markdown: &Markdown) -> Rendered {
        let now = OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap();
        parse_document(md, "/", &(2..=6), &SiteUrl::default(), markdown, now).unwrap()
    }

    /// Render `md` with little's defaults, which draw diagrams as SVG.
    fn render(md: &str) -> Rendered {
        render_with(md, &Markdown::default())
    }

    #[test]
    fn each_diagram_is_an_svg() {
        let r = render(&format!(
            "Before\n\n{FLOWCHART}\nBetween\n\n{FLOWCHART}\nAfter\n"
        ));
        assert!(
            r.html.starts_with("<p>Before</p>\n<svg id=\"mermaid-1\" "),
            "{}",
            r.html
        );
        assert!(
            r.html
                .contains("</svg>\n<p>Between</p>\n<svg id=\"mermaid-2\" "),
            "{}",
            r.html
        );
        assert!(r.html.ends_with("</svg>\n<p>After</p>\n"), "{}", r.html);
        assert!(!r.html.contains("<pre"), "{}", r.html);
        assert_eq!(r.unrendered_diagrams, []);
    }

    #[test]
    fn a_block_is_mermaid_by_its_language() {
        for fence in [
            "mermaid",
            "Mermaid",
            "MERMAID",
            "mermaid,ignore",
            "mermaid title",
        ] {
            let r = render(&format!("```{fence}\nflowchart LR\n  A --> B\n```\n"));
            assert!(
                r.html.starts_with("<svg id=\"mermaid-1\" "),
                "{fence}: {}",
                r.html
            );
        }
        for fence in ["", "mermaidjs", "text mermaid"] {
            let r = render(&format!("```{fence}\nflowchart LR\n  A --> B\n```\n"));
            assert!(r.html.starts_with("<pre"), "{fence}: {}", r.html);
        }
        // An indented block has no language.
        let r = render("    flowchart LR\n      A --> B\n");
        assert!(r.html.starts_with("<pre"), "{}", r.html);
    }

    #[test]
    fn mermaid_false_leaves_diagrams_as_code() {
        let markdown = Markdown {
            mermaid: None,
            ..Markdown::default()
        };
        let r = render_with(FLOWCHART, &markdown);
        let expected = "<pre class=\"highlight\"><code class=\"language-mermaid\">\
                        flowchart LR\n  A --&gt; B\n</code></pre>\n";
        assert_eq!(r.html, expected);
        assert_eq!(r.unrendered_diagrams, []);
    }

    #[test]
    fn a_diagram_that_cant_be_drawn_is_code() {
        let md = "---\ntitle: Flows\n---\n# Flows\n\n```mermaid\nflowchart LR\n  A -->\n```\n\n\
                  ```mermaid\nflowchat LR\n  A --> B\n```\n\n{FLOWCHART}";
        let r = render(&md.replace("{FLOWCHART}", FLOWCHART));
        assert!(
            r.html.contains(
                "<pre class=\"highlight\"><code class=\"language-mermaid\">flowchart LR\n"
            ),
            "{}",
            r.html
        );
        // Each keeps its number, so the diagram after them is still the third.
        assert!(r.html.contains("<svg id=\"mermaid-3\" "), "{}", r.html);
        // Each is noted at the line its block starts on in the file, past the
        // frontmatter, and why, on one line.
        let lines: Vec<_> = r.unrendered_diagrams.iter().map(|d| d.line).collect();
        assert_eq!(lines, [Some(6), Some(11)]);
        let reasons: Vec<_> = r
            .unrendered_diagrams
            .iter()
            .map(|d| d.reason.as_str())
            .collect();
        assert!(
            reasons[0].starts_with("Diagram parse error (flowchart"),
            "{reasons:?}"
        );
        assert!(!reasons[0].contains('\n'), "{reasons:?}");
        assert_eq!(reasons[1], "No diagram type detected");
    }

    #[test]
    fn a_diagram_is_drawn_the_same_each_time() {
        let md = "```mermaid\ngitGraph\n  commit\n  branch dev\n  commit\n  checkout main\n  \
                  merge dev\n```\n\n```mermaid\nstateDiagram-v2\n  [*] --> Still\n  Still --> \
                  [*]\n```\n\n```mermaid\ngantt\n  dateFormat YYYY-MM-DD\n  section Build\n  \
                  Draw :a1, 2025-12-01, 60d\n```\n";
        let first = render(md);
        assert_eq!(first.unrendered_diagrams, []);
        assert_eq!(first.html.matches("<svg id=\"mermaid-").count(), 3);
        assert_eq!(render(md).html, first.html);
    }
}
