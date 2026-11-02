//! The lines of a file, and the blocks of indented lines in them, as
//! docutils' state machine reads them.

use unicode_width::UnicodeWidthChar;

/// A line of the file, with its tabs expanded and its trailing whitespace
/// stripped, and its number, from 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    pub text: String,
    pub num: usize,
}

/// Whether `c` is whitespace, as Python's `str.isspace` has it.
pub(crate) fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// The lines of `text`, numbered from 1.
pub(crate) fn split(text: &str) -> Vec<Line> {
    text.split('\n')
        .enumerate()
        .map(|(index, line)| Line {
            text: clean(line),
            num: index + 1,
        })
        .collect()
}

/// `line`, with its tabs expanded to every 8th column, vertical tabs and
/// form feeds as spaces, NULs, which the parser uses itself, replaced, and
/// trailing whitespace stripped.
fn clean(line: &str) -> String {
    let line = line.strip_suffix('\r').unwrap_or(line);
    let mut out = String::with_capacity(line.len());
    let mut column = 0;
    for c in line.chars() {
        match c {
            '\t' => {
                let spaces = 8 - column % 8;
                out.extend(std::iter::repeat_n(' ', spaces));
                column += spaces;
                continue;
            }
            '\u{b}' | '\u{c}' => out.push(' '),
            '\0' => out.push('\u{fffd}'),
            c => out.push(c),
        }
        column += 1;
    }
    out.truncate(out.trim_end_matches(is_space).len());
    out
}

/// The number of spaces `text` starts with.
pub(crate) fn indent(text: &str) -> usize {
    text.bytes().take_while(|&b| b == b' ').count()
}

/// `text` after its first `chars` characters.
pub(crate) fn skip(text: &str, chars: usize) -> &str {
    match text.char_indices().nth(chars) {
        Some((at, _)) => &text[at..],
        None => "",
    }
}

/// The columns `text` takes, as docutils' `column_width` counts them: two
/// for a wide character, none for a combining one.
pub(crate) fn width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

/// The columns `c` takes, as docutils counts them: two if it's wide, none
/// if it's combining.
pub(crate) fn char_width(c: char) -> usize {
    match c.width() {
        Some(2) => 2,
        Some(0) if !c.is_control() => 0,
        _ => 1,
    }
}

/// A block of indented lines, as docutils' `get_indented` finds it.
#[derive(Debug)]
pub(crate) struct Indented {
    /// The lines, without their indent, and without blank lines at the top.
    pub lines: Vec<Line>,
    /// The number of blank lines taken off its top.
    pub skipped: usize,
    /// The index of the first line after the block.
    pub end: usize,
    /// Whether the block ends with a blank line, or the end of the lines.
    pub blank_finish: bool,
}

/// The block of indented lines in `lines` from `start`. With `first`, the
/// first line is in it, and its first `first` characters are taken off;
/// with `known`, every other line must be indented by at least `known`,
/// which is taken off, else by their least indent. With `until_blank`, a
/// blank line ends the block.
pub(crate) fn indented(
    lines: &[Line],
    start: usize,
    until_blank: bool,
    known: Option<usize>,
    first: Option<usize>,
) -> Indented {
    let first = first.or(known);
    let mut indent = known;
    let mut end = start + usize::from(first.is_some());
    let mut blank_finish = true;
    while end < lines.len() {
        let text = &lines[end].text;
        let lead = self::indent(text);
        if !text.is_empty() && (lead == 0 || known.is_some_and(|known| lead < known)) {
            blank_finish = end > start && lines[end - 1].text.is_empty();
            break;
        }
        if text.is_empty() {
            if until_blank {
                break;
            }
        } else if known.is_none() {
            indent = Some(indent.map_or(lead, |indent| indent.min(lead)));
        }
        end += 1;
    }
    let indent = indent.unwrap_or(0);
    let mut block: Vec<Line> = lines[start..end.min(lines.len())].to_vec();
    for (index, line) in block.iter_mut().enumerate() {
        let take = match (index, first) {
            (0, Some(first)) => first,
            _ => indent,
        };
        if take > 0 {
            line.text = skip(&line.text, take).to_string();
        }
    }
    let leading = block.iter().take_while(|line| line.text.is_empty()).count();
    block.drain(..leading);
    Indented {
        lines: block,
        skipped: leading,
        end,
        blank_finish,
    }
}

/// `lines` without the blank lines at their end.
pub(crate) fn trim_end(lines: &mut Vec<Line>) {
    while lines.last().is_some_and(|line| line.text.is_empty()) {
        lines.pop();
    }
}

/// The text of `lines`, joined.
pub(crate) fn join(lines: &[Line]) -> String {
    let mut text = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str(&line.text);
    }
    text
}
