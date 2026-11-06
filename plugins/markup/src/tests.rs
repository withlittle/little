//! Checks that what's written reads back as what was meant, with comrak set
//! as little sets it, under little's settings, with every extension on, and
//! with none.

use crate::testing::options;
use crate::*;

/// Every extension on, but smart punctuation, which changes text.
fn all() -> Settings {
    Settings {
        superscript: true,
        subscript: true,
        mark: true,
        insert: true,
        description_lists: true,
        inline_footnotes: true,
        ..Settings::default()
    }
}

/// No extension on, nor raw HTML.
fn none() -> Settings {
    Settings {
        tables: false,
        strikethrough: false,
        autolinks: false,
        task_lists: false,
        footnotes: false,
        alerts: false,
        emoji: false,
        raw_html: false,
        math: false,
        ..Settings::default()
    }
}

fn every() -> [Settings; 4] {
    let cjk = Settings {
        cjk_friendly_emphasis: true,
        ..all()
    };
    [Settings::default(), all(), none(), cjk]
}

fn text(text: &str) -> Inline {
    Inline::Text(text.to_string())
}

fn para(inlines: Vec<Inline>) -> Block {
    Block::Paragraph(inlines)
}

fn plain(text: &str) -> Block {
    para(vec![Inline::Text(text.to_string())])
}

fn document(blocks: Vec<Block>) -> Document {
    Document {
        blocks,
        ..Document::default()
    }
}

fn render(parts: &[Part], settings: &Settings) -> String {
    let options = options(settings);
    parts
        .iter()
        .map(|part| match part {
            Part::Markdown(text) => comrak::markdown_to_html(text, &options),
            Part::Html(html) => html.clone(),
        })
        .collect()
}

/// The Markdown `blocks` are written as, which must be one part, and what
/// it warns of.
fn markdown(blocks: Vec<Block>, settings: &Settings) -> (String, Vec<String>) {
    let (parts, warnings) = write(document(blocks), settings);
    let [Part::Markdown(text)] = parts.as_slice() else {
        panic!("not one part of Markdown: {parts:?}");
    };
    let warnings = warnings.into_iter().map(|w| w.message).collect();
    (text.clone(), warnings)
}

