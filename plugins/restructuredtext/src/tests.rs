//! Checks what each of reStructuredText's constructs is written as, rendered
//! with comrak as little renders it, with little's settings, with every
//! extension on, and with none; and what's warned of, on which line.

use little_markup::{Part, Settings};

use super::{Clock, check_settings, convert, read};

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

/// Reads `source`, and `source` cut off at every 97th character, checking
/// that the plugin reads each without a panic.
fn reads_cut_off(source: &str) {
    let ends = source.char_indices().map(|(at, _)| at).step_by(97);
    for end in ends.chain([source.len()]) {
        read(&source[..end], DEFAULTS).unwrap();
        read(&source[..end], NONE).unwrap();
    }
}

/// Reads each of the `.rst` files in the folder `RST_SAMPLES` names, cut
/// off at every 97th character, and prints how long each takes to read
/// whole.
#[test]
#[ignore]
fn reads_samples() {
    let folder = std::env::var("RST_SAMPLES").expect("RST_SAMPLES names a folder");
    let mut paths: Vec<_> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rst"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty());
    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let start = std::time::Instant::now();
        let (_, warnings) = read(&source, DEFAULTS).unwrap();
        let elapsed = start.elapsed();
        println!(
            "{}: {} bytes, {} warnings, read in {elapsed:?}",
            path.display(),
            source.len(),
            warnings.len()
        );
        reads_cut_off(&source);
    }
}

/// little's manual has a page on the plugin, in reStructuredText, and is
/// built with `--strict`, which fails on anything a plugin warns of.
#[test]
fn reads_the_manual_page_without_warnings() {
    let source = include_str!("../../../docs/plugins/restructuredtext.rst");
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(warnings, Vec::<String>::new());
    assert!(
        text.starts_with("---\ndescription: \"Build each reStructuredText file into a page, "),
        "{text}"
    );
    assert!(text.contains("\n# reStructuredText\n"), "{text}");
    reads_cut_off(source);
}

