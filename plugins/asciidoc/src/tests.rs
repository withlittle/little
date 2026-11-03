//! Checks what each of AsciiDoc's constructs is written as, rendered with
//! comrak as little renders it, with little's settings, with every extension
//! on, and with none; and what's warned of, on which line.

use little_markup::{Part, Settings};

use super::{check_settings, read};

/// The site's `markdown` settings, as little gives them, with each of
/// little's defaults.
const DEFAULTS: &str = "{}";

/// Every extension on, but smart punctuation, which changes text.
const ALL: &str = r#"{ "superscript": true, "subscript": true, "mark": true, "insert": true,
    "description_lists": true }"#;

/// Every extension off.
const NONE: &str = r#"{ "tables": false, "strikethrough": false, "autolinks": false,
    "task_lists": false, "footnotes": false, "alerts": false, "emoji": false,
    "raw_html": false, "math": false }"#;

fn options(settings: &Settings) -> comrak::Options<'static> {
    let mut options = comrak::Options::default();
    let extension = &mut options.extension;
    extension.table = settings.tables;
    extension.strikethrough = settings.strikethrough;
    extension.autolink = settings.autolinks;
    extension.tasklist = settings.task_lists;
    extension.footnotes = settings.footnotes;
    extension.alerts = settings.alerts;
    extension.shortcodes = settings.emoji;
    extension.math_dollars = settings.math;
    extension.math_code = settings.math;
    extension.superscript = settings.superscript;
    extension.subscript = settings.subscript;
    extension.highlight = settings.mark;
    extension.insert = settings.insert;
    extension.description_lists = settings.description_lists;
    extension.inline_footnotes = settings.inline_footnotes;
    extension.cjk_friendly_emphasis = settings.cjk_friendly_emphasis;
    extension.header_id_prefix = Some(String::new());
    options.parse.smart = settings.smart_punctuation;
    options.render.r#unsafe = settings.raw_html;
    options
}

/// The parts of `source`, read with `markdown`, and what's warned of, each
/// as `line: message`.
fn read_with(source: &str, markdown: &str) -> (Vec<Part>, Vec<String>) {
    let (parts, warnings) = read(source, markdown).unwrap();
    let warnings = warnings
        .into_iter()
        .map(|warning| match warning.line {
            Some(line) => format!("{line}: {}", warning.message),
            None => warning.message,
        })
        .collect();
    (parts, warnings)
}

/// The HTML comrak renders `parts` as, with `markdown`, without the first
/// part's frontmatter, which little reads itself.
fn render(parts: &[Part], markdown: &str) -> String {
    let options = options(&Settings::from_json(markdown).unwrap());
    parts
        .iter()
        .map(|part| match part {
            Part::Markdown(text) => {
                let text = little_markup::split_frontmatter(text)
                    .map_or(text.as_str(), |(_, body, _)| body);
                comrak::markdown_to_html(text, &options)
            }
            Part::Html(html) => html.clone(),
        })
        .collect()
}

/// The Markdown `source` is written as, with `markdown`, which must be one
/// part, and what's warned of.
fn markdown_with(source: &str, markdown: &str) -> (String, Vec<String>) {
    let (parts, warnings) = read_with(source, markdown);
    let [Part::Markdown(text)] = parts.as_slice() else {
        panic!("not one part of Markdown: {parts:?}");
    };
    (text.clone(), warnings)
}

/// The Markdown `source` is written as, with little's settings, which must
/// be one part, with nothing warned of.
fn md(source: &str) -> String {
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(warnings, Vec::<String>::new(), "{source}");
    text
}

/// The HTML `source` is rendered as, with `markdown`, and what's warned of.
fn html_with(source: &str, markdown: &str) -> (String, Vec<String>) {
    let (parts, warnings) = read_with(source, markdown);
    (render(&parts, markdown), warnings)
}

/// The HTML `source` is rendered as, with little's settings, with nothing
/// warned of.
fn html(source: &str) -> String {
    let (html, warnings) = html_with(source, DEFAULTS);
    assert_eq!(warnings, Vec::<String>::new(), "{source}");
    html
}

/// What's warned of in `source`, with little's settings.
fn warnings(source: &str) -> Vec<String> {
    read_with(source, DEFAULTS).1
}