/// The HTML comrak renders `blocks` as.
fn html(blocks: Vec<Block>, settings: &Settings) -> String {
    let (parts, _) = write(document(blocks), settings);
    render(&parts, settings)
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Text that Markdown, or one of its extensions, would read as more.
const TRICKY: &[&str] = &[
    "# not a heading",
    "#5 is not either",
    "- not a list",
    "+ not a list",
    "* not a list",
    "1. not a list",
    "1) not a list",
    "2024. was a year",
    "> not a quote",
    ">>> not a quote",
    "=== not a heading",
    "--- not a rule",
    "---",
    "___",
    "***",
    "- - -",
    "~~~ not a fence",
    "``` not a fence",
    "::: not a div",
    ": not a definition",
    "[ ] not a task",
    "[x] not a task",
    "<div>not HTML</div>",
    "<!-- not a comment -->",
    "<https://example.com>",
    "&amp; &copy; &#35; &",
    "a *b* c",
    "a _b_ c",
    "a **b** c",
    "a ~~b~~ c",
    "a ~b~ c",
    "a ^b^ c",
    "a ==b== c",
    "a ++b++ c",
    "[a](b)",
    "![a](b)",
    "[a]: b",
    "[a]",
    "[a] [b]",
    "a]: b",
    "a](b)",
    "[^1]",
    "^[not a footnote]",
    "a | b",
    "| a | b |",
    "$x$",
    "$$x$$",
    "$`x`$",
    "`code`",
    "\\*",
    "\\",
    "a\\",
    ":smile:",
    "https://example.com",
    "http://example.com/a_b_",
    "www.example.com",
    "WWW.example.com",
    "me@example.com",
    "a @b",
    "1.5 and 2.",
    "x^2^ and H~2~O",
    "a_b_c and a*b*c",
    "中文*強調*中文",
];

#[test]
fn text_reads_back_as_text() {
    for settings in every() {
        for tricky in TRICKY {
            let rendered = html(vec![plain(tricky)], &settings);
            let expected = format!("<p>{}</p>\n", escaped(tricky));
            assert_eq!(rendered, expected, "{tricky:?} with {settings:?}");
        }
    }
}

#[test]
fn text_after_a_line_break_reads_back_as_text() {
    for settings in every() {
        for tricky in TRICKY {
            let blocks = vec![para(vec![text("a"), Inline::LineBreak, text(tricky)])];
            let rendered = html(blocks, &settings);
            let expected = format!("<p>a<br />\n{}</p>\n", escaped(tricky));
            assert_eq!(rendered, expected, "{tricky:?} with {settings:?}");
        }
    }
}

/// The HTML of the paragraph comrak renders `inlines` as, inside its `<p>`.
fn inline_html(inlines: Vec<Inline>, settings: &Settings) -> String {
    let rendered = html(vec![para(inlines)], settings);
    let inner = rendered
        .strip_prefix("<p>")
        .and_then(|r| r.strip_suffix("</p>\n"));
    inner.unwrap_or(&rendered).to_string()
}

fn link(target: Target, content: Vec<Inline>) -> Inline {
    Inline::Link {
        target,
        title: String::new(),
        content,
    }
}

fn url(url: &str) -> Target {
    Target::Url(url.to_string())
}

#[test]
fn inlines_read_back_as_they_were() {
    use Inline::*;
    use Kind::*;
    let cases: Vec<(Vec<Inline>, &str)> = vec![
        (vec![Styled(Emphasis, vec![text("a")])], "<em>a</em>"),
        (vec![Styled(Strong, vec![text("a")])], "<strong>a</strong>"),
        (
            vec![text("a"), Styled(Emphasis, vec![text("b")]), text("c")],
            "a<em>b</em>c",
        ),
        (
            vec![text("a"), Styled(Strong, vec![text("b")]), text("c")],
            "a<strong>b</strong>c",
        ),
        (
            vec![text("x"), Styled(Strong, vec![text("(y)")]), text("z")],
            "x<strong>(y)</strong>z",
        ),
        (
            vec![Styled(
                Emphasis,
                vec![Styled(Strong, vec![text("a")]), text("b")],
            )],
            "<em><strong>a</strong>b</em>",
        ),
        (
            vec![Styled(
                Strong,
                vec![Styled(Emphasis, vec![text("a")]), text("b")],
            )],
            "<strong><em>a</em>b</strong>",
        ),
        (
            vec![
                Styled(Emphasis, vec![text("a")]),
                Styled(Emphasis, vec![text("b")]),
            ],
            "<em>ab</em>",
        ),
        (
            vec![
                Styled(Strong, vec![text("a")]),
                Styled(Strong, vec![text("b")]),
            ],
            "<strong>ab</strong>",
        ),
        (
            vec![text("x "), Styled(Emphasis, vec![text(" a ")]), text(" y")],
            "x  <em>a</em>  y",
        ),
        (vec![Styled(Emphasis, vec![text("*")])], "<em>*</em>"),
        (
            vec![Styled(Emphasis, vec![Code("x".into())])],
            "<em><code>x</code></em>",
        ),
        (
            vec![Styled(Strong, vec![text("「強調」")]), text("中文")],
            "<strong>「強調」</strong>中文",
        ),
        (vec![Styled(Strikethrough, vec![text("a")])], "<del>a</del>"),
        (vec![Styled(Underline, vec![text("a")])], "<u>a</u>"),
        (vec![Styled(Mark, vec![text("a")])], "<mark>a</mark>"),
        (
            vec![Styled(Superscript, vec![text("a b")])],
            "<sup>a b</sup>",
        ),
        (
            vec![text("H"), Styled(Subscript, vec![text("2")]), text("O")],
            "H<sub>2</sub>O",
        ),
        (
            vec![
                Styled(Subscript, vec![text("a")]),
                Styled(Subscript, vec![text("b")]),
            ],
            "<sub>ab</sub>",
        ),
        (vec![Code("a`b".into())], "<code>a`b</code>"),
        (vec![Code("`a".into())], "<code>`a</code>"),
        (vec![Code(" a ".into())], "<code> a </code>"),
        (vec![Code("  ".into())], "<code>  </code>"),
        (vec![Code("a\nb".into())], "<code>a b</code>"),
        (
            vec![Math("x^2".into())],
            "<code data-math-style=\"inline\">x^2</code>",
        ),
        (vec![Math("a`b".into())], "<code>a`b</code>"),
        (
            vec![Link {
                target: url("https://example.com/a b"),
                title: "t \"q\"".into(),
                content: vec![text("x")],
            }],
            "<a href=\"https://example.com/a%20b\" title=\"t &quot;q&quot;\">x</a>",
        ),
        (
            vec![Link {
                target: url("?a&b&amp;c&#35;d\\&copy;"),
                title: "&copy; \\&amp;".into(),
                content: vec![text("x")],
            }],
            "<a href=\"?a&amp;b&amp;amp;c&amp;#35;d%5C&amp;copy;\" \
             title=\"&amp;copy; \\&amp;amp;\">x</a>",
        ),
        (
            vec![link(url("a.adoc#x"), vec![text("y")])],
            "<a href=\"a.adoc#x\">y</a>",
        ),
        (
            vec![link(url("<a>"), vec![text("y")])],
            "<a href=\"%3Ca%3E\">y</a>",
        ),
        (
            vec![link(url("https://e.com"), vec![])],
            "<a href=\"https://e.com\">https://e.com</a>",
        ),
        (
            vec![link(Target::Id("x".into()), vec![text("y")])],
            "<a href=\"#x\">y</a>",
        ),
        (
            vec![Styled(Emphasis, vec![link(url("u"), vec![text("x")])])],
            "<em><a href=\"u\">x</a></em>",
        ),
        (
            vec![link(
                url("u"),
                vec![text("a"), Styled(Superscript, vec![text("2")])],
            )],
            "<a href=\"u\">a<sup>2</sup></a>",
        ),
        (
            vec![link(url("u"), vec![text("a^b ~c~")])],
            "<a href=\"u\">a^b ~c~</a>",
        ),
        (
            vec![text("!"), link(url("u"), vec![text("a")])],
            "!<a href=\"u\">a</a>",
        ),
        (
            vec![Image {
                url: "a b.png".into(),
                alt: "x *y* ]".into(),
                title: String::new(),
            }],
            "<img src=\"a%20b.png\" alt=\"x *y* ]\" />",
        ),
        (
            vec![Html {
                html: "<kbd>K</kbd>".into(),
                line: None,
            }],
            "<span></span><kbd>K</kbd>",
        ),
        (
            vec![
                text("a "),
                Html {
                    html: "<div>".into(),
                    line: None,
                },
                text("b"),
            ],
            "a <div>b",
        ),
        (
            vec![text("a"), Anchor("x".into()), text("b")],
            "a<a id=\"x\"></a>b",
        ),
    ];
    for settings in [Settings::default(), all()] {
        for (inlines, expected) in &cases {
            let rendered = inline_html(inlines.clone(), &settings);
            assert_eq!(rendered, *expected, "{inlines:?} with {settings:?}");
        }
    }
}

#[test]
fn what_markdown_cant_mark_is_left_plain_and_warned_of() {
    use Inline::*;
    use Kind::*;
    let inlines = vec![
        text("x"),
        Styled(Strong, vec![text("(y)")]),
        text("z "),
        Styled(Underline, vec![text("u")]),
        text(" "),
        Styled(Subscript, vec![text("s")]),
        Html {
            html: "<kbd>K</kbd>".into(),
            line: None,
        },
        Anchor("a".into()),
    ];
    let (_, warnings) = markdown(vec![para(inlines.clone())], &none());
    assert_eq!(
        warnings,
        [
            "left out underlining, as `raw_html` is off",
            "left out subscript, as `subscript` and `raw_html` are off",
            "left out raw HTML, as `raw_html` is off",
            "left out strong emphasis where Markdown can't mark it, as `raw_html` is off",
        ]
    );
    assert_eq!(inline_html(inlines, &none()), "x(y)z u s");
}

/// The values of `attribute` in `html`.
fn values<'a>(html: &'a str, attribute: &str) -> Vec<&'a str> {
    let start = format!("{attribute}=\"");
    html.match_indices(&start)
        .map(|(at, _)| {
            let value = &html[at + start.len()..];
            &value[..value.find('"').unwrap()]
        })
        .collect()
}

