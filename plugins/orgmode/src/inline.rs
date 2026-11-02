//! Org's objects, the text, markup, and links of a paragraph, a title, or
//! a cell, as inlines, with what orgize reads in a way Org doesn't read
//! as Org reads it: formulas in `$…$`, plain links, like
//! `https://orgmode.org`, a few sub- and superscripts, and emphasis right
//! after another object.

use std::collections::VecDeque;

use little_markup::{Block, Inline, Target};
use orgize::ast::{Entity, InlineSrc, Link as LinkNode, Macros, Snippet};
use orgize::config::UseSubSuperscript;
use orgize::rowan::ast::AstNode;
use orgize::{SyntaxElement, SyntaxKind, SyntaxNode};

use crate::convert::{Converter, inner, inner_raw, start};
use crate::links::{self, Link};

/// How many macros can be expanded, one in another, before the plugin
/// takes it that their expansion doesn't end.
const MACRO_DEPTH: usize = 10;

/// The macros Org's export expands that the plugin doesn't, as they need
/// Emacs, or the file's place on disk.
const UNEXPANDED: [&str; 5] = ["time", "modification-time", "input-file", "property", "n"];

/// The inlines a run of objects makes, and the plain text after the last,
/// which Org's special strings are found in.
struct Out {
    inlines: Vec<Inline>,
    text: String,
    special_strings: bool,
}

impl Out {
    fn new(special_strings: bool) -> Out {
        Out {
            inlines: Vec::new(),
            text: String::new(),
            special_strings,
        }
    }

    /// Add `text`, plain text, which may have special strings.
    fn text(&mut self, text: &str) {
        self.text.push_str(text);
    }

    /// Add `inlines`, as they are.
    fn extend(&mut self, inlines: impl IntoIterator<Item = Inline>) {
        self.flush();
        self.inlines.extend(inlines);
    }

    fn flush(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let text = unindent(&std::mem::take(&mut self.text));
        self.inlines.push(Inline::Text(if self.special_strings {
            special_strings(&text)
        } else {
            text
        }));
    }

    fn finish(mut self) -> Vec<Inline> {
        self.flush();
        self.inlines
    }
}

/// `text` with the whitespace around each line end left out, but the line
/// end, which little reads as a space, so a paragraph's lines join with one,
/// as a browser shows them.
fn unindent(text: &str) -> String {
    if !text.contains('\n') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut lines = text.split('\n').peekable();
    let mut first = true;
    while let Some(mut line) = lines.next() {
        if !first {
            line = line.trim_start_matches([' ', '\t']);
        }
        if lines.peek().is_some() {
            out.push_str(line.trim_end_matches([' ', '\t', '\r']));
            out.push('\n');
        } else {
            out.push_str(line);
        }
        first = false;
    }
    out
}

/// `text`, plain text, with Org's special strings as its HTML export shows
/// them, in its order: `\-` a soft hyphen, `---` and `--` before anything
/// but a `-` dashes, and `...` an ellipsis.
fn special_strings(text: &str) -> String {
    let text = text.replace("\\-", "\u{ad}");
    let text = dashes(&text, "---", '\u{2014}');
    let text = dashes(&text, "--", '\u{2013}');
    text.replace("...", "\u{2026}")
}

/// `text` with each `run` of dashes that's followed by anything but a `-`
/// replaced with `dash`.
fn dashes(text: &str, run: &str, dash: char) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(run) {
        out.push_str(&rest[..at]);
        let after = &rest[at + run.len()..];
        if after.starts_with(|c: char| c != '-') {
            out.push(dash);
            rest = after;
        } else {
            out.push('-');
            rest = &rest[at + 1..];
        }
    }
    out.push_str(rest);
    out
}

