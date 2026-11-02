//! Inline markup, as docutils' `Inliner` reads it: emphasis, literals,
//! interpreted text and its roles, references, inline targets, footnote and
//! substitution references, and, in the text between them, standalone URIs.
//!
//! It reads text once, from the start: what it needs to know of what's
//! ahead, like where each kind of end-string is, it works out beforehand,
//! so text with many start-strings and no end-strings doesn't take time
//! that grows with its square.

use crate::lines::is_space;
use crate::state::{Dest, Role, State};
use crate::text::{ESCAPE, collapse, escape, is_prefix, match_chars, normalize, restore, unescape};
use crate::tree::{Ref, Span};
use crate::uri::{self, suffix};

const NONE: usize = usize::MAX;

/// The spans of `text`, whose first line is `line`.
pub(crate) fn parse(state: &mut State, text: &str, line: usize) -> Vec<Span> {
    let chars: Vec<char> = escape(text).chars().collect();
    let mut inliner = Inliner::new(state, chars, line);
    inliner.run();
    inliner.spans
}

/// Whether `c` may be in a simple name, as docutils' `(?!_)\w` has it.
pub(crate) fn word(c: char) -> bool {
    c.is_alphanumeric()
}

/// Whether `c` may join two parts of a simple name.
pub(crate) fn joins(c: char) -> bool {
    matches!(c, '-' | '.' | '_' | '+' | ':')
}

/// The kinds of inline markup whose end-string is found as docutils'
/// `inline_obj` finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Emphasis,
    Strong,
    Literal,
    Target,
    Substitution,
}

impl Kind {
    /// The name docutils gives it in a warning.
    fn name(self) -> &'static str {
        match self {
            Kind::Emphasis => "emphasis",
            Kind::Strong => "strong",
            Kind::Literal => "literal",
            Kind::Target => "target",
            Kind::Substitution => "substitution_reference",
        }
    }
}

/// The length of the end-string of `kind` at `at` in `chars`, if one is
/// there: with `vacuous`, as if the text started at `at`, so nothing comes
/// before it.
fn end_at(chars: &[char], kind: Kind, at: usize, vacuous: bool) -> Option<usize> {
    let get = |k: usize| chars.get(k).copied();
    let after = |escape: bool| {
        vacuous || at == 0 || {
            let c = chars[at - 1];
            !(is_space(c) || (escape && c == ESCAPE))
        }
    };
    match kind {
        Kind::Emphasis => {
            (get(at) == Some('*') && after(true) && suffix(chars, at + 1)).then_some(1)
        }
        Kind::Strong => (get(at) == Some('*')
            && get(at + 1) == Some('*')
            && after(true)
            && suffix(chars, at + 2))
        .then_some(2),
        Kind::Literal => (get(at) == Some('`')
            && get(at + 1) == Some('`')
            && after(false)
            && suffix(chars, at + 2))
        .then_some(2),
        Kind::Target => (get(at) == Some('`') && after(true) && suffix(chars, at + 1)).then_some(1),
        Kind::Substitution => {
            if get(at) != Some('|') || !after(true) {
                None
            } else if get(at + 1) == Some('_') && get(at + 2) == Some('_') && suffix(chars, at + 3)
            {
                Some(3)
            } else if get(at + 1) == Some('_') && suffix(chars, at + 2) {
                Some(2)
            } else {
                suffix(chars, at + 1).then_some(1)
            }
        }
    }
}

/// The underscores of a reference's end at `at`, if they end markup there:
/// `__`, then `_`, as docutils tries them.
fn refend(chars: &[char], at: usize) -> Option<usize> {
    if chars.get(at) != Some(&'_') {
        None
    } else if chars.get(at + 1) == Some(&'_') && suffix(chars, at + 2) {
        Some(2)
    } else {
        suffix(chars, at + 1).then_some(1)
    }
}

