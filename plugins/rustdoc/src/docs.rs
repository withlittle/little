//! The Markdown of doc comments, as rustdoc reads it: the lines Rust code
//! blocks hide are left out, and links to paths, like [`Config`], lead to
//! where the paths do.

use std::ops::Range;

use little_api_reference::model::Section;
use pulldown_cmark::{BrokenLink, CowStr, Event, LinkType, Options, Parser, Tag, TagEnd};
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ExprLit, Lit, Meta};

/// What a path, like `["crate", "config", "Config"]`, leads to from the
/// page being written, as a link, if it leads anywhere documented.
pub type Resolve<'a> = dyn Fn(&[String]) -> Option<String> + 'a;

/// The text of the doc comments in `attrs`, and of their `#[doc = "..."]`s,
/// without the margin their lines share; empty if they have none. A
/// `#[doc]` that isn't a string, like an `include_str!`, which is in `file`,
/// is left out, and warned of.
pub fn text(attrs: &[Attribute], file: &str, warnings: &mut Vec<String>) -> String {
    let mut lines = Vec::new();
    for attr in (attrs.iter()).filter(|attr| attr.path().is_ident("doc")) {
        let Meta::NameValue(meta) = &attr.meta else {
            continue;
        };
        match &meta.value {
            Expr::Lit(ExprLit {
                lit: Lit::Str(string),
                ..
            }) => lines.extend(fragment(&string.value())),
            _ => {
                let line = attr.span().start().line;
                warnings.push(format!(
                    "can't read the `#[doc]` on line {line} of `{file}`, as it isn't a string, \
                     so it's left out"
                ));
            }
        }
    }
    unindent(&lines)
}

/// The lines of one doc comment, or `#[doc]`: a `/** ... */`'s without the
/// blank line it starts or ends with, or the `*` each of its lines starts
/// with, if they all do.
fn fragment(text: &str) -> Vec<String> {
    let mut lines: Vec<&str> = (text.split('\n'))
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    if lines.len() > 1 {
        if lines.first().is_some_and(|line| line.trim().is_empty()) {
            lines.remove(0);
        }
        if lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.pop();
        }
        let mut written = lines.iter().filter(|line| !line.trim().is_empty());
        if written.all(|line| line.trim_start().starts_with('*')) {
            let unstarred =
                (lines.into_iter()).map(|line| line.trim_start().strip_prefix('*').unwrap_or(line));
            return unstarred.map(String::from).collect();
        }
    }
    lines.into_iter().map(String::from).collect()
}

/// `lines`, without the margin of spaces and tabs they share, the blank
/// lines they start with, or the whitespace they end with.
fn unindent(lines: &[String]) -> String {
    let margin = (lines.iter())
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    let lines: Vec<&str> = (lines.iter())
        .map(|line| line.get(margin..).unwrap_or_default())
        .collect();
    trimmed(&lines.join("\n"))
}

/// `text` without the blank lines it starts with, or the whitespace it ends
/// with.
fn trimmed(text: &str) -> String {
    let first = text.len() - text.trim_start().len();
    let start = text[..first].rfind('\n').map_or(0, |newline| newline + 1);
    text[start..].trim_end().to_string()
}

/// The sections of doc comments' `text`, in Markdown, each with its links
/// to paths leading where `resolve` says: the text before the first
/// heading, untitled, then each heading's, titled by it.
pub fn render(text: &str, resolve: &Resolve) -> Vec<Section> {
    sections(&links(&fences(text), resolve))
}

/// `text` with each Rust code block in `rust`, and without the lines it
/// hides, as rustdoc reads it: a block is Rust if it has no language, or
/// only rustdoc's words for how it's tested, like `no_run`.
fn fences(text: &str) -> String {
    let mut out = Vec::new();
    // The fence of the code block the line is in, and whether it's Rust.
    let mut open: Option<(&str, bool)> = None;
    for line in text.split('\n') {
        match open {
            None => match opening(line) {
                Some((indent, fence, info)) if is_rust(info) => {
                    open = Some((fence, true));
                    out.push(format!("{indent}{fence}rust"));
                }
                Some((_, fence, _)) => {
                    open = Some((fence, false));
                    out.push(line.to_string());
                }
                None => out.push(line.to_string()),
            },
            Some((fence, rust)) => {
                if closing(line, fence) {
                    open = None;
                    out.push(line.to_string());
                } else if !rust {
                    out.push(line.to_string());
                } else if let Some(line) = shown(line) {
                    out.push(line);
                }
            }
        }
    }
    out.join("\n")
}