impl Converter<'_> {
    /// The inlines of the objects `elements`, of a paragraph, a title, or
    /// a cell, with the plain links in their text, if `links`, as there are
    /// none in a link's description.
    pub(crate) fn objects(&mut self, elements: &[SyntaxElement], links: bool) -> Vec<Inline> {
        // Their text, joined, as a formula in `$…$` can run across them.
        let mut full = String::new();
        let mut spans = Vec::with_capacity(elements.len());
        let mut flat = Vec::with_capacity(elements.len());
        let mut queue: VecDeque<SyntaxElement> = elements
            .iter()
            .filter(|element| element.kind() != SyntaxKind::BLANK_LINE)
            .cloned()
            .collect();
        while let Some(element) = queue.pop_front() {
            // orgize starts emphasis right after another object, where Org
            // needs a space or `-('"{` before it, so it's read as text.
            if let SyntaxElement::Node(node) = &element
                && is_emphasis(node.kind())
                && full.chars().next_back().is_some_and(|c| {
                    !(c.is_whitespace() || matches!(c, '-' | '(' | '\'' | '"' | '{'))
                })
            {
                for child in node
                    .children_with_tokens()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                {
                    queue.push_front(child);
                }
                continue;
            }
            let from = full.len();
            full.push_str(&element.to_string());
            spans.push((from, full.len()));
            flat.push(element);
        }
        let elements = flat;
        let mut out = Out::new(self.options.special_strings);
        let mut position = 0;
        for (element, &(from, to)) in elements.iter().zip(&spans) {
            if to <= position {
                continue;
            }
            // Where `full` would start, in the text being read, by this
            // element, to give a warning in it its line.
            let origin = usize::from(element.text_range().start()).saturating_sub(from);
            let node = match element {
                SyntaxElement::Node(node) if from >= position => node,
                _ => {
                    position = self.scan(&full, position.max(from), to, links, origin, &mut out);
                    continue;
                }
            };
            position = to;
            match node.kind() {
                SyntaxKind::AFFILIATED_KEYWORD => {}
                SyntaxKind::LATEX_FRAGMENT if is_single_dollar(&full[from..to]) => {
                    match math_at(&full, from) {
                        Some((end, tex)) if end == to => out.extend([Inline::Math(tex)]),
                        _ => position = self.scan(&full, from, to, links, origin, &mut out),
                    }
                }
                _ => {
                    let inlines = self.object(node, links);
                    out.extend(inlines);
                }
            }
        }
        out.finish()
    }
}

/// Whether `kind` is emphasis, which Org starts only after a space, the
/// start of the text, or `-`, `(`, `'`, `"`, or `{`.
fn is_emphasis(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::BOLD
            | SyntaxKind::ITALIC
            | SyntaxKind::UNDERLINE
            | SyntaxKind::STRIKE
            | SyntaxKind::VERBATIM
            | SyntaxKind::CODE
    )
}

/// Whether `raw`, a formula, is in `$…$`, not `$$…$$`.
fn is_single_dollar(raw: &str) -> bool {
    raw.starts_with('$') && !raw.starts_with("$$")
}

impl Converter<'_> {
    /// Read `full[from..to]`, text, or what orgize read in a way Org
    /// doesn't, into `out`, with each formula in `$…$` or `$$…$$` in it,
    /// and, if `links`, each plain or angle link, as Org finds them; one
    /// may run past `to`. `origin` is where `full` would start in the text
    /// being read. Returns where what it read ends.
    fn scan(
        &mut self,
        full: &str,
        from: usize,
        to: usize,
        links: bool,
        origin: usize,
        out: &mut Out,
    ) -> usize {
        let mut position = from;
        let mut text_from = from;
        while position < to {
            let rest = &full[position..];
            let Some(c) = rest.chars().next() else {
                break;
            };
            let found = match c {
                '$' => math_at(full, position).map(|(end, tex)| (end, vec![Inline::Math(tex)])),
                '_' | '^' => script_at(full, position, &self.options.config.use_sub_superscript)
                    .map(|(end, script)| (end, vec![script])),
                '<' if links => links::angle_link(rest).map(|(length, link)| {
                    let inlines = self.link_to(&link, None, origin + position);
                    (position + length, inlines)
                }),
                _ if links && c.is_ascii_alphabetic() && starts_word(full, position) => {
                    links::plain_link(rest).map(|length| {
                        let inlines = self.link_to(&rest[..length], None, origin + position);
                        (position + length, inlines)
                    })
                }
                _ => None,
            };
            match found {
                Some((end, inlines)) => {
                    out.text(&full[text_from..position]);
                    out.extend(inlines);
                    position = end;
                    text_from = end;
                }
                None => position += c.len_utf8(),
            }
        }
        out.text(&full[text_from..position]);
        position
    }
}

