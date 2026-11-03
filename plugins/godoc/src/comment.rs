//! Go's doc comments, read as go/doc/comment reads them, into paragraphs,
//! headings, code, and lists, with links to URLs and to Go's code, and
//! written as Markdown, which shows them as pkg.go.dev does.

use std::collections::{HashMap, HashSet};

use little_api_reference::markdown::fence_for;
use little_api_reference::model::Section;
use unic_ucd_category::GeneralCategory;

use crate::reader::exported;

/// Where a link to Go's code starts, in Markdown that [`render`] hasn't led
/// yet: then its target, like `example.com/m/config#Config.Load`, then
/// [`MIDDLE`], its text, in code, and [`END`]. They're in Unicode's area for
/// private use, and taken out of any doc comment that holds them.
const START: char = '\u{E000}';
const MIDDLE: char = '\u{E001}';
const END: char = '\u{E002}';

/// The packages of Go's standard library whose import path is one name,
/// which a link can name without importing them, as go/doc/comment's list.
const STD: &[&str] = &[
    "bufio", "bytes", "cmp", "context", "crypto", "embed", "encoding", "errors", "expvar", "flag",
    "fmt", "hash", "html", "image", "io", "iter", "log", "maps", "math", "mime", "net", "os",
    "path", "plugin", "reflect", "regexp", "runtime", "slices", "sort", "strconv", "strings",
    "structs", "sync", "syscall", "testing", "time", "unicode", "unique", "unsafe", "uuid", "weak",
];

/// What the links in a package's doc comments can name: the package, the
/// packages it imports, and what it declares.
pub struct Context<'a> {
    /// The package's import path, like `example.com/m/config`.
    pub import_path: &'a str,
    /// The package's name, which names it in a link, as its path does.
    pub name: &'a str,
    /// The path of each package it imports, by the name it imports it as,
    /// but for a name it imports more than one package as.
    pub imports: &'a HashMap<String, String>,
    /// What it declares that a link can name, like `Config`, or
    /// `Config.Load`, for a method or field.
    pub syms: &'a HashSet<String>,
}

/// The sections of a doc comment's `text`, as Go gives it, in Markdown:
/// what's before its first heading, untitled, then each heading's, titled by
/// it, but those with no text. Its links to Go's code are left for
/// [`render`] to lead.
pub fn sections(text: &str, context: &Context) -> Vec<Section> {
    let text = text.replace([START, MIDDLE, END], "");
    let lines: Vec<&str> = text.split('\n').collect();
    let lines = unindent(&lines);
    let mut doc = Doc {
        context,
        links: HashMap::new(),
    };
    let mut blocks = Vec::new();
    for span in spans(&lines) {
        let lines = &lines[span.start..span.end];
        let block = match span.kind {
            Kind::List => Some(Block::List(doc.list(lines))),
            Kind::Code => Some(Block::Code(code(lines))),
            Kind::Heading => Some(Block::Heading(lines[0][1..].trim().to_string())),
            Kind::Paragraph => doc.paragraph(lines).map(Block::Paragraph),
        };
        blocks.extend(block);
    }
    // Links are read once every one a comment defines is known.
    let mut sections = Vec::new();
    let mut title = None;
    let mut text = String::new();
    let mut push = |title, text: String| {
        if !text.is_empty() {
            sections.push(Section::Text { title, text });
        }
    };
    for block in blocks {
        let written = match block {
            Block::Heading(heading) => {
                push(title.replace(heading), std::mem::take(&mut text));
                continue;
            }
            Block::Paragraph(paragraph) => doc.markdown(&paragraph),
            Block::Code(code) => {
                let fence = fence_for(&code);
                format!("{fence}\n{code}{fence}")
            }
            Block::List(list) => doc.list_markdown(&list),
        };
        if !text.is_empty() {
            text += "\n\n";
        }
        text += &written;
    }
    push(title, text);
    sections
}

/// Whether a doc comment's `text` says what it documents is deprecated: a
/// paragraph of it starts with `Deprecated: `, as Go's tools read it.
pub fn deprecated(text: &str) -> bool {
    (text.split("\n\n")).any(|paragraph| paragraph.starts_with("Deprecated: "))
}

/// `text`, in Markdown from [`sections`], with each link to Go's code led to
/// where `resolve` says its target leads, or, if it leads nowhere, as its
/// text alone, in code. A target is a package's import path, then `#`, then
/// the name of what it declares, if the link names one, like `Config` or
/// `Config.Load`.
pub fn render(text: &str, resolve: &dyn Fn(&str) -> Option<String>) -> String {
    let mut rendered = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(START) {
        rendered.push_str(&rest[..start]);
        let link = &rest[start + START.len_utf8()..];
        let Some((target, link)) = link.split_once(MIDDLE) else {
            break;
        };
        let Some((code, after)) = link.split_once(END) else {
            break;
        };
        match resolve(target) {
            Some(url) => rendered += &format!("[{code}]({})", destination(&url)),
            None => rendered += code,
        }
        rest = after;
    }
    rendered.push_str(rest);
    rendered
}