/// little's manual has a page on the plugin, in AsciiDoc, and is built with
/// `--strict`, which fails on anything a plugin warns of.
#[test]
fn reads_the_manual_page_without_warnings() {
    let source = include_str!("../../../docs/plugins/asciidoc.adoc");
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(warnings, Vec::<String>::new());
    assert!(
        text.starts_with("---\ndescription: \"Build each AsciiDoc file into a page, "),
        "{text}"
    );
    assert!(text.contains("\n# AsciiDoc\n"), "{text}");
}

#[test]
fn has_no_settings_but_read() {
    assert_eq!(check_settings("{}"), Ok(()));
    assert_eq!(check_settings(r#"{ "read": ["docs"] }"#), Ok(()));
    let error = check_settings(r#"{ "doctype": "book" }"#).unwrap_err();
    assert!(error.contains("no setting `doctype`"), "{error}");
    assert!(check_settings("[]").is_err());
}

#[test]
fn stops_on_markdown_settings_it_cant_read() {
    assert!(read("= Title", "[]").is_err());
}

#[test]
fn reads_the_title_and_description() {
    let source = "= The *Title* of `little`\n\
                  Jane Doe <jane@example.com>\n\
                  v1.0, 2024-01-01\n\
                  :description: How *little* & AsciiDoc meet.\n\n\
                  Text.\n";
    assert_eq!(
        md(source),
        "---\ndescription: \"How *little* & AsciiDoc meet.\"\n---\n\n\
         # The **Title** of `little`\n\nText.\n"
    );
}

#[test]
fn reads_a_page_without_a_title() {
    assert_eq!(md("Just text.\n"), "Just text.\n");
}

#[test]
fn keeps_yaml_frontmatter_and_counts_its_lines() {
    let source = "---\nsearch: false\n---\n= Title\n:description: Said here.\n\n\
                  include::other.adoc[]\n";
    let (parts, warnings) = read_with(source, DEFAULTS);
    assert_eq!(
        parts,
        [Part::Markdown(
            "---\nsearch: false\ndescription: \"Said here.\"\n---\n\n# Title\n".to_string()
        )]
    );
    assert_eq!(
        warnings,
        ["7: left out `include::other.adoc[]`, as the plugin doesn't follow includes"]
    );
}

#[test]
fn lets_frontmatter_set_the_description() {
    let source = "---\ndescription: From YAML.\n---\n= Title\n:description: From AsciiDoc.\n";
    assert_eq!(md(source), "---\ndescription: From YAML.\n---\n\n# Title\n");
}

#[test]
fn writes_text_as_its_written() {
    let source = "Text like * # | $ :smile: _x and <b>tags</b> & [brackets] shows as written.\n";
    assert_eq!(
        html(source),
        "<p>Text like * # | $ :smile: _x and &lt;b&gt;tags&lt;/b&gt; &amp; [brackets] shows as \
         written.</p>\n"
    );
}

#[test]
fn reads_inline_markup() {
    let source = "*strong*, _emphasis_, `code`, #mark#, ^super^, ~sub~, \
                  [.underline]#under#, [.line-through]#struck# and [.big]#big#.\n";
    assert_eq!(
        md(source),
        "**strong**, *emphasis*, `code`, <mark>mark</mark>, <sup>super</sup>, <sub>sub</sub>, \
         <u>under</u>, ~~struck~~ and big.\n"
    );
    let (all, warnings) = markdown_with(source, ALL);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        all,
        "**strong**, *emphasis*, `code`, ==mark==, ^super^, ~sub~, <u>under</u>, ~~struck~~ and \
         big.\n"
    );
}

#[test]
fn reads_constrained_and_unconstrained_markup() {
    assert_eq!(
        md("**un**constrained, __it__alic, ``mono``spaced and a * b * c.\n"),
        "**un**constrained, *it*alic, `mono`spaced and a \\* b \\* c.\n"
    );
}

#[test]
fn reads_line_breaks() {
    assert_eq!(md("One +\ntwo\n"), "One\\\ntwo\n");
    assert_eq!(md("[%hardbreaks]\nOne\ntwo\n"), "One\\\ntwo\n");
    assert_eq!(md("One\ntwo\n"), "One two\n");
}

#[test]
fn reads_replacements_and_references() {
    let source = ":product: little\n\n\
                  (C) {product} -- it's... &amp; &#169; &hellip; A{nbsp}B {missing}\n";
    // Asciidoctor puts thin spaces around a spaced em dash, and a zero-width
    // space after an ellipsis, so a line can break there.
    assert_eq!(
        md(source),
        "© little\u{2009}—\u{2009}it’s…\u{200b} \\& © … A\u{a0}B {missing}\n"
    );
}

#[test]
fn reads_passthroughs() {
    assert_eq!(
        md("+*not strong*+ and `+{x}+`\n"),
        "\\*not strong\\* and `{x}`\n"
    );
    assert_eq!(
        md("pass:[<kbd>K</kbd>] and +++<i>i</i>+++\n"),
        "<span></span><kbd>K</kbd> and <i>i</i>\n"
    );
    let (text, warnings) = markdown_with("Then pass:[<kbd>K</kbd>].\n", NONE);
    assert_eq!(text, "Then .\n");
    assert_eq!(warnings, ["1: left out raw HTML, as `raw_html` is off"]);
}

#[test]
fn reads_sections_at_their_levels() {
    let source = "= Title\n\n== Two\n\n=== Three\n\n==== Four\n\n===== Five\n\n====== Six\n";
    assert_eq!(
        md(source),
        "# Title\n\n## <a id=\"_two\"></a>Two\n\n### <a id=\"_three\"></a>Three\n\n\
         #### <a id=\"_four\"></a>Four\n\n##### <a id=\"_five\"></a>Five\n\n\
         ###### <a id=\"_six\"></a>Six\n"
    );
}

#[test]
fn keeps_a_sections_ids() {
    let source = "[[first]]\n== First section\n\n[#second]\n== Second section\n\n\
                  :idprefix:\n:idseparator: -\n\n== Third section\n";
    assert_eq!(
        md(source),
        "## <a id=\"first\"></a>First section\n\n## <a id=\"second\"></a>Second section\n\n\
         ## Third section\n"
    );
}

#[test]
fn numbers_sections_and_appendices() {
    let source = ":sectnums:\n\n== One\n\n=== One and a half\n\n[discrete]\n== Aside\n\n\
                  == Two\n\n[appendix]\n== Extra\n";
    let text = md(source);
    assert!(text.contains("## <a id=\"_one\"></a>1. One\n"), "{text}");
    assert!(
        text.contains("### <a id=\"_one_and_a_half\"></a>1.1. One and a half\n"),
        "{text}"
    );
    assert!(text.contains("## <a id=\"_aside\"></a>Aside\n"), "{text}");
    assert!(text.contains("## <a id=\"_two\"></a>2. Two\n"), "{text}");
    assert!(
        text.contains("## <a id=\"_extra\"></a>Appendix A\\: Extra\n"),
        "{text}"
    );
}

#[test]
fn reads_titles_that_look_like_other_markup() {
    let source = "== * Not a list\n\n== NOTE: Not an alert\n\n== .Not a title\n\n\
                  == // Not a comment\n\n== A{nbsp}space\n";
    let text = md(source);
    for heading in [
        "\\* Not a list",
        "NOTE\\: Not an alert",
        ".Not a title",
        "// Not a comment",
        "A\u{a0}space",
    ] {
        assert!(
            text.contains(&format!("</a>{heading}\n")),
            "{heading}: {text}"
        );
    }
}

#[test]
fn warns_of_what_the_parser_warns_of() {
    let source = "= Title\n\n==== Too deep\n";
    assert_eq!(
        warnings(source),
        ["3: section heading level skipped (expected 1, found 3)"]
    );
}

#[test]
fn reads_links() {
    let source = "https://example.com, https://example.com[Example], \
                  link:guide.adoc[the guide], link:other.rst[], \
                  mailto:jane@example.com[Jane], jane@example.com and \
                  https://example.com/a_b_c[*bold* text].\n";
    assert_eq!(
        md(source),
        "[https\\://example.com](<https://example.com>), [Example](<https://example.com>), \
         [the guide](<guide.adoc>), [other.rst](<other.rst>), \
         [Jane](<mailto:jane@example.com>), [jane\\@example.com](<mailto:jane@example.com>) and \
         [**bold** text](<https://example.com/a_b_c>).\n"
    );
}

#[test]
fn reads_cross_references_to_sections() {
    let source = "== First section\n\n[[sec-2]]\n== Second\n\n\
                  See <<_first_section>>, <<First section>>, xref:_first_section[], \
                  <<sec-2,the second>> and xref:sec-2[it].\n";
    let text = md(source);
    assert!(
        text.ends_with(
            "See [First section](<#first-section>), [First section](<#first-section>), \
             [First section](<#first-section>), [the second](<#second>) and \
             [it](<#second>).\n"
        ),
        "{text}"
    );
}

#[test]
fn reads_cross_references_to_anchors() {
    let source = "[[para]]\nA paragraph with [[spot]]a spot, and anchor:place[] a place.\n\n\
                  .Example title\n[#ex]\n====\nIn an example.\n====\n\n\
                  See <<para,the paragraph>>, <<spot,the spot>>, <<place,the place>> and <<ex>>.\n";
    assert_eq!(
        md(source),
        "<div id=\"para\"></div>\n\nA paragraph with <a id=\"spot\"></a>a spot, and \
         <a id=\"place\"></a> a place.\n\n<div id=\"ex\"></div>\n\n*Example 1. Example title*\n\n\
         In an example.\n\nSee [the paragraph](<#para>), [the spot](<#spot>), \
         [the place](<#place>) and [Example title](<#ex>).\n"
    );
}

#[test]
fn keeps_cross_references_to_other_pages() {
    let source = "See <<other.adoc#intro,the intro>>, <<other#x,X>>, <<other.adoc#,Other>>, \
                  xref:guide.adoc#setup[setup] and xref:notes.org[notes].\n";
    assert_eq!(
        md(source),
        "See [the intro](<other.adoc#intro>), [X](<other.adoc#x>), [Other](<other.adoc>), \
         [setup](<guide.adoc#setup>) and [notes](<notes.org>).\n"
    );
}

#[test]
fn clamps_sections_a_level_offset_pushes_too_deep() {
    let source = ":leveloffset: +1\n\n== One\n\n===== Four\n\n====== Five\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(
        text,
        "### <a id=\"_one\"></a>One\n\n###### <a id=\"_four\"></a>Four\n\n\
         ###### <a id=\"_five\"></a>Five\n"
    );
    assert_eq!(
        warnings,
        [
            "5: section heading level skipped (expected 3, found 5)",
            "7: section heading level 6 is outside the supported range 1-5; clamped to 5"
        ]
    );
}

#[test]
fn reads_lists() {
    let source = "* One\n** One and a half\n*** Deeper\n* Two\n\nThen:\n\n\
                  . First\n.. Inner\n. Second\n\nAnd:\n\n\
                  [start=4]\n. Fourth\n. Fifth\n";
    assert_eq!(
        md(source),
        "- One\n  - One and a half\n    - Deeper\n- Two\n\nThen\\:\n\n\
         1. First\n   1. Inner\n2. Second\n\nAnd\\:\n\n\
         4. Fourth\n5. Fifth\n"
    );
}

#[test]
fn nests_a_list_of_another_kind_as_asciidoctor_does() {
    let source = "- One\n- Two\n\n1. First\n2. Second\n\n//\n\n* Three\n";
    assert_eq!(
        md(source),
        "- One\n- Two\n  1. First\n  2. Second\n\n* Three\n"
    );
}

#[test]
fn reads_list_items_with_blocks() {
    let source = "* One\n+\nMore about one.\n+\n----\ncode\n----\n* Two\n";
    assert_eq!(
        md(source),
        "- One\n\n  More about one.\n\n  ```\n  code\n  ```\n\n- Two\n"
    );
}

#[test]
fn reads_checklists() {
    let source = "* [x] Done\n* [*] Also done\n* [ ] To do\n";
    assert_eq!(md(source), "- [x] Done\n- [x] Also done\n- [ ] To do\n");
}

#[test]
fn reads_description_lists() {
    let source = "CPU:: The brain.\nRAM::\nMemory.\nDisk;; Storage.\n\n\
                  Term::\nOne.\nInner::: Nested.\n";
    let (all, warnings) = markdown_with(source, ALL);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        all,
        "CPU\n\n: The brain.\n\nRAM\n\n: Memory.\n\n  Disk\n  : Storage.\n\n\
         Term\n\n: One.\n\n  Inner\n  : Nested.\n"
    );
    let html = html(source);
    assert!(
        html.starts_with("<dl>\n<dt>\n<p>CPU</p>\n</dt>\n<dd>\n<p>The brain.</p>\n"),
        "{html}"
    );
    assert!(
        html.contains("<dt>\n<p>Disk</p>\n</dt>\n<dd>\n<p>Storage.</p>\n</dd>"),
        "{html}"
    );
}