#[test]
fn has_no_settings_but_read() {
    assert_eq!(check_settings("{}"), Ok(()));
    assert_eq!(check_settings(r#"{ "read": ["docs"] }"#), Ok(()));
    let error = check_settings(r#"{ "tab_width": 4 }"#).unwrap_err();
    assert!(error.contains("no setting `tab_width`"), "{error}");
    assert!(check_settings("[]").is_err());
}

#[test]
fn stops_on_markdown_settings_it_cant_read() {
    assert!(read("Text.", "[]").is_err());
}

#[test]
fn reads_the_title_subtitle_and_description() {
    let source = "=======================\n The *Title* of little\n=======================\n\
                  --------------\n A subtitle\n--------------\n\n\
                  .. meta::\n   :description: How *little* & reST meet.\n   :keywords: docs\n\n\
                  Text.\n";
    assert_eq!(
        md(source),
        "---\ndescription: \"How *little* & reST meet.\"\n---\n\n\
         # The *Title* of little\n\n<a id=\"a-subtitle\"></a>*A subtitle*\n\nText.\n"
    );
}

#[test]
fn reads_a_page_without_a_title() {
    assert_eq!(md("Just text.\n"), "Just text.\n");
}

#[test]
fn reads_the_title_directive_and_the_first_description() {
    let source = ".. title:: Set *Title*\n.. meta::\n   :description: First.\n   :keywords: a\n\
                  .. meta::\n   :description: Second.\n\nText.\n";
    assert_eq!(
        md(source),
        "---\ntitle: \"Set *Title*\"\ndescription: \"First.\"\n---\n\nText.\n"
    );
}

#[test]
fn keeps_yaml_frontmatter_and_counts_its_lines() {
    let source = "---\nsearch: false\n---\nTitle\n=====\n\n.. meta::\n   :description: Said here.\n\n\
                  .. include:: other.rst\n";
    let (parts, warnings) = read_with(source, DEFAULTS);
    assert_eq!(
        parts,
        [Part::Markdown(
            "---\nsearch: false\ndescription: \"Said here.\"\n---\n\n# Title\n".to_string()
        )]
    );
    assert_eq!(
        warnings,
        ["10: left out `.. include:: other.rst`, as the plugin doesn't follow includes"]
    );
}

#[test]
fn lets_frontmatter_set_the_description() {
    let source = "---\ndescription: From YAML.\n---\nTitle\n=====\n\n.. meta::\n   \
                  :description: From reST.\n";
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
    let source = "*emphasis*, **strong**, ``literal``, `title`, :emphasis:`em`, :strong:`st`,\n\
                  :literal:`lit`, :code:`x = 1`, :math:`x^2`, :title-reference:`cite`, :t:`t`,\n\
                  :abbreviation:`abbr` and :acronym:`AC`.\n";
    assert_eq!(
        md(source),
        "*emphasis*, **strong**, `literal`, *title*, *em*, **st**, `lit`, `x = 1`, $`x^2`$, \
         *cite*, *t*, abbr and AC.\n"
    );
}

#[test]
fn reads_subscripts_and_superscripts() {
    let source = ":sub:`b`, :subscript:`b`, :sup:`p` and :superscript:`p`.\n";
    assert_eq!(
        md(source),
        "<sub>b</sub>, <sub>b</sub>, <sup>p</sup> and <sup>p</sup>.\n"
    );
    assert_eq!(markdown_with(source, ALL).0, "~b~, ~b~, ^p^ and ^p^.\n");
}

#[test]
fn links_peps_and_rfcs() {
    assert_eq!(
        md(":pep:`8` and :rfc:`2822#section-3.4`.\n"),
        "[PEP 8](<https://peps.python.org/pep-0008>) and \
         [RFC 2822](<https://tools.ietf.org/html/rfc2822.html#section-3.4>).\n"
    );
}

#[test]
fn reads_inline_markup_only_where_docutils_does() {
    let source =
        "2*x*y, *, \"*\" and ``a``\\ b, re\\ *Struct*\\ ured, \\*escaped\\*, \\`not\\` x\\ y.\n";
    assert_eq!(
        md(source),
        "2\\*x\\*y, *, \"*\" and `a`b, re*Struct*ured, \\*escaped\\*, \\`not\\` xy.\n"
    );
}

#[test]
fn warns_of_inline_markup_left_open() {
    let (text, warnings) = markdown_with("Para *open and ``literal`` and `open\n", DEFAULTS);
    assert_eq!(text, "Para \\*open and `literal` and \\`open\n");
    assert_eq!(
        warnings,
        [
            "1: inline emphasis start-string without end-string",
            "1: inline interpreted text or phrase reference start-string without end-string",
        ]
    );
}

#[test]
fn reads_custom_and_default_roles() {
    let source = ".. role:: raw-html(raw)\n   :format: html\n.. role:: custom\n\
                  .. role:: py(code)\n   :language: python\n.. default-role:: literal\n\n\
                  Text :raw-html:`<i>x</i>`, :custom:`custom`, :py:`len(x)` and `default`.\n";
    assert_eq!(
        md(source),
        "Text <i>x</i>, custom, `len(x)` and `default`.\n"
    );
    let (text, warnings) = markdown_with(source, NONE);
    assert_eq!(text, "Text , custom, `len(x)` and `default`.\n");
    assert_eq!(warnings, ["8: left out raw HTML, as `raw_html` is off"]);
}

#[test]
fn keeps_the_text_of_roles_it_doesnt_read() {
    let (text, warnings) = markdown_with("Text :ref:`target` and :doc:`other`.\n", DEFAULTS);
    assert_eq!(text, "Text target and other.\n");
    assert_eq!(
        warnings,
        [
            "1: kept the text of `:ref:`target``, as `ref` isn't a role the plugin reads",
            "1: kept the text of `:doc:`other``, as `doc` isn't a role the plugin reads",
        ]
    );
}

#[test]
fn reads_sections_at_their_levels() {
    let source = "Title\n=====\n\nOne\n---\n\nText.\n\nTwo\n---\n\nDeep\n~~~~\n\n\
                  Deeper\n++++++\n\nDeepest\n*******\n\nSeventh\n#######\n\nEighth\n^^^^^^\n";
    assert_eq!(
        md(source),
        "# Title\n\n## One\n\nText.\n\n## Two\n\n### Deep\n\n#### Deeper\n\n##### Deepest\n\n\
         ###### Seventh\n\n###### Eighth\n"
    );
}

#[test]
fn writes_sections_from_level_two_without_a_title() {
    let source = "One\n===\n\nTwo\n===\n\nThree\n-----\n\n====\nBack\n====\n\nOver\n====\n";
    // An overline makes a style of its own.
    assert_eq!(
        md(source),
        "## One\n\n## Two\n\n### Three\n\n#### Back\n\n## Over\n"
    );
}

#[test]
fn warns_of_inconsistent_title_styles() {
    let source = "One\n===\n\nTwo\n---\n\nThree\n=====\n\nFour\n~~~~\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(text, "## One\n\n### Two\n\n## Three\n\nFour\n");
    assert_eq!(
        warnings,
        ["10: inconsistent title style: skip from level 1 to 3"]
    );
}

#[test]
fn reads_standalone_links_and_emails() {
    assert_eq!(
        md(
            "See https://example.com/a_(b), <https://example.com>, jane@example.com and\n\
            mailto:jane@example.com.\n"
        ),
        "See [https\\://example.com/a\\_(b](<https://example.com/a_(b>)), \
         \\<[https\\://example.com](<https://example.com>)\\>, \
         [jane\\@example.com](<mailto:jane@example.com>) and \
         [mailto\\:jane\\@example.com](<mailto:jane@example.com>).\n"
    );
}

#[test]
fn reads_references_to_targets_and_sections() {
    let source = "See Python_, `the docs <https://docs.python.org/>`_, `alias <Python_>`_,\n\
                  `Here`_, `Intro`_, `guide <guide.rst#usage>`_ and `README <../README.md>`__.\n\n\
                  .. _Python: https://www.python.org/\n.. _here:\n\nIntro\n-----\n\nText.\n";
    assert_eq!(
        md(source),
        "See [Python](<https://www.python.org/>), [the docs](<https://docs.python.org/>), \
         [alias](<https://www.python.org/>), [Here](<#intro>), [Intro](<#intro>), \
         [guide](<guide.rst#usage>) and [README](<../README.md>).\n\n\
         ## <a id=\"here\"></a>Intro\n\nText.\n"
    );
}

#[test]
fn reads_inline_and_indirect_targets() {
    let source = "See `inline target`_ and _`inline target`, and indirect_ and chain_.\n\n\
                  .. _indirect: Python_\n.. _chain: indirect_\n.. _Python: https://python.org\n";
    assert_eq!(
        md(source),
        "See [inline target](<#inline-target>) and <a id=\"inline-target\"></a>inline target, \
         and [indirect](<https://python.org>) and [chain](<https://python.org>).\n"
    );
}

#[test]
fn reads_anonymous_references_in_order() {
    let source = "See `one <a_>`_, `two`__ and three__.\n\n__ https://two.example.com\n\
                  .. __: https://three.example.com\n.. _a: https://one.example.com\n";
    assert_eq!(
        md(source),
        "See [one](<https://one.example.com>), [two](<https://two.example.com>) and \
         [three](<https://three.example.com>).\n"
    );
}

#[test]
fn gives_targets_before_a_paragraph_anchors() {
    assert_eq!(
        md(".. _a:\n.. _b:\n\nPara with two targets: a_ and b_.\n"),
        "<div id=\"a\"></div>\n\n<div id=\"b\"></div>\n\n\
         Para with two targets\\: [a](<#a>) and [b](<#b>).\n"
    );
}

#[test]
fn lets_an_explicit_target_win_over_a_sections_name() {
    let source = "Sec\n===\n\n`Sec`_ refers to the next section.\n\n.. _sec:\n\nOther\n=====\n";
    assert_eq!(
        md(source),
        "## Sec\n\n[Sec](<#other>) refers to the next section.\n\n\
         ## <a id=\"sec-1\"></a>Other\n"
    );
}

#[test]
fn keeps_references_that_lead_nowhere_as_text() {
    let source = "See nowhere_, `two`__, dup_ and loop_.\n\n.. _dup: https://a.example.com\n\
                  .. _dup: https://b.example.com\n.. _loop: loop2_\n.. _loop2: loop_\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(
        text,
        "See nowhere\\_, \\`two\\`\\_\\_, dup\\_ and loop\\_.\n"
    );
    assert_eq!(
        warnings,
        [
            "1: kept `nowhere_` as text, as no target is named `nowhere`",
            "1: kept the anonymous references as text, as there are 1 of them but 0 anonymous \
             targets",
            "1: kept `dup_` as text, as more than one target is named `dup`",
            "1: kept `loop_` as text, as its targets lead to each other in a loop",
            "4: duplicate explicit target name: \"dup\"",
        ]
    );
}

#[test]
fn reads_bullet_lists() {
    assert_eq!(
        md("- one\n- two\n\n  * nested\n\n+ another list\n"),
        "- one\n- two\n  - nested\n\n* another list\n"
    );
}

#[test]
fn reads_enumerated_lists() {
    let source = "3. three\n4. four\n\n#. auto\n#. auto\n\n(a) alpha\n(b) beta\n\n\
                  i) roman\nii) roman\n\nA. not\na list\n";
    assert_eq!(
        md(source),
        "3. three\n4. four\n5. auto\n6. auto\n\n1) alpha\n2) beta\n\n1. roman\n2. roman\n\n\
         A. not a list\n"
    );
}

#[test]
fn warns_of_a_list_that_ends_without_a_blank_line() {
    let (text, warnings) = markdown_with("- item\ntext\n", DEFAULTS);
    assert_eq!(text, "- item\n\ntext\n");
    assert_eq!(
        warnings,
        ["2: bullet list ends without a blank line; unexpected unindent"]
    );
}

#[test]
fn reads_definition_lists() {
    let source = "term\n  Definition.\nterm : classifier\n  Definition with\n  two lines.\n";
    assert_eq!(
        markdown_with(source, ALL).0,
        "term\n: Definition.\n\nterm \\: classifier\n: Definition with two lines.\n"
    );
    assert_eq!(
        html(source),
        "<dl>\n<dt>\n<p>term</p>\n</dt>\n<dd>\n<p>Definition.</p>\n</dd>\n<dt>\n\
         <p>term : classifier</p>\n</dt>\n<dd>\n<p>Definition with two lines.</p>\n</dd>\n\
         </dl>\n"
    );
    assert_eq!(
        markdown_with(source, NONE).0,
        "- **term**\n\n  Definition.\n\n- **term : classifier**\n\n  Definition with two lines.\n"
    );
}

#[test]
fn reads_field_and_option_lists_as_definitions() {
    let source = ":Field: Value.\n:Another field: With *markup*.\n\n\
                  -a         Option a.\n-b FILE, --both=FILE  Both.\n/V  DOS style.\n";
    assert_eq!(
        markdown_with(source, ALL).0,
        "Field\n: Value.\n\nAnother field\n: With *markup*.\n\n`-a`\n: Option a.\n\n\
         `-b FILE`, `--both=FILE`\n: Both.\n\n`/V`\n: DOS style.\n"
    );
    let source = "-f <fichier-é>  Description.\n";
    assert_eq!(
        markdown_with(source, ALL).0,
        "`-f <fichier-é>`\n: Description.\n"
    );
}

#[test]
fn reads_literal_blocks() {
    let source = "Paragraph::\n\n    literal *text*\n      indented\n\nExpanded form:\n\n::\n\n\
                  \x20   code\n\nPartial form: ::\n\n    code\n\nQuoted::\n\n> quoted\n> lines\n";
    assert_eq!(
        md(source),
        "Paragraph\\:\n\n```\nliteral *text*\n  indented\n```\n\nExpanded form\\:\n\n\
         ```\ncode\n```\n\nPartial form\\:\n\n```\ncode\n```\n\nQuoted\\:\n\n\
         ```\n> quoted\n> lines\n```\n"
    );
}

#[test]
fn reads_doctest_blocks_as_python_sessions() {
    assert_eq!(
        md(">>> print(\"hi\")\nhi\n"),
        "```pycon\n>>> print(\"hi\")\nhi\n```\n"
    );
}

#[test]
fn reads_line_blocks() {
    assert_eq!(
        md("| Line one\n| Line *two*\n| continued\n  here\n"),
        "Line one\\\nLine *two*\\\ncontinued here\n"
    );
}

#[test]
fn reads_block_quotes_and_attributions() {
    let source = "Paragraph.\n\n    Block quote.\n\n    -- Attribution\n\n    Another.\n";
    assert_eq!(
        md(source),
        "Paragraph.\n\n> Block quote.\n>\n> — Attribution\n\n> Another.\n"
    );
}

#[test]
fn warns_of_unexpected_indentation() {
    let (text, warnings) = markdown_with("Text,\nmore text.\n  Indented.\n", DEFAULTS);
    assert_eq!(text, "Text, more text.\n\n> Indented.\n");
    assert_eq!(warnings, ["3: unexpected indentation"]);
}

#[test]
fn reads_transitions() {
    assert_eq!(
        md("Text.\n\n----------\n\nMore text.\n"),
        "Text.\n\n___\n\nMore text.\n"
    );
}

#[test]
fn leaves_out_comments() {
    let source = ".. A comment.\n   Still the comment.\n\n..\n\n   Not a comment: a block quote.\n";
    assert_eq!(md(source), "> Not a comment\\: a block quote.\n");
}

#[test]
fn numbers_footnotes_and_citations_by_their_first_reference() {
    let source = "Text [2]_, [1]_, [#]_, [#named]_, [*]_, [#named]_ and [CIT2002]_.\n\n\
                  .. [1] One.\n.. [2] Two.\n.. [#] Auto.\n.. [#named] Named.\n.. [*] Symbol.\n\
                  .. [CIT2002] A citation.\n";
    assert_eq!(
        md(source),
        "Text [^1], [^2], [^3], [^4], [^5], [^4] and [^6].\n\n[^1]:\n    Two.\n\n\
         [^2]:\n    One.\n\n[^3]:\n    Auto.\n\n[^4]:\n    Named.\n\n[^5]:\n    Symbol.\n\n\
         [^6]:\n    A citation.\n"
    );
    let (text, warnings) = markdown_with(source, NONE);
    assert_eq!(text, "Text , , , , ,  and .\n");
    assert_eq!(warnings, ["left out footnotes, as `footnotes` is off"]);
}

#[test]
fn leaves_out_footnotes_nothing_refers_to() {
    let (text, warnings) = markdown_with("Text [2]_.\n\n.. [2] Used.\n.. [3] Unused.\n", DEFAULTS);
    assert_eq!(text, "Text [^1].\n\n[^1]:\n    Used.\n");
    assert_eq!(
        warnings,
        ["4: left out footnote `[3]`, as nothing refers to it"]
    );
}

#[test]
fn reads_substitutions() {
    let source = "|name| and |Name| and |link|_ and |img| and |nested|.\n\n\
                  .. |name| replace:: *replaced*\n.. |link| replace:: link\n\
                  .. _link: https://example.com\n.. |img| image:: pic.png\n   :alt: A picture\n\
                  .. |nested| replace:: |name| inside\n";
    assert_eq!(
        md(source),
        "*replaced* and *replaced* and [link](<https://example.com>) and \
         ![A picture](<pic.png>) and *replaced* inside.\n"
    );
}

#[test]
fn keeps_substitutions_that_lead_nowhere_as_text() {
    let source = "Text |a| and |none|.\n\n.. |a| replace:: x |b|\n.. |b| replace:: y |a|\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(text, "Text x y \\|a\\| and \\|none\\|.\n");
    assert_eq!(
        warnings,
        [
            "1: kept `|none|` as text, as no substitution is named `none`",
            "4: kept `|a|` as text, as its definition refers back to it",
        ]
    );
}

#[test]
fn reads_unicode_substitutions() {
    let source = "Copyright |copy| 2003, |BogusMegaCorp (TM)| |---|\nall rights reserved, |x| b.\n\n\
                  .. |copy| unicode:: 0xA9 .. copyright sign\n\
                  .. |BogusMegaCorp (TM)| unicode:: BogusMegaCorp U+2122\n   .. with trademark sign\n\
                  .. |---| unicode:: U+02014 .. em dash\n   :trim:\n\
                  .. |x| unicode:: &#x41; \\u0042 x43 U+44 1234\n   :ltrim:\n";
    assert_eq!(
        md(source),
        "Copyright © 2003, BogusMegaCorp™—all rights reserved,ABCDӒ b.\n"
    );
}

#[test]
fn warns_of_unicode_it_cant_read() {
    let (text, warnings) = markdown_with("a |x| b\n\n.. |x| unicode:: 0x110000\n", DEFAULTS);
    assert_eq!(text, "a \\|x\\| b\n\n```\nunicode:: 0x110000\n```\n");
    assert_eq!(
        warnings,
        [
            "1: kept `|x|` as text, as no substitution is named `x`",
            "3: kept `.. unicode::` as code: invalid character code: 0x110000",
            "3: substitution definition \"x\" empty or invalid",
        ]
    );
}

#[test]
fn writes_dates_as_strftime_does() {
    let source = ".. |d| date::\n\
                  .. |t| date:: %H:%M:%S %a %A %b %B %y %p %I %j %e %w %u %U %W %-d %c %x %X %%\n\n\
                  |d| |t|\n";
    // 29 February 2024, at 13:05:09, in UTC.
    let (document, warnings) = convert::document(
        source,
        "",
        Clock {
            seconds: 1_709_211_909,
        },
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let (parts, _) = little_markup::write(document, &Settings::from_json(DEFAULTS).unwrap());
    assert_eq!(
        render(&parts, DEFAULTS),
        "<p>2024-02-29 13:05:09 Thu Thursday Feb February 24 PM 01 060 29 4 4 08 09 29 \
         Thu Feb 29 13:05:09 2024 02/29/24 13:05:09 %</p>\n"
    );
}

#[test]
fn dates_a_page_when_its_built() {
    let year: u32 = md("|year|\n\n.. |year| date:: %Y\n")
        .trim()
        .parse()
        .unwrap();
    assert!(year >= 2026, "{year}");
}

#[test]
fn warns_of_dates_it_cant_write() {
    let (text, warned) = markdown_with("|q|\n\n.. |q| date:: %Q and %%\n", DEFAULTS);
    assert_eq!(text, "%Q and %\n");
    assert_eq!(
        warned,
        ["3: kept `%Q` in `.. date::`, as the plugin doesn't know it"]
    );
    assert_eq!(
        warnings(".. date::\n"),
        ["1: kept `.. date::` as code: it can only be used within a substitution definition"]
    );
}

#[test]
fn reads_admonitions_as_alerts() {
    let source = ".. note:: A note.\n\n.. tip::\n\n   A tip.\n\n.. hint:: Hint.\n.. important:: Important.\n\
                  .. warning:: Warning.\n.. attention:: Attention.\n.. caution:: Caution.\n\
                  .. danger:: Danger.\n.. error:: Error.\n.. admonition:: Custom *title*\n\n   Its text.\n";
    assert_eq!(
        md(source),
        "> [!NOTE]\n> A note.\n\n> [!TIP]\n> A tip.\n\n> [!TIP] Hint\n> Hint.\n\n\
         > [!IMPORTANT]\n> Important.\n\n> [!WARNING]\n> Warning.\n\n\
         > [!WARNING] Attention\n> Attention.\n\n> [!WARNING] Caution\n> Caution.\n\n\
         > [!CAUTION] Danger\n> Danger.\n\n> [!CAUTION] Error\n> Error.\n\n\
         > [!NOTE] Custom title\n> Its text.\n"
    );
    let (text, _) = markdown_with(
        ".. note:: A note.\n\n.. admonition:: Custom\n\n   Text.\n",
        NONE,
    );
    assert_eq!(
        text,
        "> **Note**\n>\n> A note.\n\n> **Custom**\n>\n> Text.\n"
    );
}

#[test]
fn reads_images_and_figures() {
    let source = ".. image:: pic.png\n   :alt: Alt text\n   :width: 200px\n\n\
                  .. image:: pic.png\n   :target: https://example.com\n\n\
                  .. figure:: pic.png\n   :alt: Figure alt\n\n   The caption.\n\n   The legend.\n";
    assert_eq!(
        md(source),
        "![Alt text](<pic.png>)\n\n[![pic.png](<pic.png>)](<https://example.com>)\n\n\
         ![Figure alt](<pic.png>)\n\n*The caption.*\n\nThe legend.\n"
    );
}

#[test]
fn reads_topics_sidebars_and_rubrics() {
    let source = ".. topic:: Topic *title*\n\n   Topic text.\n\n\
                  .. sidebar:: Sidebar\n   :subtitle: Sub\n\n   Sidebar text.\n\n\
                  .. rubric:: A rubric\n";
    assert_eq!(
        md(source),
        "__Topic *title*__\n\nTopic text.\n\n**Sidebar**\n\n*Sub*\n\nSidebar text.\n\n\
         **A rubric**\n"
    );
}

#[test]
fn reads_quotes_compounds_and_containers() {
    let source = ".. epigraph::\n\n   Words.\n\n   -- Someone\n\n.. highlights::\n\n   Highlighted.\n\n\
                  .. pull-quote::\n\n   Pulled.\n\n.. compound::\n\n   Compound one.\n\n   \
                  Compound two.\n\n.. container:: custom\n\n   Contained.\n";
    assert_eq!(
        md(source),
        "> Words.\n>\n> — Someone\n\n> Highlighted.\n\n> Pulled.\n\nCompound one.\n\n\
         Compound two.\n\nContained.\n"
    );
}

#[test]
fn writes_parsed_literals_as_code() {
    assert_eq!(
        md(".. parsed-literal::\n\n   plain text\n"),
        "```\nplain text\n```\n"
    );
    let (text, warnings) = markdown_with(
        ".. parsed-literal::\n\n   some *parsed* text with `link <https://example.com>`_\n",
        DEFAULTS,
    );
    assert_eq!(text, "```\nsome parsed text with link\n```\n");
    assert_eq!(
        warnings,
        [
            "1: wrote `.. parsed-literal::` as code, leaving out its markup, as a code block \
             can't hold any"
        ]
    );
}

#[test]
fn reads_math() {
    let source = ".. math::\n\n   a^2 + b^2 = c^2\n\n.. math:: x\n\n   y\n\nInline :math:`x`.\n";
    let expected = "```math\na^2 + b^2 = c^2\n```\n\n```math\nx\n```\n\n```math\ny\n```\n\n";
    assert_eq!(md(source), format!("{expected}Inline $`x`$.\n"));
    let (text, warnings) = markdown_with(source, NONE);
    assert_eq!(text, format!("{expected}Inline `x`.\n"));
    assert_eq!(warnings, ["wrote formulas as code, as `math` is off"]);
}

#[test]
fn reads_raw_html() {
    let source = ".. raw:: html\n\n   <b>raw</b>\n\n.. raw:: latex\n\n   \\LaTeX\n\nText.\n";
    let (parts, warnings) = read_with(source, DEFAULTS);
    assert_eq!(
        parts,
        [
            Part::Html("<b>raw</b>".to_string()),
            Part::Markdown("Text.\n".to_string())
        ]
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let (text, warnings) = markdown_with(source, NONE);
    assert_eq!(text, "Text.\n");
    assert_eq!(warnings, ["1: left out raw HTML, as `raw_html` is off"]);
}

#[test]
fn reads_code() {
    let source = ".. code:: python\n\n   print(\"hi\")\n\n.. code-block:: rust\n   :number-lines:\n\n   \
                  fn main() {}\n\n.. sourcecode::\n\n   plain\n";
    assert_eq!(
        md(source),
        "```python\nprint(\"hi\")\n```\n\n```rust\nfn main() {}\n```\n\n```\nplain\n```\n"
    );
}

#[test]
fn warns_of_directives_it_leaves_out() {
    let source = ".. sectnum::\n.. header:: Header text.\n.. footer:: Footer text.\n\
                  .. include:: other.rst\n.. target-notes::\n";
    let (parts, warnings) = read_with(source, DEFAULTS);
    assert_eq!(parts, []);
    assert_eq!(
        warnings,
        [
            "1: left out `.. sectnum::`, as the plugin doesn't number sections",
            "2: left out `.. header::`, as the theme writes the page's header and footer",
            "3: left out `.. footer::`, as the theme writes the page's header and footer",
            "4: left out `.. include:: other.rst`, as the plugin doesn't follow includes",
            "5: left out `.. target-notes::`, as the plugin doesn't list the targets' URLs",
        ]
    );
}

#[test]
fn keeps_directives_it_doesnt_read_as_code() {
    let source = "Text.\n\n.. toctree::\n   :maxdepth: 2\n\n   intro\n\nMore text.\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(
        text,
        "Text.\n\n```\n.. toctree::\n   :maxdepth: 2\n\n   intro\n```\n\nMore text.\n"
    );
    assert_eq!(
        warnings,
        ["3: kept `.. toctree::` as code, as `toctree` isn't a directive the plugin reads"]
    );
}

#[test]
fn keeps_directives_written_wrongly_as_code() {
    let source = "Paragraph.\n\n.. image:: x.png\n   :bogus: 1\n\n.. note::\n\n.. topic::\n\n   No title.\n\n\
                  .. code:: python\n\n.. class:: !!\n\nText.\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(
        text,
        "Paragraph.\n\n```\n.. image:: x.png\n   :bogus: 1\n```\n\n```\n.. note::\n```\n\n\
         ```\n.. topic::\n\n   No title.\n```\n\n```\n.. code:: python\n```\n\n\
         ```\n.. class:: !!\n```\n\nText.\n"
    );
    assert_eq!(
        warnings,
        [
            "3: kept `.. image::` as code: unknown option: \"bogus\"",
            "6: kept `.. note::` as code: content block expected; none found",
            "8: kept `.. topic::` as code: 1 argument(s) required, 0 supplied",
            "12: kept `.. code::` as code: content block expected; none found",
            "14: kept `.. class::` as code: invalid class attribute value: \"!!\"",
        ]
    );
}

#[test]
fn leaves_out_classes() {
    assert_eq!(md(".. class:: Hello-World a_b\n\npara\n"), "para\n");
}

#[test]
fn marks_where_the_table_of_contents_would_be() {
    let source = "Title\n=====\n\n.. contents:: Table of *Contents*\n   :depth: 2\n\nOne\n---\n";
    assert_eq!(
        md(source),
        "# Title\n\n<div id=\"table-of-contents\"></div>\n\n## One\n"
    );
    assert_eq!(
        md(".. contents::\n\nOne\n===\n"),
        "<div id=\"contents\"></div>\n\n## One\n"
    );
}

#[test]
fn reads_grid_tables() {
    let source = ".. table:: The *title*\n   :widths: auto\n\n   +------+-----------------+\n   \
                  | Head | Spanning header |\n   +======+========+========+\n   \
                  | a    | b      | c      |\n   +------+--------+--------+\n   \
                  | d    | e spans         |\n   |      | columns         |\n   \
                  +------+-----------------+\n   | f    | * a list        |\n   \
                  |      | * in a cell     |\n   +------+-----------------+\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(
        text,
        "*The title*\n\n| Head | Spanning header |  |\n| --- | --- | --- |\n| a | b | c |\n\
         | d | e spans columns |  |\n| f | • a list<br>• in a cell |  |\n"
    );
    let merged = "wrote a merged cell in the first of its places, and left the rest empty, as \
                  Markdown's tables can't merge cells";
    assert_eq!(
        warnings,
        [
            format!("5: {merged}"),
            format!("9: {merged}"),
            format!("12: {merged}"),
            "4: wrote each cell of a table on a line, as a cell in Markdown holds no more"
                .to_string(),
        ]
    );
}

#[test]
fn reads_simple_tables() {
    let source = "=====  =====  ======\n   Inputs     Output\n------------  ------\n  \
                  A      B    A or B\n=====  =====  ======\nFalse  False  False\n\
                  True   False  True\n=====  =====  ======\n";
    let (text, warnings) = markdown_with(source, DEFAULTS);
    assert_eq!(
        text,
        "| Inputs |  | Output |\n| --- | --- | --- |\n| A | B | A or B |\n\
         | False | False | False |\n| True | False | True |\n"
    );
    assert_eq!(
        warnings,
        [
            "1: joined the table's header rows after the first to its body, as Markdown's \
             tables have one header row",
            "2: wrote a merged cell in the first of its places, and left the rest empty, as \
             Markdown's tables can't merge cells",
        ]
    );
}

#[test]
fn reads_csv_and_list_tables() {
    let source = ".. csv-table:: *Fruit*\n   :header: \"Name\", \"Price\"\n   :widths: 10, 5\n\n   \
                  \"Apple, red\", 1.50\n   Pear, \"0.75\"\n\n\
                  .. list-table:: Pets\n   :header-rows: 1\n\n   * - Name\n     - Kind\n   \
                  * - Rex\n     - A dog\n";
    assert_eq!(
        md(source),
        "*Fruit*\n\n| Name | Price |\n| --- | --- |\n| Apple, red | 1.50 |\n| Pear | 0.75 |\n\n\
         *Pets*\n\n| Name | Kind |\n| --- | --- |\n| Rex | A dog |\n"
    );
}

#[test]
fn keeps_malformed_tables_as_code() {
    let (text, warnings) =
        markdown_with("Para.\n\n+-----+-----+\n| a   | b   |\n+-----+\n", DEFAULTS);
    assert_eq!(
        text,
        "Para.\n\n```\n+-----+-----+\n| a   | b   |\n+-----+\n```\n"
    );
    assert_eq!(
        warnings,
        ["5: kept a malformed table as code: right border not aligned or missing"]
    );
}

#[test]
fn keeps_table_directives_written_wrongly_as_code() {
    let source =
        ".. csv-table::\n\n   \"a\", \"b\n\n.. list-table::\n\n   * - a\n     - b\n   * - c\n";
    assert_eq!(
        warnings(source),
        [
            "1: kept `.. csv-table::` as code: error with its CSV data: unexpected end of data",
            "5: kept `.. list-table::` as code: uniform two-level bullet list expected, but row 2 \
             does not contain the same number of items as row 1 (1 vs 2)",
        ]
    );
}

#[test]
fn writes_tables_as_lists_without_the_extension() {
    let (text, warnings) = markdown_with(
        "=====  =====\na      b\n=====  =====\nc      d\n=====  =====\n",
        NONE,
    );
    assert_eq!(text, "- **a** \\| **b**\n- c \\| d\n");
    assert_eq!(warnings, ["wrote tables as lists, as `tables` is off"]);
}

#[test]
fn promotes_a_lone_section_to_the_subtitle() {
    let source = "Title\n=====\n\nOne\n---\n\nDeep\n~~~~\n\nText.\n";
    assert_eq!(
        md(source),
        "# Title\n\n<a id=\"one\"></a>*One*\n\n## Deep\n\nText.\n"
    );
}

#[test]
fn lets_frontmatter_set_the_title() {
    let source = "---\ntitle: YAML\n---\n.. title:: reST\n\nText.\n";
    assert_eq!(md(source), "---\ntitle: YAML\n---\n\nText.\n");
}

#[test]
fn reads_role_directives_before_their_roles() {
    let (text, warnings) = markdown_with(
        "Text :later:`x`.\n\n.. role:: later\n\nText :later:`y`.\n",
        DEFAULTS,
    );
    assert_eq!(text, "Text x.\n\nText y.\n");
    assert_eq!(
        warnings,
        ["1: kept the text of `:later:`x``, as `later` isn't a role the plugin reads"]
    );
}

#[test]
fn reads_docinfo() {
    let source = "=========\n Title\n=========\n\n:Author: Jane Doe\n\
                  :Version: $Revision: 1.2 $\n:Date: $Date: 2024-01-02 10:00:00 $\n\
                  :Status: Draft\n:Custom: Value\n:Dedication: To you.\n\
                  :Abstract: What it's about.\n\n   In two paragraphs.\n\nText.\n";
    assert_eq!(
        markdown_with(source, ALL).0,
        "# Title\n\nAuthor\n: Jane Doe\n\nVersion\n: 1\\.2\n\nDate\n: 2024-01-02\n\n\
         Status\n: Draft\n\nCustom\n: Value\n\n**Dedication**\n\nTo you.\n\n**Abstract**\n\n\
         What it's about.\n\nIn two paragraphs.\n\nText.\n"
    );
}

#[test]
fn reads_docinfo_after_a_comment_but_not_a_paragraph() {
    let source = "Title\n=====\n\n.. comment\n\n:Version: $Revision: 1.2 $\n";
    assert_eq!(
        markdown_with(source, ALL).0,
        "# Title\n\nVersion\n: 1\\.2\n"
    );
    let source = "Title\n=====\n\nPara.\n\n:Version: $Revision: 1.2 $\n";
    assert_eq!(
        markdown_with(source, ALL).0,
        "# Title\n\nPara.\n\nVersion\n: \\$Revision\\: 1.2 \\$\n"
    );
}

#[test]
fn reads_blocks_nested_too_deep_as_text() {
    let (text, warnings) = markdown_with(&format!("{}item\n", "- ".repeat(40)), DEFAULTS);
    // The 33rd list's item holds the rest as text.
    assert_eq!(text, format!("{}\\- - - - - - - item\n", "- ".repeat(33)));
    assert_eq!(
        warnings,
        ["1: blocks are nested over 32 deep; read the deeper ones as text"]
    );
}

#[test]
fn warns_of_the_parsers_findings_by_line_then_the_writers() {
    let source = "Text :sub:`b` and nowhere_.\n\n.. include:: x.rst\n";
    assert_eq!(
        read_with(source, NONE).1,
        [
            "1: kept `nowhere_` as text, as no target is named `nowhere`",
            "3: left out `.. include:: x.rst`, as the plugin doesn't follow includes",
            "left out subscript, as `subscript` and `raw_html` are off",
        ]
    );
}

#[test]
fn gives_sections_docutils_ids_too() {
    let source =
        "Über straße\n===========\n\nSee `Über straße`_ and `2. Step`_.\n\n2. Step\n=======\n";
    assert_eq!(
        md(source),
        "## <a id=\"uber-strasze\"></a>Über straße\n\n\
         See [Über straße](<#über-straße>) and [2. Step](<#2-step>).\n\n\
         ## <a id=\"step\"></a>2. Step\n"
    );
}
