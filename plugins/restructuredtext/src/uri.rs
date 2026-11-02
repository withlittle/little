//! Standalone URIs and email addresses, as docutils finds them in text
//! that isn't inline markup, in time linear in the text.

use crate::data::SCHEMES;
use crate::text::{ESCAPE, is_prefix, is_suffix};

const NONE: usize = usize::MAX;

/// Whether `c` may be in a URI.
fn uric(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-_.!~*'()[];/:@&=+$,%".contains(c) || c == ESCAPE
}

/// Whether a URI may end with `c`.
fn urilast(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_~*/=+".contains(c)
}

/// Whether `c` may be in an email address, but for `.` and `@`.
fn emailc(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-_!~*'{|}/#?^`&=+$%".contains(c) || c == ESCAPE
}

/// Whether `c` may be in a URI's scheme.
fn scheme_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || ".+-".contains(c)
}

/// A URI or email address found in text, from `start` to `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Found {
    pub start: usize,
    pub end: usize,
    /// Whether it's an email address, which links with `mailto:`.
    pub email: bool,
}

/// What's known of each place in some text, worked out from its end, so
/// each URI is found without reading the text after it again.
struct Scanner<'a> {
    chars: &'a [char],
    /// The end of the run of URI characters from each place.
    uric_end: Vec<usize>,
    /// The furthest place in the run of URI characters from each place, but
    /// for it, that a URI may end at, or 0.
    best: Vec<usize>,
    /// The `:` after the scheme that starts at each place, if it's one.
    colon: Vec<usize>,
    /// The `@` after the name of an email address that starts at each place.
    at: Vec<usize>,
    uri_cache: Option<(usize, Option<usize>)>,
    host_cache: Option<(usize, Option<usize>)>,
}

impl<'a> Scanner<'a> {
    fn new(chars: &'a [char]) -> Self {
        let n = chars.len();
        let get = |i: usize| chars.get(i).copied();
        let mut uric_end = vec![n; n + 1];
        let mut best = vec![0; n + 1];
        let mut colon = vec![NONE; n + 1];
        let mut at = vec![NONE; n + 1];
        for i in (0..n).rev() {
            let c = chars[i];
            if uric(c) {
                uric_end[i] = uric_end[i + 1];
                let end = uric_end[i];
                best[i] = if i + 1 < end && best[i + 1] != 0 {
                    best[i + 1]
                } else if (urilast(c) || (i + 1 == end && get(end) == Some('>')))
                    && suffix(chars, i + 1)
                {
                    i + 1
                } else {
                    0
                };
            } else {
                uric_end[i] = i;
            }
            if scheme_char(c) {
                colon[i] = match get(i + 1) {
                    Some(':') => i + 1,
                    Some(next) if scheme_char(next) => colon[i + 1],
                    _ => NONE,
                };
            }
            if emailc(c) {
                at[i] = match get(i + 1) {
                    Some('@') if c != ESCAPE => i + 1,
                    Some(next) if emailc(next) => at[i + 1],
                    Some('.') if get(i + 2).is_some_and(emailc) => at[i + 2],
                    _ => NONE,
                };
            }
        }
        Scanner {
            chars,
            uric_end,
            best,
            colon,
            at,
            uri_cache: None,
            host_cache: None,
        }
    }

    /// Whether a part of a URI may end at `end`, the end of its run.
    fn can_end(&self, end: usize) -> bool {
        urilast(self.chars[end - 1]) || self.chars.get(end) == Some(&'>')
    }

    /// The best end of a part of a URI from `start`, the end of its run, and
    /// whether the part may end there, if it has a character.
    fn segment(&self, start: usize) -> Option<(usize, usize, bool)> {
        let c = *self.chars.get(start)?;
        if !uric(c) {
            return None;
        }
        let end = self.uric_end[start];
        Some((self.best[start], end, self.can_end(end)))
    }

    /// The end of the URI whose scheme ends with the `:` at `colon`: the
    /// furthest its path, query and fragment may take it.
    fn uri(&mut self, colon: usize) -> Option<usize> {
        if let Some((cached, end)) = self.uri_cache
            && cached == colon
        {
            return end;
        }
        let end = self.find_uri(colon);
        self.uri_cache = Some((colon, end));
        end
    }