/// The name go/doc assumes a package imported from `import_path` without
/// one has: the path's last element, or the one before a last like `v2`,
/// without `go-` before it, up to what can't be in a name.
pub fn assumed_name(import_path: &str) -> String {
    let (dir, mut base) = split_path(import_path);
    if let Some(version) = base.strip_prefix('v')
        && version.parse::<i64>().is_ok()
        && let Some(dir) = dir
    {
        base = split_path(dir).1;
    }
    let base = base.strip_prefix("go-").unwrap_or(base);
    let end = base.find(|c: char| {
        let category = GeneralCategory::of(c);
        !(c.is_ascii_alphanumeric()
            || c == '_'
            || (!c.is_ascii()
                && (category.is_letter() || category == GeneralCategory::DecimalNumber)))
    });
    base[..end.unwrap_or(base.len())].to_string()
}

/// `path`'s folder, if it has one, and its last element.
fn split_path(path: &str) -> (Option<&str>, &str) {
    let path = path.trim_end_matches('/');
    match path.rsplit_once('/') {
        Some((dir, base)) => (Some(dir), base),
        None => (None, path),
    }
}

/// A block of a doc comment.
enum Block {
    /// Its lines, joined by newlines.
    Paragraph(String),
    Heading(String),
    /// Its lines, unindented, each ending in a newline.
    Code(String),
    List(List),
}

struct List {
    items: Vec<Item>,
    /// Whether a blank line is between two of its items.
    loose: bool,
}

struct Item {
    /// Its number, or nothing, in a list of bullets.
    number: String,
    /// Each paragraph's lines, joined by newlines.
    paragraphs: Vec<String>,
}

/// A part of a paragraph's text.
enum Inline {
    Text(String),
    Link {
        text: String,
        url: String,
    },
    /// A link to Go's code, to its target, like `io#Reader`.
    DocLink {
        target: String,
        text: String,
    },
}

/// What a doc comment's lines are, by where they start and end.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Code,
    Heading,
    List,
    Paragraph,
}

struct Span {
    start: usize,
    end: usize,
    kind: Kind,
}

/// A doc comment being read: the links it defines, like
/// `[Go home page]: https://go.dev`, by their text.
struct Doc<'a> {
    context: &'a Context<'a>,
    links: HashMap<String, String>,
}

/// The spans of a doc comment's `lines`, unindented, as go/doc/comment
/// finds them: an indented span is a list if it starts with a list's
/// marker, else code, and an unindented span is a heading if it's one line
/// starting with `# `, else a paragraph. Unindented code or lists, which
/// gofmt indents, are guessed at as go/doc/comment does.
fn spans(lines: &[&str]) -> Vec<Span> {
    let mut spans = Vec::new();
    // Each line is read at most twice; this stops a loop, where Go panics.
    let mut watchdog = 2 * lines.len();
    let mut i = 0;
    let mut force_indent = 0;
    loop {
        while i < lines.len() && lines[i].is_empty() {
            i += 1;
        }
        if i >= lines.len() || watchdog == 0 {
            break;
        }
        watchdog -= 1;
        let start = i;
        let mut end;
        let kind = if i < force_indent || indented(lines[i]) {
            // Up to the next unindented line, past blank ones, or, in an
            // unindented list taken as indented, past unindented items too.
            let mut unindented_list = is_list(lines[i]) && i < force_indent;
            i += 1;
            while i < lines.len()
                && (lines[i].is_empty()
                    || i < force_indent
                    || indented(lines[i])
                    || (unindented_list && is_list(lines[i])))
            {
                if lines[i].is_empty() {
                    unindented_list = false;
                }
                i += 1;
            }
            end = i;
            while end > start && lines[end - 1].is_empty() {
                end -= 1;
            }
            // The `}` of code whose last line wasn't indented.
            if end < lines.len() && lines[end].starts_with('}') {
                end += 1;
            }
            if is_list(lines[start]) {
                Kind::List
            } else {
                Kind::Code
            }
        } else {
            i += 1;
            while i < lines.len() && !lines[i].is_empty() && !indented(lines[i]) {
                i += 1;
            }
            end = i;
            // Unindented lines followed by indented ones, without a blank
            // line between, may be an unindented list, or the first line of
            // unindented code, ending in `{` or `\`, which go with them.
            if i < lines.len() && !lines[i].is_empty() && !is_list(lines[i]) {
                if is_list(lines[i - 1]) {
                    force_indent = end;
                    end -= 1;
                    while end > start && is_list(lines[end - 1]) {
                        end -= 1;
                    }
                } else if lines[i - 1].ends_with('{') || lines[i - 1].ends_with('\\') {
                    force_indent = end;
                    end -= 1;
                }
                if start == end && force_indent > start {
                    i = start;
                    continue;
                }
            }
            if end - start == 1 && is_heading(lines[start]) {
                Kind::Heading
            } else {
                Kind::Paragraph
            }
        };
        spans.push(Span { start, end, kind });
        i = end;
    }
    spans
}