/// Whether a plain link can start at `position` in `text`, at the start of
/// a word, as Emacs's `\<` matches in Org: after anything but a letter, a
/// digit, `$`, `%`, or `'`.
fn starts_word(text: &str, position: usize) -> bool {
    text[..position]
        .chars()
        .next_back()
        .is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '$' | '%' | '\'')))
}

/// Where the formula at `position` in `text`, at a `$`, ends, and its TeX,
/// if Org reads one there, as `org-element-latex-fragment-parser` does:
/// `$$…$$`, up to the next `$$`; or `$…$`, after anything but a `$`, with
/// no space, `.`, `,`, or `;` just inside its first `$`, and no space,
/// `.`, or `,` just inside its last, the next `$`, which is followed by
/// space, punctuation, a bracket, a quote, or the end of the text.
fn math_at(text: &str, position: usize) -> Option<(usize, String)> {
    let rest = &text[position..];
    if let Some(inner) = rest.strip_prefix("$$") {
        let close = inner.find("$$")?;
        return Some((position + 2 + close + 2, inner[..close].to_string()));
    }
    let inner = rest.strip_prefix('$')?;
    if text[..position].ends_with('$')
        || inner.starts_with([' ', '\t', '\n', '\r', '.', ',', ';', '$'])
    {
        return None;
    }
    let close = inner.find('$')?;
    let tex = &inner[..close];
    if tex.is_empty() || tex.ends_with([' ', '\t', '\n', '\r', '.', ',']) {
        return None;
    }
    let after = inner[close + 1..].chars().next();
    if !after.is_none_or(ends_math) {
        return None;
    }
    Some((position + 1 + close + 1, tex.to_string()))
}

/// Where the sub- or superscript at `position` in `text`, at a `_` or
/// `^`, ends, and the script, if Org reads one there, as
/// `org-match-substring-regexp` matches it, and `scripts` allows it.
/// orgize reads most, but not one right after another object, as in
/// `snake_case_name`, nor one whose characters end in `.` or `,`, which
/// Org leaves out, as in `H_2O.`.
fn script_at(text: &str, position: usize, scripts: &UseSubSuperscript) -> Option<(usize, Inline)> {
    let after_space = text[..position]
        .chars()
        .next_back()
        .is_none_or(char::is_whitespace);
    if scripts.is_nil() || after_space {
        return None;
    }
    let rest = &text[position + 1..];
    let (content, length) = if let Some(braced) = rest.strip_prefix('{') {
        let mut depth = 1;
        let close = braced.char_indices().find_map(|(at, c)| {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(at)
        })?;
        (&braced[..close], close + 2)
    } else if scripts.is_brace() {
        return None;
    } else if rest.starts_with('*') {
        ("*", 1)
    } else {
        let sign = usize::from(rest.starts_with(['+', '-']));
        let run = rest[sign..]
            .find(|c: char| !(c.is_alphanumeric() || matches!(c, '.' | ',' | '\\')))
            .unwrap_or(rest.len() - sign);
        let length = sign
            + rest[sign..sign + run]
                .trim_end_matches(|c: char| !c.is_alphanumeric())
                .len();
        if length == sign {
            return None;
        }
        (&rest[..length], length)
    };
    let content = vec![Inline::Text(content.to_string())];
    let script = if text[position..].starts_with('^') {
        Inline::Superscript(content)
    } else {
        Inline::Subscript(content)
    };
    Some((position + 1 + length, script))
}