/// The indent, fence, and info string of `line`, if it opens a fenced code
/// block.
fn opening(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.trim_start_matches(' ');
    let indent = &line[..line.len() - rest.len()];
    let mark = rest.chars().next().filter(|&c| c == '`' || c == '~')?;
    let length = rest.len() - rest.trim_start_matches(mark).len();
    let (fence, info) = rest.split_at(length);
    let opens = indent.len() <= 3 && length >= 3 && !(mark == '`' && info.contains('`'));
    opens.then(|| (indent, fence, info.trim()))
}

/// Whether `line` closes the code block `fence` opened.
fn closing(line: &str, fence: &str) -> bool {
    let rest = line.trim_start_matches(' ');
    let mark = fence.chars().next().unwrap_or('`');
    let length = rest.len() - rest.trim_start_matches(mark).len();
    line.len() - rest.len() <= 3 && length >= fence.len() && rest[length..].trim().is_empty()
}

/// Whether rustdoc reads a code block whose info string is `info` as Rust.
fn is_rust(info: &str) -> bool {
    let mut words = info.split([',', ' ', '\t']).filter(|word| !word.is_empty());
    words.all(|word| {
        let tested = matches!(
            word,
            "rust"
                | "ignore"
                | "no_run"
                | "should_panic"
                | "compile_fail"
                | "test_harness"
                | "standalone_crate"
                | "allow_fail"
        );
        tested || word.starts_with("edition") || word.starts_with("ignore-")
    })
}

/// `line` of a Rust code block as rustdoc shows it, or none if it's hidden,
/// with `# `; `##` starts a line shown with `#`.
fn shown(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("##") {
        return Some(line.replacen("##", "#", 1));
    }
    let hidden = trimmed == "#" || trimmed.starts_with("# ");
    (!hidden).then(|| line.to_string())
}

/// The sections of `text`: what's before its first heading, untitled, then
/// each heading's, titled by it, but those with no text.
fn sections(text: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut title = None;
    let mut lines = Vec::new();
    let mut fence: Option<&str> = None;
    for line in text.split('\n') {
        match fence {
            Some(open) if closing(line, open) => fence = None,
            Some(_) => {}
            None => {
                if let Some((_, open, _)) = opening(line) {
                    fence = Some(open);
                } else if let Some(heading) = heading(line) {
                    push(&mut sections, title.replace(heading), &lines);
                    lines.clear();
                    continue;
                }
            }
        }
        lines.push(line);
    }
    push(&mut sections, title, &lines);
    sections
}

/// Push the section titled `title`, of `lines`, to `sections`, if it has
/// text.
fn push(sections: &mut Vec<Section>, title: Option<String>, lines: &[&str]) {
    let text = trimmed(&lines.join("\n"));
    if !text.is_empty() {
        sections.push(Section::Text { title, text });
    }
}

/// The title of the heading `line` is, if it's one, like `Examples` for
/// `# Examples`.
fn heading(line: &str) -> Option<String> {
    let rest = line.trim_start_matches(' ');
    let level = rest.len() - rest.trim_start_matches('#').len();
    let text = &rest[level..];
    let starts = line.len() - rest.len() <= 3 && (1..=6).contains(&level);
    if !starts || !(text.is_empty() || text.starts_with([' ', '\t'])) {
        return None;
    }
    let title = text.trim();
    let unclosed = title.trim_end_matches('#');
    let title = match unclosed.is_empty() || unclosed.ends_with([' ', '\t']) {
        true => unclosed.trim_end(),
        false => title,
    };
    (!title.is_empty()).then(|| title.to_string())
}

/// A link being read, and the text inside its brackets, so far.
struct Link<'a> {
    link_type: LinkType,
    dest: CowStr<'a>,
    range: Range<usize>,
    inner: Option<Range<usize>>,
}

