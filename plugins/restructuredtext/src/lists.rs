//! Lists, as docutils reads them: bullet lists, enumerated lists, field
//! lists and option lists. Each item is a marker, then a block, indented
//! past it; a list goes on while the next item, after any blank lines, has
//! a marker of the same kind.

use crate::body::{Body, nested};
use crate::lines::indented;
use crate::text::from_roman;
use crate::tree::{Field, Node, Span};

/// The bullet `text` starts with, and where its item's text starts.
pub(crate) fn bullet(text: &str) -> Option<(char, usize)> {
    let mut chars = text.chars();
    let bullet = chars.next()?;
    if !matches!(
        bullet,
        '-' | '+' | '*' | '\u{2022}' | '\u{2023}' | '\u{2043}'
    ) {
        return None;
    }
    let rest = chars.as_str();
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    Some((bullet, 1 + rest.bytes().take_while(|&b| b == b' ').count()))
}

/// How an enumerator is written: `(1)`, `1)` or `1.`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    Parens,
    Rparen,
    Period,
}

/// The kind of numbers an enumerated list is numbered with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sequence {
    Arabic,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
    /// `#`, numbered automatically.
    Auto,
}

const SEQUENCES: [Sequence; 5] = [
    Sequence::Arabic,
    Sequence::LowerAlpha,
    Sequence::UpperAlpha,
    Sequence::LowerRoman,
    Sequence::UpperRoman,
];

impl Sequence {
    /// Whether `text` is a number in this sequence, as it's written.
    fn matches(self, text: &str) -> bool {
        let one = text.len() == 1;
        let all = |set: &str| !text.is_empty() && text.chars().all(|c| set.contains(c));
        match self {
            Sequence::Arabic => !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()),
            Sequence::LowerAlpha => one && text.bytes().all(|b| b.is_ascii_lowercase()),
            Sequence::UpperAlpha => one && text.bytes().all(|b| b.is_ascii_uppercase()),
            Sequence::LowerRoman => all("ivxlcdm"),
            Sequence::UpperRoman => all("IVXLCDM"),
            Sequence::Auto => text == "#",
        }
    }

    /// The number `text` is, in this sequence, if it's a valid one.
    fn ordinal(self, text: &str) -> Option<u64> {
        let letter = |base: u8| u64::from(text.as_bytes()[0] - base + 1);
        match self {
            Sequence::Arabic => text.parse().ok(),
            Sequence::LowerAlpha => Some(letter(b'a')),
            Sequence::UpperAlpha => Some(letter(b'A')),
            Sequence::LowerRoman | Sequence::UpperRoman => from_roman(&text.to_uppercase()),
            Sequence::Auto => Some(1),
        }
    }

    /// `ordinal`, written in this sequence, if it can be.
    fn write(self, ordinal: u64) -> Option<String> {
        match self {
            Sequence::Arabic => Some(ordinal.to_string()),
            Sequence::LowerAlpha | Sequence::UpperAlpha => {
                let base = if self == Sequence::LowerAlpha {
                    b'a'
                } else {
                    b'A'
                };
                (1..=26)
                    .contains(&ordinal)
                    .then(|| char::from(base + (ordinal - 1) as u8).to_string())
            }
            Sequence::LowerRoman => crate::text::to_roman(ordinal).map(|r| r.to_lowercase()),
            Sequence::UpperRoman => crate::text::to_roman(ordinal),
            Sequence::Auto => Some("#".to_string()),
        }
    }
}

/// An enumerator: how it's written, its text, and where its item's text
/// starts.
#[derive(Debug, Clone)]
pub(crate) struct Enumerator {
    pub format: Format,
    pub text: String,
    pub end: usize,
}

