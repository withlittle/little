//! The blocks of a run of lines, as docutils' `Body` state reads them:
//! each starts with a line that says what it is, a bullet, a field's name,
//! `..`, and so on, else it's text: a paragraph, a definition list, or a
//! section's title. Lists are read in [`crate::lists`], explicit markup in
//! [`crate::explicit`], and tables in [`crate::tables`].

use crate::inline;
use crate::lines::{Line, indented, join, skip, trim_end, width};
use crate::lists;
use crate::state::{Dest, State};
use crate::tables;
use crate::text::normalize;
use crate::tree::{Node, Span};

/// What the first line of a block says it is, as docutils tries each in
/// turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transition {
    Bullet,
    Enumerator,
    Field,
    Option,
    Doctest,
    LineBlock,
    GridTable,
    SimpleTable,
    Explicit,
    Anonymous,
    Line,
    Text,
}

/// What the unindented line `text` starts.
pub(crate) fn transition(text: &str) -> Transition {
    if lists::bullet(text).is_some() {
        Transition::Bullet
    } else if lists::enumerator(text).is_some() {
        Transition::Enumerator
    } else if lists::field_marker(text).is_some() {
        Transition::Field
    } else if lists::option_marker(text).is_some() {
        Transition::Option
    } else if marker(text, ">>>") {
        Transition::Doctest
    } else if marker(text, "|") {
        Transition::LineBlock
    } else if tables::grid_top(text) {
        Transition::GridTable
    } else if tables::simple_top(text) {
        Transition::SimpleTable
    } else if marker(text, "..") {
        Transition::Explicit
    } else if marker(text, "__") {
        Transition::Anonymous
    } else if is_line(text) {
        Transition::Line
    } else {
        Transition::Text
    }
}

/// Whether `text` starts with `start`, then a space, or ends.
pub(crate) fn marker(text: &str, start: &str) -> bool {
    text.strip_prefix(start)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// Whether `text` is one punctuation character, repeated, like a section
/// title's underline, or a transition.
pub(crate) fn is_line(text: &str) -> bool {
    let mut chars = text.trim_end_matches(' ').chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_punctuation() && chars.all(|c| c == first))
}

/// The blocks of `lines`, where section titles may be, or not.
pub(crate) fn parse(state: &mut State, lines: &[Line], titles: bool) -> Vec<Node> {
    let mut body = Body {
        state,
        lines,
        at: 0,
        titles,
        nodes: Vec::new(),
    };
    body.run();
    body.nodes
}

/// The blocks of `lines`, nested in another, like a list item's: or, if
/// they're nested too deep, their text, as a paragraph.
pub(crate) fn nested(state: &mut State, lines: &[Line]) -> Vec<Node> {
    let Some(first) = lines.first() else {
        return Vec::new();
    };
    if !state.may_nest(first.num) {
        return vec![Node::Para(vec![Span::Text(join(lines))])];
    }
    state.depth += 1;
    let nodes = parse(state, lines, false);
    state.depth -= 1;
    nodes
}

/// A literal block, with no language.
pub(crate) fn code(text: String) -> Node {
    Node::Code {
        language: String::new(),
        text,
    }
}

/// Reads the blocks of a run of lines.
pub(crate) struct Body<'s, 'l> {
    pub state: &'s mut State,
    pub lines: &'l [Line],
    /// The index of the line to read next.
    pub at: usize,
    /// Whether section titles may be here.
    pub titles: bool,
    pub nodes: Vec<Node>,
}

impl Body<'_, '_> {
    fn run(&mut self) {
        let lines = self.lines;
        while let Some(line) = lines.get(self.at) {
            if line.text.is_empty() {
                self.at += 1;
                continue;
            }
            if line.text.starts_with(' ') {
                self.block_quote();
                continue;
            }
            let read = match transition(&line.text) {
                Transition::Bullet => self.bullet_list(),
                Transition::Enumerator => self.enumerated_list(),
                Transition::Field => self.field_list(),
                Transition::Option => self.option_list(),
                Transition::Doctest => self.doctest(),
                Transition::LineBlock => self.line_block(),
                Transition::GridTable => self.grid_table(),
                Transition::SimpleTable => self.simple_table(),
                Transition::Explicit => self.explicit(),
                Transition::Anonymous => self.anonymous_target(),
                Transition::Line => self.line(),
                Transition::Text => false,
            };
            if !read {
                self.text();
            }
        }
    }

    /// The number of the line at `index`, or of the line after the last.
    pub fn num(&self, index: usize) -> usize {
        match self.lines.get(index) {
            Some(line) => line.num,
            None => self.lines.last().map_or(1, |line| line.num + 1),
        }
    }

    /// Warn that `what` ends before the line at `index`, without a blank
    /// line.
    pub fn unindent_warning(&mut self, what: &str, index: usize) {
        let line = self.num(index);
        self.state.warn(
            line,
            format!("{what} ends without a blank line; unexpected unindent"),
        );
    }