/// Whether `line` starts with a space or tab.
fn indented(line: &str) -> bool {
    line.starts_with([' ', '\t'])
}

/// `lines` without blank lines at either end, or the spaces and tabs they
/// all start with, and with lines of only spaces made blank.
fn unindent<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    let mut lines = lines;
    while let [first, rest @ ..] = lines
        && first.is_empty()
    {
        lines = rest;
    }
    while let [rest @ .., last] = lines
        && last.is_empty()
    {
        lines = rest;
    }
    let Some(first) = lines.first() else {
        return Vec::new();
    };
    let mut prefix = leading_space(first);
    for line in &lines[1..] {
        if !line.is_empty() {
            let common = (prefix.bytes().zip(leading_space(line).bytes()))
                .take_while(|(a, b)| a == b)
                .count();
            prefix = &prefix[..common];
        }
    }
    let unindented = lines.iter().map(|line| {
        let line = line.strip_prefix(prefix).unwrap_or(line);
        if line.trim().is_empty() { "" } else { line }
    });
    let mut unindented: Vec<&str> = unindented.collect();
    while unindented.last().is_some_and(|line| line.is_empty()) {
        unindented.pop();
    }
    let blank = unindented.iter().take_while(|line| line.is_empty()).count();
    unindented.split_off(blank)
}