/// `text` with each link to a path, like [`Config`] or
/// [its config](crate::config::Config), leading where `resolve` says the
/// path does; if it leads nowhere, the link is its text, in code if that's
/// the path, as [`Vec`] is. A definition of a link to a path, like
/// `[config]: crate::config`, is left out.
pub fn links(text: &str, resolve: &Resolve) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_with_broken_link_callback(text, options, Some(unknown));
    let definitions = parser.reference_definitions().iter();
    let mut edits: Vec<(Range<usize>, String)> = definitions
        .filter(|(_, definition)| path(&definition.dest).is_some())
        .map(|(_, definition)| (definition.span.clone(), String::new()))
        .collect();
    let mut open: Option<Link> = None;
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) => {
                open = Some(Link {
                    link_type,
                    dest: dest_url,
                    range,
                    inner: None,
                })
            }
            Event::End(TagEnd::Link) => {
                edits.extend(open.take().and_then(|link| edit(text, link, resolve)));
            }
            _ => {
                if let Some(link) = &mut open {
                    link.inner = Some(match link.inner.take() {
                        Some(inner) => inner.start.min(range.start)..inner.end.max(range.end),
                        None => range,
                    });
                }
            }
        }
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut text = text.to_string();
    for (range, replacement) in edits.into_iter().rev() {
        text.replace_range(range, &replacement);
    }
    text
}

/// A link to a reference that isn't defined, like [`Config`], which leads
/// to the reference, so it can be read as a path.
fn unknown(link: BrokenLink<'_>) -> Option<(CowStr<'_>, CowStr<'_>)> {
    Some((link.reference, CowStr::Borrowed("")))
}

/// What `link`, in `text`, is written as, if it's to a path.
fn edit(text: &str, link: Link, resolve: &Resolve) -> Option<(Range<usize>, String)> {
    let path = path(&link.dest)?;
    let inner = (link.inner.clone()).map_or("", |inner| &text[inner]);
    let shortcut = matches!(
        link.link_type,
        LinkType::Shortcut
            | LinkType::ShortcutUnknown
            | LinkType::Collapsed
            | LinkType::CollapsedUnknown
    );
    let shown = match shortcut {
        true => without_kind(inner),
        false => inner.to_string(),
    };
    let written = match resolve(&path) {
        Some(url) => format!("[{shown}]({url})"),
        None if shortcut && !(shown.starts_with('`') && shown.ends_with('`')) => {
            format!("`{shown}`")
        }
        None => shown,
    };
    Some((link.range, written))
}

/// The path `dest` names, if it's one, as rustdoc reads it: names with `::`
/// between them, in code or not, perhaps after a kind, like `struct@`, and
/// before `()` or `!`, with no generics, like `<T>`.
fn path(dest: &str) -> Option<Vec<String>> {
    let dest = dest.trim().trim_matches('`');
    let dest = without_kind(dest);
    let dest = ["()", "!()", "![]", "!{}", "!"]
        .iter()
        .find_map(|call| dest.strip_suffix(call))
        .unwrap_or(dest.as_str());
    let path: Vec<String> = (without_generics(dest)?.split("::"))
        .map(|name| name.strip_prefix("r#").unwrap_or(name).to_string())
        .collect();
    path.iter().all(|name| is_identifier(name)).then_some(path)
}