/// The enumerator `text` starts with, if it does.
pub(crate) fn enumerator(text: &str) -> Option<Enumerator> {
    let (open, rest) = match text.strip_prefix('(') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let first = *rest.as_bytes().first()?;
    let len = if first == b'#' {
        1
    } else if first.is_ascii_digit() {
        rest.bytes().take_while(u8::is_ascii_digit).count()
    } else if first.is_ascii_alphabetic() {
        let roman: &[u8] = if first.is_ascii_lowercase() {
            b"ivxlcdm"
        } else {
            b"IVXLCDM"
        };
        rest.bytes()
            .take_while(|b| roman.contains(b))
            .count()
            .max(1)
    } else {
        return None;
    };
    let after = &rest[len..];
    let (format, after) = if open {
        (Format::Parens, after.strip_prefix(')')?)
    } else if let Some(after) = after.strip_prefix(')') {
        (Format::Rparen, after)
    } else {
        (Format::Period, after.strip_prefix('.')?)
    };
    if !(after.is_empty() || after.starts_with(' ')) {
        return None;
    }
    let spaces = after.bytes().take_while(|&b| b == b' ').count();
    Some(Enumerator {
        format,
        text: rest[..len].to_string(),
        end: text.len() - after.len() + spaces,
    })
}

/// The sequence an enumerator's `text` is in, trying `expected` first.
fn sequence(text: &str, expected: Option<Sequence>) -> Sequence {
    if text == "#" {
        return Sequence::Auto;
    }
    match expected {
        Some(expected) if expected.matches(text) => return expected,
        Some(_) => {}
        None if text == "i" => return Sequence::LowerRoman,
        None if text == "I" => return Sequence::UpperRoman,
        None => {}
    }
    SEQUENCES
        .into_iter()
        .find(|sequence| sequence.matches(text))
        .unwrap_or(Sequence::Arabic)
}

/// `text` written as an enumerator in `format`.
fn enclose(format: Format, text: &str) -> String {
    match format {
        Format::Parens => format!("({text}) "),
        Format::Rparen => format!("{text}) "),
        Format::Period => format!("{text}. "),
    }
}

/// The name of the field whose marker `text` starts with, and where its
/// body starts, in characters, as docutils' pattern matches it: a colon,
/// the name, which doesn't start with a space or colon, or end with a
/// space, then a colon, then spaces, or the line's end.
pub(crate) fn field_marker(text: &str) -> Option<(String, usize)> {
    let rest = text.strip_prefix(':')?;
    if rest.starts_with([':', ' ']) {
        return None;
    }
    let mut chars = rest.char_indices().peekable();
    let mut count = 1;
    let mut before = ':';
    while let Some((at, c)) = chars.next() {
        count += 1;
        match c {
            '\\' => {
                let (_, escaped) = chars.next()?;
                count += 1;
                before = escaped;
                continue;
            }
            ':' => match chars.peek() {
                None | Some((_, ' ')) => {
                    if before == ' ' {
                        return None;
                    }
                    let spaces = rest[at + 1..].bytes().take_while(|&b| b == b' ').count();
                    return Some((rest[..at].to_string(), count + spaces));
                }
                Some((_, '`')) => return None,
                Some(_) => {}
            },
            _ => {}
        }
        before = c;
    }
    None
}

