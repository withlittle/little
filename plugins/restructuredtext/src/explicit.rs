//! Explicit markup, as docutils' `Body` and `Explicit` states read it: each
//! block that starts with `..`, a footnote, a citation, a hyperlink target,
//! a substitution definition, a directive, which [`crate::directives`]
//! reads, or a comment; and anonymous targets that start with `__`.

use crate::body::{Body, Transition, nested, transition};
use crate::inline::{joins, word};
use crate::lines::{Line, indent, indented, is_space, skip, trim_end};
use crate::state::{Dest, Footnote, Kind};
use crate::text::{ESCAPE, collapse, escape, normalize, restore, unescape};
use crate::tree::Node;
use crate::uri;

/// The length, in bytes, of the simple name `text` starts with, as
/// docutils' `simplename` pattern matches it: words, each joined to the
/// next by one hyphen, period, underscore, plus or colon.
pub(crate) fn simplename(text: &str) -> usize {
    let mut end = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if word(c) {
            end = at + c.len_utf8();
        } else if end == at
            && end > 0
            && joins(c)
            && chars.peek().is_some_and(|&(_, next)| word(next))
        {
            continue;
        } else {
            break;
        }
    }
    end
}

/// A URI, escaped, as docutils' `uri` option converter reads it: without
/// whitespace, but for escaped whitespace, which is a space.
pub(crate) fn uri(escaped: &str) -> String {
    let mut parts = vec![String::new()];
    let mut chars = escaped.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ESCAPE && matches!(chars.peek(), Some(' ' | '\n')) {
            chars.next();
            parts.push(String::new());
        } else if !is_space(c) {
            parts.last_mut().expect("parts start with one").push(c);
        }
    }
    parts
        .iter()
        .map(|part| unescape(part))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The name `reference`, escaped and collapsed, refers to, if it's a
/// reference, `name_` or `` `a phrase`_ ``, as docutils' `is_reference`
/// reads it.
fn reference_name(reference: &str) -> Option<&str> {
    let body = reference.strip_suffix('_')?;
    if !body.is_empty() && simplename(body) == body.len() {
        return Some(body);
    }
    let phrase = body.strip_prefix('`')?.strip_suffix('`')?;
    let first = phrase.chars().next()?;
    let last = phrase.chars().next_back()?;
    (first != ' ' && !is_space(last) && last != ESCAPE).then_some(phrase)
}

/// Where a hyperlink target leads, from `block`, the lines of what follows
/// its name, escaped, as docutils' `parse_target` reads them: to the
/// target a reference names, if they end with one; else to a URI, if they
/// have any text; else to its own place.
pub(crate) fn parse_target(block: &[String]) -> Dest {
    let last = block.last().map(|line| line.trim_end_matches(is_space));
    if last.is_some_and(|line| line.ends_with('_')) {
        let reference = collapse(&block.join(" "));
        if let Some(name) = reference_name(&reference) {
            return Dest::Name(normalize(&unescape(name)));
        }
    }
    let uri = uri(&block.join(" "));
    if uri.is_empty() {
        Dest::Here
    } else {
        Dest::Url(uri)
    }
}

/// Where the name of a hyperlink target is in `s`, the lines after its
/// `_`, escaped and run together, as docutils' `target` pattern finds it,
/// the shortest it can be: from where to where, or none, for an anonymous
/// target, `__:`; and where the colon after it is.
fn target_name(s: &[char]) -> Option<(Option<(usize, usize)>, usize)> {
    let n = s.len();
    let after = |k: usize| k >= n || s[k] == ' ';
    let colon = |p: usize| {
        if p + 1 < n && s[p] == ' ' && s[p + 1] == ':' && after(p + 2) {
            Some(p + 1)
        } else if p < n && s[p] == ':' && after(p + 1) {
            Some(p)
        } else {
            None
        }
    };
    if s.first() == Some(&'_') {
        return colon(1).map(|at| (None, at));
    }
    let quoted = s.first() == Some(&'`');
    let start = usize::from(quoted);
    if s.get(start).is_none_or(|&c| c == ' ' || c == '`') {
        return None;
    }
    for end in start + 1..=n {
        if is_space(s[end - 1]) || s[end - 1] == ESCAPE {
            continue;
        }
        let p = if quoted {
            if s.get(end) != Some(&'`') {
                continue;
            }
            end + 1
        } else {
            // A name may end with a colon only if it's escaped.
            if s[end - 1] == ':' && !(end >= 2 && s[end - 2] == ESCAPE) {
                continue;
            }
            end
        };
        if let Some(at) = colon(p) {
            return Some((Some((start, end)), at));
        }
    }
    None
}