/// The spaces and tabs `line` starts with.
fn leading_space(line: &str) -> &str {
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// Whether `line` is a heading, like `# Usage`.
fn is_heading(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() >= 2 && bytes[0] == b'#' && matches!(bytes[1], b' ' | b'\t') && line.trim() != "#"
}

/// The number of the item of a list `line` starts, or nothing, for a
/// bullet, like `-`, `*`, `+`, or `•`, and the rest of the line, which must
/// be after a space or tab.
fn list_marker(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    let first = line.chars().next()?;
    let (number, rest) = if matches!(first, '•' | '*' | '+' | '-') {
        ("", &line[first.len_utf8()..])
    } else if first.is_ascii_digit() {
        let digits = line.bytes().take_while(u8::is_ascii_digit).count();
        match line.as_bytes().get(digits) {
            Some(b'.' | b')') => (&line[..digits], &line[digits + 1..]),
            _ => return None,
        }
    } else {
        return None;
    };
    (indented(rest) && !rest.trim().is_empty()).then_some((number, rest))
}

fn is_list(line: &str) -> bool {
    list_marker(line).is_some()
}

/// Code's `lines`, unindented, each ending in a newline.
fn code(lines: &[&str]) -> String {
    unindent(lines)
        .iter()
        .map(|line| format!("{line}\n"))
        .collect()
}

/// The text and URL of the link `line` defines, like
/// `[Go home page]: https://go.dev`, if it defines one.
fn parse_link(line: &str) -> Option<(&str, &str)> {
    if !line.starts_with('[') {
        return None;
    }
    let close = line.find("]:")?;
    let bytes = line.as_bytes();
    if close + 3 >= bytes.len() || !matches!(bytes[close + 2], b' ' | b'\t') {
        return None;
    }
    let url = line[close + 3..].trim();
    let scheme = url.find("://")?;
    is_scheme(&url[..scheme]).then_some((&line[1..close], url))
}

impl Doc<'_> {
    /// The text of a paragraph of `lines`, or nothing, if each defines a
    /// link, which it keeps, unless one with the same text came before.
    fn paragraph(&mut self, lines: &[&str]) -> Option<String> {
        let links: Option<Vec<(&str, &str)>> = lines.iter().map(|line| parse_link(line)).collect();
        let Some(links) = links else {
            return Some(lines.join("\n"));
        };
        for (text, url) in links {
            (self.links.entry(text.to_string())).or_insert_with(|| url.to_string());
        }
        None
    }

    /// The list of `lines`, whose items start where a line starts with a
    /// marker of the kind the first does, a number or a bullet.
    fn list(&mut self, lines: &[&str]) -> List {
        let numbered = list_marker(lines[0]).is_some_and(|(number, _)| !number.is_empty());
        let mut list = List {
            items: Vec::new(),
            loose: false,
        };
        let mut text: Vec<&str> = Vec::new();
        for &line in lines {
            let mut line = line;
            if let Some((number, rest)) = list_marker(line)
                && number.is_empty() != numbered
            {
                self.flush(&mut list, &mut text);
                list.items.push(Item {
                    number: number.to_string(),
                    paragraphs: Vec::new(),
                });
                line = rest;
            }
            let line = line.trim();
            if line.is_empty() {
                list.loose = true;
                self.flush(&mut list, &mut text);
                continue;
            }
            text.push(line);
        }
        self.flush(&mut list, &mut text);
        list
    }

    /// Ends the paragraph of `text` in `list`'s last item.
    fn flush(&mut self, list: &mut List, text: &mut Vec<&str>) {
        if let Some(item) = list.items.last_mut()
            && let Some(paragraph) = self.paragraph(text)
        {
            item.paragraphs.push(paragraph);
        }
        text.clear();
    }

    /// `list` in Markdown, each item with its number, or `-`, and each of
    /// its paragraphs after the first indented under it.
    fn list_markdown(&self, list: &List) -> String {
        let loose = list.loose || list.items.iter().any(|item| item.paragraphs.len() != 1);
        let items: Vec<String> = (list.items.iter())
            .map(|item| {
                let marker = match item.number.is_empty() {
                    true => "-".to_string(),
                    false => format!("{}.", item.number),
                };
                let indent = " ".repeat(marker.len() + 1);
                let mut written = marker;
                for (n, paragraph) in item.paragraphs.iter().enumerate() {
                    match n {
                        0 => written.push(' '),
                        _ => written += &format!("\n\n{indent}"),
                    }
                    written += &self.markdown(paragraph);
                }
                written
            })
            .collect();
        items.join(if loose { "\n\n" } else { "\n" })
    }

    /// A paragraph's `text` in Markdown, on one line, escaped, with its
    /// links, and each link to Go's code left for [`render`].
    fn markdown(&self, text: &str) -> String {
        let mut out = String::new();
        for inline in self.linked(text) {
            match inline {
                Inline::Text(text) => escape(&mut out, &text),
                Inline::Link { text, url } => {
                    out.push('[');
                    escape(&mut out, &text);
                    out += &format!("]({})", destination(&url));
                }
                // A name, which has no backticks.
                Inline::DocLink { target, text } => {
                    out += &format!("{START}{target}{MIDDLE}`{text}`{END}");
                }
            }
        }
        out
    }

    /// The parts of a paragraph's `text`: links in brackets, to a URL the
    /// comment defines, or to Go's code, like `[io.Reader]`, and the text
    /// between them.
    fn linked(&self, text: &str) -> Vec<Inline> {
        let mut out = Vec::new();
        // Where the text not yet in `out` starts.
        let mut wrote = 0;
        let mut start = None;
        // The text since the first `[`, with each newline and tab a space,
        // as go/doc/comment keeps it, for a link's text.
        let mut buffer = Vec::new();
        let bytes = text.as_bytes();
        for (i, &byte) in bytes.iter().enumerate() {
            let byte = if matches!(byte, b'\n' | b'\t') {
                b' '
            } else {
                byte
            };
            match byte {
                b'[' => start = Some(i),
                b']' => {
                    if let Some(start) = start {
                        let inner = &text[start + 1..i];
                        let defined = std::str::from_utf8(&buffer).ok();
                        let link = match defined.and_then(|text| self.links.get(text)) {
                            Some(url) => Some(Inline::Link {
                                text: plain(inner),
                                url: url.clone(),
                            }),
                            None => self.doc_link(inner, &text[..start], &text[i + 1..]),
                        };
                        if let Some(link) = link {
                            out.extend(parse_text(&text[wrote..start], true));
                            out.push(link);
                            wrote = i + 1;
                        }
                    }
                    start = None;
                    buffer.clear();
                }
                _ => {}
            }
            if start.is_some_and(|start| start != i) {
                buffer.push(byte);
            }
        }
        out.extend(parse_text(&text[wrote..], true));
        out
    }

    /// A link to the Go code that `text`, in brackets, names, if it names
    /// any: a package, by its path, or the name the package imports it as,
    /// or a name in one, like `io.Reader`, or `Config.Load`. It must have
    /// punctuation or a space before and after it, or nothing, so a type,
    /// like `map[K]V`, isn't a link.
    fn doc_link(&self, text: &str, before: &str, after: &str) -> Option<Inline> {
        let apart = |c: Option<char>| {
            c.is_none_or(|c| {
                GeneralCategory::of(c).is_punctuation() || matches!(c, ' ' | '\t' | '\n')
            })
        };
        if !apart(before.chars().next_back()) || !apart(after.chars().next()) {
            return None;
        }
        let name = text.strip_prefix('*').unwrap_or(text);
        let (mut package, name, found) = split_doc_name(name);
        let mut receiver = "";
        if found {
            (package, receiver, _) = split_doc_name(package);
        }
        let symbol = match receiver.is_empty() {
            true => name.to_string(),
            false => format!("{receiver}.{name}"),
        };
        let path = match package.is_empty() {
            false => self.lookup_package(package)?,
            true if self.context.syms.contains(&symbol) => self.context.import_path.to_string(),
            true => return None,
        };
        Some(Inline::DocLink {
            target: format!("{path}#{symbol}"),
            text: text.to_string(),
        })
    }

    /// The import path of the package a link calls `name`: a path with a
    /// `/`, the name the package imports one as, the package's own, or that
    /// of a package of Go's standard library whose path is one name.
    fn lookup_package(&self, name: &str) -> Option<String> {
        if name.contains('/') {
            return valid_import_path(name).then(|| name.to_string());
        }
        if let Some(path) = self.context.imports.get(name) {
            return Some(path.clone());
        }
        if name == self.context.name {
            return Some(self.context.import_path.to_string());
        }
        STD.contains(&name).then(|| name.to_string())
    }
}