/// `text`, with each `%` escape decoded, as a browser decodes a fragment
/// before it finds the id.
fn decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok());
        match hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
            Some(byte) if bytes[at] == b'%' => {
                out.push(byte);
                at += 3;
            }
            _ => {
                out.push(bytes[at]);
                at += 1;
            }
        }
    }
    String::from_utf8(out).unwrap()
}

fn heading(level: u8, content: Vec<Inline>, ids: &[&str]) -> Block {
    Block::Heading {
        level,
        content,
        ids: ids.iter().map(|id| id.to_string()).collect(),
    }
}

#[test]
fn links_lead_to_the_ids_headings_and_anchors_are_given() {
    use Inline::*;
    use Kind::*;
    let headings = vec![
        vec![text("Hello, World!")],
        vec![text("Hello, World!")],
        vec![text("Ünïcödé Straße")],
        vec![text("a_b-c  d")],
        vec![text("code "), Code("x()".into())],
        vec![
            Styled(Emphasis, vec![text("em")]),
            text(" and "),
            Styled(Strong, vec![text("strong")]),
        ],
        vec![link(url("u"), vec![text("link")]), text(" text")],
        vec![text("math "), Math("x^2".into())],
        vec![text("anchor"), Anchor("inline-anchor".into())],
        vec![
            text("html "),
            Html {
                html: "<kbd>K</kbd>".into(),
                line: None,
            },
        ],
        vec![
            text("image "),
            Image {
                url: "i.png".into(),
                alt: "alt".into(),
                title: String::new(),
            },
        ],
        vec![text("footnote"), Footnote("f".into())],
        vec![text("1. numbered")],
        vec![text("# hash")],
        vec![text("deep")],
    ];
    let mut blocks = Vec::new();
    let mut links = vec![
        link(Target::Id("custom".into()), vec![text("custom")]),
        link(Target::Id("inline-anchor".into()), vec![text("inline")]),
        link(Target::Id("block-anchor".into()), vec![text("block")]),
    ];
    for (index, content) in headings.into_iter().enumerate() {
        let ids: &[&str] = if index == 0 {
            &["custom", "hello-world"]
        } else {
            &[]
        };
        links.push(link(Target::Heading(plain_text(&content)), vec![]));
        blocks.push(heading(if index == 14 { 9 } else { 2 }, content, ids));
        blocks.push(plain("text"));
    }
    blocks.push(Block::Anchor("block-anchor".into()));
    blocks.push(para(links));
    let mut document = document(blocks);
    document.footnotes.push(crate::Footnote {
        label: "f".into(),
        blocks: vec![plain("note")],
    });
    for settings in [Settings::default(), all()] {
        let (parts, _) = write(document.clone(), &settings);
        let rendered = render(&parts, &settings);
        let ids = values(&rendered, "id");
        for href in values(&rendered, "href") {
            if let Some(fragment) = href.strip_prefix('#') {
                let fragment = decoded(fragment);
                assert!(
                    ids.contains(&fragment.as_str()),
                    "no id {fragment:?} in {rendered}"
                );
            }
        }
        assert!(rendered.contains("<h2 id=\"hello-world-1\">"), "{rendered}");
        assert!(rendered.contains("<h6 id=\"deep\">"), "{rendered}");
        assert_eq!(
            values(&rendered, "id")
                .iter()
                .filter(|id| **id == "custom")
                .count(),
            1
        );
    }
}