#[test]
fn reads_description_lists_with_shared_terms() {
    let html = html("One::\nTwo:: Both.\n");
    assert_eq!(
        html,
        "<dl>\n<dt>\n<p>One</p>\n</dt>\n<dt>\n<p>Two</p>\n</dt>\n<dd>\n<p>Both.</p>\n</dd>\n</dl>\n"
    );
}

#[test]
fn reads_callout_lists() {
    let source = "[source,ruby]\n----\nputs 'hello' # <1>\nexit # <2>\n----\n\
                  <1> Says hello.\n<2> Stops.\n";
    assert_eq!(
        md(source),
        "```ruby\nputs 'hello' # <1>\nexit # <2>\n```\n\n1. Says hello.\n2. Stops.\n"
    );
}

#[test]
fn reads_code() {
    let source = "[source,rust]\n----\nfn main() {}\n----\n\n\
                  ----\nplain *listing*\n----\n\n\
                  ```python\nprint(1)\n```\n\n\
                  [listing]\nA listing paragraph.\n";
    assert_eq!(
        md(source),
        "```rust\nfn main() {}\n```\n\n```\nplain *listing*\n```\n\n\
         ```python\nprint(1)\n```\n\n```\nA listing paragraph.\n```\n"
    );
}

#[test]
fn reads_source_paragraphs_as_written() {
    let source = "[source,ruby]\nputs *x* {nbsp} <b>\n";
    assert_eq!(md(source), "```ruby\nputs *x* {nbsp} <b>\n```\n");
}