    /// The spans of `text`, whose first line is `line`.
    pub fn inline(&mut self, text: &str, line: usize) -> Vec<Span> {
        inline::parse(self.state, text, line)
    }

    /// A block quote: an indented block, split at each attribution in it.
    fn block_quote(&mut self) {
        let block = indented(self.lines, self.at, false, None, None);
        self.at = block.end;
        let quotes = self.quotes(&block.lines);
        self.nodes.extend(quotes);
        if !block.blank_finish {
            self.unindent_warning("block quote", block.end);
        }
    }

    /// The block quotes in `lines`, split at each attribution in them.
    pub(crate) fn quotes(&mut self, lines: &[Line]) -> Vec<Node> {
        let mut quotes = Vec::new();
        let mut rest = lines;
        while !rest.is_empty() {
            let (quote, attribution, after) = split_attribution(rest);
            let blocks = nested(self.state, quote);
            let attribution = match attribution {
                Some((text, line)) => self.inline(&text, line),
                None => Vec::new(),
            };
            quotes.push(Node::Quote {
                blocks,
                attribution,
            });
            let blank = after.iter().take_while(|line| line.text.is_empty());
            rest = &after[blank.count()..];
        }
        quotes
    }

    /// A block that starts with text: a paragraph, a definition list, or a
    /// section's title, underlined.
    fn text(&mut self) {
        match self.lines.get(self.at + 1) {
            Some(next) if next.text.starts_with(' ') => self.definition_list(),
            Some(next) if is_line(&next.text) => {
                if !self.underlined() {
                    self.paragraph();
                }
            }
            _ => self.paragraph(),
        }
    }

    /// A paragraph: lines of text up to a blank line, and the literal block
    /// after it, if it ends with `::`.
    fn paragraph(&mut self) {
        let lines = self.lines;
        let start = self.at;
        let mut end = start + 1;
        let mut indented_at = None;
        while let Some(line) = lines.get(end) {
            if line.text.is_empty() {
                break;
            }
            if line.text.starts_with(' ') {
                indented_at = Some(line.num);
                break;
            }
            end += 1;
        }
        self.at = end;
        let (text, literal) = literal_marker(&join(&lines[start..end]));
        if let Some(text) = text {
            let spans = self.inline(&text, lines[start].num);
            self.nodes.push(Node::Para(spans));
        }
        if let Some(line) = indented_at {
            self.state.warn(line, "unexpected indentation");
        }
        if literal {
            self.literal_block();
        }
    }

    /// The literal block after a paragraph that ends with `::`: indented,
    /// else quoted.
    fn literal_block(&mut self) {
        let block = indented(self.lines, self.at, false, None, None);
        let mut lines = block.lines;
        trim_end(&mut lines);
        if lines.is_empty() {
            self.quoted_literal_block();
            return;
        }
        self.at = block.end;
        self.nodes.push(code(join(&lines)));
        if !block.blank_finish {
            self.unindent_warning("literal block", block.end);
        }
    }

    /// A literal block whose lines each start with the same punctuation.
    fn quoted_literal_block(&mut self) {
        let lines = self.lines;
        let mut at = self.at;
        while lines.get(at).is_some_and(|line| line.text.is_empty()) {
            at += 1;
        }
        let quote = lines.get(at).and_then(|line| line.text.chars().next());
        let Some(quote) = quote.filter(char::is_ascii_punctuation) else {
            let line = self.num(at);
            self.state.warn(line, "literal block expected; none found");
            self.at = at;
            return;
        };
        let start = at;
        while let Some(line) = lines.get(at) {
            if line.text.is_empty() {
                break;
            }
            if line.text.starts_with(' ') {
                self.state.warn(line.num, "unexpected indentation");
                break;
            }
            if !line.text.starts_with(quote) {
                self.state
                    .warn(line.num, "inconsistent literal block quoting");
                break;
            }
            at += 1;
        }
        self.at = at;
        self.nodes.push(code(join(&lines[start..at])));
    }

    /// A definition list: each item a line of text, its term, then an
    /// indented block, its definition.
    fn definition_list(&mut self) {
        let lines = self.lines;
        let mut items = Vec::new();
        loop {
            let term = &lines[self.at];
            let term = self.inline(&term.text, term.num);
            let block = indented(lines, self.at + 1, false, None, None);
            items.push((term, nested(self.state, &block.lines)));
            self.at = block.end;
            let mut next = self.at;
            while lines.get(next).is_some_and(|line| line.text.is_empty()) {
                next += 1;
            }
            let more = lines.get(next).is_some_and(|line| {
                !line.text.starts_with(' ') && transition(&line.text) == Transition::Text
            }) && lines
                .get(next + 1)
                .is_some_and(|line| line.text.starts_with(' '));
            if !more {
                if !block.blank_finish {
                    self.unindent_warning("definition list", block.end);
                }
                break;
            }
            self.at = next;
        }
        self.nodes.push(Node::Definitions(items));
    }