fn list(start: Option<u64>, items: Vec<Vec<Block>>) -> Block {
    Block::List {
        start,
        items: items
            .into_iter()
            .map(|blocks| Item {
                checked: None,
                blocks,
            })
            .collect(),
    }
}

fn task(checked: bool, blocks: Vec<Block>) -> Item {
    Item {
        checked: Some(checked),
        blocks,
    }
}

#[test]
fn lists_side_by_side_stay_apart() {
    let blocks = vec![
        list(None, vec![vec![plain("a")]]),
        list(None, vec![vec![plain("b")]]),
        list(None, vec![vec![plain("c")]]),
        list(Some(1), vec![vec![plain("d")]]),
        list(Some(2), vec![vec![plain("e")]]),
    ];
    let expected = "<ul>\n<li>a</li>\n</ul>\n<ul>\n<li>b</li>\n</ul>\n<ul>\n<li>c</li>\n</ul>\n\
        <ol>\n<li>d</li>\n</ol>\n<ol start=\"2\">\n<li>e</li>\n</ol>\n";
    for settings in every() {
        assert_eq!(html(blocks.clone(), &settings), expected);
    }
}

#[test]
fn lists_are_tight_unless_an_item_needs_a_blank_line() {
    let nested = vec![plain("a"), list(None, vec![vec![plain("b")], vec![]])];
    let tight = list(Some(1), vec![nested, vec![plain("c")], vec![]]);
    let expected =
        "<ol>\n<li>a\n<ul>\n<li>b</li>\n<li></li>\n</ul>\n</li>\n<li>c</li>\n<li></li>\n</ol>\n";
    assert_eq!(html(vec![tight], &Settings::default()), expected);

    let loose = list(None, vec![vec![plain("a"), plain("b")], vec![plain("c")]]);
    let expected = "<ul>\n<li>\n<p>a</p>\n<p>b</p>\n</li>\n<li>\n<p>c</p>\n</li>\n</ul>\n";
    assert_eq!(html(vec![loose], &Settings::default()), expected);

    // A list numbered from 3 can't start on the line after a paragraph.
    let nested = vec![plain("a"), list(Some(3), vec![vec![plain("b")]])];
    let expected = "<ul>\n<li>\n<p>a</p>\n<ol start=\"3\">\n<li>b</li>\n</ol>\n</li>\n</ul>\n";
    assert_eq!(
        html(vec![list(None, vec![nested])], &Settings::default()),
        expected
    );
}

#[test]
fn tasks_are_checkboxes_where_task_lists_are_on() {
    let items = vec![
        task(false, vec![plain("a")]),
        task(true, vec![]),
        task(
            false,
            vec![Block::Code {
                language: String::new(),
                text: "x".into(),
            }],
        ),
    ];
    let blocks = vec![Block::List { start: None, items }];
    let rendered = html(blocks.clone(), &Settings::default());
    assert_eq!(
        rendered.matches("type=\"checkbox\"").count(),
        3,
        "{rendered}"
    );
    assert_eq!(rendered.matches("checked=\"\"").count(), 1, "{rendered}");
    let rendered = html(blocks, &none());
    assert!(rendered.contains("<li>\n<p>[ ] a</p>\n</li>"), "{rendered}");
    assert!(rendered.contains("<li>\n<p>[x]</p>\n</li>"), "{rendered}");
}