/// `path` without its generics, like `<T>`, or a `::` before them, or none
/// if they don't close.
fn without_generics(path: &str) -> Option<String> {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in path.chars() {
        match c {
            '<' => {
                if depth == 0 && out.ends_with("::") {
                    out.truncate(out.len() - 2);
                }
                depth += 1;
            }
            '>' => depth = depth.checked_sub(1)?,
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    (depth == 0).then_some(out)
}

/// Whether `name` is an identifier, like `Config` or `my_crate`.
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let first = chars.next().is_some_and(|c| c.is_alphabetic() || c == '_');
    first && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// `text` without the kind its path starts with, like `struct@`, in code or
/// not.
fn without_kind(text: &str) -> String {
    let (ticks, inner) = match text.strip_prefix('`').and_then(|t| t.strip_suffix('`')) {
        Some(inner) => ("`", inner),
        None => ("", text),
    };
    let inner = match inner.split_once('@') {
        Some((kind, path)) if !kind.is_empty() && kind.chars().all(|c| c.is_ascii_lowercase()) => {
            path
        }
        _ => inner,
    };
    format!("{ticks}{inner}{ticks}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(code: &str) -> Vec<Attribute> {
        syn::parse_str::<syn::ItemFn>(&format!("{code}\nfn f() {{}}"))
            .unwrap()
            .attrs
    }

    /// What `path` leads to in a crate with `Config`, at `config.md`, and
    /// its method `load`, from a page beside it.
    fn resolve(path: &[String]) -> Option<String> {
        match path.join("::").as_str() {
            "Config" | "crate::config::Config" => Some("config.md#config".to_string()),
            "Config::load" => Some("config.md#configload".to_string()),
            _ => None,
        }
    }

    #[test]
    fn doc_comments_lose_their_margin() {
        let mut warnings = Vec::new();
        let code = "/// The config.\n///\n///     indented\n#[doc = \" More.\"]";
        let text = text(&attrs(code), "src/lib.rs", &mut warnings);
        assert_eq!(text, "The config.\n\n    indented\nMore.");
        let block = "/**\n * A block.\n *\n *  Indented.\n */";
        assert_eq!(text_of(block), "A block.\n\n Indented.");
        assert_eq!(text_of("/** Inline. */"), "Inline.");
        assert_eq!(text_of("#[doc(hidden)]"), "");
        assert!(warnings.is_empty());
    }

    fn text_of(code: &str) -> String {
        text(&attrs(code), "src/lib.rs", &mut Vec::new())
    }

    #[test]
    fn a_doc_that_isnt_a_string_is_warned_of() {
        let mut warnings = Vec::new();
        let code = "/// Read.\n#[doc = include_str!(\"../README.md\")]";
        assert_eq!(text(&attrs(code), "src/lib.rs", &mut warnings), "Read.");
        assert_eq!(
            warnings,
            [
                "can't read the `#[doc]` on line 2 of `src/lib.rs`, as it isn't a string, so it's \
              left out"
            ]
        );
    }

    #[test]
    fn rust_code_blocks_hide_lines() {
        let text = "Use it:\n\n```\n# use x::Y;\n#\nlet y = Y;\n## not hidden\n#[attr]\n```\n\n\
                    ```no_run, edition2021\nrun();\n```\n\n~~~text\n# shown\n~~~";
        assert_eq!(
            fences(text),
            "Use it:\n\n```rust\nlet y = Y;\n# not hidden\n#[attr]\n```\n\n\
             ```rust\nrun();\n```\n\n~~~text\n# shown\n~~~"
        );
        assert!(is_rust("rust,ignore") && is_rust("ignore-wasm32") && !is_rust("sh"));
    }

    #[test]
    fn headings_start_sections() {
        let text = "The lead.\n\n# Examples\n\n```sh\n# a comment\n```\n\n## Errors ##\n\n\
                    #not a heading\n\n# Empty\n\n    # indented code";
        let sections = sections(text);
        let titles: Vec<_> = (sections.iter())
            .map(|section| match section {
                Section::Text { title, .. } => title.as_deref(),
                Section::Items { .. } => None,
            })
            .collect();
        assert_eq!(
            titles,
            [None, Some("Examples"), Some("Errors"), Some("Empty")]
        );
        let Section::Text { text, .. } = &sections[1] else {
            unreachable!()
        };
        assert_eq!(text, "```sh\n# a comment\n```");
        assert_eq!(heading("# C#"), Some("C#".to_string()));
        assert_eq!(heading("# #"), None);
        assert_eq!(heading("    # Code"), None);
    }

    #[test]
    fn links_to_paths_lead_where_they_do() {
        let text = "See [`Config`], [the method](Config::load), [`struct@Config`], \
                    [`Vec<T>`], [Config::load()], [load][`Config::load`], [missing][m], \
                    [a site](https://example.com), [^1], [1], and [`Vec`].\n\n\
                    [m]: crate::Missing\n[^1]: A note.";
        assert_eq!(
            links(text, &resolve),
            "See [`Config`](config.md#config), [the method](config.md#configload), \
             [`Config`](config.md#config), `Vec<T>`, [Config::load()](config.md#configload), \
             [load](config.md#configload), missing, [a site](https://example.com), [^1], \
             [1], and `Vec`.\n\n\n[^1]: A note."
        );
    }

    #[test]
    fn paths_are_read_as_rustdoc_reads_them() {
        let path_of = |dest: &str| path(dest).map(|path| path.join("::"));
        assert_eq!(path_of("`crate::Config`"), Some("crate::Config".into()));
        assert_eq!(path_of("fn@load()"), Some("load".into()));
        assert_eq!(path_of("vec!"), Some("vec".into()));
        assert_eq!(path_of("Vec::<T>::new"), Some("Vec::new".into()));
        assert_eq!(path_of("r#type::r#in"), Some("type::in".into()));
        for dest in [
            "https://example.com",
            "page.md",
            "#id",
            "1",
            "!NOTE",
            "a b",
            "",
        ] {
            assert_eq!(path(dest), None, "{dest}");
        }
    }
}
