//! Checks what each part of Org becomes, as little renders it, with comrak
//! set as little sets it.

use little_markup::{Part, Settings};

use super::{check_settings, read};

/// comrak's options, as little sets them for `settings`.
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

/// The parts `org` is read into, with the site's `markdown` settings, as
/// JSON, and what it warns of, each as little shows it, on `page.org`.
fn read_with(org: &str, markdown: &str) -> (Vec<Part>, Vec<String>) {
    let (parts, warnings) = read(org, markdown).unwrap();
    let warnings = warnings
        .iter()
        .map(|warning| warning.display("page.org"))
        .collect();
    (parts, warnings)
}

/// The HTML comrak renders `parts` as, with `settings`, but the first's
/// frontmatter, which little reads, and the links comrak puts in headings.
fn render(parts: &[Part], settings: &Settings) -> String {
    let options = options(settings);
    let mut html: String = parts
        .iter()
        .enumerate()
        .map(|(index, part)| match part {
            Part::Markdown(text) => {
                let text = match little_markup::split_frontmatter(text) {
                    Some((_, rest, _)) if index == 0 => rest,
                    _ => text,
                };
                comrak::markdown_to_html(text, &options)
            }
            Part::Html(html) => html.clone(),
        })
        .collect();
    while let Some(end) = html.find(" class=\"anchor\"></a>") {
        let start = html[..end].rfind("<a href=\"#").expect("a heading's link");
        html.replace_range(start..end + " class=\"anchor\"></a>".len(), "");
    }
    html
}

/// The frontmatter of the page `org` is read into, with little's settings.
fn frontmatter(org: &str) -> String {
    let (parts, _) = read_with(org, "{}");
    let Some(Part::Markdown(text)) = parts.first() else {
        panic!("no Markdown first: {parts:?}");
    };
    little_markup::split_frontmatter(text)
        .map(|(yaml, _, _)| yaml.to_string())
        .unwrap_or_default()
}

/// The HTML of `org`, with little's settings, and what it warns of.
fn warned(org: &str) -> (String, Vec<String>) {
    let (parts, warnings) = read_with(org, "{}");
    (render(&parts, &Settings::default()), warnings)
}

/// The HTML of `org`, with little's settings, which warns of nothing.
fn html(org: &str) -> String {
    let (html, warnings) = warned(org);
    assert_eq!(warnings, Vec::<String>::new(), "warnings of {org:?}");
    html
}

/// The Markdown of `org`, with little's settings, which warns of nothing,
/// in one part.
fn markdown(org: &str) -> String {
    let (parts, warnings) = read_with(org, "{}");
    assert_eq!(warnings, Vec::<String>::new(), "warnings of {org:?}");
    let [Part::Markdown(text)] = parts.as_slice() else {
        panic!("not one part of Markdown: {parts:?}");
    };
    text.clone()
}

/// The HTML of a paragraph of `inline`, which warns of nothing.
fn para(inline: &str) -> String {
    html(inline)
        .strip_prefix("<p>")
        .and_then(|html| html.strip_suffix("</p>\n"))
        .unwrap_or_else(|| panic!("not a paragraph: {inline:?}"))
        .to_string()
}

#[test]
fn reads_the_manual_page_without_warnings() {
    let page = include_str!("../../../docs/plugins/org-mode.org");
    let (parts, warnings) = read_with(page, "{}");
    assert_eq!(warnings, Vec::<String>::new());
    let Some(Part::Markdown(first)) = parts.first() else {
        panic!("no Markdown first: {parts:?}");
    };
    assert!(first.starts_with("---\ndescription: "), "{first}");
    assert!(first.contains("\n# Org mode\n"), "{first}");
}