/// `text` before its last `.`, or nothing, if it has none, and the name
/// after it, if that's exported, else `text`, nothing, and false.
fn split_doc_name(text: &str) -> (&str, &str, bool) {
    let dot = text.rfind('.');
    let name = &text[dot.map_or(0, |dot| dot + 1)..];
    if !is_name(name) {
        return (text, "", false);
    }
    (&text[..dot.unwrap_or(0)], name, true)
}

/// Whether `text` is an exported name, like `Reader`.
fn is_name(text: &str) -> bool {
    ident(text) == Some(text) && exported(text)
}

/// The name `text` starts with, if it starts with one: letters, `_`, and,
/// after the first, ASCII digits.
fn ident(text: &str) -> Option<&str> {
    let mut end = 0;
    for (at, c) in text.char_indices() {
        let letter = match c.is_ascii() {
            true => c.is_ascii_alphabetic() || c == '_' || (at > 0 && c.is_ascii_digit()),
            false => GeneralCategory::of(c).is_letter(),
        };
        if !letter {
            break;
        }
        end = at + c.len_utf8();
    }
    (end > 0).then(|| &text[..end])
}

/// The text of `text`, with no links.
fn plain(text: &str) -> String {
    let parts = parse_text(text, false).into_iter();
    (parts.map(|part| match part {
        Inline::Text(text) => text,
        _ => String::new(),
    }))
    .collect()
}

/// The parts of `text`, with no links in brackets: each URL in it, if
/// `auto_link`, as a link, and the text between, with each ` `` ` as `“`
/// and each `''` as `”`.
fn parse_text(text: &str, auto_link: bool) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut written = String::new();
    // Where the text not yet in `written` starts.
    let mut wrote = 0;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Nothing starts within a character.
        if !text.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let rest = &text[i..];
        if auto_link {
            if let Some(url) = auto_url(rest) {
                written += &text[wrote..i];
                if !written.is_empty() {
                    out.push(Inline::Text(std::mem::take(&mut written)));
                }
                out.push(Inline::Link {
                    text: url.to_string(),
                    url: url.to_string(),
                });
                i += url.len();
                wrote = i;
                continue;
            }
            // A URL doesn't start within a name.
            if let Some(name) = ident(rest) {
                i += name.len();
                continue;
            }
        }
        if rest.starts_with("```") {
            // Markdown's code, whose backticks are left as they are. As
            // go/doc/comment does, this counts the backticks after the
            // first three from `rest`'s start by `i`, `text`'s index.
            i += 3;
            while i < rest.len() && rest.as_bytes()[i] == b'`' {
                i += 1;
            }
        } else if rest.starts_with("``") || rest.starts_with("''") {
            written += &text[wrote..i];
            written.push(if rest.starts_with('`') { '“' } else { '”' });
            i += 2;
            wrote = i;
        } else {
            i += 1;
        }
    }
    written += &text[wrote..];
    if !written.is_empty() {
        out.push(Inline::Text(written));
    }
    out
}

