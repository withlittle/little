//! Writing a page's Markdown: text escaped, links, descriptions with their
//! headings moved down, and the ids comrak gives headings.

use std::collections::HashSet;
use std::ops::Range;

use finl_unicode::categories::CharacterCategories;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde_json::Value;

pub use little_api_reference::markdown::{fence_for, table};

/// `text` on one line, with each run of whitespace a space.
pub fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` in code, or `""` if it's empty, so it shows.
pub fn code(text: &str) -> String {
    match text {
        "" => "`\"\"`".into(),
        text => little_api_reference::markdown::code(text),
    }
}

/// `value` in code: a string as it is, and anything else as JSON.
pub fn value(value: &Value) -> String {
    match value {
        Value::String(text) => code(text),
        value => code(&value.to_string()),
    }
}

/// `text` with every character that Markdown, with any of the extensions a
/// site can turn on, might read as markup escaped, for a link's text, where
/// nothing is linked; and `@`, so no email address is.
pub fn escape(text: &str) -> String {
    let mut escaped = String::new();
    for c in collapse(text).chars() {
        if "\\`*_[]<>#&~$^=+:@".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// `text`, which isn't Markdown, like a summary, on one line, escaped as
/// [`escape`] escapes it, but for each URL in it, which an escape would
/// break where comrak links it. An email address isn't linked, as an
/// escape in it would link only its end.
pub fn inline(text: &str) -> String {
    let words: Vec<String> = (text.split_whitespace())
        .map(|word| match linked(word) {
            Some(at) => format!("{}{}", escape(&word[..at]), &word[at..]),
            None => escape(word),
        })
        .collect();
    words.join(" ")
}

/// Where comrak's autolinks would read a URL in `word`, as they read them:
/// from `http://`, `https://`, or `ftp://`, not after a letter, or from
/// `www.`, at the word's start or after one of `*_~([`, to the word's end;
/// but not if there's a `<` or `>` in it, where comrak would end the link
/// with the escape before it, or go on.
fn linked(word: &str) -> Option<usize> {
    word.char_indices().find_map(|(at, _)| {
        let (before, rest) = word.split_at(at);
        let scheme = ["http://", "https://", "ftp://"].iter().find_map(|scheme| {
            let after = rest.strip_prefix(scheme)?;
            (!before.ends_with(|c: char| c.is_ascii_alphabetic())).then_some(after)
        });
        let www = rest
            .strip_prefix("www.")
            .filter(|_| before.is_empty() || before.ends_with(['*', '_', '~', '(', '[']));
        let after = scheme.or(www)?;
        (domain(after) && !rest.contains(['<', '>'])).then_some(at)
    })
}

/// Whether `text` starts with a domain, as comrak's autolinks read one: up
/// to a space, punctuation, or a symbol, but `-`, `_`, or `.`, with a `.`,
/// and no `_` in its last two parts.
fn domain(text: &str) -> bool {
    let (mut dots, mut underscores) = (0, [0, 0]);
    for (at, c) in text.char_indices() {
        match c {
            '\\' if at + 1 < text.len() => {}
            '-' => {}
            '_' => underscores[1] += 1,
            '.' => {
                underscores = [underscores[1], 0];
                dots += 1;
            }
            c if c.is_whitespace() || c.is_punctuation() || c.is_symbol() => break,
            _ => {}
        }
    }
    dots > 0 && underscores == [0, 0]
}

/// A link to `url`, with `text`, which is Markdown.
pub fn link(text: &str, url: &str) -> String {
    let mut destination = String::new();
    for c in url.chars() {
        match c {
            '\\' => destination += "\\\\",
            // comrak reads character references in a link's URL before
            // escapes.
            '&' => destination += "&amp;",
            '<' => destination += "%3C",
            '>' => destination += "%3E",
            '\n' | '\r' => {}
            c => destination.push(c),
        }
    }
    format!("[{text}](<{destination}>)")
}

/// Whether `url` is to another site, on the web.
pub fn web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// A link to `url`, with the URL itself, in code, as its text, if it's to
/// another site; else the URL, in code.
pub fn url(url: &str) -> String {
    match web(url) {
        true => link(&code(url), url),
        false => code(url),
    }
}

/// `items` joined with commas and `or`, like `a, b, or c`.
pub fn either(items: &[String]) -> String {
    joined(items, "or")
}

/// `items` joined with commas and `and`, like `a, b, and c`.
pub fn both(items: &[String]) -> String {
    joined(items, "and")
}

fn joined(items: &[String], word: &str) -> String {
    match items {
        [] => String::new(),
        [item] => item.clone(),
        [a, b] => format!("{a} {word} {b}"),
        [rest @ .., last] => format!("{}, {word} {last}", rest.join(", ")),
    }
}

/// `notes` as sentences: each but the last, like a description, ended with
/// a full stop if it hasn't one.
pub fn sentences(notes: &[String]) -> String {
    let notes: Vec<String> = (notes.iter().enumerate())
        .map(
            |(at, note)| match at + 1 < notes.len() && !note.ends_with(['.', '!', '?']) {
                true => format!("{note}."),
                false => note.clone(),
            },
        )
        .collect();
    notes.join(" ")
}

/// A label, `**Request body:**`, with what goes after it, `rest`, which
/// can be empty, and `after` the bold, if there's more to the label, like
/// `**Request body**, as `application/xml`:`.
pub fn label(bold: &str, after: &str, rest: &str) -> String {
    let label = match after {
        "" => format!("**{bold}:**"),
        after => format!("**{bold}**{after}:"),
    };
    match rest {
        "" => label,
        rest => format!("{label} {rest}"),
    }
}

/// What pulldown-cmark reads in a description: CommonMark, and the
/// extensions to it that can hold a heading, or look like one.
fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH
}

/// The headings in `markdown`: where each starts and ends, and its level.
fn headings(markdown: &str) -> Vec<(Range<usize>, usize)> {
    let events = Parser::new_ext(markdown, options()).into_offset_iter();
    (events)
        .filter_map(|(event, range)| match event {
            Event::Start(Tag::Heading { level, .. }) => Some((range, level as usize)),
            _ => None,
        })
        .collect()
}

/// `description`, Markdown, under a heading at `level`, with each of its
/// own headings moved down, as far as the highest of them must go to be
/// below it, but no further than `######`.
pub fn shift(description: &str, level: usize) -> String {
    let text = description.replace("\r\n", "\n");
    let headings = headings(&text);
    let Some(highest) = headings.iter().map(|(_, level)| *level).min() else {
        return text;
    };
    let by = (level + 1).saturating_sub(highest);
    if by == 0 {
        return text;
    }
    let mut shifted = text.clone();
    for (range, from) in headings.into_iter().rev() {
        let to = (from + by).min(6);
        let heading = &text[range.clone()];
        let body = heading.strip_suffix('\n').unwrap_or(heading);
        let end = &heading[body.len()..];
        let moved = match body.rsplit_once('\n') {
            // An ATX heading, which starts with its `#`s.
            None => {
                let at = heading.find('#').unwrap_or(0);
                let (indent, heading) = heading.split_at(at);
                format!("{indent}{}{heading}", "#".repeat(to - from))
            }
            // A setext heading, underlined, which has no third level.
            Some((lines, underline)) if to == 2 => {
                format!("{lines}\n{}{end}", underline.replace('=', "-"))
            }
            Some((lines, _)) => {
                let lines: Vec<&str> = (lines.lines().enumerate())
                    .map(|(at, line)| match at {
                        0 => line.trim(),
                        _ => line.trim_start_matches([' ', '\t', '>']).trim(),
                    })
                    .collect();
                let text = lines.join(" ");
                // A `#` at its end would be read as the heading's own end.
                let close = if text.ends_with('#') { " #" } else { "" };
                format!("{} {text}{close}{end}", "#".repeat(to))
            }
        };
        shifted.replace_range(range, &moved);
    }
    shifted
}

/// The id comrak gives each heading in `markdown`, by where it starts: its
/// text, made an id as comrak makes it, with a number after it if an
/// earlier heading has the same.
pub fn ids(markdown: &str) -> Vec<(usize, String)> {
    let mut taken = HashSet::new();
    let mut ids = Vec::new();
    let mut heading: Option<(usize, String)> = None;
    for (event, range) in Parser::new_ext(markdown, options()).into_offset_iter() {
        match (event, &mut heading) {
            (Event::Start(Tag::Heading { .. }), _) => heading = Some((range.start, String::new())),
            (Event::End(TagEnd::Heading(_)), Some((start, text))) => {
                ids.push((*start, anchor(text, &mut taken)));
                heading = None;
            }
            (
                Event::Text(text)
                | Event::Code(text)
                | Event::InlineMath(text)
                | Event::DisplayMath(text),
                Some((_, heading)),
            ) => *heading += &text,
            (Event::SoftBreak | Event::HardBreak, Some((_, heading))) => heading.push(' '),
            _ => {}
        }
    }
    ids
}

/// The id comrak's `Anchorizer` gives the heading with `text`, if `taken`
/// holds those it's given before.
fn anchor(text: &str, taken: &mut HashSet<String>) -> String {
    let id: String = (text.to_lowercase().chars())
        .filter(|&c| {
            matches!(c, ' ' | '-')
                || c.is_letter()
                || c.is_mark()
                || c.is_number()
                || c.is_punctuation_connector()
        })
        .map(|c| if c == ' ' { '-' } else { c })
        .collect();
    let mut anchor = id.clone();
    let mut n = 0;
    while taken.contains(&anchor) {
        n += 1;
        anchor = format!("{id}-{n}");
    }
    taken.insert(anchor.clone());
    anchor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::html;

    #[test]
    fn text_is_escaped() {
        let text = "a *b* _c_ `d` [e](f) <g> #h &amp; ~i~ $j$ ^k^ ==l== ++m++ :smile: \\n";
        assert_eq!(
            html(&inline(text), true),
            format!("<p>{}</p>\n", {
                "a *b* _c_ `d` [e](f) &lt;g&gt; #h &amp;amp; ~i~ $j$ ^k^ ==l== ++m++ :smile: \\n"
            })
        );
        assert_eq!(inline(" a\n\tb  "), "a b");
    }

    #[test]
    fn urls_are_linked_but_not_email_addresses() {
        let text = "See https://example.com/a_b*c?d=1&e=2, www.example.com/x_y, \
                    my_name+tag@example.com, or (http://example.com/z_z).";
        let linked = html(&inline(text), true);
        for url in [
            r#"<a href="https://example.com/a_b*c?d=1&amp;e=2">"#,
            r#"<a href="http://www.example.com/x_y">"#,
            r#"<a href="http://example.com/z_z">"#,
        ] {
            assert!(linked.contains(url), "no {url} in {linked}");
        }
        assert!(linked.contains(" my_name+tag@example.com, "), "{linked}");
        assert!(!linked.contains("mailto"), "{linked}");
        let unlinked = html(&inline(text), false);
        assert!(unlinked.contains("https://example.com/a_b*c?d=1&amp;e=2,"));
        assert!(unlinked.contains("my_name+tag@example.com,"));
        let angled = "<https://a.com> https://a.com/<b>";
        assert_eq!(
            html(&inline(angled), true),
            format!("<p>{}</p>\n", {
                "&lt;https://a.com&gt; https://a.com/&lt;b&gt;"
            })
        );
    }

    #[test]
    fn urls_are_found_as_comrak_finds_them() {
        let none = [
            "x_https://a_b",
            "xhttps://a.com",
            "https://a_b.com/x",
            "https://localhost",
        ];
        for word in
            none.into_iter()
                .chain(["awww.a.com", "www.a", "<https://a.com>", "a@www.a.com"])
        {
            assert_eq!(linked(word), None, "{word}");
        }
        for (word, url) in [
            ("1https://a.com/*x*.", "https://a.com/*x*."),
            ("_https://a_b.c.com", "https://a_b.c.com"),
            ("(www.a.com)", "www.a.com)"),
            ("*www.a.com", "www.a.com"),
            ("me@x.com/https://y.com", "https://y.com"),
            ("https://a.com<x>https://b.com", "https://b.com"),
        ] {
            assert_eq!(linked(word).map(|at| &word[at..]), Some(url), "{word}");
        }
        assert_eq!(inline("x_https://a_b"), "x\\_https\\://a\\_b");
        let html = html(&inline("*me_1@x.com_* me@x.com/https://y.com"), true);
        assert_eq!(
            html,
            format!("<p>{}</p>\n", {
                "*me_1@x.com_* me@x.com/<a href=\"https://y.com\">https://y.com</a>"
            })
        );
    }

    #[test]
    fn links_lead_where_their_urls_do() {
        let html = html(&link("a", "https://e.com/a b?c=1&d=<2>\\"), true);
        assert_eq!(
            html,
            "<p><a href=\"https://e.com/a%20b?c=1&amp;d=%3C2%3E%5C\">a</a></p>\n"
        );
        assert_eq!(url("./terms"), "`./terms`");
        // comrak reads `&copy;` in a URL as `©`, even after a `\`, which it
        // keeps, so `&` is written `&amp;`.
        assert!(crate::tests::html("[a](<x?\\&copy;>)", true).contains("x?%5C%C2%A9"));
        assert_eq!(
            crate::tests::html(&link("a", "x?a&copy;"), true),
            "<p><a href=\"x?a&amp;copy;\">a</a></p>\n"
        );
    }

    #[test]
    fn values_are_code() {
        assert_eq!(value(&Value::from("a")), "`a`");
        assert_eq!(value(&Value::from("")), "`\"\"`");
        assert_eq!(value(&serde_json::json!([1, "b"])), "`[1,\"b\"]`");
    }

    #[test]
    fn lists_are_joined() {
        let items = |items: &[&str]| {
            items
                .iter()
                .map(|item| item.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(either(&items(&["a"])), "a");
        assert_eq!(either(&items(&["a", "b"])), "a or b");
        assert_eq!(both(&items(&["a", "b", "c"])), "a, b, and c");
        assert_eq!(
            sentences(&items(&["A pet", "Read-only.", "Odd"])),
            "A pet. Read-only. Odd"
        );
    }

    #[test]
    fn labels_are_bold() {
        assert_eq!(label("Version", "", "`1`"), "**Version:** `1`");
        assert_eq!(label("Example", ", as `x`", ""), "**Example**, as `x`:");
    }

    #[test]
    fn headings_move_below_the_one_theyre_under() {
        let text = "# A\n\ntext\n\n## B #\n\n  ### C\n";
        assert_eq!(shift(text, 2), "### A\n\ntext\n\n#### B #\n\n  ##### C\n");
        assert_eq!(shift("#### A\n", 2), "#### A\n");
        assert_eq!(shift("# A\n## B\n###### C", 3), "#### A\n##### B\n###### C");
        assert_eq!(shift("text", 1), "text");
        assert_eq!(shift("    # code\n", 1), "    # code\n");
    }

    #[test]
    fn setext_headings_become_atx_below_the_second_level() {
        assert_eq!(shift("A\n===\n\nB\n---\n", 1), "A\n---\n\n### B\n");
        assert_eq!(shift("A\nb #\n==\n", 2), "### A b # #\n");
        assert_eq!(shift("> A\n> b\n> ===\n\ntext", 2), "> ### A b\n\ntext");
        assert_eq!(shift("A\r\n===\r\n", 3), "#### A\n");
    }

    #[test]
    fn ids_are_made_as_comrak_makes_them() {
        let markdown = "# Pets\n\n## `Pet`\n\ntext\n\n### Pet\n\n> # Ça_va? *1*\n\n```\n# no\n```\n\nA\nB\n-\n";
        let ids: Vec<String> = ids(markdown).into_iter().map(|(_, id)| id).collect();
        assert_eq!(ids, ["pets", "pet", "pet-1", "ça_va-1", "a-b"]);
        let html = html(markdown, true);
        for id in ids {
            assert!(html.contains(&format!("id=\"{id}\"")), "no {id} in {html}");
        }
    }
}