/// Where the option at `at` in `text` ends, if one starts there: a short
/// option, like `-a` or `+a`, or a long one, like `--all` or `/A`, each
/// with its argument, if it has one.
fn option(text: &[u8], at: usize) -> Option<usize> {
    let rest = &text[at..];
    let (start, long) = match rest {
        [b'-', b'-', ..] => (2, true),
        [b'-' | b'+', ..] => (1, false),
        [b'/', ..] => (1, true),
        _ => return None,
    };
    if !rest.get(start)?.is_ascii_alphanumeric() {
        return None;
    }
    let mut end = start + 1;
    if long {
        end += rest[end..]
            .iter()
            .take_while(|&&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            .count();
    }
    let argument = match rest.get(end) {
        Some(b' ' | b'=') if long => argument(&rest[end + 1..]).map(|len| len + 1),
        Some(b' ') => argument(&rest[end + 1..]).map(|len| len + 1),
        Some(_) if !long => argument(&rest[end..]),
        _ => None,
    };
    Some(at + end + argument.unwrap_or(0))
}

/// The length of the option's argument `text` starts with: a name, or
/// anything but angle brackets, in angle brackets.
fn argument(text: &[u8]) -> Option<usize> {
    match text.first()? {
        b'<' => {
            let len = text[1..].iter().position(|&b| b == b'<' || b == b'>')?;
            (len > 0 && text[1 + len] == b'>').then_some(len + 2)
        }
        first if first.is_ascii_alphabetic() => Some(
            1 + text[1..]
                .iter()
                .take_while(|&&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                .count(),
        ),
        _ => None,
    }
}

/// The options `text` starts with, as written, and where their
/// description starts, in characters: after two spaces or more, else at
/// the line's end.
pub(crate) fn option_marker(text: &str) -> Option<(Vec<String>, usize)> {
    let bytes = text.as_bytes();
    let mut options = Vec::new();
    let mut at = 0;
    loop {
        let end = option(bytes, at)?;
        options.push(text[at..end].to_string());
        at = end;
        if bytes[at..].starts_with(b", ") {
            at += 2;
        } else {
            break;
        }
    }
    let spaces = bytes[at..].iter().take_while(|&&b| b == b' ').count();
    if at + spaces == bytes.len() || spaces >= 2 {
        // An argument in angle brackets can hold any character.
        Some((options, text[..at + spaces].chars().count()))
    } else {
        None
    }
}

impl Body<'_, '_> {
    /// The blocks of the item on the line at `self.at`, whose marker ends
    /// at `end`, in characters, and whether a blank line ends it. Its
    /// lines are indented as far as its text, if the marker's line has
    /// some, else as far as the least of those after it.
    fn item(&mut self, end: usize) -> (Vec<Node>, bool) {
        let text = &self.lines[self.at].text;
        let block = if text.chars().nth(end).is_some() {
            indented(self.lines, self.at, false, Some(end), None)
        } else {
            indented(self.lines, self.at, false, None, Some(end))
        };
        self.at = block.end;
        (nested(self.state, &block.lines), block.blank_finish)
    }

    /// The index of the next line that isn't blank, if it isn't indented,
    /// so it may be a list's next item.
    fn next_item(&self) -> Option<usize> {
        let blank = self.lines[self.at..]
            .iter()
            .take_while(|line| line.text.is_empty());
        let next = self.at + blank.count();
        self.lines
            .get(next)
            .is_some_and(|line| !line.text.starts_with(' '))
            .then_some(next)
    }

    /// Ends a list of `what`, warning if no blank line ends its last item.
    fn end_list(&mut self, what: &str, blank_finish: bool, node: Node) -> bool {
        self.nodes.push(node);
        if !blank_finish {
            self.unindent_warning(what, self.at);
        }
        true
    }

    /// A bullet list, whose items each start with the same bullet.
    pub fn bullet_list(&mut self) -> bool {
        let Some((first, mut end)) = bullet(&self.lines[self.at].text) else {
            return false;
        };
        let mut items = Vec::new();
        let blank_finish = loop {
            let (blocks, blank_finish) = self.item(end);
            items.push(blocks);
            let next = self.next_item().and_then(|next| {
                let marker = bullet(&self.lines[next].text)?;
                (marker.0 == first).then_some((next, marker.1))
            });
            match next {
                Some((next, marker)) => (self.at, end) = (next, marker),
                None => break blank_finish,
            }
        };
        let list = Node::List { start: None, items };
        self.end_list("bullet list", blank_finish, list)
    }

    /// Whether the enumerator on the line at `index` starts an item: it's
    /// a valid number, and the next line is blank, indented, or starts
    /// with the next enumerator, or `#`.
    fn is_item(
        &self,
        index: usize,
        ordinal: Option<u64>,
        sequence: Sequence,
        format: Format,
    ) -> bool {
        let Some(ordinal) = ordinal else {
            return false;
        };
        let Some(next) = self.lines.get(index + 1) else {
            return true;
        };
        if next.text.is_empty() || next.text.starts_with(' ') {
            return true;
        }
        let following = ordinal.checked_add(1).and_then(|next| sequence.write(next));
        following.is_some_and(|following| {
            next.text.starts_with(&enclose(format, &following))
                || next.text.starts_with(&enclose(format, "#"))
        })
    }

    /// An enumerated list, whose items are numbered in turn, in the same
    /// sequence and format, or with `#`.
    pub fn enumerated_list(&mut self) -> bool {
        let Some(first) = enumerator(&self.lines[self.at].text) else {
            return false;
        };
        let kind = sequence(&first.text, None);
        let start = kind.ordinal(&first.text);
        if !self.is_item(self.at, start, kind, first.format) {
            return false;
        }
        let mut auto = kind == Sequence::Auto;
        let kind = if auto { Sequence::Arabic } else { kind };
        let mut last = start.unwrap_or(1);
        let mut end = first.end;
        let mut items = Vec::new();
        let blank_finish = loop {
            let (blocks, blank_finish) = self.item(end);
            items.push(blocks);
            let Some(next) = self.next_item() else {
                break blank_finish;
            };
            let Some(marker) = enumerator(&self.lines[next].text) else {
                break blank_finish;
            };
            let sequence = sequence(&marker.text, Some(kind));
            let ordinal = sequence.ordinal(&marker.text);
            let numbered = sequence == Sequence::Auto
                || (sequence == kind && !auto && ordinal == last.checked_add(1));
            if marker.format != first.format
                || !numbered
                || !self.is_item(next, ordinal, sequence, marker.format)
            {
                break blank_finish;
            }
            auto |= sequence == Sequence::Auto;
            last = ordinal.unwrap_or(1);
            (self.at, end) = (next, marker.end);
        };
        let list = Node::List { start, items };
        self.end_list("enumerated list", blank_finish, list)
    }

    /// A field list: each field a name, between colons, then its body.
    pub fn field_list(&mut self) -> bool {
        let mut fields = Vec::new();
        let blank_finish = loop {
            let line = &self.lines[self.at];
            let Some((raw, end)) = field_marker(&line.text) else {
                return false;
            };
            let name = self.inline(&raw, line.num);
            let block = indented(self.lines, self.at, false, None, Some(end));
            self.at = block.end;
            let body = nested(self.state, &block.lines);
            fields.push(Field { name, raw, body });
            match self.next_item() {
                Some(next) if field_marker(&self.lines[next].text).is_some() => self.at = next,
                _ => break block.blank_finish,
            }
        };
        self.end_list("field list", blank_finish, Node::Fields(fields))
    }

    /// An option list: each item options, then their description. An
    /// option with no description is text.
    pub fn option_list(&mut self) -> bool {
        let mut items = Vec::new();
        let mut blank_finish = true;
        while let Some((options, end)) = option_marker(&self.lines[self.at].text) {
            let block = indented(self.lines, self.at, false, None, Some(end));
            if block.lines.is_empty() {
                break;
            }
            self.at = block.end;
            let mut term = Vec::new();
            for option in options {
                if !term.is_empty() {
                    term.push(Span::Text(", ".to_string()));
                }
                term.push(Span::Code(option));
            }
            items.push((term, nested(self.state, &block.lines)));
            blank_finish = block.blank_finish;
            match self.next_item() {
                Some(next) if option_marker(&self.lines[next].text).is_some() => self.at = next,
                _ => break,
            }
        }
        if items.is_empty() {
            return false;
        }
        self.end_list("option list", blank_finish, Node::Definitions(items))
    }
}