#[test]
fn quotes_hold_their_blocks_and_who_said_them() {
    let quote = Block::Quote {
        blocks: vec![
            plain("a"),
            list(None, vec![vec![plain("b")]]),
            plain("c"),
            Block::Quote {
                blocks: vec![plain("d")],
                attribution: vec![],
            },
        ],
        attribution: vec![
            text("Some "),
            Inline::Styled(Kind::Emphasis, vec![text("one")]),
        ],
    };
    let expected = "<blockquote>\n<p>a</p>\n<ul>\n<li>b</li>\n</ul>\n<p>c</p>\n\
        <blockquote>\n<p>d</p>\n</blockquote>\n<p>— Some <em>one</em></p>\n</blockquote>\n";
    for settings in every() {
        assert_eq!(html(vec![quote.clone()], &settings), expected);
    }
    let empty = Block::Quote {
        blocks: vec![],
        attribution: vec![],
    };
    let rendered = html(vec![plain("a"), empty, plain("b")], &Settings::default());
    assert_eq!(
        rendered,
        "<p>a</p>\n<blockquote>\n</blockquote>\n<p>b</p>\n"
    );
}

fn alert(kind: AlertKind, title: &str, blocks: Vec<Block>) -> Block {
    Block::Alert {
        kind,
        title: title.to_string(),
        blocks,
    }
}

#[test]
fn alerts_are_githubs_else_quotes_with_their_titles() {
    let blocks = vec![
        alert(AlertKind::Warning, "", vec![plain("a")]),
        alert(AlertKind::Tip, "Mind <the>\n& gap \\ here", vec![]),
        list(
            None,
            vec![vec![alert(
                AlertKind::Caution,
                "",
                vec![plain("b"), plain("c")],
            )]],
        ),
    ];
    let expected = "<div class=\"markdown-alert markdown-alert-warning\">\n\
        <p class=\"markdown-alert-title\">Warning</p>\n<p>a</p>\n</div>\n\
        <div class=\"markdown-alert markdown-alert-tip\">\n\
        <p class=\"markdown-alert-title\">Mind &lt;the&gt; &amp; gap \\ here</p>\n</div>\n\
        <ul>\n<li>\n<div class=\"markdown-alert markdown-alert-caution\">\n\
        <p class=\"markdown-alert-title\">Caution</p>\n<p>b</p>\n<p>c</p>\n</div>\n</li>\n</ul>\n";
    assert_eq!(html(blocks.clone(), &Settings::default()), expected);
    let expected = "<blockquote>\n<p><strong>Warning</strong></p>\n<p>a</p>\n</blockquote>\n\
        <blockquote>\n<p><strong>Mind &lt;the&gt; &amp; gap \\ here</strong></p>\n</blockquote>\n\
        <ul>\n<li>\n<blockquote>\n<p><strong>Caution</strong></p>\n<p>b</p>\n<p>c</p>\n\
        </blockquote>\n</li>\n</ul>\n";
    assert_eq!(html(blocks, &none()), expected);
}

fn definition(term: &str, blocks: Vec<Block>) -> Definition {
    Definition {
        term: vec![text(term)],
        blocks,
    }
}

#[test]
fn definitions_are_a_description_list_else_html_else_a_list() {
    let blocks = vec![
        Block::Definitions(vec![
            definition("t1", vec![plain("d1")]),
            definition("t2", vec![plain("d2")]),
        ]),
        plain("x"),
        Block::Definitions(vec![definition("t3", vec![plain("d3"), plain("d4")])]),
        list(
            None,
            vec![vec![Block::Definitions(vec![definition(
                "t4",
                vec![plain("d5")],
            )])]],
        ),
    ];
    let expected = "<dl>\n<dt>t1</dt>\n<dd>d1</dd>\n<dt>t2</dt>\n<dd>d2</dd>\n</dl>\n<p>x</p>\n\
        <dl>\n<dt>t3</dt>\n<dd>\n<p>d3</p>\n<p>d4</p>\n</dd>\n</dl>\n\
        <ul>\n<li>\n<dl>\n<dt>t4</dt>\n<dd>d5</dd>\n</dl>\n</li>\n</ul>\n";
    assert_eq!(html(blocks.clone(), &all()), expected);

    let expected = "<dl>\n<dt>\n<p>t1</p>\n</dt>\n<dd>\n<p>d1</p>\n</dd>\n\
        <dt>\n<p>t2</p>\n</dt>\n<dd>\n<p>d2</p>\n</dd>\n</dl>\n<p>x</p>\n\
        <dl>\n<dt>\n<p>t3</p>\n</dt>\n<dd>\n<p>d3</p>\n<p>d4</p>\n</dd>\n</dl>\n\
        <ul>\n<li>\n<dl>\n<dt>\n<p>t4</p>\n</dt>\n<dd>\n<p>d5</p>\n</dd>\n</dl>\n</li>\n</ul>\n";
    assert_eq!(html(blocks.clone(), &Settings::default()), expected);

    let expected = "<ul>\n<li>\n<p><strong>t1</strong></p>\n<p>d1</p>\n</li>\n\
        <li>\n<p><strong>t2</strong></p>\n<p>d2</p>\n</li>\n</ul>\n<p>x</p>\n\
        <ul>\n<li>\n<p><strong>t3</strong></p>\n<p>d3</p>\n<p>d4</p>\n</li>\n</ul>\n\
        <ul>\n<li>\n<ul>\n<li>\n<p><strong>t4</strong></p>\n<p>d5</p>\n</li>\n</ul>\n</li>\n</ul>\n";
    assert_eq!(html(blocks, &none()), expected);

    // A term with nothing to say can't be written as Markdown.
    let blocks = vec![Block::Definitions(vec![
        definition("", vec![plain("d1")]),
        definition("t2", vec![]),
    ])];
    let expected = "<dl>\n<dd>\n<p>d1</p>\n</dd>\n<dt>\n<p>t2</p>\n</dt>\n</dl>\n";
    assert_eq!(html(blocks, &all()), expected);
}