/// Whether `c` can follow the `$` that ends a formula, as Org's syntax
/// table has it: whitespace, punctuation, a bracket, or a quote. Outside
/// ASCII, anything but a letter or a digit is taken as punctuation.
fn ends_math(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_whitespace() || c.is_ascii_control() || ".,;:?!#@^`'\"([{<)]}>".contains(c)
    } else {
        !c.is_alphanumeric()
    }
}

impl Converter<'_> {
    /// The inlines of the object `node`, with the plain links in it, if
    /// `links`.
    fn object(&mut self, node: &SyntaxNode, links: bool) -> Vec<Inline> {
        let raw = || vec![Inline::Text(node.to_string())];
        match node.kind() {
            SyntaxKind::BOLD => vec![Inline::Strong(self.objects(&inner(node), links))],
            SyntaxKind::ITALIC => vec![Inline::Emphasis(self.objects(&inner(node), links))],
            SyntaxKind::UNDERLINE => vec![Inline::Underline(self.objects(&inner(node), links))],
            SyntaxKind::STRIKE => vec![Inline::Strikethrough(self.objects(&inner(node), links))],
            SyntaxKind::VERBATIM | SyntaxKind::CODE => {
                vec![Inline::Code(unindent(&inner_raw(node)))]
            }
            SyntaxKind::SUPERSCRIPT | SyntaxKind::SUBSCRIPT => {
                let mut children: Vec<SyntaxElement> =
                    node.children_with_tokens().skip(1).collect();
                if children.first().map(SyntaxElement::kind) == Some(SyntaxKind::L_CURLY)
                    && children.last().map(SyntaxElement::kind) == Some(SyntaxKind::R_CURLY)
                {
                    children = children[1..children.len() - 1].to_vec();
                }
                let inlines = self.objects(&children, links);
                vec![if node.kind() == SyntaxKind::SUPERSCRIPT {
                    Inline::Superscript(inlines)
                } else {
                    Inline::Subscript(inlines)
                }]
            }
            SyntaxKind::ENTITY => {
                let entity = Entity::cast(node.clone()).expect("an entity");
                vec![Inline::Text(entity_text(node, &entity))]
            }
            SyntaxKind::LINE_BREAK => vec![Inline::LineBreak],
            SyntaxKind::TIMESTAMP_ACTIVE
            | SyntaxKind::TIMESTAMP_INACTIVE
            | SyntaxKind::TIMESTAMP_DIARY => {
                // As `org-html-timestamp` shows a range, with an en dash.
                vec![Inline::Text(node.to_string().replace("--", "\u{2013}"))]
            }
            SyntaxKind::SNIPPET => {
                let snippet = Snippet::cast(node.clone()).expect("an export snippet");
                if snippet.backend().eq_ignore_ascii_case("html") {
                    let line = self.line(start(node));
                    vec![Inline::Html {
                        html: snippet.value().to_string(),
                        line: Some(line),
                    }]
                } else {
                    Vec::new()
                }
            }
            SyntaxKind::INLINE_SRC => {
                let source = InlineSrc::cast(node.clone()).expect("an inline block of code");
                vec![Inline::Code(source.value().to_string())]
            }
            SyntaxKind::INLINE_CALL => {
                self.warn(
                    start(node),
                    format!(
                        "left out `{}`, as the plugin doesn't run code",
                        one_line(&node.to_string())
                    ),
                );
                Vec::new()
            }
            SyntaxKind::TARGET => vec![Inline::Anchor(links::id_of(&inner_raw(node)))],
            SyntaxKind::RADIO_TARGET => {
                let mut inlines = vec![Inline::Anchor(links::id_of(&inner_raw(node)))];
                inlines.extend(self.objects(&inner(node), links));
                inlines
            }
            SyntaxKind::LATEX_FRAGMENT => {
                let text = node.to_string();
                let tex = text
                    .strip_prefix("\\(")
                    .and_then(|rest| rest.strip_suffix("\\)"))
                    .or_else(|| text.strip_prefix("\\[")?.strip_suffix("\\]"))
                    .or_else(|| text.strip_prefix("$$")?.strip_suffix("$$"))
                    .or_else(|| text.strip_prefix('$')?.strip_suffix('$'));
                match tex {
                    Some(tex) => vec![Inline::Math(tex.trim().to_string())],
                    // A command, like `\ref{x}`, which MathJax doesn't show.
                    None => raw(),
                }
            }
            SyntaxKind::FN_REF => self.footnote_reference(node),
            SyntaxKind::MACROS => self.expand(node),
            SyntaxKind::LINK => self.link(node),
            _ => raw(),
        }
    }
}

