//! Writes a block's own run of text as Markdown: its text escaped, so it
//! reads back as it was, and each delimited text, like emphasis, in the
//! first form the site reads that Markdown wouldn't misread where it is,
//! else in HTML, where `raw_html` is on, else as plain text.

use crate::block::Writer;
use crate::model::Inline;
use crate::normalize::Kind;
use crate::settings::Settings;
use crate::targets::lines_as_spaces;
use finl_unicode::categories::CharacterCategories;
use std::ops::Range;

/// Where a run of text is written, which changes what has to be escaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Context {
    /// A paragraph, or a term or attribution, whose lines could start
    /// blocks of their own.
    Paragraph,
    /// A heading, on a line of its own.
    Heading,
    /// A cell of a table, where `|` ends the cell.
    Cell,
}

/// How delimited text is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    /// Between these delimiters.
    Delimiters(&'static str),
    /// In this HTML element.
    Html(&'static str),
    /// As plain text, losing what it marked.
    Plain,
}

/// The delimiters of a delimited text, where they're written.
struct Run {
    /// Which delimited text it is, counted in the order they start.
    node: usize,
    /// The delimiters' character.
    byte: u8,
    /// Where its opening delimiters are.
    open: Range<usize>,
    /// Where its closing delimiters are.
    close: Range<usize>,
    /// Whether it's in a link's text.
    link: bool,
}

/// How a run of text is being written.
struct State<'f> {
    context: Context,
    /// The form each delimited text is written in, counted in the order
    /// they start, as an index into those it can be written in.
    forms: &'f [usize],
    /// The number of delimited texts started so far.
    node: usize,
    /// The delimiters written so far.
    runs: Vec<Run>,
    /// Whether it's in a link's text.
    link: bool,
}

impl Writer<'_> {
    /// `inlines`, written as Markdown in `context`, from the start of a
    /// block.
    pub(crate) fn inlines(&mut self, inlines: &[Inline], context: Context) -> String {
        let mut out = String::new();
        self.inlines_into(&mut out, inlines, context);
        out
    }

    /// Writes `inlines` at the end of `out`, which holds what's before them
    /// in their block, in `context`. Each delimited text is written in its
    /// first form; while Markdown would misread any, each it would is
    /// written in its next, until none is.
    pub(crate) fn inlines_into(&mut self, out: &mut String, inlines: &[Inline], context: Context) {
        let start = out.len();
        let mut kinds = Vec::new();
        kinds_of(inlines, &mut kinds);
        let mut forms = vec![0; kinds.len()];
        loop {
            out.truncate(start);
            let mut state = State {
                context,
                forms: &forms,
                node: 0,
                runs: Vec::new(),
                link: false,
            };
            self.run(out, inlines, &mut state);
            let wrong = misread(out.as_str(), &state.runs, self.settings);
            if wrong.is_empty() {
                break;
            }
            for node in wrong {
                forms[node] += 1;
            }
        }
        for (&kind, &index) in kinds.iter().zip(&forms) {
            if self.form(kind, index) == Form::Plain {
                self.warnings.push(
                    None,
                    format!(
                        "left out {} where Markdown can't mark it, as `raw_html` is off",
                        kind.name()
                    ),
                );
            }
        }
    }

    /// The form `kind` is written in at `index` among those it can be: each
    /// of its delimiters the site reads, then HTML, where `raw_html` is on,
    /// then plain text.
    fn form(&self, kind: Kind, index: usize) -> Form {
        let delimiters = delimiters(kind, self.settings);
        match delimiters.get(index) {
            Some(&delimiter) => Form::Delimiters(delimiter),
            None if index == delimiters.len() && self.settings.raw_html => Form::Html(tag(kind)),
            None => Form::Plain,
        }
    }

    fn run(&mut self, out: &mut String, inlines: &[Inline], state: &mut State) {
        for inline in inlines {
            match Kind::of(inline) {
                Some((kind, children)) => self.delimited(out, kind, children, state),
                None => self.inline(out, inline, state),
            }
        }
    }

    fn delimited(&mut self, out: &mut String, kind: Kind, children: &[Inline], state: &mut State) {
        let node = state.node;
        state.node += 1;
        match self.form(kind, state.forms[node]) {
            Form::Delimiters(delimiter) => {
                let open = out.len()..out.len() + delimiter.len();
                out.push_str(delimiter);
                self.run(out, children, state);
                let close = out.len()..out.len() + delimiter.len();
                out.push_str(delimiter);
                state.runs.push(Run {
                    node,
                    byte: delimiter.as_bytes()[0],
                    open,
                    close,
                    link: state.link,
                });
            }
            Form::Html(tag) => {
                out.push_str(&format!("<{tag}>"));
                self.run(out, children, state);
                out.push_str(&format!("</{tag}>"));
            }
            Form::Plain => self.run(out, children, state),
        }
    }

    fn inline(&mut self, out: &mut String, inline: &Inline, state: &mut State) {
        let context = state.context;
        match inline {
            Inline::Text(text) => escape(out, text, context, self.settings),
            Inline::Code(code) => code_span(out, &lines_as_spaces(code), context),
            Inline::Math(tex) => self.math(out, tex, context),
            Inline::Link {
                target,
                title,
                content,
            } => {
                guard_bang(out);
                out.push('[');
                let link = std::mem::replace(&mut state.link, true);
                self.run(out, content, state);
                state.link = link;
                out.push_str("](");
                destination(out, &self.targets.resolve(target), title, context);
                out.push(')');
            }
            Inline::Image { url, alt, title } => {
                out.push_str("![");
                escape(out, alt, context, self.settings);
                out.push_str("](");
                destination(out, url, title, context);
                out.push(')');
            }
            Inline::Footnote(label) => {
                let name = self.footnote_name(label);
                out.push_str(&format!("[^{name}]"));
            }
            Inline::LineBreak => match context {
                Context::Paragraph => out.push_str("\\\n"),
                Context::Cell if self.settings.raw_html => out.push_str("<br>"),
                Context::Cell | Context::Heading => out.push(' '),
            },
            Inline::Html { html, .. } => html_into(out, &lines_as_spaces(html), context),
            Inline::Anchor(id) => {
                if self.settings.raw_html {
                    verbatim(out, &format!("<a id=\"{}\"></a>", attribute(id)), context);
                }
            }
            _ => unreachable!("delimited text is written by `delimited`"),
        }
    }

    fn math(&mut self, out: &mut String, tex: &str, context: Context) {
        let tex = lines_as_spaces(tex);
        let tex = tex.trim();
        if self.settings.math && !tex.contains('`') {
            out.push_str("$`");
            verbatim(out, tex, context);
            out.push_str("`$");
            return;
        }
        let message = if self.settings.math {
            "wrote a formula with a backtick as code, as Markdown can't hold it"
        } else {
            "wrote formulas as code, as `math` is off"
        };
        self.warnings.push(None, message);
        code_span(out, tex, context);
    }
}