/// The end of interpreted text or a phrase reference: its backquote, the
/// role after it, if it has one, its underscores, and where it ends.
#[derive(Debug, Clone, Copy)]
struct End {
    at: usize,
    role: Option<(usize, usize)>,
    refend: usize,
    end: usize,
}

/// What reading markup at a place found: where the text before it ends,
/// the spans it's read as, if it's markup, and where the text after it
/// starts.
struct Found {
    before: usize,
    spans: Option<Vec<Span>>,
    next: usize,
}

/// Reads the inline markup in some text.
struct Inliner<'a> {
    state: &'a mut State,
    /// The text, escaped.
    chars: Vec<char>,
    /// The line it starts on, and where each of its newlines is.
    line: usize,
    newlines: Vec<usize>,
    /// For each place in a simple name, the furthest end of the name from
    /// it that a reference, a role before a backquote, a citation's label,
    /// or a role after a backquote can end at.
    reference: Vec<usize>,
    role: Vec<usize>,
    label: Vec<usize>,
    suffix_role: Vec<usize>,
    /// Each end-string of each kind, and its length, found once one's
    /// needed, and the first that may still be read.
    ends: [Option<Vec<(usize, usize)>>; 5],
    cursors: [usize; 5],
    /// The ends of interpreted text, and the first that may still be read.
    interpreted: Option<Vec<End>>,
    interpreted_cursor: usize,
    spans: Vec<Span>,
}

impl<'a> Inliner<'a> {
    fn new(state: &'a mut State, chars: Vec<char>, line: usize) -> Self {
        let newlines = (0..chars.len()).filter(|&at| chars[at] == '\n').collect();
        let n = chars.len();
        let mut run_end = vec![NONE; n + 1];
        for at in (0..n).rev() {
            if word(chars[at]) {
                run_end[at] = if at + 1 < n && word(chars[at + 1]) {
                    run_end[at + 1]
                } else {
                    at + 1
                };
            }
        }
        let best = |fits: &dyn Fn(usize) -> bool| {
            let mut best = vec![NONE; n + 1];
            for at in (0..n).rev() {
                if !word(chars[at]) {
                    continue;
                }
                let end = run_end[at];
                let next = if end + 1 < n && joins(chars[end]) && word(chars[end + 1]) {
                    best[end + 1]
                } else {
                    NONE
                };
                best[at] = if next != NONE {
                    next
                } else if fits(end) {
                    end
                } else {
                    NONE
                };
            }
            best
        };
        let get = |k: usize| chars.get(k).copied();
        let reference = best(&|end| refend(&chars, end).is_some());
        let role = best(&|end| {
            get(end) == Some(':')
                && get(end + 1) == Some('`')
                && !get(end + 2).is_some_and(|c| c == '`' || is_space(c))
        });
        let label = best(&|end| {
            get(end) == Some(']') && get(end + 1) == Some('_') && suffix(&chars, end + 2)
        });
        let suffix_role = best(&|end| {
            get(end) == Some(':') && (refend(&chars, end + 1).is_some() || suffix(&chars, end + 1))
        });
        Inliner {
            state,
            chars,
            line,
            newlines,
            reference,
            role,
            label,
            suffix_role,
            ends: Default::default(),
            cursors: [0; 5],
            interpreted: None,
            interpreted_cursor: 0,
            spans: Vec::new(),
        }
    }

    /// The line of the place `at`.
    fn line_at(&self, at: usize) -> usize {
        self.line + self.newlines.partition_point(|&newline| newline < at)
    }

    /// The text from `start` to `end`, escaped.
    fn slice(&self, start: usize, end: usize) -> String {
        self.chars[start..end].iter().collect()
    }

    /// Read the text, as docutils' `Inliner.parse` does: find the next
    /// place markup may start at, after whitespace or punctuation, or where
    /// the last markup ended, and read it there.
    fn run(&mut self) {
        let n = self.chars.len();
        // Where the text not yet read as markup starts, and where the text
        // after the last markup starts, which markup may start at.
        let (mut chunk, mut rest, mut at) = (0, 0, 0);
        while at < n {
            if at != rest && !is_prefix(self.chars[at - 1]) {
                at += 1;
                continue;
            }
            let Some(found) = self.markup(at, rest) else {
                at += 1;
                continue;
            };
            if let Some(spans) = found.spans {
                self.implicit(chunk, found.before);
                self.spans.extend(spans);
                chunk = found.next;
            }
            (rest, at) = (found.next, found.next);
        }
        self.implicit(chunk, n);
    }