/// The text of the entity `node`, like `\alpha`, as Org's UTF-8 export
/// shows it, with a space for each in `\_  `, and a soft hyphen for
/// `\shy`, as its HTML does.
fn entity_text(node: &SyntaxNode, entity: &Entity) -> String {
    if node
        .children_with_tokens()
        .any(|child| child.kind() == SyntaxKind::UNDERSCORE)
    {
        let spaces = node.to_string().chars().filter(|c| *c == ' ').count();
        return "\u{2002}".repeat(spaces);
    }
    match entity.utf8() {
        "" => "\u{ad}".to_string(),
        // orgize writes a few, like `\nbsp`'s, as `\x00A0`.
        text if text.starts_with("\\x") => text
            .split("\\x")
            .filter_map(|code| u32::from_str_radix(code, 16).ok())
            .filter_map(char::from_u32)
            .collect(),
        text => text.to_string(),
    }
}

impl Converter<'_> {
    /// The footnote reference `node`: `[fn:label]`; or an inline
    /// definition, `[fn::text]` or `[fn:label:text]`, which it defines.
    fn footnote_reference(&mut self, node: &SyntaxNode) -> Vec<Inline> {
        let children: Vec<SyntaxElement> = node.children_with_tokens().collect();
        let mut index = children
            .iter()
            .position(|child| child.kind() == SyntaxKind::COLON)
            .map_or(children.len(), |colon| colon + 1);
        let mut label = String::new();
        if let Some(SyntaxElement::Token(token)) = children.get(index)
            && token.kind() == SyntaxKind::TEXT
        {
            label = token.text().to_string();
            index += 1;
        }
        if children.get(index).map(SyntaxElement::kind) == Some(SyntaxKind::COLON) {
            let definition = children
                .get(index + 1..children.len().saturating_sub(1))
                .unwrap_or_default();
            let inlines = self.objects(definition, true);
            if label.is_empty() {
                self.anonymous += 1;
                label = format!(":{}", self.anonymous);
            }
            self.define(label.clone(), vec![Block::Paragraph(inlines)], start(node));
        } else if label.is_empty() {
            return vec![Inline::Text(node.to_string())];
        }
        vec![Inline::Footnote(label)]
    }

    /// The link `node`, `[[path]]` or `[[path][description]]`.
    fn link(&mut self, node: &SyntaxNode) -> Vec<Inline> {
        let link = LinkNode::cast(node.clone()).expect("a link");
        let path = links::unescape(&link.path());
        let description = link.has_description().then(|| {
            let elements: Vec<SyntaxElement> = link.description().collect();
            let raw: String = elements.iter().map(ToString::to_string).collect();
            match image_alone(raw.trim()) {
                Some(image) => vec![image],
                None => self.objects(&elements, false),
            }
        });
        self.link_to(&path, description, start(node))
    }

    /// The inlines of a link to `path`, with `description`, or none, at
    /// `offset` in the text being read: a link, if little can follow it,
    /// an image, if it's to one, and has no description, as Org's HTML
    /// export shows one, or else its text, warned of.
    pub(crate) fn link_to(
        &mut self,
        path: &str,
        description: Option<Vec<Inline>>,
        offset: usize,
    ) -> Vec<Inline> {
        let to = |target: Target, description: Option<Vec<Inline>>| {
            vec![Inline::Link {
                target,
                title: String::new(),
                content: description.unwrap_or_default(),
            }]
        };
        let target = match Link::of(path) {
            Link::Web(url) => {
                let web = url.starts_with("http:") || url.starts_with("https:");
                if description.is_none() && web && links::is_image(&url) {
                    return vec![image(url)];
                }
                Target::Url(url)
            }
            Link::File(file, search) => {
                let url = match search.as_deref().map(|search| search.strip_prefix('#')) {
                    None => file,
                    Some(Some(id)) => format!("{file}#{id}"),
                    Some(None) => {
                        self.warn(
                            offset,
                            format!(
                                "kept only the file of the link to `{path}`, as little links \
                                 only to a file, or an id in it"
                            ),
                        );
                        file
                    }
                };
                if description.is_none() && links::is_image(&url) {
                    return vec![image(url)];
                }
                Target::Url(url)
            }
            Link::CustomId(id) => Target::Id(id),
            Link::Headline(title) => match self.headlines.get(&links::normalize(&title)) {
                Some(&at) => Converter::headline_link(at),
                None => {
                    return self.unlinked(
                        path,
                        description,
                        offset,
                        "no headline on the page has its title",
                    );
                }
            },
            Link::Id(id) => match self.headline_ids.get(&id) {
                Some(&at) => Converter::headline_link(at),
                None => {
                    return self.unlinked(
                        path,
                        description,
                        offset,
                        "no headline on the page has its `ID`",
                    );
                }
            },
            Link::Other => {
                return self.unlinked(path, description, offset, "little can't follow it");
            }
            Link::Fuzzy(text) => {
                if self.targets.contains(&links::words(&text)) {
                    let content = description.unwrap_or_else(|| vec![Inline::Text(text.clone())]);
                    return to(Target::Id(links::id_of(&text)), Some(content));
                }
                match self.headlines.get(&links::normalize(&text)) {
                    Some(&at) => Converter::headline_link(at),
                    None => {
                        return self.unlinked(
                            path,
                            description,
                            offset,
                            "no target, name, or headline on the page matches it",
                        );
                    }
                }
            }
        };
        to(target, description)
    }

    /// The text of a link to `path`, with `description`, that leads
    /// nowhere little can go, `why`, warned of.
    fn unlinked(
        &mut self,
        path: &str,
        description: Option<Vec<Inline>>,
        offset: usize,
        why: &str,
    ) -> Vec<Inline> {
        self.warn(
            offset,
            format!("kept the link to `{path}` as text, as {why}"),
        );
        description.unwrap_or_else(|| vec![Inline::Text(path.to_string())])
    }
}

