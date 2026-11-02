//! Text as docutils reads it: the characters inline markup may follow and
//! be followed by, names and the ids made from them, backslash escapes, and
//! Roman numerals.

use crate::data::{CLOSERS, END_SUFFIX, FOLD, OPENERS, START_PREFIX};
use crate::lines::is_space;

/// The character the parser puts in place of an escaping backslash.
pub(crate) const ESCAPE: char = '\0';

/// Whether `c` is in one of `ranges`.
fn in_ranges(ranges: &[(char, char)], c: char) -> bool {
    ranges
        .binary_search_by(|&(low, high)| {
            if high < c {
                std::cmp::Ordering::Less
            } else if low > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Whether inline markup may start after `c`.
pub(crate) fn is_prefix(c: char) -> bool {
    is_space(c) || in_ranges(START_PREFIX, c)
}

/// Whether inline markup may end before `c`.
pub(crate) fn is_suffix(c: char) -> bool {
    is_space(c) || c == ESCAPE || in_ranges(END_SUFFIX, c)
}

/// Whether `open` and `close` are a pair, like `(` and `)`, which a
/// start-string between them doesn't start markup in.
pub(crate) fn match_chars(open: char, close: char) -> bool {
    let Some(index) = OPENERS.chars().position(|c| c == open) else {
        return false;
    };
    let others = match open {
        '\u{bb}' => "\u{bb}",
        '\u{2018}' => "\u{201a}",
        '\u{2019}' => "\u{2019}",
        '\u{201a}' => "\u{2018}\u{2019}",
        '\u{201c}' => "\u{201e}",
        '\u{201e}' => "\u{201c}\u{201d}",
        '\u{201d}' => "\u{201d}",
        '\u{203a}' => "\u{203a}",
        _ => "",
    };
    CLOSERS.chars().nth(index) == Some(close) || others.contains(close)
}

/// `text`, with its whitespace collapsed to single spaces, as docutils'
/// `whitespace_normalize_name` has it.
pub(crate) fn collapse(text: &str) -> String {
    text.split(is_space)
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The name `text` gives a target, as docutils' `normalize_name` has it:
/// collapsed and lowercased.
pub(crate) fn normalize(text: &str) -> String {
    collapse(text).to_lowercase()
}

/// The id docutils' `make_id` makes of `text`: lowercase ASCII letters,
/// digits and hyphens, starting with a letter.
pub(crate) fn make_id(text: &str) -> String {
    let mut ascii = String::with_capacity(text.len());
    for c in text.to_lowercase().chars() {
        if c.is_ascii() {
            ascii.push(c);
        } else if let Ok(index) = FOLD.binary_search_by(|(key, _)| key.cmp(&c)) {
            ascii.push_str(FOLD[index].1);
        }
    }
    let mut id = String::with_capacity(ascii.len());
    for c in ascii.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            id.push(c);
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_start_matches(|c: char| c == '-' || c.is_ascii_digit());
    id.trim_end_matches('-').to_string()
}

/// `text`, with each backslash and the character after it written as
/// [`ESCAPE`] and that character, as docutils' `escape2null` has it.
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            out.push(ESCAPE);
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Escaped `text`, without its escapes: an escaped space or newline goes
/// with its escape.
pub(crate) fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ESCAPE {
            if matches!(chars.peek(), Some(' ' | '\n')) {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Escaped `text`, with its backslashes put back.
pub(crate) fn restore(text: &str) -> String {
    text.replace(ESCAPE, "\\")
}

const ROMAN: [(u64, &str); 13] = [
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
];

/// `number`, from 1 to 4,999, as an uppercase Roman numeral.
pub(crate) fn to_roman(mut number: u64) -> Option<String> {
    if !(1..5000).contains(&number) {
        return None;
    }
    let mut out = String::new();
    for (value, letters) in ROMAN {
        while number >= value {
            out.push_str(letters);
            number -= value;
        }
    }
    Some(out)
}

/// The number the uppercase Roman numeral `text` is, if it's written as
/// docutils reads them, from I to MMMMCMXCIX.
pub(crate) fn from_roman(text: &str) -> Option<u64> {
    let mut number = 0;
    let mut rest = text;
    for (value, letters) in ROMAN {
        while let Some(after) = rest.strip_prefix(letters) {
            number += value;
            rest = after;
        }
    }
    (rest.is_empty() && to_roman(number).as_deref() == Some(text)).then_some(number)
}