/// Where the name of a substitution definition ends in `s`, the lines after
/// its `|`, escaped, stripped and joined with spaces, as docutils'
/// `substitution` pattern finds it, the shortest it can be: at the `|` that
/// closes it.
fn substitution_name(s: &[char]) -> Option<usize> {
    if s.first().is_none_or(|&c| c == ' ') {
        return None;
    }
    (1..s.len()).find(|&k| {
        s[k] == '|'
            && !is_space(s[k - 1])
            && s[k - 1] != ESCAPE
            && s.get(k + 1).is_none_or(|&c| c == ' ')
    })
}

/// The label in brackets `text` starts with, if `valid` accepts it, then
/// spaces, or the line's end, and where what follows starts, in characters.
fn label(text: &str, valid: impl Fn(&str) -> bool) -> Option<(&str, usize)> {
    let inner = text.strip_prefix('[')?;
    let close = inner.find(']')?;
    let (label, rest) = (&inner[..close], &inner[close + 1..]);
    if !valid(label) || !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    let end = text.len() - rest.len() + indent(rest);
    Some((label, text[..end].chars().count()))
}

/// Whether `label` is a footnote's: a number, `#`, `#` and a name, or `*`.
fn footnote_label(label: &str) -> bool {
    let simple = |name: &str| !name.is_empty() && simplename(name) == name.len();
    (!label.is_empty() && label.bytes().all(|b| b.is_ascii_digit()))
        || label == "#"
        || label == "*"
        || label.strip_prefix('#').is_some_and(simple)
}

/// Whether `label` is a citation's: a simple name.
fn citation_label(label: &str) -> bool {
    !label.is_empty() && simplename(label) == label.len()
}

/// The name of the directive `text` starts with, `name::`, or, with
/// `space`, `name ::`, as docutils' patterns match it, and where its
/// arguments start, in characters.
fn directive_marker(text: &str, space: bool) -> Option<(&str, usize)> {
    let end = simplename(text);
    let rest = &text[end..];
    let rest = if space {
        rest.strip_prefix(' ').unwrap_or(rest)
    } else {
        rest
    };
    let after = rest.strip_prefix("::").filter(|_| end > 0)?;
    if !after.is_empty() && !after.starts_with(' ') {
        return None;
    }
    let start = text.len() - after.len() + indent(after);
    Some((&text[..end], text[..start].chars().count()))
}

impl Body<'_, '_> {
    /// Explicit markup, at a line that starts with `..`, and the explicit
    /// markup that follows it with no blank line between.
    pub fn explicit(&mut self) -> bool {
        let finish = self.construct();
        self.explicit_list(finish);
        true
    }

    /// An anonymous target, at a line that starts with `__`, and the
    /// explicit markup that follows it with no blank line between.
    pub fn anonymous_target(&mut self) -> bool {
        let finish = self.anonymous_construct();
        self.explicit_list(finish);
        true
    }

    /// The explicit markup that follows a construct, up to a line that isn't
    /// explicit markup: warning, if the last construct, which ended with a
    /// blank line if `finish`, doesn't.
    fn explicit_list(&mut self, mut finish: bool) {
        let lines = self.lines;
        while let Some(line) = lines.get(self.at) {
            if line.text.is_empty() || line.text.starts_with(' ') {
                break;
            }
            finish = match transition(&line.text) {
                Transition::Explicit => self.construct(),
                Transition::Anonymous => self.anonymous_construct(),
                _ => break,
            };
        }
        if !finish {
            self.unindent_warning("explicit markup", self.at);
        }
    }