fn cell(text: &str) -> Cell {
    if text.is_empty() {
        vec![]
    } else {
        vec![plain(text)]
    }
}

#[test]
fn tables_are_gfms_with_a_line_in_each_cell() {
    let table = Block::Table(Table {
        line: Some(3),
        align: vec![Align::Left, Align::Center, Align::Right],
        header: Some(vec![cell("a|b"), cell("`c`"), cell("")]),
        rows: vec![
            vec![cell("d")],
            vec![
                cell("e"),
                vec![plain("f"), plain("g")],
                cell("h\\|"),
                cell("i"),
            ],
        ],
    });
    let (text, warnings) = markdown(vec![table.clone()], &Settings::default());
    let expected = "<table>\n<thead>\n<tr>\n<th align=\"left\">a|b</th>\n\
        <th align=\"center\">`c`</th>\n<th align=\"right\"></th>\n<th></th>\n</tr>\n</thead>\n\
        <tbody>\n<tr>\n<td align=\"left\">d</td>\n<td align=\"center\"></td>\n\
        <td align=\"right\"></td>\n<td></td>\n</tr>\n<tr>\n<td align=\"left\">e</td>\n\
        <td align=\"center\">f<br>g</td>\n<td align=\"right\">h\\|</td>\n<td>i</td>\n</tr>\n\
        </tbody>\n</table>\n";
    assert_eq!(
        render(&[Part::Markdown(text)], &Settings::default()),
        expected
    );
    let flattened = "wrote each cell of a table on a line, as a cell in Markdown holds no more";
    assert_eq!(warnings, [flattened]);

    let settings = Settings {
        raw_html: false,
        ..Settings::default()
    };
    let rendered = html(vec![table.clone()], &settings);
    assert!(
        rendered.contains("<td align=\"center\">f g</td>"),
        "{rendered}"
    );

    let (text, warnings) = markdown(vec![table], &none());
    let expected = "<ul>\n<li><strong>a|b</strong> | <strong>`c`</strong> |</li>\n<li>d</li>\n\
        <li>e | f<br />\ng | h\\| | i</li>\n</ul>\n";
    assert_eq!(render(&[Part::Markdown(text)], &none()), expected);
    assert_eq!(
        warnings,
        [flattened, "wrote tables as lists, as `tables` is off"]
    );

    // Markdown's tables need a row of headers, which the theme hides.
    let table = Block::Table(Table {
        rows: vec![vec![cell("a")]],
        ..Table::default()
    });
    let expected = "<table>\n<thead>\n<tr>\n<th></th>\n</tr>\n</thead>\n\
        <tbody>\n<tr>\n<td>a</td>\n</tr>\n</tbody>\n</table>\n";
    assert_eq!(html(vec![table], &Settings::default()), expected);
    assert_eq!(
        html(vec![Block::Table(Table::default())], &Settings::default()),
        ""
    );
}

fn code(language: &str, text: &str) -> Block {
    Block::Code {
        language: language.to_string(),
        text: text.to_string(),
    }
}

#[test]
fn code_reads_back_as_it_was() {
    let texts = [
        "",
        "x",
        "x\n",
        "\n\nx\n\n",
        "a ``` b\n````\n",
        "~~~\n```",
        "\tx\n    y\n  \n\tz",
        "<div>\n\n</div>",
        "- [ ] *a*\n> b\n",
    ];
    for settings in every() {
        for text in texts {
            // A block of code ends with a line break.
            let mut shown = escaped(text);
            if !shown.is_empty() && !shown.ends_with('\n') {
                shown.push('\n');
            }
            let expected = format!("<code>{shown}</code>");
            let blocks = vec![code("", text)];
            let rendered = html(blocks.clone(), &settings);
            assert!(rendered.contains(&expected), "{text:?} as {rendered}");
            let nested = |blocks| vec![list(Some(1), vec![blocks])];
            let rendered = html(nested(blocks.clone()), &settings);
            assert!(
                rendered.contains(&expected),
                "{text:?} in a list as {rendered}"
            );
            let quoted = vec![Block::Quote {
                blocks,
                attribution: vec![],
            }];
            let rendered = html(quoted, &settings);
            assert!(
                rendered.contains(&expected),
                "{text:?} in a quote as {rendered}"
            );
        }
    }
    let rendered = html(
        vec![code("rust,ignore `x`\nmore", "x")],
        &Settings::default(),
    );
    assert_eq!(
        rendered,
        "<pre><code class=\"language-rust,ignore\">x\n</code></pre>\n"
    );
}