    /// The markup that starts at `at`, if any, as docutils tries each kind.
    fn markup(&mut self, at: usize, rest: usize) -> Option<Found> {
        let get = |k: usize| self.chars.get(k).copied();
        let open = |k: usize| !get(k).is_some_and(is_space);
        let c = self.chars[at];
        let next = get(at + 1);
        Some(match c {
            '*' if next == Some('*') => {
                if !open(at + 2) {
                    return None;
                }
                self.inline_obj(at, 2, rest, Kind::Strong)
            }
            '*' if open(at + 1) => self.inline_obj(at, 1, rest, Kind::Emphasis),
            '`' if next == Some('`') => {
                if !open(at + 2) {
                    return None;
                }
                self.inline_obj(at, 2, rest, Kind::Literal)
            }
            '`' if open(at + 1) => self.interpreted(at, at, rest),
            '_' if next == Some('`') && open(at + 2) => self.inline_obj(at, 2, rest, Kind::Target),
            '|' if next != Some('|') && open(at + 1) => {
                self.inline_obj(at, 1, rest, Kind::Substitution)
            }
            '[' => return self.footnote(at),
            ':' if next.is_some_and(word) && self.role[at + 1] != NONE => {
                let backquote = self.role[at + 1] + 1;
                self.interpreted(at, backquote, rest)
            }
            c if word(c) && self.reference[at] != NONE => self.reference(at),
            _ => return None,
        })
    }

    /// Whether the start-string from `start` to `end` is quoted: between
    /// a pair of characters, like `(` and `)`, or at the end of the text.
    fn quoted(&self, start: usize, end: usize, rest: usize) -> bool {
        if start == rest {
            return false;
        }
        match self.chars.get(end) {
            Some(&after) => match_chars(self.chars[start - 1], after),
            None => true,
        }
    }

    /// The first end-string of `kind` after its start-string, which ends at
    /// `start`, and its length, unless the first is right there.
    fn find_end(&mut self, kind: Kind, start: usize) -> Option<(usize, usize)> {
        if end_at(&self.chars, kind, start, true).is_some() {
            return None;
        }
        let index = kind as usize;
        let chars = &self.chars;
        let ends = self.ends[index].get_or_insert_with(|| {
            (0..chars.len())
                .filter_map(|at| end_at(chars, kind, at, false).map(|len| (at, len)))
                .collect()
        });
        let cursor = &mut self.cursors[index];
        while ends.get(*cursor).is_some_and(|&(at, _)| at <= start) {
            *cursor += 1;
        }
        ends.get(*cursor).copied()
    }