#[test]
fn gives_code_the_source_language() {
    let source = ":source-language: python\n\n[source]\n----\nprint(1)\n----\n\n\
                  ```\nprint(2)\n```\n";
    assert_eq!(
        md(source),
        "```python\nprint(1)\n```\n\n```python\nprint(2)\n```\n"
    );
}

#[test]
fn reads_literal_blocks() {
    let source = "....\nliteral  *text*\n....\n\n  An indented\n    literal paragraph.\n\n\
                  [literal]\nA literal paragraph.\n";
    assert_eq!(
        md(source),
        "```\nliteral  *text*\n```\n\n```\nAn indented\n  literal paragraph.\n```\n\n\
         ```\nA literal paragraph.\n```\n"
    );
}

#[test]
fn writes_code_with_backticks_in_it() {
    let source = "----\n```\nfenced\n```\n----\n";
    assert_eq!(html(source), "<pre><code>```\nfenced\n```\n</code></pre>\n");
}

#[test]
fn reads_admonitions() {
    let source = "NOTE: A note.\n\nTIP: A tip.\n\nIMPORTANT: Important.\n\n\
                  WARNING: A warning.\n\nCAUTION: Careful.\n";
    assert_eq!(
        md(source),
        "> [!NOTE]\n> A note.\n\n> [!TIP]\n> A tip.\n\n> [!IMPORTANT]\n> Important.\n\n\
         > [!WARNING]\n> A warning.\n\n> [!CAUTION]\n> Careful.\n"
    );
}