#[test]
fn has_no_settings_but_read() {
    assert_eq!(check_settings("{}"), Ok(()));
    assert_eq!(check_settings(r#"{ "read": ["docs"] }"#), Ok(()));
    let error = check_settings(r#"{ "exports": "both" }"#).unwrap_err();
    assert!(error.contains("no setting `exports`"), "{error}");
    assert!(check_settings("[]").is_err());
}

#[test]
fn stops_on_markdown_settings_it_cant_read() {
    assert!(read("Text.", "[]").is_err());
}

#[test]
fn reads_the_title_and_description() {
    let org = "#+TITLE: A *bold* title\n#+DESCRIPTION: What it's about.\nText.\n";
    assert_eq!(
        markdown(org),
        "---\ndescription: \"What it's about.\"\n---\n\n# A **bold** title\n\nText.\n"
    );
    assert_eq!(
        html(org),
        "<h1 id=\"a-bold-title\">A <strong>bold</strong> title</h1>\n<p>Text.</p>\n"
    );
}

#[test]
fn leaves_out_other_keywords() {
    let org = "#+AUTHOR: Ann\n#+DATE: 2024\n#+OPTIONS: toc:nil num:nil\n#+STARTUP: overview\n\
               #+LANGUAGE: en\n#+SETUPFILE: setup.org\nText.\n";
    assert_eq!(markdown(org), "Text.\n");
}

#[test]
fn reads_a_page_without_a_title() {
    assert_eq!(
        html("Text.\n* Heading\n"),
        "<p>Text.</p>\n<h2 id=\"heading\">Heading</h2>\n"
    );
}

#[test]
fn keeps_yaml_frontmatter_and_counts_its_lines() {
    let org = "---\nsearch: false\ndescription: From YAML.\n---\n#+DESCRIPTION: From Org.\n\
               #+TITLE: T\n{{{nope}}}\n";
    assert_eq!(frontmatter(org), "search: false\ndescription: From YAML.\n");
    let (html, warnings) = warned(org);
    assert_eq!(html, "<h1 id=\"t\">T</h1>\n<p>{{{nope}}}</p>\n");
    assert_eq!(
        warnings,
        ["page.org:7: kept `{{{nope}}}` as text, as the file doesn't define the macro `nope`"]
    );
}

#[test]
fn reads_headlines_a_level_down() {
    let org = "* One\n** Two\n*** Three\n**** Four\n***** Five\n****** Six\n******* Seven\n";
    assert_eq!(
        html(org),
        "<h2 id=\"one\">One</h2>\n<h3 id=\"two\">Two</h3>\n<h4 id=\"three\">Three</h4>\n\
         <h5 id=\"four\">Four</h5>\n<h6 id=\"five\">Five</h6>\n<h6 id=\"six\">Six</h6>\n\
         <h6 id=\"seven\">Seven</h6>\n"
    );
}

#[test]
fn leaves_out_what_a_headline_has_but_its_title() {
    let org = "#+TODO: TODO WAIT | DONE\n* WAIT [#A] A task  :work:home:\nSCHEDULED: <2024-05-01 Wed>\n\
               :PROPERTIES:\n:EFFORT: 1h\n:END:\n:LOGBOOK:\n- Note taken\n:END:\nText.\n";
    assert_eq!(html(org), "<h2 id=\"a-task\">A task</h2>\n<p>Text.</p>\n");
}

#[test]
fn leaves_out_what_org_leaves_out() {
    let org = "Shown.\n# A comment\n#+BEGIN_COMMENT\nHidden.\n#+END_COMMENT\n\
               * COMMENT Draft\nHidden.\n** Below it\nHidden.\n* Notes :noexport:\nHidden.\n\
               ** Below them\nHidden.\n* Kept\nShown.\n";
    assert_eq!(
        html(org),
        "<p>Shown.</p>\n<h2 id=\"kept\">Kept</h2>\n<p>Shown.</p>\n"
    );
}

#[test]
fn writes_text_as_its_written() {
    let org = "A # hash, | pipe, a :smile:, <b>tag</b>, [text](url), 2 * 3 * 4, `tick`, &amp; \
               and $5 or $6.\n#hashtag\n";
    assert_eq!(
        html(org),
        "<p>A # hash, | pipe, a :smile:, &lt;b&gt;tag&lt;/b&gt;, [text](url), 2 * 3 * 4, `tick`, \
         &amp;amp; and $5 or $6. #hashtag</p>\n"
    );
}

#[test]
fn reads_inline_markup() {
    assert_eq!(
        para("*bold* /italic/ _under_ +strike+ =verb= ~code~ *a /b/*"),
        "<strong>bold</strong> <em>italic</em> <u>under</u> <del>strike</del> \
         <code>verb</code> <code>code</code> <strong>a <em>b</em></strong>"
    );
}

#[test]
fn reads_subscripts_and_superscripts_as_org_does() {
    assert_eq!(
        para("x^2 H_{2}O a_b, H_2O. mc^2. x^{2}_{3} a_-1 b^+ c_."),
        "x<sup>2</sup> H<sub>2</sub>O a<sub>b</sub>, H<sub>2O</sub>. mc<sup>2</sup>. \
         x<sup>2</sup><sub>3</sub> a<sub>-1</sub> b^+ c_."
    );
}

#[test]
fn reads_subscripts_and_superscripts_as_options_say() {
    assert_eq!(
        para("#+OPTIONS: toc:nil ^:{}\na_b c^d e_{f} g^{h}_{i}"),
        "a_b c^d e<sub>f</sub> g<sup>h</sup><sub>i</sub>"
    );
    assert_eq!(para("#+OPTIONS: ^:nil\na_b e_{f}"), "a_b e_{f}");
}

#[test]
fn reads_entities() {
    assert_eq!(
        para("\\alpha, \\to{}, \\Alpha{}bet, \\nbsp{}x, \\S and \\foo"),
        "α, →, Αbet, \u{a0}x, § and \\foo"
    );
}

#[test]
fn reads_special_strings_unless_options_say_not_to() {
    assert_eq!(
        para("Dashes -- and --- and dots... and a\\-b."),
        "Dashes \u{2013} and \u{2014} and dots\u{2026} and a\u{ad}b."
    );
    assert_eq!(
        para("#+OPTIONS: -:nil\nDashes -- and --- and dots..."),
        "Dashes -- and --- and dots..."
    );
}

#[test]
fn reads_line_breaks() {
    assert_eq!(para("one\\\\\ntwo"), "one<br />\ntwo");
}

#[test]
fn reads_math() {
    assert_eq!(
        para("\\(x^2\\), $y$, $a+b$, and $5 or $ a$."),
        "<code data-math-style=\"inline\">x^2</code>, <code data-math-style=\"inline\">y</code>, \
         <code data-math-style=\"inline\">a+b</code>, and $5 or $ a$."
    );
    assert_eq!(
        html("Before $$z$$ and \\[w\\] after."),
        "<p>Before</p>\n\
         <pre><code class=\"language-math\" data-math-style=\"display\">z\n</code></pre>\n\
         <p>and</p>\n\
         <pre><code class=\"language-math\" data-math-style=\"display\">w\n</code></pre>\n\
         <p>after.</p>\n"
    );
}

#[test]
fn reads_latex_environments_as_math() {
    assert_eq!(
        html("\\begin{equation}\nE = mc^2\n\\end{equation}\n"),
        "<pre><code class=\"language-math\" data-math-style=\"display\">\\begin{equation}\n\
         E = mc^2\n\\end{equation}\n</code></pre>\n"
    );
}

#[test]
fn reads_timestamps_as_their_text() {
    assert_eq!(
        para("<2024-05-01 Wed>--<2024-05-02 Thu> and [2024-05-03 Fri 10:00]"),
        "&lt;2024-05-01 Wed&gt;\u{2013}&lt;2024-05-02 Thu&gt; and [2024-05-03 Fri 10:00]"
    );
}

#[test]
fn reads_inline_code() {
    assert_eq!(
        para("src_sh{ls -l} and src_python[:exports both]{1+1}"),
        "<code>ls -l</code> and <code>1+1</code>"
    );
}

#[test]
fn keeps_statistics_cookies_and_targets() {
    assert_eq!(
        para("Done [1/3] and [50%], <<here>>a target and a <<<radio>>> one."),
        "Done [1/3] and [50%], <a id=\"here\"></a>a target and a <a id=\"radio\"></a>radio one."
    );
}

#[test]
fn reads_rules() {
    assert_eq!(
        html("Before.\n\n-----\n\nAfter.\n"),
        "<p>Before.</p>\n<hr />\n<p>After.</p>\n"
    );
}

#[test]
fn reads_links_to_other_sites() {
    assert_eq!(
        para(
            "[[https://example.com][Example]] [[https://example.com]] <https://orgmode.org> \
             see https://example.org/a_(b) or mailto:me@example.com."
        ),
        "<a href=\"https://example.com\">Example</a> \
         <a href=\"https://example.com\">https://example.com</a> \
         <a href=\"https://orgmode.org\">https://orgmode.org</a> see \
         <a href=\"https://example.org/a_(b)\">https://example.org/a_(b)</a> or \
         <a href=\"mailto:me@example.com\">mailto:me@example.com</a>."
    );
}

#[test]
fn keeps_links_to_files_for_little_to_lead_to_their_pages() {
    assert_eq!(
        para(
            "[[file:guide.org][Guide]] [[file:guide.org::#setup]] [[./other.md]] [[file:api.rst]]"
        ),
        "<a href=\"guide.org\">Guide</a> <a href=\"guide.org#setup\">guide.org#setup</a> \
         <a href=\"./other.md\">./other.md</a> <a href=\"api.rst\">api.rst</a>"
    );
    let (html, warnings) = warned("See [[file:notes.org::*Intro][the intro]].\n");
    assert_eq!(html, "<p>See <a href=\"notes.org\">the intro</a>.</p>\n");
    assert_eq!(
        warnings,
        [
            "page.org:1: kept only the file of the link to `file:notes.org::*Intro`, as little \
             links only to a file, or an id in it"
        ]
    );
}

#[test]
fn reads_links_to_headlines() {
    let org = "* Heading\n:PROPERTIES:\n:CUSTOM_ID: here\n:END:\n\
               [[#here]], [[*Heading]] and [[*Heading][the heading]].\n";
    assert_eq!(
        html(org),
        "<h2 id=\"heading\"><a id=\"here\"></a>Heading</h2>\n\
         <p><a href=\"#heading\">Heading</a>, <a href=\"#heading\">Heading</a> and \
         <a href=\"#heading\">the heading</a>.</p>\n"
    );
}

#[test]
fn reads_links_to_targets_names_and_headlines_by_their_text() {
    let org = "<<here>>A target, and [[here]].\n#+NAME: fig\n: code\n\n\
               A [[fig]] link and [[Heading]].\n* Heading\n";
    assert_eq!(
        html(org),
        "<p><a id=\"here\"></a>A target, and <a href=\"#here\">here</a>.</p>\n\
         <div id=\"fig\"></div>\n<pre><code>code\n</code></pre>\n\
         <p>A <a href=\"#fig\">fig</a> link and <a href=\"#heading\">Heading</a>.</p>\n\
         <h2 id=\"heading\">Heading</h2>\n"
    );
}

#[test]
fn reads_links_to_headlines_by_their_id() {
    let org = "* Heading\n:PROPERTIES:\n:ID: 1234-abcd\n:END:\nSee [[id:1234-abcd][it]].\n";
    assert_eq!(
        html(org),
        "<h2 id=\"heading\">Heading</h2>\n<p>See <a href=\"#heading\">it</a>.</p>\n"
    );
}

#[test]
fn keeps_links_it_cant_follow_as_text() {
    let (html, warnings) = warned(
        "[[nothing]] [[id:abc]] [[elisp:(foo)][run it]]\n[[info:emacs]] [[doi:10.1000/182]]\n",
    );
    assert_eq!(
        html,
        "<p>nothing id:abc run it info:emacs \
         <a href=\"https://doi.org/10.1000/182\">https://doi.org/10.1000/182</a></p>\n"
    );
    assert_eq!(
        warnings,
        [
            "page.org:1: kept the link to `nothing` as text, as no target, name, or headline on \
             the page matches it",
            "page.org:1: kept the link to `id:abc` as text, as no headline on the page has its `ID`",
            "page.org:1: kept the link to `elisp:(foo)` as text, as little can't follow it",
            "page.org:2: kept the link to `info:emacs` as text, as little can't follow it",
        ]
    );
}

#[test]
fn reads_images() {
    assert_eq!(
        para(
            "[[./img/cat.png]] [[https://example.com/a.svg]] \
             [[https://example.com][file:thumb.webp]] [[file:cat.jpg][a cat]]"
        ),
        "<img src=\"./img/cat.png\" alt=\"cat.png\" /> \
         <img src=\"https://example.com/a.svg\" alt=\"a.svg\" /> \
         <a href=\"https://example.com\"><img src=\"thumb.webp\" alt=\"thumb.webp\" /></a> \
         <a href=\"cat.jpg\">a cat</a>"
    );
}

#[test]
fn reads_an_images_alt_and_caption() {
    assert_eq!(
        html("#+CAPTION: The cat\n#+ATTR_HTML: :alt A cat on a mat :width 300\n[[file:cat.jpg]]\n"),
        "<p><img src=\"cat.jpg\" alt=\"A cat on a mat\" /></p>\n<p><em>The cat</em></p>\n"
    );
}

#[test]
fn reads_lists() {
    assert_eq!(
        html("- one\n  1. a\n  2) b\n- two\n"),
        "<ul>\n<li>one\n<ol>\n<li>a</li>\n<li>b</li>\n</ol>\n</li>\n<li>two</li>\n</ul>\n"
    );
    assert_eq!(
        html("+ plus\n+ items\n  * star\n"),
        "<ul>\n<li>plus</li>\n<li>items\n<ul>\n<li>star</li>\n</ul>\n</li>\n</ul>\n"
    );
}

#[test]
fn starts_a_list_at_its_first_counter() {
    assert_eq!(
        html("4. [@4] four\n5. five\n"),
        "<ol start=\"4\">\n<li>four</li>\n<li>five</li>\n</ol>\n"
    );
    let (html, warnings) = warned("1. one\n2. [@7] seven\n");
    assert_eq!(html, "<ol>\n<li>one</li>\n<li>seven</li>\n</ol>\n");
    assert_eq!(
        warnings,
        ["page.org:2: left out the counter `[@7]`, as Markdown numbers a list only from its start"]
    );
}

#[test]
fn reads_checkboxes() {
    assert_eq!(
        html("- [X] done\n- [ ] to do\n- [-] partly\n"),
        "<ul>\n<li><input type=\"checkbox\" checked=\"\" disabled=\"\" /> done</li>\n\
         <li><input type=\"checkbox\" disabled=\"\" /> to do</li>\n\
         <li><input type=\"checkbox\" disabled=\"\" /> partly</li>\n</ul>\n"
    );
}

#[test]
fn reads_descriptive_lists_as_definitions() {
    assert_eq!(
        html("- term :: what it means\n- other :: more\n"),
        "<dl>\n<dt>\n<p>term</p>\n</dt>\n<dd>\n<p>what it means</p>\n</dd>\n\
         <dt>\n<p>other</p>\n</dt>\n<dd>\n<p>more</p>\n</dd>\n</dl>\n"
    );
}

#[test]
fn reads_source_blocks_as_code() {
    assert_eq!(
        html("#+NAME: hello\n#+CAPTION: Says hello.\n#+BEGIN_SRC rust\nfn main() {}\n#+END_SRC\n"),
        "<div id=\"hello\"></div>\n<p><em>Says hello.</em></p>\n\
         <pre><code class=\"language-rust\">fn main() {}\n</code></pre>\n"
    );
}

#[test]
fn shows_what_a_source_blocks_exports_say() {
    let block = |exports: &str, results: &str| {
        format!("#+BEGIN_SRC python{exports}\nprint(\"hi\")\n#+END_SRC\n\n{results}After.\n")
    };
    let code = "<pre><code class=\"language-python\">print(&quot;hi&quot;)\n</code></pre>\n";
    let saved = "#+RESULTS:\n#+begin_example\nhi\n#+end_example\n\n";
    let results = "<pre><code>hi\n</code></pre>\n";
    let after = "<p>After.</p>\n";
    assert_eq!(html(&block("", saved)), format!("{code}{after}"));
    assert_eq!(
        html(&block(" :exports code", saved)),
        format!("{code}{after}")
    );
    assert_eq!(
        html(&block(" :exports results", saved)),
        format!("{results}{after}")
    );
    assert_eq!(
        html(&block(" :exports both", saved)),
        format!("{code}{results}{after}")
    );
    assert_eq!(html(&block(" :exports none", saved)), after);
    let (html, warnings) = warned(&block(" :exports results", ""));
    assert_eq!(html, after);
    assert_eq!(
        warnings,
        [
            "page.org:1: left out the block's results, as none are saved, and the plugin doesn't \
             run code"
        ]
    );
}

#[test]
fn reads_exports_from_header_args() {
    let org = "#+PROPERTY: header-args :exports both\n\
               #+PROPERTY: header-args:sh :exports none\n\
               #+BEGIN_SRC python\n1\n#+END_SRC\n\n#+RESULTS:\n: 1\n\n\
               #+BEGIN_SRC sh\nls\n#+END_SRC\n\n#+RESULTS:\n: file\n";
    assert_eq!(
        html(org),
        "<pre><code class=\"language-python\">1\n</code></pre>\n<pre><code>1\n</code></pre>\n"
    );
}

#[test]
fn reads_examples_and_fixed_width_lines_as_code() {
    assert_eq!(
        html("#+BEGIN_EXAMPLE\n,* not a headline\n*not bold* <b>\n#+END_EXAMPLE\n"),
        "<pre><code>* not a headline\n*not bold* &lt;b&gt;\n</code></pre>\n"
    );
    assert_eq!(
        html(": fixed\n:   width\n"),
        "<pre><code>fixed\n  width\n</code></pre>\n"
    );
}

#[test]
fn reads_quotes_verse_and_centered_text() {
    assert_eq!(
        html("#+BEGIN_QUOTE\nQuoted /text/.\n\n-- Someone\n#+END_QUOTE\n"),
        "<blockquote>\n<p>Quoted <em>text</em>.</p>\n<p>– Someone</p>\n</blockquote>\n"
    );
    assert_eq!(
        html("#+BEGIN_VERSE\n  Indented line\nNext line\n\nNew stanza\n#+END_VERSE\n"),
        "<p>\u{a0}\u{a0}Indented line<br />\nNext line</p>\n<p>New stanza</p>\n"
    );
    assert_eq!(
        html("#+BEGIN_CENTER\nCentered.\n#+END_CENTER\n"),
        "<p>Centered.</p>\n"
    );
}

#[test]
fn reads_special_blocks_named_as_alerts_as_alerts() {
    let alert = |kind: &str, title: &str| {
        format!(
            "<div class=\"markdown-alert markdown-alert-{kind}\">\n\
             <p class=\"markdown-alert-title\">{title}</p>\n<p>Text.</p>\n</div>\n"
        )
    };
    for (name, kind, title) in [
        ("NOTE", "note", "Note"),
        ("tip", "tip", "Tip"),
        ("Hint", "tip", "Tip"),
        ("IMPORTANT", "important", "Important"),
        ("warning", "warning", "Warning"),
        ("CAUTION", "caution", "Caution"),
        ("danger", "caution", "Caution"),
        ("ERROR", "caution", "Caution"),
    ] {
        assert_eq!(
            html(&format!("#+BEGIN_{name}\nText.\n#+END_{name}\n")),
            alert(kind, title),
            "{name}"
        );
    }
    assert_eq!(
        html("#+BEGIN_aside\nAn aside.\n#+END_aside\n"),
        "<p>An aside.</p>\n"
    );
}

#[test]
fn keeps_html_and_leaves_out_other_exports() {
    let org = "#+BEGIN_EXPORT html\n<div class=\"x\">raw</div>\n#+END_EXPORT\n\
               #+HTML: <hr class=\"y\">\n\
               #+BEGIN_EXPORT latex\n\\newpage\n#+END_EXPORT\n#+LATEX: \\clearpage\n\
               Press @@html:<kbd>@@C-c@@html:</kbd>@@@@latex:\\LaTeX@@.\n";
    assert_eq!(
        html(org),
        "<div class=\"x\">raw</div><hr class=\"y\"><p>Press <kbd>C-c</kbd>.</p>\n"
    );
}

#[test]
fn leaves_out_html_when_raw_html_is_off() {
    let org = "#+BEGIN_EXPORT html\n<div class=\"x\">raw</div>\n#+END_EXPORT\n\
               #+HTML: <hr class=\"y\">\nPress @@html:<kbd>@@C-c@@html:</kbd>@@.\n";
    let markdown = r#"{"raw_html": false}"#;
    let (parts, warnings) = read_with(org, markdown);
    let settings = Settings::from_json(markdown).unwrap();
    assert_eq!(render(&parts, &settings), "<p>Press C-c.</p>\n");
    assert_eq!(
        warnings,
        [
            "page.org:1: left out raw HTML, as `raw_html` is off",
            "page.org:4: left out raw HTML, as `raw_html` is off",
            "page.org:5: left out raw HTML, as `raw_html` is off",
        ]
    );
}

#[test]
fn reads_tables() {
    // A column is aligned right if at least half its cells, the header's
    // too, are numbers, as Org's export aligns it: `Qty`, but not `Price`.
    let org = "#+CAPTION: Prices.\n| Item | Price | Qty |\n|------+-------+-----|\n\
               | *Tea* | 2.50 | 1 |\n| [[https://example.com][Cake]] | 3 \\vert 4 | 12 |\n";
    assert_eq!(
        markdown(org),
        "*Prices.*\n\n| Item | Price | Qty |\n| --- | --- | --: |\n| **Tea** | 2.50 | 1 |\n\
         | [Cake](<https://example.com>) | 3 \\| 4 | 12 |\n"
    );
}

#[test]
fn aligns_columns_as_their_cookies_say() {
    let org = "| <l> | <c> | <r10> |\n| <5> |     |       |\n| a   | b   | c     |\n\
               |-----+-----+-------|\n| 1x  | 2y  | 3z    |\n";
    assert_eq!(
        markdown(org),
        "| a | b | c |\n| :-- | :-: | --: |\n| 1x | 2y | 3z |\n"
    );
}

#[test]
fn reads_tables_markdown_cant_hold_as_well_as_it_can() {
    assert_eq!(
        markdown("| a | b |\n| c |\n"),
        "|  |  |\n| --- | --- |\n| a | b |\n| c |  |\n"
    );
    let (parts, warnings) = read_with("Text.\n| H1 |\n| H2 |\n|----|\n| b  |\n", "{}");
    assert_eq!(
        parts,
        [Part::Markdown(
            "Text.\n\n| H1 |\n| --- |\n| H2 |\n| b |\n".into()
        )]
    );
    assert_eq!(
        warnings,
        [
            "page.org:3: joined the table's header rows after the first to its body, as \
             Markdown's tables have one header row"
        ]
    );
    let (html, warnings) = warned("+---+---+\n| a | b |\n+---+---+\n");
    assert_eq!(
        html,
        "<pre><code>+---+---+\n| a | b |\n+---+---+\n</code></pre>\n"
    );
    assert_eq!(
        warnings,
        ["page.org:1: kept a table.el table as code, as the plugin reads only Org's own tables"]
    );
}

#[test]
fn reads_footnotes() {
    let org = "[fn:early] Defined first.\n\n\n\
               Text[fn:1], inline[fn::An *inline* note.], named[fn:nm:Named note.], \
               again[fn:nm], early[fn:early].\n\n\
               [fn:1] First paragraph.\n\nSecond paragraph.\n\n\nNot in the note.\n";
    assert_eq!(
        markdown(org),
        "Text[^1], inline[^2], named[^3], again[^3], early[^4].\n\nNot in the note.\n\n\
         [^1]:\n    First paragraph.\n\n    Second paragraph.\n\n\
         [^2]:\n    An **inline** note.\n\n[^3]:\n    Named note.\n\n[^4]:\n    Defined first.\n"
    );
}

#[test]
fn expands_macros() {
    let org = "#+TITLE: Macros\n#+AUTHOR: Ann\n#+EMAIL: ann@example.com\n#+DATE: 2024-05-01\n\
               #+MACRO: greet Hello, $1 and $2!\n#+MACRO: twice $1$1\n\
               {{{greet(World, you)}}} {{{twice(ab)}}} {{{greet(a\\, b)}}}\n\
               {{{title}}}, {{{author}}}, {{{email}}}, {{{date}}}, {{{keyword(AUTHOR)}}}.\n";
    // Org keeps the space after an argument's comma, so `$2` is ` you`.
    assert_eq!(
        markdown(org),
        "# Macros\n\nHello, World and  you! abab Hello, a, b and ! \
         Macros, Ann, ann\\@example.com, 2024-05-01, Ann.\n"
    );
}
