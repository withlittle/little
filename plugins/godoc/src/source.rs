//! A Go file's tokens, and what Go's parser makes of them: the semicolons
//! its scanner inserts at the ends of lines, the declarations and fields
//! they end, and the doc comment and line comment of each.

use gosyn::LexicalToken;
use gosyn::token::{Keyword, LitKind, Operator, Token};

/// A group of comments, as the tokens from the first to before the last.
pub type Group = (usize, usize);

/// What Go's parser steps through.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// A comment, at its index in the tokens.
    Comment(usize),
    /// Any other token, at its index in the tokens.
    Token(usize),
    /// A semicolon Go's scanner inserts at the end of a line.
    Semicolon,
    /// The end of the file.
    End,
}

#[derive(Debug)]
struct Step {
    kind: Kind,
    /// The line it starts on.
    line: usize,
    /// The line it ends on, which, for a comment, may be a later one.
    end_line: usize,
    /// The comments Go's parser has as a doc comment as it reaches it.
    lead: Option<Group>,
    /// The comments Go's parser has as a line comment as it reaches it.
    trailing: Option<Group>,
}

/// A Go file's text and tokens, as gosyn reads them, at offsets in chars.
pub struct Source<'a> {
    text: &'a str,
    /// The byte offset of each char, then that of the end of the text.
    bytes: Vec<usize>,
    /// The offset each line starts at.
    lines: Vec<usize>,
    /// The tokens, with the comments, but not the semicolons Go inserts.
    pub tokens: Vec<LexicalToken>,
    /// The tokens, with the semicolons Go inserts, and the end of the file.
    steps: Vec<Step>,
    /// The index in `steps` of each token.
    step: Vec<usize>,
}

impl<'a> Source<'a> {
    /// Reads where Go's parser finds comments in `text`, from its tokens.
    pub fn new(text: &'a str, tokens: Vec<LexicalToken>) -> Self {
        let bytes = text.char_indices().map(|(byte, _)| byte);
        let newlines = text.chars().enumerate().filter(|&(_, c)| c == '\n');
        let mut source = Source {
            text,
            bytes: bytes.chain([text.len()]).collect(),
            lines: [0]
                .into_iter()
                .chain(newlines.map(|(i, _)| i + 1))
                .collect(),
            tokens,
            steps: Vec::new(),
            step: Vec::new(),
        };
        source.scan();
        source.parse();
        source
    }

    /// Steps through the tokens as Go's scanner does, inserting a semicolon
    /// after one that can end a statement, at the end of its line: at the
    /// first newline after it that isn't in a comment, else after the first
    /// comment with a newline in it, or at the end of the file.
    fn scan(&mut self) {
        let mut steps = Vec::new();
        let mut step = Vec::with_capacity(self.tokens.len());
        let (mut insert, mut end) = (false, 0);
        for (i, token) in self.tokens.iter().enumerate() {
            if insert && let Some(newline) = self.newline(end, token.start) {
                steps.push(self.at(Kind::Semicolon, newline, newline + 1));
                insert = false;
            }
            step.push(steps.len());
            if let Token::Comment(comment) = &token.token {
                steps.push(self.at(Kind::Comment(i), token.start, token.end));
                if insert
                    && comment.starts_with("/*")
                    && let Some(newline) = self.newline(token.start, token.end)
                {
                    steps.push(self.at(Kind::Semicolon, newline, newline + 1));
                    insert = false;
                }
            } else {
                steps.push(self.at(Kind::Token(i), token.start, token.end));
                insert = ends_statement(&token.token);
            }
            end = token.end;
        }
        let length = self.bytes.len() - 1;
        if insert {
            let newline = self.newline(end, length).unwrap_or(length);
            steps.push(self.at(Kind::Semicolon, newline, newline + 1));
        }
        steps.push(self.at(Kind::End, length, length + 1));
        self.steps = steps;
        self.step = step;
    }