#[test]
fn reads_admonition_blocks_with_titles() {
    let source = ".Before you start\n[WARNING]\n====\nOne.\n\n* Two\n====\n\n\
                  :tip-caption: Hint\n\nTIP: Captioned.\n";
    assert_eq!(
        md(source),
        "> [!WARNING] Before you start\n> One.\n>\n> - Two\n\n> [!TIP] Hint\n> Captioned.\n"
    );
}

#[test]
fn falls_back_from_admonitions_without_alerts() {
    let (text, warnings) = markdown_with("NOTE: A note.\n", NONE);
    assert_eq!(text, "> **Note**\n>\n> A note.\n");
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn reads_quotes_and_verses() {
    let source = "[quote,Ada Lovelace,Notes]\n____\nThe engine weaves.\n____\n\n\
                  [verse,A Poet]\n____\nOne line\nand another\n\nA second stanza\n____\n";
    assert_eq!(
        md(source),
        "> The engine weaves.\n>\n> — Ada Lovelace, *Notes*\n\n\
         > One line\\\n> and another\n>\n> A second stanza\n>\n> — A Poet\n"
    );
}

#[test]
fn reads_quote_paragraphs_and_markdown_quotes() {
    let source = "[quote,Someone]\nSaid briefly.\n\n> Quoted\n> *in* Markdown's way.\n";
    assert_eq!(
        md(source),
        "> Said briefly.\n>\n> — Someone\n\n> Quoted **in** Markdown’s way.\n"
    );
}

#[test]
fn reads_air_quotes() {
    assert_eq!(
        md("\"I said so.\"\n-- Someone\n"),
        "> I said so.\n>\n> — Someone\n"
    );
}

#[test]
fn gives_lines_in_markdown_quotes() {
    let source = "Before.\n\n> One.\n>\n> include::other.adoc[]\n";
    assert_eq!(
        warnings(source),
        ["5: kept `include::other.adoc[]` as text, as the plugin doesn't follow includes"]
    );
}

#[test]
fn writes_the_contents_of_examples_sidebars_and_open_blocks() {
    let source = "====\nIn an example.\n====\n\n****\nIn a sidebar.\n****\n\n\
                  --\nIn an open block.\n--\n\n.A sidebar\n****\nTitled.\n****\n";
    assert_eq!(
        md(source),
        "In an example.\n\nIn a sidebar.\n\nIn an open block.\n\n*A sidebar*\n\nTitled.\n"
    );
}

#[test]
fn writes_block_titles_before_blocks() {
    let source = ".Install it\n----\nlittle build\n----\n\n.A list\n* One\n\n\
                  .A paragraph's title\nThe paragraph.\n";
    assert_eq!(
        md(source),
        "*Install it*\n\n```\nlittle build\n```\n\n*A list*\n\n- One\n\n\
         *A paragraph’s title*\n\nThe paragraph.\n"
    );
}

#[test]
fn numbers_titles_as_asciidoctor_does() {
    let source = ".One\n====\nA.\n====\n\n.Two\n====\nB.\n====\n\n.A table\n|===\n|x\n|===\n\n\
                  :example-caption!:\n\n.Three\n====\nC.\n====\n";
    let text = md(source);
    assert!(
        text.starts_with("*Example 1. One*\n\nA.\n\n*Example 2. Two*\n\nB.\n\n"),
        "{text}"
    );
    assert!(text.contains("*Table 1. A table*"), "{text}");
    assert!(text.ends_with("*Three*\n\nC.\n"), "{text}");
}

#[test]
fn writes_breaks() {
    assert_eq!(
        md("One.\n\n'''\n\nTwo.\n\n<<<\n\nThree.\n\ntoc::[]\n"),
        "One.\n\n___\n\nTwo.\n\nThree.\n"
    );
}

#[test]
fn leaves_out_comments() {
    let source = "One. // not a comment\n// A comment.\n\n////\nA comment block.\n////\n\nTwo.\n";
    assert_eq!(md(source), "One. // not a comment\n\nTwo.\n");
}

#[test]
fn reads_images() {
    let source = "image::diagram.png[A diagram]\n\n\
                  image::logo.svg[The logo,link=https://example.com]\n\n\
                  .A figure\nimage::pic-of_me.png[]\n\n\
                  An inline image:icon.png[Icon] and image:https://example.com/x.png[].\n";
    assert_eq!(
        md(source),
        "![A diagram](<diagram.png>)\n\n\
         [![The logo](<logo.svg>)](<https://example.com>)\n\n\
         ![pic of me](<pic-of_me.png>)\n\n*Figure 1. A figure*\n\n\
         An inline ![Icon](<icon.png>) and ![x](<https://example.com/x.png>).\n"
    );
}

#[test]
fn finds_images_in_the_imagesdir() {
    let source = ":imagesdir: images/\n\nimage::a&b.png[A & B]\n\nimage::/root.png[Root]\n\n\
                  image:https://example.com/x.png[X]\n";
    assert_eq!(
        md(source),
        "![A \\& B](<images/a&amp;b.png>)\n\n![Root](</root.png>)\n\n\
         ![X](<https://example.com/x.png>)\n"
    );
}

#[test]
fn reads_videos_and_audio() {
    let source = "video::intro.mp4[]\n\nvideo::abc123[youtube]\n\naudio::talk.mp3[]\n";
    let parts = read_with(source, DEFAULTS).0;
    assert_eq!(
        parts,
        [
            Part::Html("<video src=\"intro.mp4\" controls></video>".to_string()),
            Part::Html(
                "<iframe src=\"https://www.youtube.com/embed/abc123\" allowfullscreen></iframe>"
                    .to_string()
            ),
            Part::Html("<audio src=\"talk.mp3\" controls></audio>".to_string()),
        ]
    );
}

#[test]
fn reads_footnotes() {
    let source = "One.footnote:[The first.] Two.footnote:second[The *second*.] \
                  Again.footnote:second[]\n";
    assert_eq!(
        md(source),
        "One.[^1] Two.[^2] Again.[^2]\n\n[^1]:\n    The first.\n\n[^2]:\n    The **second**.\n"
    );
}

#[test]
fn warns_of_footnotes_that_arent_defined() {
    let (text, warnings) = markdown_with("Text.\n\nMore.footnote:missing[]\n", DEFAULTS);
    assert_eq!(text, "Text.\n\nMore.\n");
    assert_eq!(
        warnings,
        ["3: left out a reference to footnote `missing`, which isn't defined"]
    );
}

#[test]
fn reads_tables() {
    let source = ".Sizes\n[cols=\"<,^,>\",options=header]\n|===\n|Name |Size |Price\n\n\
                  |Small |*1* |`$1`\n|Large |2 |$2 \\| more\n|===\n";
    assert_eq!(
        md(source),
        "*Table 1. Sizes*\n\n| Name | Size | Price |\n| --- | :-: | --: |\n\
         | Small | **1** | `$1` |\n| Large | 2 | \\$2 \\| more |\n"
    );
}

#[test]
fn reads_tables_with_implicit_headers_and_footers() {
    let source = "|===\n|A |B\n\n|1 |2\n|3 |4\n|===\n\n\
                  [%footer]\n|===\n|x |y\n|Total |10\n|===\n";
    assert_eq!(
        md(source),
        "| A | B |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n\n\
         |  |  |\n| --- | --- |\n| x | y |\n| Total | 10 |\n"
    );
}

#[test]
fn reads_csv_and_dsv_tables() {
    let source = "[%header,format=csv]\n|===\nName,Size\n\"Small, really\",1\n|===\n\n\
                  ,===\na,b\n,===\n\n:===\nc:d\n:===\n";
    assert_eq!(
        md(source),
        "| Name | Size |\n| --- | --- |\n| Small, really | 1 |\n\n\
         |  |  |\n| --- | --- |\n| a | b |\n\n\
         |  |  |\n| --- | --- |\n| c | d |\n"
    );
}

#[test]
fn writes_merged_cells_in_their_first_places() {
    let source = "[cols=3*]\n|===\n|A |B |C\n\n2+|Wide |c\n.2+|Tall |b |c\n|b |c\n|===\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(
        text,
        "| A | B | C |\n| --- | --- | --- |\n| Wide |  | c |\n| Tall | b | c |\n|  | b | c |\n"
    );
    let merged = "wrote a merged cell in the first of its places, and left the rest empty, \
                  as Markdown's tables can't merge cells";
    assert_eq!(warnings, [format!("5: {merged}"), format!("6: {merged}")]);
}