    /// Read the markup of `kind` whose start-string, `len` long, is at
    /// `start`, as docutils' `inline_obj` does.
    fn inline_obj(&mut self, start: usize, len: usize, rest: usize, kind: Kind) -> Found {
        let open = start + len;
        if self.quoted(start, open, rest) {
            return Found {
                before: open,
                spans: None,
                next: open,
            };
        }
        let line = self.line_at(start);
        let Some((close, end_len)) = self.find_end(kind, open) else {
            self.state.warn(
                line,
                format!("inline {} start-string without end-string", kind.name()),
            );
            let text = restore(&self.slice(start, open));
            return Found {
                before: start,
                spans: Some(vec![Span::Text(text)]),
                next: open,
            };
        };
        let inner = self.slice(open, close);
        let end = close + end_len;
        let spans = match kind {
            Kind::Emphasis => vec![Span::Emphasis(vec![Span::Text(unescape(&inner))])],
            Kind::Strong => vec![Span::Strong(vec![Span::Text(unescape(&inner))])],
            Kind::Literal => vec![Span::Code(restore(&inner))],
            Kind::Target => {
                let text = unescape(&inner);
                let name = normalize(&text);
                let names: Vec<String> = Some(name)
                    .filter(|name| !name.is_empty())
                    .into_iter()
                    .collect();
                let index = self.state.target(Dest::Here, &names, true, "target", line);
                vec![
                    Span::Anchor(self.state.targets[index].id.clone()),
                    Span::Text(text),
                ]
            }
            Kind::Substitution => {
                let text = unescape(&inner);
                let substitution = Span::Substitution {
                    name: collapse(&text),
                    raw: format!("|{}|", restore(&inner)),
                    line,
                };
                let raw = restore(&self.slice(start, end));
                match end_len {
                    1 => vec![substitution],
                    2 => vec![Span::Link {
                        content: vec![substitution],
                        to: Ref::Name(normalize(&text)),
                        raw,
                        line,
                    }],
                    _ => vec![Span::Link {
                        content: vec![substitution],
                        to: self.anonymous(),
                        raw,
                        line,
                    }],
                }
            }
        };
        Found {
            before: start,
            spans: Some(spans),
            next: end,
        }
    }

    /// The next anonymous reference.
    fn anonymous(&mut self) -> Ref {
        self.state.anonymous_refs += 1;
        Ref::Anonymous(self.state.anonymous_refs - 1)
    }

    /// The end of interpreted text or a phrase reference at `at`, if one is
    /// there, as docutils' pattern finds it.
    fn interpreted_end(&self, at: usize) -> Option<End> {
        let chars = &self.chars;
        if chars[at] != '`' {
            return None;
        }
        if at > 0 {
            let c = chars[at - 1];
            if (is_space(c) || c == ESCAPE) && !(at >= 2 && chars[at - 2] == ESCAPE) {
                return None;
            }
        }
        if chars.get(at + 1) == Some(&':')
            && chars.get(at + 2).is_some_and(|&c| word(c))
            && self.suffix_role[at + 2] != NONE
        {
            let close = self.suffix_role[at + 2];
            let refend = refend(chars, close + 1).unwrap_or(0);
            return Some(End {
                at,
                role: Some((at + 2, close)),
                refend,
                end: close + 1 + refend,
            });
        }
        let refend = match refend(chars, at + 1) {
            Some(refend) => refend,
            None if suffix(chars, at + 1) => 0,
            None => return None,
        };
        Some(End {
            at,
            role: None,
            refend,
            end: at + 1 + refend,
        })
    }

    /// The first end of interpreted text after its backquote, `open`.
    fn find_interpreted_end(&mut self, open: usize) -> Option<End> {
        if self.interpreted.is_none() {
            let ends = (0..self.chars.len())
                .filter_map(|at| self.interpreted_end(at))
                .collect();
            self.interpreted = Some(ends);
        }
        let ends = self.interpreted.as_ref()?;
        while ends
            .get(self.interpreted_cursor)
            .is_some_and(|end| end.at < open)
        {
            self.interpreted_cursor += 1;
        }
        ends.get(self.interpreted_cursor).copied()
    }