    fn at(&self, kind: Kind, start: usize, end: usize) -> Step {
        Step {
            kind,
            line: self.line(start),
            end_line: self.line(end.max(start + 1) - 1),
            lead: None,
            trailing: None,
        }
    }

    /// Steps through as Go's parser does, noting the doc comment and line
    /// comment it has as it reaches each step that isn't a comment: a line
    /// comment is on the line of the step before, with nothing after it
    /// there, and a doc comment is on the lines just before the step.
    fn parse(&mut self) {
        let (mut previous, mut k) = (None, 0);
        loop {
            let (mut lead, mut trailing) = (None, None);
            if self.is_comment_step(k) {
                if previous == Some(self.steps[k].line) {
                    let (group, end_line, next) = self.group(k, 0);
                    k = next;
                    if self.steps[k].line != end_line
                        || self.is_semicolon(k)
                        || self.steps[k].kind == Kind::End
                    {
                        trailing = Some(group);
                    }
                }
                let mut last = None;
                while self.is_comment_step(k) {
                    let (group, end_line, next) = self.group(k, 1);
                    last = Some((group, end_line));
                    k = next;
                }
                if let Some((group, end_line)) = last
                    && end_line + 1 == self.steps[k].line
                {
                    lead = Some(group);
                }
            }
            let step = &mut self.steps[k];
            (step.lead, step.trailing) = (lead, trailing);
            if step.kind == Kind::End {
                break;
            }
            previous = Some(step.line);
            k += 1;
        }
    }

    /// The comments from step `k` that Go's parser groups, each starting
    /// no more than `lines` lines after the one before ends, the line they
    /// end on, and the step after them.
    fn group(&self, mut k: usize, lines: usize) -> (Group, usize, usize) {
        let Kind::Comment(start) = self.steps[k].kind else {
            unreachable!("a group starts with a comment");
        };
        let (first, mut end_line) = (k, self.steps[k].line);
        while self.is_comment_step(k) && self.steps[k].line <= end_line + lines {
            end_line = self.steps[k].end_line;
            k += 1;
        }
        ((start, start + k - first), end_line, k)
    }

    fn is_comment_step(&self, k: usize) -> bool {
        matches!(self.steps[k].kind, Kind::Comment(_))
    }

    fn is_semicolon(&self, k: usize) -> bool {
        match self.steps[k].kind {
            Kind::Semicolon => true,
            Kind::Token(i) => self.tokens[i].token == Token::Operator(Operator::SemiColon),
            _ => false,
        }
    }

    /// The step after `k` that isn't a comment.
    fn after(&self, k: usize) -> usize {
        (k + 1..)
            .find(|&k| !self.is_comment_step(k))
            .expect("the file ends after its comments")
    }

    /// The doc comment of what starts at the token `first`: the comments
    /// Go's parser has as a lead comment as it reaches it.
    pub fn doc(&self, first: usize) -> Option<Group> {
        self.steps[self.step[first]].lead
    }

    /// The line comment of what ends at the token `last`, as Go's parser
    /// reads it with the semicolon after it: the comments after `last` on
    /// its line, or after a `;` there, but none before a `)` or `}`.
    pub fn line_comment(&self, last: usize) -> Option<Group> {
        let next = self.after(self.step[last]);
        if self.steps[next].kind == Kind::Semicolon {
            // Go's scanner inserts it after the comments.
            self.steps[next].trailing
        } else if self.is_semicolon(next) {
            self.steps[self.after(next)].trailing
        } else {
            None
        }
    }

    /// The file's elements, as the semicolons outside brackets end them,
    /// each from its first token to its last, but for comments.
    pub fn elements(&self) -> Vec<(usize, usize)> {
        self.split(0, self.steps.len())
    }

    /// The elements between the brackets at the tokens `open` and `close`.
    pub fn elements_in(&self, open: usize, close: usize) -> Vec<(usize, usize)> {
        self.split(self.step[open] + 1, self.step[close])
    }