/// The URL `text` starts with, if it starts with one, like
/// `https://go.dev/doc`: with a scheme Go links, a host, and a path, up to
/// before punctuation that would end a sentence, like `.`, or a bracket it
/// doesn't open.
fn auto_url(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes.len() < 7 {
        return None;
    }
    let colon = (3..=6).find(|&i| bytes[i] == b':')?;
    if !bytes[colon..].starts_with(b"://") || !is_scheme(&text[..colon]) {
        return None;
    }
    let mut i = colon + 3;
    if i >= bytes.len() || !is_host(bytes[i]) || is_punct(bytes[i]) {
        return None;
    }
    i += 1;
    let mut end = i;
    while i < bytes.len() && is_host(bytes[i]) {
        if !is_punct(bytes[i]) {
            end = i + 1;
        }
        i += 1;
    }
    i = end;
    let mut open = Vec::new();
    while i < bytes.len() {
        let byte = bytes[i];
        i += 1;
        if is_punct(byte) {
            continue;
        }
        if !is_path(byte) {
            break;
        }
        match byte {
            b'(' => open.push(b')'),
            b'{' => open.push(b'}'),
            b'[' => open.push(b']'),
            // A bracket that closes none that's open ends the URL.
            b')' | b'}' | b']' if open.pop() != Some(byte) => break,
            _ => {}
        }
        if open.is_empty() {
            end = i;
        }
    }
    Some(&text[..end])
}

fn is_scheme(scheme: &str) -> bool {
    let schemes = ["file", "ftp", "gopher", "http", "https", "mailto", "nntp"];
    schemes.contains(&scheme)
}

/// Whether `byte` can be in a URL's host, like `user@[::1]:8080`.
fn is_host(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"_@-.[]:".contains(&byte)
}

/// Whether `byte` is punctuation that can be in a URL, but not at its end.
fn is_punct(byte: u8) -> bool {
    b".,:;?!".contains(&byte)
}

/// Whether `byte` can be in a URL's path, but for [`is_punct`]'s.
fn is_path(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"$'()*+&#=@~_/-[]{}%".contains(&byte)
}

/// Whether `path` is a valid import path, as go/doc/comment checks it.
fn valid_import_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('-')
        && !path.contains("//")
        && !path.ends_with('/')
        && path.split('/').all(|element| {
            !element.is_empty()
                && !element.starts_with('.')
                && !element.ends_with('.')
                && (element.bytes())
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-.~_+".contains(&byte))
        })
}

/// Writes `text` at the end of `out`, a paragraph's Markdown so far,
/// escaping each character that Markdown, or an extension to it a site can
/// turn on, could read as more than text, and, at the paragraph's start,
/// each that would start a block there, with each newline as a space. A
/// `|` isn't escaped, since a table's cell escapes it, and a paragraph on
/// one line can't be a table.
fn escape(out: &mut String, text: &str) {
    // Whether the paragraph so far holds only spaces, and only digits after
    // them.
    let mut blank = out.is_empty();
    let mut digits = false;
    for c in text.chars() {
        let c = if c == '\n' { ' ' } else { c };
        let escaped = match c {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '&' | '#' | '!' | '$' | '~' | '^'
            | '=' | '+' | ':' | '@' => true,
            '.' => digits || after_www(out),
            ')' => digits,
            '|' => false,
            _ => blank && c.is_ascii_punctuation(),
        };
        if escaped {
            out.push('\\');
        }
        out.push(c);
        digits = c.is_ascii_digit() && (blank || digits);
        blank = blank && c == ' ';
    }
}

/// Whether `out` ends with `www`, which a `.` would make a link of.
fn after_www(out: &str) -> bool {
    out.len() >= 3 && out.as_bytes()[out.len() - 3..].eq_ignore_ascii_case(b"www")
}