/// The delimiters `kind` can be written between, as the site reads them, in
/// the order they're tried. With CJK's rules, `_` is left out, as it isn't
/// read as delimiters beside CJK text.
fn delimiters(kind: Kind, settings: &Settings) -> &'static [&'static str] {
    let cjk = settings.cjk_friendly_emphasis;
    match kind {
        Kind::Emphasis if cjk => &["*"],
        Kind::Emphasis => &["*", "_"],
        Kind::Strong if cjk => &["**"],
        Kind::Strong => &["**", "__"],
        Kind::Strikethrough if settings.strikethrough => &["~~"],
        Kind::Mark if settings.mark => &["=="],
        Kind::Superscript if settings.superscript => &["^"],
        Kind::Subscript if settings.subscript => &["~"],
        _ => &[],
    }
}

/// The HTML element `kind` is written in where delimiters can't be.
fn tag(kind: Kind) -> &'static str {
    match kind {
        Kind::Emphasis => "em",
        Kind::Strong => "strong",
        Kind::Strikethrough => "del",
        Kind::Underline => "u",
        Kind::Mark => "mark",
        Kind::Superscript => "sup",
        Kind::Subscript => "sub",
    }
}

/// Writes `text` at the end of `out`, escaping each character Markdown would
/// read as more than text where it is: those it always could, those the
/// site's extensions read, those that start a block at the start of a
/// paragraph's line, and those that would make a link of a `]` before them.
fn escape(out: &mut String, text: &str, context: Context, settings: &Settings) {
    let paragraph = context == Context::Paragraph;
    let line = out[out.rfind('\n').map_or(0, |at| at + 1)..].trim_start_matches(' ');
    // Whether the line so far holds only spaces, and only digits after them.
    let mut blank = line.is_empty();
    let mut digits = !blank && line.bytes().all(|b| b.is_ascii_digit());
    for c in text.chars() {
        let start = paragraph && blank;
        let number = paragraph && digits;
        let bracket = out.ends_with(']');
        let escaped = match c {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '&' | '#' | '|' => true,
            '$' => settings.math,
            '~' => settings.strikethrough || settings.subscript || start,
            '^' => settings.superscript || settings.inline_footnotes,
            '=' => settings.mark || start,
            '+' => settings.insert || start,
            '-' => start,
            ':' => settings.emoji || settings.autolinks || start || bracket,
            '(' => bracket,
            '@' => settings.autolinks,
            '.' => number || (settings.autolinks && after_www(out)),
            ')' => number,
            _ => false,
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

/// Writes `code` as code in a line: between the fewest backticks that aren't
/// a run of them in it, and inside a space each, where it would otherwise
/// lose a space, or join its backticks to theirs.
fn code_span(out: &mut String, code: &str, context: Context) {
    let mut runs = Vec::new();
    let mut run = 0;
    for b in code.bytes().chain(*b" ") {
        if b == b'`' {
            run += 1;
        } else if run > 0 {
            runs.push(run);
            run = 0;
        }
    }
    let mut fence = 1;
    while runs.contains(&fence) {
        fence += 1;
    }
    let fence = "`".repeat(fence);
    let pad = code.starts_with('`')
        || code.ends_with('`')
        || (code.starts_with(' ') && code.ends_with(' ') && code.bytes().any(|b| b != b' '));
    let pad = if pad { " " } else { "" };
    out.push_str(&fence);
    out.push_str(pad);
    verbatim(out, code, context);
    out.push_str(pad);
    out.push_str(&fence);
}

/// Writes a link's or image's destination, between `<` and `>`, and its
/// title, if it has one, each with its `\` and what would end it escaped,
/// and its `&` as `&amp;`, since comrak reads character references in them
/// before backslash escapes, so `\&copy;` would be `\©`.
fn destination(out: &mut String, url: &str, title: &str, context: Context) {
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
    if !title.is_empty() {
        written.push_str(" \"");
        for c in title.chars() {
            match c {
                '&' => written.push_str("&amp;"),
                '"' | '\\' => {
                    written.push('\\');
                    written.push(c);
                }
                _ => written.push(c),
            }
        }
        written.push('"');
    }
    verbatim(out, &written, context);
}

/// Writes raw HTML: its tags as they are, and its text with its punctuation
/// escaped, so it isn't read as Markdown, after an empty `<span>` at the
/// start of a paragraph's line, where it could start a block of HTML or
/// code.
fn html_into(out: &mut String, html: &str, context: Context) {
    let line = &out[out.rfind('\n').map_or(0, |at| at + 1)..];
    if context == Context::Paragraph
        && line.bytes().all(|b| b == b' ')
        && html.starts_with(|c: char| c == '<' || c.is_whitespace())
    {
        out.push_str("<span></span>");
    }
    for (tag, piece) in html_pieces(html) {
        if tag {
            verbatim(out, piece, context);
            continue;
        }
        for c in piece.chars() {
            if c.is_ascii_punctuation() && c != '&' {
                out.push('\\');
            }
            out.push(c);
        }
    }
}

/// `value`, as the value of an HTML attribute between `"`s.
pub(crate) fn attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

/// Escapes a `!` at the end of `out`, which would make an image of a link
/// after it.
fn guard_bang(out: &mut String) {
    if let Some(before) = out.strip_suffix('!')
        && before.bytes().rev().take_while(|&b| b == b'\\').count() % 2 == 0
    {
        out.insert(out.len() - 1, '\\');
    }
}

/// Writes `text` as it is, but in a cell, where each `|` is escaped, as it
/// would end the cell, by as many backslashes as keep it escaped after the
/// ones before it.
pub(crate) fn verbatim(out: &mut String, text: &str, context: Context) {
    if context != Context::Cell {
        out.push_str(text);
        return;
    }
    for c in text.chars() {
        if c == '|' {
            let before = out.bytes().rev().take_while(|&b| b == b'\\').count();
            out.push_str(if before % 2 == 0 { "\\|" } else { "\\\\|" });
        } else {
            out.push(c);
        }
    }
}

/// Lists the kinds of the delimited texts in `inlines`, in the order they
/// start, as `delimited` counts them.
fn kinds_of(inlines: &[Inline], kinds: &mut Vec<Kind>) {
    for inline in inlines {
        if let Some((kind, children)) = Kind::of(inline) {
            kinds.push(kind);
            kinds_of(children, kinds);
        } else if let Inline::Link { content, .. } = inline {
            kinds_of(content, kinds);
        }
    }
}

/// The delimited texts in `out` that Markdown would misread, by `runs`, their
/// delimiters: those whose delimiters can't open or close where they are;
/// superscript in a link's text, where `^` is only text, or before a `[`,
/// where it starts a footnote; and, where two runs of the same delimiter
/// touch, which would be read as one, the outer, or the first.
fn misread(out: &str, runs: &[Run], settings: &Settings) -> Vec<usize> {
    let footnotes = settings.footnotes && settings.inline_footnotes;
    let mut wrong = Vec::new();
    for run in runs {
        let (opens, _) = flanks(out, run.open.clone(), run.byte, settings);
        let (_, closes) = flanks(out, run.close.clone(), run.byte, settings);
        let caret = run.byte == b'^'
            && (run.link
                || footnotes
                    && (out[run.open.end..].starts_with('[')
                        || out[run.close.end..].starts_with('[')));
        if !opens || !closes || caret {
            wrong.push(run.node);
        }
    }
    let mut edges: Vec<(&Range<usize>, u8, usize)> = runs
        .iter()
        .flat_map(|run| {
            [
                (&run.open, run.byte, run.node),
                (&run.close, run.byte, run.node),
            ]
        })
        .collect();
    edges.sort_by_key(|(range, ..)| range.start);
    for pair in edges.windows(2) {
        let ((a, a_byte, a_node), (b, b_byte, b_node)) = (pair[0], pair[1]);
        if a.end == b.start
            && a_byte == b_byte
            && !wrong.contains(&a_node)
            && !wrong.contains(&b_node)
        {
            wrong.push(a_node.min(b_node));
        }
    }
    wrong.sort_unstable();
    wrong.dedup();
    wrong
}

/// Whether the run of `byte` at `range` in `subject` can open and close
/// delimited text, as comrak reads it: by the characters either side of it,
/// past the `~`, `=`, and `+` the site's extensions read, which count as a
/// line ending at the end. This is without CJK's rules, which allow more.
fn flanks(subject: &str, range: Range<usize>, byte: u8, settings: &Settings) -> (bool, bool) {
    let bytes = subject.as_bytes();
    let skip = |b: u8| match b {
        b'~' => settings.strikethrough || settings.subscript,
        b'=' => settings.mark,
        b'+' => settings.insert,
        _ => false,
    };
    let char_at = |at: usize| match subject[at..].chars().next() {
        Some(c) if !(c.is_ascii() && skip(c as u8)) => c,
        _ => '\n',
    };
    let before = if range.start == 0 {
        '\n'
    } else {
        let mut at = range.start - 1;
        while at > 0 && (bytes[at] >> 6 == 2 || skip(bytes[at])) {
            at -= 1;
        }
        char_at(at)
    };
    let after = if range.end == bytes.len() {
        '\n'
    } else {
        let mut at = range.end;
        while at < bytes.len() - 1 && skip(bytes[at]) {
            at += 1;
        }
        char_at(at)
    };
    let punctuation = |c: char| c.is_punctuation() || c.is_symbol();
    let left = !after.is_whitespace()
        && (!punctuation(after)
            || (settings.superscript && byte == b'^')
            || (settings.subscript && byte == b'~')
            || before.is_whitespace()
            || punctuation(before));
    let right = !before.is_whitespace()
        && (!punctuation(before) || after.is_whitespace() || punctuation(after));
    if byte == b'_' {
        (
            left && (!right || punctuation(before)),
            right && (!left || punctuation(after)),
        )
    } else {
        (left, right)
    }
}

/// Splits raw HTML into its tags, comments, and the like, each `true`, and
/// the text between them, each `false`. A `<` that starts none is text.
pub(crate) fn html_pieces(html: &str) -> Vec<(bool, &str)> {
    let mut pieces = Vec::new();
    let mut text = 0;
    let mut at = 0;
    while let Some(found) = html[at..].find('<') {
        let start = at + found;
        match tag_end(&html[start..]) {
            Some(end) => {
                if text < start {
                    pieces.push((false, &html[text..start]));
                }
                pieces.push((true, &html[start..start + end]));
                at = start + end;
                text = at;
            }
            None => at = start + 1,
        }
    }
    if text < html.len() {
        pieces.push((false, &html[text..]));
    }
    pieces
}

/// The length of the tag, comment, or the like at the start of `html`, if
/// it's closed.
fn tag_end(html: &str) -> Option<usize> {
    let closed = |from: usize, end: &str| html[from..].find(end).map(|at| from + at + end.len());
    if html.starts_with("<!--") {
        closed(2, "-->")
    } else if html.starts_with("<?") {
        closed(2, "?>")
    } else if html.starts_with("<![CDATA[") {
        closed(9, "]]>")
    } else if html.starts_with("<!") {
        closed(2, ">")
    } else if html[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/') {
        let mut quote = None;
        for (at, c) in html.char_indices().skip(1) {
            match quote {
                Some(q) if c == q => quote = None,
                Some(_) => {}
                None if c == '"' || c == '\'' => quote = Some(c),
                None if c == '>' => return Some(at + 1),
                None => {}
            }
        }
        None
    } else {
        None
    }
}