/// The image `url`, with the name of its file as its `alt`, as Org's HTML
/// export gives it.
fn image(url: String) -> Inline {
    Inline::Image {
        alt: links::file_name(&url).to_string(),
        url,
        title: String::new(),
    }
}

/// The image a link's description, `raw`, is, if it's a plain link alone,
/// to one, as Org shows an image that links somewhere.
fn image_alone(raw: &str) -> Option<Inline> {
    if links::plain_link(raw) != Some(raw.len()) {
        return None;
    }
    let url = match Link::of(raw) {
        Link::Web(url) if url.starts_with("http:") || url.starts_with("https:") => url,
        Link::File(file, None) => file,
        _ => return None,
    };
    links::is_image(&url).then(|| image(url))
}

impl Converter<'_> {
    /// The expansion of the macro `node`, `{{{name(arguments)}}}`, read as
    /// Org, as export reads it: one `#+MACRO:` defines, or one of Org's,
    /// `title`, `author`, `date`, `email`, `keyword`, or `results`, from
    /// the file's keywords; or else its text, warned of.
    fn expand(&mut self, node: &SyntaxNode) -> Vec<Inline> {
        let call = Macros::cast(node.clone()).expect("a macro");
        let written = node.to_string();
        let name = call.key().to_lowercase();
        let arguments = call.args().map(|args| arguments(&args)).unwrap_or_default();
        let template = match self.keywords.macros.get(&name) {
            Some(template) => template.clone(),
            None => match name.as_str() {
                "title" | "author" | "date" => self
                    .keywords
                    .joined(&name.to_ascii_uppercase())
                    .unwrap_or_default(),
                "email" => self.keywords.last("EMAIL").unwrap_or_default().to_string(),
                "keyword" => arguments
                    .first()
                    .and_then(|key| self.keywords.joined(&key.trim().to_ascii_uppercase()))
                    .unwrap_or_default(),
                "results" => "$1".to_string(),
                _ => {
                    let why = if UNEXPANDED.contains(&name.as_str()) {
                        format!("the plugin doesn't expand Org's macro `{name}`")
                    } else {
                        format!("the file doesn't define the macro `{name}`")
                    };
                    return self.unexpanded(node, written, &why);
                }
            },
        };
        if template.starts_with("(eval") {
            return self.unexpanded(
                node,
                written,
                "the macro runs code, which the plugin doesn't",
            );
        }
        if self.macro_depth >= MACRO_DEPTH {
            return self.unexpanded(node, written, "its expansion doesn't end");
        }
        let expansion = substitute(&template, &arguments);
        let line = self.line(start(node));
        self.macro_depth += 1;
        let inlines = self.snippet(&expansion, line);
        self.macro_depth -= 1;
        inlines
    }

    /// The macro `node`'s text, `written`, warned of, as it isn't expanded,
    /// as `why`.
    fn unexpanded(&mut self, node: &SyntaxNode, written: String, why: &str) -> Vec<Inline> {
        self.warn(
            start(node),
            format!("kept `{}` as text, as {why}", one_line(&written)),
        );
        vec![Inline::Text(unindent(&written))]
    }
}