#[test]
fn formulas_are_math_where_math_is_on() {
    let blocks = vec![
        Block::Math(" x^2\n".into()),
        Block::Math(" \n".into()),
        Block::Rule,
    ];
    let (text, warnings) = markdown(blocks.clone(), &Settings::default());
    assert_eq!(text, "```math\nx^2\n```\n\n___\n");
    assert!(warnings.is_empty());
    let (_, warnings) = markdown(blocks, &none());
    assert_eq!(warnings, ["wrote formulas as code, as `math` is off"]);
}

#[test]
fn anchors_are_ids_else_links_lead_to_the_heading_before_them() {
    let blocks = vec![
        heading(2, vec![text("H")], &[]),
        plain("a"),
        Block::Anchor("x".into()),
        para(vec![link(Target::Id("x".into()), vec![text("b")])]),
    ];
    let expected = "<h2 id=\"h\">H<a href=\"#h\" aria-label=\"Link to heading 'H'\" \
        data-heading-content=\"H\" class=\"anchor\"></a></h2>\n<p>a</p>\n<div id=\"x\"></div>\n\
        <p><a href=\"#x\">b</a></p>\n";
    assert_eq!(html(blocks.clone(), &Settings::default()), expected);
    let rendered = html(blocks, &none());
    assert!(
        rendered.ends_with("<p>a</p>\n<p><a href=\"#h\">b</a></p>\n"),
        "{rendered}"
    );
}

fn footnote(label: &str, blocks: Vec<Block>) -> crate::Footnote {
    crate::Footnote {
        label: label.to_string(),
        blocks,
    }
}

fn reference(label: &str) -> Inline {
    Inline::Footnote(label.to_string())
}

/// Each part `document` is written as, with its kind, and what it warns of.
fn parts(document: Document, settings: &Settings) -> (Vec<String>, Vec<String>) {
    let (parts, warnings) = write(document, settings);
    let parts = parts
        .into_iter()
        .map(|part| match part {
            Part::Markdown(text) => format!("markdown: {text}"),
            Part::Html(html) => format!("html: {html}"),
        })
        .collect();
    (parts, warnings.into_iter().map(|w| w.message).collect())
}

#[test]
fn footnotes_are_defined_in_each_part_that_refers_to_them() {
    let mut page = document(vec![
        para(vec![text("x"), reference("a"), reference("missing")]),
        Block::Html {
            html: "<hr>".into(),
            line: None,
        },
        para(vec![
            text("y"),
            reference("a"),
            text(" z"),
            reference("empty"),
        ]),
    ]);
    page.footnotes = vec![
        footnote(
            "a",
            vec![para(vec![text("see"), reference("b")]), plain("more")],
        ),
        footnote("b", vec![plain("b")]),
        footnote("empty", vec![]),
        footnote("unused", vec![plain("u")]),
    ];
    let (written, warnings) = parts(page.clone(), &Settings::default());
    let expected = [
        "markdown: x[^1]\n\n[^1]:\n    see[^2]\n\n    more\n\n[^2]:\n    b\n",
        "html: <hr>",
        "markdown: y[^3] z[^4]\n\n[^3]:\n    see[^5]\n\n    more\n\n[^4]:\n\n[^5]:\n    b\n",
    ];
    assert_eq!(written, expected);
    let missing = "left out a reference to footnote `missing`, which isn't defined";
    assert_eq!(warnings, [missing]);

    let (written, _) = write(page.clone(), &Settings::default());
    let rendered = render(&written, &Settings::default());
    let ids = values(&rendered, "id");
    for href in values(&rendered, "href") {
        let fragment = href.strip_prefix('#').unwrap();
        assert!(ids.contains(&fragment), "no id {fragment:?} in {rendered}");
    }
    for name in 1..=5 {
        assert!(ids.contains(&format!("fn-{name}").as_str()), "{rendered}");
    }

    let (written, warnings) = parts(page, &none());
    assert_eq!(written, ["markdown: x\n\ny z\n"]);
    let off = [
        "left out footnotes, as `footnotes` is off",
        "left out raw HTML, as `raw_html` is off",
    ];
    assert_eq!(warnings, off);
}

fn raw(html: &str) -> Block {
    Block::Html {
        html: html.to_string(),
        line: Some(1),
    }
}