    fn split(&self, from: usize, to: usize) -> Vec<(usize, usize)> {
        let mut elements = Vec::new();
        let mut current: Option<(usize, usize)> = None;
        let mut depth = 0usize;
        for step in &self.steps[from..to] {
            let i = match step.kind {
                Kind::Token(i) => i,
                Kind::Semicolon if depth == 0 => {
                    elements.extend(current.take());
                    continue;
                }
                _ => continue,
            };
            match self.tokens[i].token {
                Token::Operator(Operator::SemiColon) if depth == 0 => {
                    elements.extend(current.take());
                    continue;
                }
                Token::Operator(
                    Operator::ParenLeft | Operator::BarackLeft | Operator::BraceLeft,
                ) => {
                    depth += 1;
                }
                Token::Operator(
                    Operator::ParenRight | Operator::BarackRight | Operator::BraceRight,
                ) => depth = depth.saturating_sub(1),
                _ => {}
            }
            current = Some((current.map_or(i, |(first, _)| first), i));
        }
        elements.extend(current);
        elements
    }

    /// The index of the first token that starts at `position` or after it.
    pub fn index(&self, position: usize) -> usize {
        self.tokens.partition_point(|token| token.start < position)
    }

    pub fn is_comment(&self, i: usize) -> bool {
        matches!(self.tokens[i].token, Token::Comment(_))
    }

    /// The tokens from `first` to `last` that aren't comments.
    pub fn significant(&self, first: usize, last: usize) -> Vec<usize> {
        (first..=last).filter(|&i| !self.is_comment(i)).collect()
    }

    /// The line `position` is on, from 0.
    pub fn line(&self, position: usize) -> usize {
        self.lines.partition_point(|&start| start <= position) - 1
    }

    /// The first newline from `from` to before `to`.
    fn newline(&self, from: usize, to: usize) -> Option<usize> {
        let next = self.lines.get(self.line(from) + 1)?;
        (next - 1 < to).then_some(next - 1)
    }

    fn slice(&self, start: usize, end: usize) -> &'a str {
        &self.text[self.bytes[start]..self.bytes[end]]
    }

    /// The spaces and tabs before the token `i` on its line, unless
    /// something else is there too.
    pub fn indent(&self, i: usize) -> Option<&'a str> {
        let start = self.tokens[i].start;
        let before = self.slice(self.lines[self.line(start)], start);
        before
            .chars()
            .all(|c| c == ' ' || c == '\t')
            .then_some(before)
    }

    /// The code from the token `first` to the token `last`, as it's
    /// written, without the indentation of `first`'s line.
    pub fn code(&self, first: usize, last: usize) -> String {
        self.code_within(first, last, self.indent(first).unwrap_or(""))
    }

    /// The code from the token `first` to the token `last`, with `margin`
    /// taken from the start of each line after the first, but in a string.
    pub fn code_within(&self, first: usize, last: usize, margin: &str) -> String {
        let (start, end) = (self.tokens[first].start, self.tokens[last].end);
        let mut code = String::new();
        let mut from = start;
        for &line in &self.lines[self.line(start) + 1..] {
            if line >= end {
                break;
            }
            code.push_str(self.slice(from, line));
            from = line;
            if !margin.is_empty()
                && !self.in_string(line)
                && self.slice(line, end).starts_with(margin)
            {
                from += margin.chars().count();
            }
        }
        code.push_str(self.slice(from, end));
        code.replace('\r', "")
    }

    /// Whether `position` is in a string, after its first char.
    fn in_string(&self, position: usize) -> bool {
        let i = self.index(position);
        i > 0 && {
            let token = &self.tokens[i - 1];
            token.end > position && matches!(token.token, Token::Literal(LitKind::String, _))
        }
    }

    /// The text of a group of comments, as go/ast's `CommentGroup.Text`
    /// gives it: without their markers, the space after each `//`, or
    /// directives, like `//go:generate`, or blank lines at the start, with
    /// no more than one blank line in a row, and a newline at the end.
    pub fn text(&self, (start, end): Group) -> String {
        let mut lines = Vec::new();
        for token in &self.tokens[start..end] {
            let Token::Comment(comment) = &token.token else {
                continue;
            };
            let comment = comment.replace('\r', "");
            let text = if let Some(text) = comment.strip_prefix("//") {
                match text.strip_prefix(' ') {
                    Some(text) => text,
                    None if is_directive(text) => continue,
                    None => text,
                }
            } else {
                let text = comment.strip_prefix("/*").unwrap_or(&comment);
                text.strip_suffix("*/").unwrap_or(text)
            };
            for line in text.split('\n') {
                lines.push(line.trim_end_matches([' ', '\t', '\n', '\r']).to_owned());
            }
        }
        let mut kept: Vec<String> = Vec::new();
        for line in lines {
            if !line.is_empty() || kept.last().is_some_and(|last| !last.is_empty()) {
                kept.push(line);
            }
        }
        if kept.last().is_some_and(|last| !last.is_empty()) {
            kept.push(String::new());
        }
        kept.join("\n")
    }
}

