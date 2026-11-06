//! What the tests of the plugins that read a format share. They read a
//! file with the plugin's `read`, and render the parts of its page with
//! comrak, set as little sets it.
//!
//! It's built only for tests, with the `testing` feature, so comrak isn't
//! in a plugin's release.

use crate::{Part, Settings, Warning};

/// The site's `markdown` settings, as little gives them, with each of
/// little's defaults.
pub const DEFAULTS: &str = "{}";

/// Every extension on, but smart punctuation, which changes text.
pub const ALL: &str = r#"{ "superscript": true, "subscript": true, "mark": true, "insert": true,
    "description_lists": true }"#;

/// Every extension off.
pub const NONE: &str = r#"{ "tables": false, "strikethrough": false, "autolinks": false,
    "task_lists": false, "footnotes": false, "alerts": false, "emoji": false,
    "raw_html": false, "math": false }"#;

/// A plugin's `read`: the parts of the page of a file's text, with the
/// site's `markdown` settings, as JSON, and what's warned of.
pub type Read = fn(&str, &str) -> Result<(Vec<Part>, Vec<Warning>), String>;

/// comrak's options, as little sets them for `settings`.
pub fn options(settings: &Settings) -> comrak::Options<'static> {
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
pub fn read_with(read: Read, source: &str, markdown: &str) -> (Vec<Part>, Vec<String>) {
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
pub fn render(parts: &[Part], markdown: &str) -> String {
    let options = options(&Settings::from_json(markdown).unwrap());
    parts
        .iter()
        .map(|part| match part {
            Part::Markdown(text) => {
                let text =
                    crate::split_frontmatter(text).map_or(text.as_str(), |(_, body, _)| body);
                comrak::markdown_to_html(text, &options)
            }
            Part::Html(html) => html.clone(),
        })
        .collect()
}

/// The Markdown `source` is written as, with `markdown`, which must be one
/// part, and what's warned of.
pub fn markdown_with(read: Read, source: &str, markdown: &str) -> (String, Vec<String>) {
    let (parts, warnings) = read_with(read, source, markdown);
    let [Part::Markdown(text)] = parts.as_slice() else {
        panic!("not one part of Markdown: {parts:?}");
    };
    (text.clone(), warnings)
}

/// The Markdown `source` is written as, with little's settings, which must
/// be one part, with nothing warned of.
pub fn md(read: Read, source: &str) -> String {
    let (text, warnings) = markdown_with(read, source, DEFAULTS);
    assert_eq!(warnings, Vec::<String>::new(), "{source}");
    text
}

/// The HTML `source` is rendered as, with `markdown`, and what's warned of.
fn html_with(read: Read, source: &str, markdown: &str) -> (String, Vec<String>) {
    let (parts, warnings) = read_with(read, source, markdown);
    (render(&parts, markdown), warnings)
}

/// The HTML `source` is rendered as, with little's settings, with nothing
/// warned of.
pub fn html(read: Read, source: &str) -> String {
    let (html, warnings) = html_with(read, source, DEFAULTS);
    assert_eq!(warnings, Vec::<String>::new(), "{source}");
    html
}

/// What's warned of in `source`, with little's settings.
pub fn warnings(read: Read, source: &str) -> Vec<String> {
    read_with(read, source, DEFAULTS).1
}