    fn find_uri(&self, colon: usize) -> Option<usize> {
        let (first, end, can) = self.segment(colon + 1)?;
        let mut found = (first != 0).then_some(first);
        if can {
            let fragment = |start: usize| self.segment(start).map(|(best, ..)| best);
            match self.chars.get(end) {
                Some('?') => {
                    if let Some((query, query_end, can)) = self.segment(end + 1) {
                        if query != 0 {
                            found = Some(query);
                        }
                        if can && self.chars.get(query_end) == Some(&'#') {
                            found = fragment(query_end + 1).filter(|&best| best != 0).or(found);
                        }
                    }
                }
                Some('#') => found = fragment(end + 1).filter(|&best| best != 0).or(found),
                _ => {}
            }
        }
        found
    }

    /// The end of the email address whose name starts at `start`.
    fn email(&mut self, start: usize) -> Option<usize> {
        let at = self.at[start];
        if at == NONE {
            return None;
        }
        let host = at + 1;
        if let Some((cached, end)) = self.host_cache
            && cached == host
        {
            return end;
        }
        let end = self.find_host(host);
        self.host_cache = Some((host, end));
        end
    }

    /// The end of the host from `host`: its furthest character a URI may
    /// end with, after at least one other.
    fn find_host(&self, host: usize) -> Option<usize> {
        let chars = self.chars;
        if !chars.get(host).copied().is_some_and(emailc) {
            return None;
        }
        let run = host
            + chars[host..]
                .iter()
                .take_while(|&&c| emailc(c) || c == '.')
                .count();
        let after = |end: usize| chars.get(end) == Some(&'>');
        if run < chars.len() && uric(chars[run]) && after(run + 1) && suffix(chars, run + 1) {
            return Some(run + 1);
        }
        (host + 2..=run).rev().find(|&end| {
            let last = chars[end - 1];
            (urilast(last) || (end == run && uric(last) && after(end))) && suffix(chars, end)
        })
    }
}

/// Whether inline markup, or a URI, may end before `end` in `chars`.
pub(crate) fn suffix(chars: &[char], end: usize) -> bool {
    chars.get(end).is_none_or(|&c| is_suffix(c))
}

/// The standalone URIs and email addresses in `chars`, escaped text, as
/// docutils finds them: each with a scheme it knows, and none after a URI
/// with one it doesn't.
pub(crate) fn find(chars: &[char]) -> Vec<Found> {
    let mut scanner = Scanner::new(chars);
    let mut found = Vec::new();
    // Where the text after the last URI starts, which a URI may start at.
    let mut start = 0;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if at == start || is_prefix(chars[at - 1]) {
            let colon = scanner.colon[at];
            if c.is_ascii_alphabetic()
                && colon != NONE
                && let Some(end) = scanner.uri(colon)
            {
                let scheme: String = chars[at..colon].iter().collect();
                if SCHEMES
                    .binary_search(&scheme.to_lowercase().as_str())
                    .is_err()
                {
                    return found;
                }
                found.push(Found {
                    start: at,
                    end,
                    email: false,
                });
                (start, at) = (end, end);
                continue;
            }
            if emailc(c)
                && let Some(end) = scanner.email(at)
            {
                found.push(Found {
                    start: at,
                    end,
                    email: true,
                });
                (start, at) = (end, end);
                continue;
            }
        }
        at += 1;
    }
    found
}

/// Whether `chars` start with a URI or an email address, whatever its
/// scheme.
pub(crate) fn starts_with_uri(chars: &[char]) -> bool {
    let mut scanner = Scanner::new(chars);
    let Some(&first) = chars.first() else {
        return false;
    };
    let colon = scanner.colon[0];
    (first.is_ascii_alphabetic() && colon != NONE && scanner.uri(colon).is_some())
        || (emailc(first) && scanner.email(0).is_some())
}

/// Whether `text` is an email address, and nothing more.
pub(crate) fn is_email(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if !chars.first().copied().is_some_and(emailc) {
        return false;
    }
    let at = Scanner::new(&chars).at[0];
    if at == NONE {
        return false;
    }
    let host = at + 1;
    chars.len() >= host + 2
        && emailc(chars[host])
        && chars[host..].iter().all(|&c| emailc(c) || c == '.')
        && urilast(chars[chars.len() - 1])
}