    /// Read interpreted text or a phrase reference whose backquote is at
    /// `backquote`, after its role, from `start`, if it has one, as
    /// docutils' `interpreted_or_phrase_ref` does.
    fn interpreted(&mut self, start: usize, backquote: usize, rest: usize) -> Found {
        let open = backquote + 1;
        let prefix = (start < backquote).then(|| self.slice(start + 1, backquote - 1));
        if prefix.is_none() && self.quoted(backquote, open, rest) {
            return Found {
                before: open,
                spans: None,
                next: open,
            };
        }
        let line = self.line_at(start);
        let Some(end) = self.find_interpreted_end(open) else {
            self.state.warn(
                line,
                "inline interpreted text or phrase reference start-string without end-string",
            );
            return Found {
                before: backquote,
                spans: Some(vec![Span::Text("`".to_string())]),
                next: open,
            };
        };
        let problem = |inliner: &mut Self, message: &str| {
            inliner.state.warn(line, message);
            Found {
                before: start,
                spans: Some(vec![Span::Text(restore(&inliner.slice(start, end.end)))]),
                next: end.end,
            }
        };
        let suffix = end.role.map(|(from, to)| self.slice(from, to));
        if prefix.is_some() && suffix.is_some() {
            return problem(
                self,
                "multiple roles in interpreted text (both prefix and suffix present; only one \
                 allowed)",
            );
        }
        let escaped = self.slice(open, end.at);
        if end.refend > 0 {
            if prefix.is_some() || suffix.is_some() {
                let position = if prefix.is_some() { "prefix" } else { "suffix" };
                let message =
                    format!("mismatch: both interpreted text role {position} and reference suffix");
                return problem(self, &message);
            }
            let raw = restore(&self.slice(backquote, end.end));
            let spans = self.phrase_ref(&escaped, raw, end.refend == 2, line);
            return Found {
                before: backquote,
                spans: Some(spans),
                next: end.end,
            };
        }
        let raw = restore(&self.slice(start, end.end));
        let spans = self.interpret(prefix.or(suffix), &escaped, raw, line);
        Found {
            before: start,
            spans: Some(spans),
            next: end.end,
        }
    }

    /// A phrase reference, `` `text`_ ``, or anonymous, `` `text`__ ``,
    /// with an embedded URI or alias, `` `text <url>`_ ``, or not, as
    /// docutils' `phrase_ref` reads it.
    fn phrase_ref(
        &mut self,
        escaped: &str,
        raw: String,
        anonymous: bool,
        line: usize,
    ) -> Vec<Span> {
        let chars: Vec<char> = escaped.chars().collect();
        let Some((text_end, alias_start, alias_end)) = embedded_link(&chars) else {
            let text = unescape(escaped);
            let to = if anonymous {
                self.anonymous()
            } else {
                Ref::Name(normalize(&text))
            };
            return vec![Span::Link {
                content: vec![Span::Text(text)],
                to,
                raw,
                line,
            }];
        };
        let alias_chars = &chars[alias_start..alias_end];
        let alias_text: String = alias_chars.iter().collect();
        let is_name = alias_text.ends_with('_')
            && !restore(&alias_text).ends_with("\\_")
            && !uri::starts_with_uri(alias_chars);
        let alias = if is_name {
            normalize(&unescape(&alias_text[..alias_text.len() - 1]))
        } else {
            uri_alias(&alias_text)
        };
        let text: String = chars[..text_end].iter().collect();
        let text = if text.is_empty() {
            alias.clone()
        } else {
            unescape(&text)
        };
        let to = if is_name {
            Ref::Name(alias.clone())
        } else {
            Ref::Url(alias.clone())
        };
        if !anonymous {
            let name = normalize(&text);
            if !name.is_empty() {
                let dest = if is_name {
                    Dest::Name(alias)
                } else {
                    Dest::Url(alias)
                };
                self.state.target(dest, &[name], false, "target", line);
            }
        }
        vec![Span::Link {
            content: vec![Span::Text(text)],
            to,
            raw,
            line,
        }]
    }