/// Whether Go's scanner inserts a semicolon after `token`, at the end of
/// its line.
fn ends_statement(token: &Token) -> bool {
    match token {
        Token::Literal(..) => true,
        Token::Keyword(keyword) => matches!(
            keyword,
            Keyword::Break | Keyword::Continue | Keyword::FallThrough | Keyword::Return
        ),
        Token::Operator(operator) => matches!(
            operator,
            Operator::Inc
                | Operator::Dec
                | Operator::ParenRight
                | Operator::BarackRight
                | Operator::BraceRight
        ),
        Token::Comment(_) => false,
    }
}

/// Whether a `//` comment, without its `//`, is a directive, which isn't
/// part of a doc comment: `//line`, `//extern`, `//export`, or a word in
/// lowercase letters and digits, a colon, and one of them, like `//go:embed`.
fn is_directive(comment: &str) -> bool {
    if ["line ", "extern ", "export "]
        .iter()
        .any(|prefix| comment.starts_with(prefix))
    {
        return true;
    }
    let bytes = comment.as_bytes();
    let Some(colon) = comment.find(':') else {
        return false;
    };
    colon > 0
        && colon + 1 < bytes.len()
        && (0..=colon + 1)
            .filter(|&i| i != colon)
            .all(|i| bytes[i].is_ascii_lowercase() || bytes[i].is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(text: &str) -> Source<'_> {
        Source::new(text, gosyn::tokenize_source(text).unwrap())
    }

    /// The index of the first token that's `text`.
    fn token(source: &Source, text: &str) -> usize {
        (0..source.tokens.len())
            .find(|&i| source.slice(source.tokens[i].start, source.tokens[i].end) == text)
            .unwrap_or_else(|| panic!("no token {text:?}"))
    }

    fn doc(source: &Source, first: &str) -> Option<String> {
        source
            .doc(token(source, first))
            .map(|group| source.text(group))
    }

    fn line_comment(source: &Source, last: &str) -> Option<String> {
        source
            .line_comment(token(source, last))
            .map(|group| source.text(group))
    }

    #[test]
    fn a_doc_comment_is_on_the_lines_just_before() {
        let source = source(
            "// Package p is p.\npackage p\n\n// A is a.\n//\n// It's an int.\nvar A int\n\n// Not B's.\n\nvar B int\n",
        );
        assert_eq!(doc(&source, "package"), Some("Package p is p.\n".into()));
        assert_eq!(
            doc(&source, "var"),
            Some("A is a.\n\nIt's an int.\n".into())
        );
        let b = token(&source, "B") - 1;
        assert_eq!(source.doc(b), None);
    }

    #[test]
    fn a_line_comment_isnt_the_next_doc_comment() {
        let source = source("package p\n\nvar a = 1 // a\n// B is b.\nvar B = 2\n");
        assert_eq!(line_comment(&source, "1"), Some("a\n".into()));
        let b = token(&source, "B") - 1;
        assert_eq!(
            source.doc(b).map(|group| source.text(group)),
            Some("B is b.\n".into())
        );
    }

    #[test]
    fn a_comment_after_a_string_across_lines_is_no_line_comment() {
        let source = source("package p\n\nvar a = `x\ny` // y\n// B is b.\nvar B = 2\n");
        assert_eq!(line_comment(&source, "`x\ny`"), None);
        let b = token(&source, "B") - 1;
        assert_eq!(
            source.doc(b).map(|group| source.text(group)),
            Some("B is b.\n".into())
        );
    }

    #[test]
    fn a_comment_with_a_newline_ends_the_line() {
        let source = source("package p\n\nvar a = 1 /* one\ntwo */\nvar B = 2\n");
        assert_eq!(line_comment(&source, "1"), Some(" one\ntwo\n".into()));
        let b = token(&source, "B") - 1;
        assert_eq!(source.doc(b), None);
    }

    #[test]
    fn a_line_comment_can_follow_a_semicolon() {
        let source = source("package p\n\nvar a = 1; // a\nvar b = 2 /* b */; // not b's\n");
        assert_eq!(line_comment(&source, "1"), Some("a\n".into()));
        assert_eq!(line_comment(&source, "2"), Some("not b's\n".into()));
    }

    #[test]
    fn nothing_before_a_closing_brace_is_a_line_comment() {
        let source = source(
            "package p\n\ntype T struct { A int /* a */ }\ntype U struct {\n\tB int // b\n}\n",
        );
        assert_eq!(line_comment(&source, "int"), None);
        let b = token(&source, "B");
        assert_eq!(
            source.line_comment(b + 1).map(|group| source.text(group)),
            Some("b\n".into())
        );
    }

    #[test]
    fn a_file_is_its_elements() {
        let source = source(
            "package p\n\nimport (\n\t\"fmt\"\n)\n\n// F is f.\nfunc F() {\n\tfmt.Println(1)\n\treturn\n}\n\nvar a, b = 1, 2; const C = 3\n",
        );
        let elements: Vec<String> = source
            .elements()
            .into_iter()
            .map(|(first, last)| source.code(first, last))
            .collect();
        assert_eq!(
            elements,
            [
                "package p",
                "import (\n\t\"fmt\"\n)",
                "func F() {\n\tfmt.Println(1)\n\treturn\n}",
                "var a, b = 1, 2",
                "const C = 3",
            ]
        );
        let open = token(&source, "(");
        let close = token(&source, ")");
        assert_eq!(source.elements_in(open, close).len(), 1);
    }

    #[test]
    fn code_is_taken_from_its_indentation_but_in_strings() {
        let source = source("package p\n\nvar (\n\tA = f(\n\t\t`x\n\ty`,\n\t)\n)\n");
        let a = token(&source, "A");
        let last = source.tokens.len() - 2;
        assert_eq!(source.code(a, last), "A = f(\n\t`x\n\ty`,\n)");
        assert_eq!(source.indent(a), Some("\t"));
        assert_eq!(source.indent(a + 1), None);
    }

    #[test]
    fn offsets_are_in_chars() {
        let source = source("package p\n\n// É is é.\nvar É = \"é\" // é\n");
        assert_eq!(doc(&source, "var"), Some("É is é.\n".into()));
        assert_eq!(line_comment(&source, "\"é\""), Some("é\n".into()));
        let first = token(&source, "var");
        assert_eq!(source.code(first, first + 3), "var É = \"é\"");
    }

    #[test]
    fn a_comments_text_is_as_go_gives_it() {
        let source = source(
            "package p\n\n//go:generate x\n//  A is\n//\n//\n// a.   \r\n/*\n\tB\n*/\nvar A int\n",
        );
        assert_eq!(doc(&source, "var"), Some(" A is\n\na.\n\n\tB\n".into()));
        assert!(is_directive("go:embed x"));
        assert!(is_directive("line x.go:1"));
        assert!(!is_directive("Note: x"));
        assert!(!is_directive("x:"));
    }
}