/// The arguments of a macro, from `raw`, what's between its `()`, as
/// `org-macro-extract-arguments` reads them: split at each comma that
/// isn't escaped, with each run of whitespace a space, and each run of
/// backslashes before a comma halved, an odd one escaping it.
fn arguments(raw: &str) -> Vec<String> {
    let raw = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut arguments = vec![String::new()];
    let mut backslashes = 0;
    for c in raw.chars() {
        let argument = arguments.last_mut().expect("an argument");
        match c {
            '\\' => backslashes += 1,
            ',' => {
                argument.extend(std::iter::repeat_n('\\', backslashes / 2));
                if backslashes % 2 == 1 {
                    argument.push(',');
                } else {
                    arguments.push(String::new());
                }
                backslashes = 0;
            }
            _ => {
                argument.extend(std::iter::repeat_n('\\', backslashes));
                argument.push(c);
                backslashes = 0;
            }
        }
    }
    if let Some(argument) = arguments.last_mut() {
        argument.extend(std::iter::repeat_n('\\', backslashes));
    }
    arguments
}

/// `template` with each `$N` in it the `N`th of `arguments`, or nothing.
fn substitute(template: &str, arguments: &[String]) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let digits = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 {
            out.push('$');
        } else {
            // As Emacs's `nth` reads `$0`, as the first.
            let n = after[..digits].parse::<usize>().unwrap_or(usize::MAX);
            if let Some(argument) = arguments.get(n.saturating_sub(1)) {
                out.push_str(argument);
            }
        }
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

/// `text` on one line, with each run of whitespace a space, as a warning
/// shows what it's of.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