    /// The explicit markup construct at the line `self.at`, which starts
    /// with `..`: whether it ends with a blank line. One that's malformed
    /// is warned of, and read as a comment, as docutils reads it.
    fn construct(&mut self) -> bool {
        let lines = self.lines;
        let at = self.at;
        let text = &lines[at].text;
        let rest = &text[2..];
        if rest.is_empty() && lines.get(at + 1).is_none_or(|line| line.text.is_empty()) {
            self.at += 1;
            self.nodes.push(Node::Nothing { prebib: true });
            return true;
        }
        let start = 2 + indent(rest);
        let after = &text[start..];
        let bar = |marker: char| {
            let mut chars = after.chars();
            chars.next() == Some(marker) && chars.next().is_some_and(|c| c != ' ')
        };
        let read = if rest.is_empty() {
            None
        } else if let Some((label, end)) = label(after, footnote_label) {
            Some(Ok(self.footnote(label, start + end, false)))
        } else if let Some((label, end)) = label(after, citation_label) {
            Some(Ok(self.footnote(label, start + end, true)))
        } else if bar('_') {
            Some(self.hyperlink_target(start + 1))
        } else if bar('|') {
            Some(self.substitution_definition(start + 1))
        } else if let Some((name, end)) = directive_marker(after, true) {
            Some(Ok(self.directive(name, start + end, None).1))
        } else {
            None
        };
        match read {
            Some(Ok(finish)) => finish,
            Some(Err(message)) => {
                self.state.warn(lines[at].num, message);
                self.comment(start)
            }
            None => self.comment(start),
        }
    }

    /// A comment, whose text starts after the first `start` characters of
    /// its line: whether it ends with a blank line.
    fn comment(&mut self, start: usize) -> bool {
        let block = indented(self.lines, self.at, false, None, Some(start));
        self.at = block.end;
        self.nodes.push(Node::Nothing { prebib: true });
        block.blank_finish
    }

    /// A footnote, or, with `citation`, a citation, with `label`, as it's
    /// written, whose text starts after the first `start` characters of its
    /// line: whether it ends with a blank line.
    fn footnote(&mut self, label: &str, start: usize, citation: bool) -> bool {
        let line = self.lines[self.at].num;
        let block = indented(self.lines, self.at, false, None, Some(start));
        self.at = block.end;
        let (kind, names, prefix) = if citation {
            (Kind::Citation, vec![normalize(label)], "citation")
        } else if label == "*" {
            (Kind::Symbol, Vec::new(), "footnote")
        } else if let Some(name) = label.strip_prefix('#') {
            let labelled = !name.is_empty();
            let names = labelled.then(|| normalize(name)).into_iter().collect();
            (Kind::Auto { labelled }, names, "footnote")
        } else {
            (Kind::Manual, vec![label.to_string()], "footnote")
        };
        let index = self.state.footnotes.len();
        self.state.footnotes.push(Footnote {
            kind,
            label: label.to_string(),
            blocks: Vec::new(),
            line,
        });
        self.state
            .target(Dest::Footnote(index), &names, true, prefix, line);
        if block.lines.is_empty() {
            let what = if citation { "citation" } else { "footnote" };
            self.state.warn(line, format!("{what} content expected"));
        }
        let blocks = nested(self.state, &block.lines);
        self.state.footnotes[index].blocks = blocks;
        self.nodes.push(Node::Nothing { prebib: false });
        block.blank_finish
    }

    /// A hyperlink target, whose name starts after the first `start`
    /// characters of its line, after its `_`: whether it ends with a blank
    /// line, or an error, if it's malformed.
    fn hyperlink_target(&mut self, start: usize) -> Result<bool, String> {
        let lines = self.lines;
        let at = self.at;
        let line = lines[at].num;
        let block = indented(lines, at, true, None, Some(start));
        let raw: Vec<String> = std::iter::once(skip(&lines[at].text, start))
            .chain(
                lines[at + 1..block.end]
                    .iter()
                    .map(|line| line.text.as_str()),
            )
            .map(escape)
            .collect();
        let mut s = Vec::new();
        let mut ends = Vec::new();
        for text in &raw {
            s.extend(text.chars());
            ends.push(s.len());
        }
        let (name, colon) = target_name(&s).ok_or("malformed hyperlink target")?;
        let last = ends
            .iter()
            .position(|&end| end > colon)
            .expect("the colon is in a line");
        let rest: String = s[colon + 1..ends[last]].iter().collect();
        let mut target = vec![rest.trim_matches(is_space).to_string()];
        target.extend(raw[last + 1..].iter().cloned());
        let names = match name {
            Some((from, to)) => vec![normalize(&unescape(
                &s[from..to].iter().collect::<String>(),
            ))],
            None => Vec::new(),
        };
        let mut dest = parse_target(&target);
        if let Dest::Url(url) = &mut dest
            && !names.is_empty()
            && uri::is_email(url)
        {
            *url = format!("mailto:{url}");
        }
        self.at = block.end;
        let index = self.state.target(dest, &names, true, "target", line);
        if names.is_empty() {
            self.state.anonymous.push(index);
        }
        self.nodes.push(Node::Target(index));
        Ok(block.blank_finish)
    }