#[test]
fn flattens_blocks_in_asciidoc_cells() {
    let source = "|===\n|Cell\n\na|\n* One\n* Two\n|===\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(text, "| Cell |\n| --- |\n| • One<br>• Two |\n");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].starts_with("1: wrote each cell of a table on a line"),
        "{warnings:?}"
    );
}

#[test]
fn falls_back_from_tables_without_tables() {
    let (text, warnings) = markdown_with("|===\n|A |B\n\n|1 |2\n|===\n", NONE);
    assert!(!text.contains("| --- |"), "{text}");
    assert_eq!(warnings, ["wrote tables as lists, as `tables` is off"]);
}

#[test]
fn reads_latex_math() {
    let source = "[latexmath]\n++++\n\\sum_{i=1}^n i\n++++\n\n\
                  Inline latexmath:[x^2] and more.\n";
    assert_eq!(
        md(source),
        "```math\n\\sum_{i=1}^n i\n```\n\nInline $`x^2`$ and more.\n"
    );
}

#[test]
fn reads_stem_as_latex_when_the_document_says_so() {
    let source = ":stem: latexmath\n\n[stem]\n++++\na + b\n++++\n\nThen stem:[c].\n";
    assert_eq!(md(source), "```math\na + b\n```\n\nThen $`c`$.\n");
}