    /// A section's title, underlined: or false, if the underline is too
    /// short to be one, so it's text.
    fn underlined(&mut self) -> bool {
        let lines = self.lines;
        let (title, underline) = (&lines[self.at], &lines[self.at + 1]);
        if width(&title.text) > underline.text.chars().count() {
            if underline.text.chars().count() < 4 {
                return false;
            }
            self.state.warn(underline.num, "title underline too short");
        }
        self.at += 2;
        if !self.titles {
            self.state.warn(underline.num, "unexpected section title");
            let spans = self.inline(&title.text, title.num);
            self.nodes.push(Node::Para(spans));
            return true;
        }
        let style = (underline.text.chars().next().unwrap_or('='), false);
        self.section(&title.text, style, title.num);
        true
    }

    /// A line of punctuation: a transition, or a section title's overline;
    /// or false, if it's text.
    fn line(&mut self) -> bool {
        let lines = self.lines;
        let overline = &lines[self.at];
        let short = overline.text.chars().count() < 4;
        if !self.titles {
            if overline.text == "::" || short {
                return false;
            }
            self.state
                .warn(overline.num, "unexpected section title or transition");
            self.nodes.push(code(overline.text.clone()));
            self.at += 1;
            return true;
        }
        match lines.get(self.at + 1) {
            next if next.is_none_or(|next| next.text.is_empty()) => {
                if short {
                    return false;
                }
                self.nodes.push(Node::Rule);
                self.at += 1;
                true
            }
            Some(next) if is_line(&next.text) => {
                if short {
                    return false;
                }
                let message = "invalid section title or transition marker";
                self.state.warn(overline.num, message);
                self.nodes
                    .push(code(format!("{}\n{}", overline.text, next.text)));
                self.at += 2;
                true
            }
            _ => self.overlined(),
        }
    }

    /// A section's title, overlined and underlined: or false, if it isn't
    /// one, and the overline is so short it's text.
    fn overlined(&mut self) -> bool {
        let lines = self.lines;
        let (overline, title) = (&lines[self.at], &lines[self.at + 1]);
        let short = overline.text.chars().count() < 4;
        let Some(underline) = lines.get(self.at + 2) else {
            if short {
                return false;
            }
            self.state.warn(overline.num, "incomplete section title");
            self.nodes
                .push(code(format!("{}\n{}", overline.text, title.text)));
            self.at += 2;
            return true;
        };
        let source = format!("{}\n{}\n{}", overline.text, title.text, underline.text);
        let error = if !is_line(&underline.text) {
            Some("missing matching underline for section title overline")
        } else if overline.text != underline.text {
            Some("title overline & underline mismatch")
        } else {
            None
        };
        if let Some(error) = error {
            if short {
                return false;
            }
            self.state.warn(overline.num, error);
            self.nodes.push(code(source));
            self.at += 3;
            return true;
        }
        if width(&title.text) > overline.text.chars().count() {
            if short {
                return false;
            }
            self.state.warn(overline.num, "title overline too short");
        }
        self.at += 3;
        let style = (overline.text.chars().next().unwrap_or('='), true);
        self.section(title.text.trim_start(), style, title.num);
        true
    }

    /// A section, from its title, if its style puts it at a level that
    /// can follow the current one: else its title, as a paragraph.
    fn section(&mut self, title: &str, style: (char, bool), line: usize) {
        let state = &mut *self.state;
        let old = state.section_level;
        let new = match state.styles.iter().position(|&known| known == style) {
            Some(index) => index + 1,
            None => state.styles.len() + 1,
        };
        if new > old + 1 {
            state.warn(
                line,
                format!("inconsistent title style: skip from level {old} to {new}"),
            );
            let spans = self.inline(title, line);
            self.nodes.push(Node::Para(spans));
            return;
        }
        if new > state.styles.len() {
            state.styles.push(style);
        }
        state.section_level = new;
        let title = self.inline(title, line);
        let name = normalize(&Span::text(&title));
        let index = self
            .state
            .target(Dest::Here, &[name], false, "section", line);
        let id = self.state.targets[index].id.clone();
        self.nodes.push(Node::Heading {
            depth: new,
            title,
            ids: vec![id],
        });
    }

    /// A doctest block: lines up to a blank line, as Python's console.
    fn doctest(&mut self) -> bool {
        let lines = self.lines;
        let start = self.at;
        while lines.get(self.at).is_some_and(|line| !line.text.is_empty()) {
            self.at += 1;
        }
        self.nodes.push(Node::Code {
            language: "pycon".to_string(),
            text: join(&lines[start..self.at]),
        });
        true
    }