    /// The anonymous target at the line `self.at`, which starts with `__`:
    /// whether it ends with a blank line.
    fn anonymous_construct(&mut self) -> bool {
        let lines = self.lines;
        let at = self.at;
        let start = 2 + indent(&lines[at].text[2..]);
        let block = indented(lines, at, true, None, Some(start));
        self.at = block.end;
        let target: Vec<String> = block.lines.iter().map(|line| escape(&line.text)).collect();
        let dest = parse_target(&target);
        let index = self.state.target(dest, &[], true, "target", lines[at].num);
        self.state.anonymous.push(index);
        self.nodes.push(Node::Target(index));
        block.blank_finish
    }

    /// A substitution definition, whose name starts after the first `start`
    /// characters of its line, after its `|`: whether it ends with a blank
    /// line, or an error, if it's malformed. What follows its name is read
    /// as a directive on lines of its own.
    fn substitution_definition(&mut self, start: usize) -> Result<bool, String> {
        let lines = self.lines;
        let at = self.at;
        let line = lines[at].num;
        let block = indented(lines, at, false, None, Some(start));
        // The lines, stripped, as docutils joins them to find the name.
        let texts: Vec<&str> = std::iter::once(skip(&lines[at].text, start))
            .chain(
                lines[at + 1..block.end]
                    .iter()
                    .map(|line| line.text.trim_matches(is_space)),
            )
            .collect();
        let mut s = Vec::new();
        let mut starts = Vec::new();
        for (index, text) in texts.iter().enumerate() {
            if index > 0 {
                s.push(' ');
            }
            starts.push(s.len());
            s.extend(escape(text).chars());
        }
        let close = substitution_name(&s).ok_or("malformed substitution definition")?;
        self.at = block.end;
        let mut finish = block.blank_finish;
        let escaped: String = s[..close].iter().collect();
        let written = collapse(&restore(&escaped));
        let name = collapse(&unescape(&escaped));
        let j = starts
            .iter()
            .rposition(|&start| start <= close)
            .expect("a line starts at 0");
        let rest = skip(texts[j], close - starts[j] + 1).trim_matches(is_space);
        let mut content = Vec::new();
        if !rest.is_empty() {
            let num = lines[at + j].num;
            content.push(Line {
                text: rest.to_string(),
                num,
            });
        }
        content.extend(lines[at + j + 1..block.end].iter().cloned());
        trim_end(&mut content);
        if content.is_empty() {
            let message = format!("substitution definition \"{written}\" missing contents");
            self.state.warn(line, message);
            self.nodes.push(Node::Nothing { prebib: true });
            return Ok(finish);
        }
        content[0].text = content[0].text.trim_matches(is_space).to_string();
        let directive = directive_marker(&content[0].text, false);
        let directive = directive.map(|(directive, end)| (directive.to_string(), end));
        let substitution = match directive {
            Some((directive, end)) => {
                let mut body = Body {
                    state: &mut *self.state,
                    lines: &content,
                    at: 0,
                    titles: false,
                    nodes: Vec::new(),
                };
                let (substitution, _) = body.directive(&directive, end, Some(&name));
                let nodes = body.nodes;
                self.nodes.extend(nodes);
                substitution
            }
            None => {
                // docutils reads a line of text as the end of the definition.
                if !content[0].text.is_empty() && content.len() > 1 {
                    finish = content[1].text.is_empty();
                }
                None
            }
        };
        match substitution {
            Some(substitution) => {
                if self.state.substitutions.contains_key(&name) {
                    let message = format!("duplicate substitution definition name: \"{written}\"");
                    self.state.warn(line, message);
                }
                self.state
                    .substitution_names
                    .insert(name.to_lowercase(), name.clone());
                self.state.substitutions.insert(name, substitution);
            }
            None => {
                let message = format!("substitution definition \"{written}\" empty or invalid");
                self.state.warn(line, message);
            }
        }
        self.nodes.push(Node::Nothing { prebib: true });
        Ok(finish)
    }
}