/// `url` as a link's destination, in angle brackets, so it can hold spaces
/// and parentheses, with its `\` and what would end it escaped, and its `&`
/// written as `&amp;`, since comrak reads an entity there before an escape.
fn destination(url: &str) -> String {
    let mut written = String::from("<");
    for c in url.chars() {
        match c {
            '\n' | '\r' => {}
            '&' => written.push_str("&amp;"),
            '\\' | '<' | '>' => {
                written.push('\\');
                written.push(c);
            }
            _ => written.push(c),
        }
    }
    written.push('>');
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sections of `text`, a doc comment in `example.com/m/config`,
    /// which imports `gopkg.in/yaml.v3`, with each link to the module's code
    /// led to its target, and the rest left as code.
    fn parsed(text: &str) -> Vec<Section> {
        let imports = HashMap::from([("yaml".to_string(), "gopkg.in/yaml.v3".to_string())]);
        let syms = ["Config", "Config.Load", "Config.Path", "Load", "New"];
        let syms: HashSet<String> = syms.into_iter().map(String::from).collect();
        let context = Context {
            import_path: "example.com/m/config",
            name: "config",
            imports: &imports,
            syms: &syms,
        };
        let resolve = |target: &str| {
            target
                .starts_with("example.com/m")
                .then(|| target.to_string())
        };
        let mut sections = sections(text, &context);
        for section in &mut sections {
            if let Section::Text { text, .. } = section {
                *text = render(text, &resolve);
            }
        }
        sections
    }

    fn section(title: Option<&str>, text: &str) -> Section {
        let title = title.map(String::from);
        let text = text.to_string();
        Section::Text { title, text }
    }

    /// The Markdown of `text`, a doc comment with no headings.
    fn markdown(text: &str) -> String {
        match parsed(text).as_slice() {
            [Section::Text { title: None, text }] => text.clone(),
            sections => panic!("not one untitled section: {sections:?}"),
        }
    }

    #[test]
    fn blocks_are_read_as_go_reads_them() {
        let text = "Package config loads a site's config.\nIt reads YAML.\n\n\
                    # Usage\n\nCall [Load]:\n\n\tcfg, err := config.Load(path)\n\n\
                    # Empty\n\n# Options\n\n  - one\n  - two\n";
        let usage = "Call [`Load`](<example.com/m/config#Load>)\\:\n\n\
                     ```\ncfg, err := config.Load(path)\n```";
        assert_eq!(
            parsed(text),
            [
                section(None, "Package config loads a site's config. It reads YAML."),
                section(Some("Usage"), usage),
                section(Some("Options"), "- one\n- two"),
            ]
        );
        assert!(parsed("# Only a heading\n").is_empty());
        assert!(parsed("[Go]: https://go.dev\n").is_empty());
    }

    #[test]
    fn headings_are_lines_of_their_own() {
        assert_eq!(markdown("#not a heading"), "\\#not a heading");
        assert_eq!(markdown("# Heading\ncontinued"), "\\# Heading continued");
        assert_eq!(markdown("#"), "\\#");
    }

    #[test]
    fn unindented_code_and_lists_are_guessed_at() {
        assert_eq!(
            markdown("Example:\nfunc main() {\n\tfmt.Println(1)\n}\n"),
            "Example\\:\n\n```\nfunc main() {\n\tfmt.Println(1)\n}\n```"
        );
        assert_eq!(
            markdown("Options:\n- one\n  wraps\n- two\n"),
            "Options\\:\n\n- one wraps\n- two"
        );
    }

    #[test]
    fn lists_keep_their_numbers_and_spacing() {
        assert_eq!(
            markdown("Steps:\n\n 1. one\n 2) two\n"),
            "Steps\\:\n\n1. one\n2. two"
        );
        assert_eq!(
            markdown("Items:\n\n - a\n\n   more\n - b\n"),
            "Items\\:\n\n- a\n\n  more\n\n- b"
        );
        // A bullet in a numbered list is its item's text.
        assert_eq!(
            markdown("Steps:\n\n 3. one\n - two\n"),
            "Steps\\:\n\n3. one - two"
        );
    }

    #[test]
    fn links_lead_to_the_go_code_a_package_can_name() {
        assert_eq!(
            markdown(
                "See [Config.Load], [*Config], [config.New], [yaml.Node], [io], \
                 [encoding/json], [Missing], and map[Config]string. Wow![New]"
            ),
            "See [`Config.Load`](<example.com/m/config#Config.Load>), \
             [`*Config`](<example.com/m/config#Config>), \
             [`config.New`](<example.com/m/config#New>), `yaml.Node`, `io`, \
             `encoding/json`, \\[Missing\\], and map\\[Config\\]string. \
             Wow\\![`New`](<example.com/m/config#New>)"
        );
    }

    #[test]
    fn links_to_code_have_punctuation_or_spaces_around_them() {
        let config = "[`Config`](<example.com/m/config#Config>)";
        assert_eq!(markdown("«[Config]»"), format!("«{config}»"));
        assert_eq!(markdown("(\n[Config]\t)"), format!("\\( {config}\t)"));
        assert_eq!(markdown("a+[Config]"), "a\\+\\[Config\\]");
        assert_eq!(markdown("[Config]s"), "\\[Config\\]s");
    }

    #[test]
    fn links_lead_where_the_comment_defines_them() {
        let text = "Read the [Go home page] and the [Spec].\n\n\
                    [Go home page]: https://go.dev\n\
                    [Go home page]: https://example.com\n\n\
                    Items:\n\n  - [Spec]\n\n    [Spec]: https://go.dev/ref/spec\n";
        assert_eq!(
            markdown(text),
            "Read the [Go home page](<https://go.dev>) and the \
             [Spec](<https://go.dev/ref/spec>).\n\n\
             Items\\:\n\n- [Spec](<https://go.dev/ref/spec>)"
        );
    }

    #[test]
    fn urls_are_links_without_the_punctuation_after_them() {
        assert_eq!(
            markdown("See https://go.dev/doc/(effective_go), or (https://example.com/x)."),
            "See [https\\://go.dev/doc/(effective\\_go)](<https://go.dev/doc/(effective_go)>), \
             or ([https\\://example.com/x](<https://example.com/x>))."
        );
        // A URL doesn't start within a word.
        assert_eq!(markdown("xhttps://go.dev"), "xhttps\\://go.dev");
    }

    #[test]
    fn quotes_are_curly() {
        assert_eq!(markdown("``quoted''"), "“quoted”");
        // go/doc/comment leaves ``` alone, but counts the backticks after
        // it from the wrong place.
        assert_eq!(markdown("a ```b``` c"), "a \\`\\`\\`b\\`“ c");
    }

    #[test]
    fn markers_of_links_to_code_are_taken_out() {
        assert_eq!(markdown("a\u{E000}b\u{E001}c\u{E002}d"), "abcd");
    }

    #[test]
    fn links_that_lead_nowhere_are_code() {
        let text = format!("a {START}io#Reader{MIDDLE}`io.Reader`{END} b");
        assert_eq!(render(&text, &|_| None), "a `io.Reader` b");
        let url = |_: &str| Some("../io/README.md#reader".to_string());
        assert_eq!(
            render(&text, &url),
            "a [`io.Reader`](<../io/README.md#reader>) b"
        );
    }

    #[test]
    fn deprecation_is_a_paragraph_that_says_so() {
        assert!(deprecated("Load loads.\n\nDeprecated: use [New].\n"));
        assert!(deprecated("Deprecated: use New.\n"));
        assert!(!deprecated("Load is not\nDeprecated: really.\n"));
        assert!(!deprecated("Deprecated:use New.\n"));
    }

    #[test]
    fn packages_are_assumed_to_be_named_as_go_assumes() {
        assert_eq!(assumed_name("gopkg.in/yaml.v3"), "yaml");
        assert_eq!(assumed_name("example.com/go-yaml/v3"), "yaml");
        assert_eq!(assumed_name("github.com/mattn/go-sqlite3"), "sqlite3");
        assert_eq!(assumed_name("example.com/v2"), "example");
        assert_eq!(assumed_name("v2"), "v2");
        assert_eq!(assumed_name("example.com/héllo-world"), "héllo");
    }

    /// comrak's options, as little sets them, with every extension on, but
    /// smart punctuation, which changes text, or with none.
    fn options(on: bool) -> comrak::Options<'static> {
        let mut options = comrak::Options::default();
        let extension = &mut options.extension;
        extension.table = on;
        extension.strikethrough = on;
        extension.autolink = on;
        extension.tasklist = on;
        extension.footnotes = on;
        extension.alerts = on;
        extension.shortcodes = on;
        extension.math_dollars = on;
        extension.math_code = on;
        extension.superscript = on;
        extension.subscript = on;
        extension.highlight = on;
        extension.insert = on;
        extension.description_lists = on;
        extension.inline_footnotes = on;
        extension.block_directive = on;
        extension.multiline_block_quotes = on;
        extension.cjk_friendly_emphasis = on;
        extension.header_id_prefix = Some(String::new());
        options.render.hardbreaks = on;
        options.render.r#unsafe = on;
        options
    }

    fn escaped(text: &str) -> String {
        (text.replace('&', "&amp;"))
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
        "!",
    ];

    #[test]
    fn text_reads_back_as_text() {
        for on in [false, true] {
            let html = |markdown: &str| comrak::markdown_to_html(markdown, &options(on));
            for tricky in TRICKY {
                let mut paragraph = String::new();
                escape(&mut paragraph, tricky);
                let expected = format!("<p>{}</p>\n", escaped(tricky));
                assert_eq!(html(&paragraph), expected, "{tricky:?}, extensions {on}");

                let item = format!("- {paragraph}\n- {paragraph}");
                let expected =
                    format!("<ul>\n<li>{0}</li>\n<li>{0}</li>\n</ul>\n", escaped(tricky));
                assert_eq!(
                    html(&item),
                    expected,
                    "{tricky:?} in a list, extensions {on}"
                );

                let mut link = String::from("[");
                escape(&mut link, tricky);
                link += &format!("]({})", destination("https://go.dev/"));
                let expected = format!(
                    "<p><a href=\"https://go.dev/\">{}</a></p>\n",
                    escaped(tricky)
                );
                assert_eq!(
                    html(&link),
                    expected,
                    "{tricky:?} in a link, extensions {on}"
                );
            }
        }
    }

    #[test]
    fn destinations_hold_anything_on_their_line() {
        let link = format!("[a]({})", destination("https://go.dev/a b\\(c)<d>&amp;\n"));
        assert_eq!(
            comrak::markdown_to_html(&link, &options(true)),
            "<p><a href=\"https://go.dev/a%20b%5C(c)%3Cd%3E&amp;amp;\">a</a></p>\n"
        );
    }
}