#[test]
fn keeps_asciimath_as_code() {
    let source = "Inline stem:[sqrt(4)] and asciimath:[x].\n\n[stem]\n++++\nsum_(i=1)^n i\n++++\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(
        text,
        "Inline `sqrt(4)` and `x`.\n\n```asciimath\nsum_(i=1)^n i\n```\n"
    );
    assert_eq!(
        warnings,
        [
            "1: kept AsciiMath `sqrt(4)` as code, as little writes only TeX math",
            "1: kept AsciiMath `x` as code, as little writes only TeX math",
            "3: kept an AsciiMath formula as code, as little writes only TeX math",
        ]
    );
}

#[test]
fn falls_back_from_math_without_math() {
    let (text, _) = markdown_with("latexmath:[x^2]\n", r#"{"math": false}"#);
    assert_eq!(text, "`x^2`\n");
}

#[test]
fn reads_passthrough_blocks_as_html() {
    let source = "Before.\n\n++++\n<p class=\"custom\">Raw.</p>\n++++\n\nAfter.\n";
    assert_eq!(
        read_with(source, DEFAULTS).0,
        [
            Part::Markdown("Before.\n".to_string()),
            Part::Html("<p class=\"custom\">Raw.</p>".to_string()),
            Part::Markdown("After.\n".to_string()),
        ]
    );
    let (text, warnings) = markdown_with(source, NONE);
    assert_eq!(text, "Before.\n\nAfter.\n");
    assert_eq!(warnings, ["3: left out raw HTML, as `raw_html` is off"]);
}

#[test]
fn leaves_out_includes() {
    let source = "One.\n\ninclude::part.adoc[]\n\n----\ninclude::code.rs[]\nfn main() {}\n----\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(text, "One.\n\n```\nfn main() {}\n```\n");
    assert_eq!(
        warnings,
        [
            "3: left out `include::part.adoc[]`, as the plugin doesn't follow includes",
            "6: left out `include::code.rs[]`, as the plugin doesn't follow includes",
        ]
    );
}

#[test]
fn reads_conditionals() {
    let source = ":shown:\n\nifdef::shown[Shown.]\n\nifndef::shown[]\nHidden.\nendif::[]\n\n\
                  ifeval::[1 < 2]\nEvaluated.\nendif::[]\n";
    assert_eq!(md(source), "Shown.\n\nEvaluated.\n");
}

#[test]
fn warns_of_macros_it_doesnt_read() {
    let source = "Text with\nfoo:bar[baz] in it.\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(text, "Text with foo\\:bar\\[baz\\] in it.\n");
    assert_eq!(
        warnings,
        ["2: kept `foo:bar[baz]` as text, as `foo` isn't a macro the plugin reads"]
    );
}

#[test]
fn reads_ui_macros_when_experimental() {
    let source = ":experimental:\n\nPress kbd:[Ctrl+C], then btn:[OK], in menu:File[Save > As].\n";
    assert_eq!(
        md(source),
        "Press `Ctrl+C`, then **OK**, in File › Save › As.\n"
    );
    let (text, warnings) = markdown_with("Press kbd:[Ctrl+C].\n", DEFAULTS);
    assert_eq!(text, "Press kbd\\:\\[Ctrl+C\\].\n");
    assert_eq!(
        warnings,
        ["1: kept `kbd:[Ctrl+C]` as text, as `kbd` needs `:experimental:`, which isn't set"]
    );
}

#[test]
fn reads_index_terms() {
    assert_eq!(
        md("A ((visible)) term and a (((hidden))) one.\n"),
        "A visible term and a  one.\n"
    );
}

#[test]
fn reads_counters_and_attributes() {
    let source =
        ":name: little\n:nothing:\n\n{name}{nothing}{empty} {counter:n} {counter:n} {set:x:y}\n";
    assert_eq!(md(source), "little 1 2 {set\\:x\\:y}\n");
}

#[test]
fn keeps_references_to_ids_nothing_has() {
    // little's link check warns of these, as it does in Markdown.
    assert_eq!(
        md("See <<nowhere>> and xref:report.pdf[].\n"),
        "See [nowhere](<#nowhere>) and [report.pdf](<report.pdf>).\n"
    );
}

#[test]
fn sorts_warnings_by_line() {
    let source = "Text stem:[x].\n\n== A\n\n==== Deep\n\ninclude::x.adoc[]\n";
    assert_eq!(
        warnings(source),
        [
            "1: kept AsciiMath `x` as code, as little writes only TeX math",
            "5: section heading level skipped (expected 2, found 3)",
            "7: left out `include::x.adoc[]`, as the plugin doesn't follow includes",
        ]
    );
}