    /// Interpreted text, `escaped`, in `role`, else the default role: what
    /// the role makes of it.
    fn interpret(
        &mut self,
        role: Option<String>,
        escaped: &str,
        raw: String,
        line: usize,
    ) -> Vec<Span> {
        let (name, role) = match role {
            None => {
                let (name, role) = &self.state.default_role;
                (name.clone(), Some(role.clone()))
            }
            Some(name) => {
                let lower = name.to_lowercase();
                let role = self
                    .state
                    .roles
                    .get(&lower)
                    .cloned()
                    .or_else(|| Role::standard(&lower));
                (name, role)
            }
        };
        let text = || unescape(escaped);
        let Some(role) = role else {
            let message =
                format!("kept the text of `{raw}`, as `{name}` isn't a role the plugin reads");
            self.state.warn(line, message);
            let chars: Vec<char> = escaped.chars().collect();
            let title = match embedded_link(&chars) {
                Some((end, ..)) if end > 0 => chars[..end].iter().collect(),
                _ => escaped.to_string(),
            };
            return vec![Span::Text(unescape(&title))];
        };
        match role {
            Role::Emphasis | Role::Title => vec![Span::Emphasis(vec![Span::Text(text())])],
            Role::Strong => vec![Span::Strong(vec![Span::Text(text())])],
            Role::Literal => vec![Span::Code(text())],
            Role::Code => vec![Span::Code(restore(escaped))],
            Role::Math => vec![Span::Math(restore(escaped))],
            Role::Sub => vec![Span::Sub(vec![Span::Text(text())])],
            Role::Sup => vec![Span::Sup(vec![Span::Text(text())])],
            Role::Plain => vec![Span::Text(text())],
            Role::Pep => {
                let text = text();
                match text.trim().parse::<u32>() {
                    Ok(number) if number <= 9999 => vec![Span::Link {
                        content: vec![Span::Text(format!("PEP {text}"))],
                        to: Ref::Url(format!("https://peps.python.org/pep-{number:04}")),
                        raw,
                        line,
                    }],
                    _ => {
                        self.state.warn(
                            line,
                            format!(
                                "PEP number must be a number from 0 to 9999; \"{text}\" is invalid"
                            ),
                        );
                        vec![Span::Text(raw)]
                    }
                }
            }
            Role::Rfc => {
                let text = text();
                let (number, section) = match text.split_once('#') {
                    Some((number, section)) => (number, Some(section)),
                    None => (text.as_str(), None),
                };
                match number.trim().parse::<u64>() {
                    Ok(number) if number >= 1 => {
                        let mut url = format!("https://tools.ietf.org/html/rfc{number}.html");
                        if let Some(section) = section {
                            url.push('#');
                            url.push_str(section);
                        }
                        vec![Span::Link {
                            content: vec![Span::Text(format!("RFC {number}"))],
                            to: Ref::Url(url),
                            raw,
                            line,
                        }]
                    }
                    _ => {
                        self.state.warn(
                            line,
                            format!(
                                "RFC number must be a number greater than or equal to 1; \"{text}\" \
                                 is invalid"
                            ),
                        );
                        vec![Span::Text(raw)]
                    }
                }
            }
            Role::Raw(Some(true)) => vec![Span::Html(restore(escaped), line)],
            Role::Raw(Some(false)) => Vec::new(),
            Role::Raw(None) => {
                let message = format!(
                    "kept `{raw}` as text, as `{name}` can't be used directly, only through a \
                     role based on it"
                );
                self.state.warn(line, message);
                vec![Span::Text(raw)]
            }
            Role::Unimplemented => {
                let message = format!(
                    "kept the text of `{raw}`, as docutils doesn't implement the `{name}` role"
                );
                self.state.warn(line, message);
                vec![Span::Text(text())]
            }
        }
    }