#[test]
fn html_at_the_top_is_a_part_of_its_own() {
    let mut page = document(vec![
        raw("<a>"),
        plain("b"),
        raw("<c>"),
        raw("<d>"),
        plain("e"),
        raw("<f>"),
    ]);
    page.frontmatter = "title: T".into();
    page.metadata = vec![
        ("title".into(), "U".into()),
        ("description".into(), "A \"page\"".into()),
    ];
    let frontmatter = "---\ntitle: T\ndescription: \"A \\\"page\\\"\"\n---\n";
    let (written, warnings) = parts(page.clone(), &Settings::default());
    let expected = [
        format!("markdown: {frontmatter}"),
        "html: <a>".into(),
        "markdown: b\n".into(),
        "html: <c>".into(),
        "html: <d>".into(),
        "markdown: e\n".into(),
        "html: <f>".into(),
    ];
    assert_eq!(written, expected);
    assert!(warnings.is_empty());

    let (written, warnings) = parts(page.clone(), &none());
    assert_eq!(written, [format!("markdown: {frontmatter}\nb\n\ne\n")]);
    assert_eq!(warnings, ["left out raw HTML, as `raw_html` is off"]);

    page.blocks = vec![Block::Math(" ".into())];
    assert_eq!(parts(page, &none()).0, [format!("markdown: {frontmatter}")]);
    assert!(parts(document(vec![]), &none()).0.is_empty());
}

#[test]
fn html_within_a_block_is_one_block_of_html() {
    let item = vec![plain("a"), raw("\n<b>x</b>\n\n  \n<i>y</i>\n\n")];
    let blocks = vec![list(None, vec![item])];
    let expected = "<ul>\n<li>\n<p>a</p>\n<div>\n<b>x</b>\n<!-- -->\n<!-- -->\n<i>y</i>\n\
        </div>\n</li>\n</ul>\n";
    assert_eq!(html(blocks.clone(), &Settings::default()), expected);
    let (text, warnings) = markdown(blocks, &none());
    assert_eq!(text, "- a\n");
    assert_eq!(warnings, ["left out raw HTML, as `raw_html` is off"]);
}

#[test]
fn settings_are_read_from_the_sites_markdown() {
    let json = r#"{"tables": false, "superscript": true, "raw_html": "yes",
        "math": "mathml", "mermaid": "svg", "highlight": "pygments"}"#;
    let expected = Settings {
        tables: false,
        superscript: true,
        ..Settings::default()
    };
    assert_eq!(Settings::from_json(json), Ok(expected));
    assert!(!Settings::from_json(r#"{"math": false}"#).unwrap().math);
    assert!(Settings::from_json(r#"{"math": "tex"}"#).unwrap().math);
    assert_eq!(
        Settings::from_json("[]"),
        Err("`markdown` isn't a map".into())
    );
    assert!(Settings::from_json("{").is_err());
}

#[test]
fn frontmatter_is_split_from_the_top_of_a_page() {
    let page = "---\n# a comment\n\ntitle: Guide\n---\nText\n";
    let expected = ("# a comment\n\ntitle: Guide\n", "Text\n", 5);
    assert_eq!(split_frontmatter(page), Some(expected));
    let page = "---\r\ntitle:\r\n  Guide\r\n---\r\n";
    assert_eq!(
        split_frontmatter(page),
        Some(("title:\r\n  Guide\r\n", "", 4))
    );
    // A title over- and underlined, as reStructuredText writes one, isn't
    // frontmatter, nor is a list, or YAML that doesn't end.
    for page in [
        "---\nTitle\n---\n",
        "---\nA title: with a colon\n",
        "---\n- a\n---\n",
        "---\n---\n",
        "text\n---\ntitle: x\n---\n",
    ] {
        assert_eq!(split_frontmatter(page), None, "{page:?}");
    }
}

#[test]
fn warnings_name_the_page_and_line() {
    let warning = Warning::new(Some(3), "left out raw HTML");
    assert_eq!(
        warning.display("docs/guide.rst"),
        "docs/guide.rst:3: left out raw HTML"
    );
    let warning = Warning::new(None, "left out raw HTML");
    assert_eq!(
        warning.display("docs/guide.rst"),
        "docs/guide.rst: left out raw HTML"
    );
}

#[test]
fn a_plugin_has_no_settings_but_read() {
    assert_eq!(check_settings("{}", "Org"), Ok(()));
    assert_eq!(check_settings(r#"{ "read": ["docs"] }"#, "Org"), Ok(()));
    assert_eq!(
        check_settings(r#"{ "exports": "both" }"#, "Org"),
        Err(
            "it has no setting `exports`, only `read`, which it doesn't need: little gives it \
             each Org file"
                .to_string()
        )
    );
    assert_eq!(
        check_settings("[]", "Org"),
        Err("its settings must be a mapping, not []".to_string())
    );
}