    /// A line block: each line, after a `|`, kept apart, with its indent,
    /// as a paragraph, with a line break between each.
    fn line_block(&mut self) -> bool {
        let lines = self.lines;
        let mut entries = Vec::new();
        loop {
            let line = &lines[self.at];
            let spaces = line.text[1..].bytes().take_while(|&b| b == b' ').count();
            let indent = (line.text.len() > 1).then(|| spaces.saturating_sub(1));
            let block = indented(lines, self.at, true, None, Some(1 + spaces));
            entries.push((indent, join(&block.lines), line.num));
            self.at = block.end;
            if block.blank_finish {
                break;
            }
            if !lines
                .get(self.at)
                .is_some_and(|line| marker(&line.text, "|"))
            {
                let line = self.num(self.at);
                self.state
                    .warn(line, "line block ends without a blank line");
                break;
            }
        }
        let spans = self.line_spans(&entries);
        self.nodes.push(Node::Para(spans));
        true
    }

    /// The text of a line block, from each of its lines' indent, or none,
    /// if it's blank, text, and number: each line, with its indent, and a
    /// line break between each.
    pub(crate) fn line_spans(&mut self, entries: &[(Option<usize>, String, usize)]) -> Vec<Span> {
        let mut previous = 0;
        let indents: Vec<usize> = entries
            .iter()
            .map(|entry| {
                previous = entry.0.unwrap_or(previous);
                previous
            })
            .collect();
        let least = indents.iter().copied().min().unwrap_or(0);
        let mut spans = Vec::new();
        for ((_, text, num), indent) in entries.iter().zip(indents) {
            if !spans.is_empty() {
                spans.push(Span::Break);
            }
            if indent > least {
                spans.push(Span::Text("\u{a0}".repeat(indent - least)));
            }
            let line = self.inline(text, *num);
            if line.is_empty() && indent == least {
                spans.push(Span::Text("\u{a0}".to_string()));
            }
            spans.extend(line);
        }
        spans
    }
}

/// A paragraph's text, without the `::` that ends it, if one does, as
/// docutils takes it off, or none, if that's all it is; and whether a
/// literal block follows.
fn literal_marker(data: &str) -> (Option<String>, bool) {
    if let Some(before) = data.strip_suffix("::") {
        let backslashes = before.chars().rev().take_while(|&c| c == '\\').count();
        if backslashes % 2 == 0 {
            if before.is_empty() {
                return (None, true);
            }
            if before.ends_with([' ', '\n']) {
                return (Some(before.trim_end().to_string()), true);
            }
            return (Some(data[..data.len() - 1].to_string()), true);
        }
    }
    (Some(data.to_string()), false)
}

/// The lines of a block quote, split at its attribution, if it has one: its
/// content, the attribution's text and line, and the lines after it.
fn split_attribution(lines: &[Line]) -> (&[Line], Option<(String, usize)>, &[Line]) {
    let mut blank = None;
    let mut seen = false;
    for (index, line) in lines.iter().enumerate() {
        if line.text.is_empty() {
            blank = Some(index);
            continue;
        }
        if seen && blank == Some(index - 1) {
            let start = attribution_start(&line.text);
            if let Some((start, (end, indent))) = start.zip(attribution_end(lines, index)) {
                let mut text = skip(&line.text, start).to_string();
                for line in &lines[index + 1..end] {
                    text.push('\n');
                    text.push_str(skip(&line.text, indent));
                }
                let text = text.trim_end().to_string();
                return (&lines[..index], Some((text, line.num)), &lines[end..]);
            }
        }
        seen = true;
    }
    (lines, None, &[])
}

/// Where the text of an attribution starts in `text`, after its dash, if
/// it is one.
fn attribution_start(text: &str) -> Option<usize> {
    let dash = if let Some(rest) = text.strip_prefix("---") {
        (!rest.starts_with('-')).then_some(3)
    } else if let Some(rest) = text.strip_prefix("--") {
        (!rest.starts_with('-')).then_some(2)
    } else {
        text.starts_with('\u{2014}').then_some(1)
    }?;
    let spaces = skip(text, dash).chars().take_while(|&c| c == ' ').count();
    (!skip(text, dash + spaces).is_empty()).then_some(dash + spaces)
}

/// The index after the last line of the attribution that starts at
/// `start`, and the indent of its lines after the first, if they're all
/// indented alike.
fn attribution_end(lines: &[Line], start: usize) -> Option<(usize, usize)> {
    let mut indent = None;
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        if line.text.is_empty() {
            return Some((index, indent.unwrap_or(0)));
        }
        let lead = crate::lines::indent(&line.text);
        if *indent.get_or_insert(lead) != lead {
            return None;
        }
    }
    Some((lines.len(), indent.unwrap_or(0)))
}