    /// A footnote or citation reference at `at`, `[label]_`, if one is
    /// there.
    fn footnote(&mut self, at: usize) -> Option<Found> {
        let chars = &self.chars;
        let get = |k: usize| chars.get(k).copied();
        let closes = |end: usize| {
            get(end) == Some(']') && get(end + 1) == Some('_') && suffix(chars, end + 2)
        };
        let digits = chars[at + 1..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        let named = |from: usize| {
            (get(from).is_some_and(word) && self.label[from] != NONE).then(|| self.label[from])
        };
        let manual = digits > 0 && closes(at + 1 + digits);
        let close = if manual {
            at + 1 + digits
        } else if get(at + 1) == Some('#') {
            match named(at + 2) {
                Some(close) => close,
                None if closes(at + 2) => at + 2,
                None => return None,
            }
        } else if get(at + 1) == Some('*') && closes(at + 2) {
            at + 2
        } else {
            named(at + 1)?
        };
        let label = self.slice(at + 1, close);
        let key = if manual {
            label.clone()
        } else if label == "#" {
            self.state.auto_refs += 1;
            format!("# {}", self.state.auto_refs)
        } else if label == "*" {
            self.state.symbol_refs += 1;
            format!("* {}", self.state.symbol_refs)
        } else if label.starts_with('#') {
            normalize(&label)
        } else {
            format!("cite:{}", normalize(&label))
        };
        let line = self.line_at(at);
        Some(Found {
            before: at,
            spans: Some(vec![Span::Footnote {
                key,
                raw: format!("[{label}]_"),
                line,
            }]),
            next: close + 2,
        })
    }

    /// A reference to a simple name at `at`, `name_`, or anonymous,
    /// `name__`.
    fn reference(&mut self, at: usize) -> Found {
        let close = self.reference[at];
        let len = refend(&self.chars, close).unwrap_or(1);
        let name = self.slice(at, close);
        let line = self.line_at(at);
        let to = if len == 2 {
            self.anonymous()
        } else {
            Ref::Name(normalize(&name))
        };
        Found {
            before: at,
            spans: Some(vec![Span::Link {
                content: vec![Span::Text(name.clone())],
                to,
                raw: format!("{name}{}", "_".repeat(len)),
                line,
            }]),
            next: close + len,
        }
    }

    /// Read the text from `start` to `end`, which isn't markup, for the
    /// standalone URIs and email addresses in it.
    fn implicit(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let chunk = &self.chars[start..end];
        let mut last = 0;
        for found in uri::find(chunk) {
            if found.start > last {
                let text: String = chunk[last..found.start].iter().collect();
                self.spans.push(Span::Text(unescape(&text)));
            }
            let text: String = chunk[found.start..found.end].iter().collect();
            let shown = unescape(&text);
            let url = if found.email {
                format!("mailto:{shown}")
            } else {
                shown.clone()
            };
            let line = self.line
                + self
                    .newlines
                    .partition_point(|&at| at < start + found.start);
            self.spans.push(Span::Link {
                content: vec![Span::Text(shown)],
                to: Ref::Url(url),
                raw: restore(&text),
                line,
            });
            last = found.end;
        }
        if last < chunk.len() {
            let text: String = chunk[last..].iter().collect();
            self.spans.push(Span::Text(unescape(&text)));
        }
    }
}

/// The embedded URI or alias at the end of a phrase reference's text,
/// escaped, `text <url>`, as docutils' `embedded_link` pattern finds it:
/// where the text before it ends, and where the URI or alias starts and
/// ends.
pub(crate) fn embedded_link(chars: &[char]) -> Option<(usize, usize, usize)> {
    let n = chars.len();
    if n < 3 || chars[n - 1] != '>' || is_space(chars[n - 2]) || chars[n - 2] == ESCAPE {
        return None;
    }
    let unescaped = |at: usize| at == 0 || chars[at - 1] != ESCAPE;
    let open = (0..n - 1)
        .rev()
        .find(|&at| matches!(chars[at], '<' | '>') && unescaped(at))?;
    if chars[open] != '<' || open + 1 >= n - 1 || is_space(chars[open + 1]) {
        return None;
    }
    let mut text_end = open;
    while text_end > 0 && matches!(chars[text_end - 1], ' ' | '\n') {
        text_end -= 1;
    }
    if text_end == open && open != 0 {
        return None;
    }
    Some((text_end, open + 1, n - 1))
}

/// An embedded URI, escaped, as docutils reads it: without whitespace,
/// but for escaped whitespace, which is a space, and with `mailto:` before
/// an email address.
fn uri_alias(escaped: &str) -> String {
    let mut alias = crate::explicit::uri(escaped);
    if uri::is_email(&alias) {
        alias.insert_str(0, "mailto:");
    }
    if let Some(stripped) = alias.strip_suffix("\\_") {
        alias = format!("{stripped}_");
    }
    alias
}
