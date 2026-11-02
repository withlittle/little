//! Leads each reference to where it goes, as docutils' transforms do once
//! the document is read: a name to its target, through indirect ones; an
//! anonymous reference to the anonymous target in its place; a footnote
//! reference to its footnote, numbered as docutils numbers them; and a
//! substitution reference to what it's replaced with. It writes the tree
//! as the markup library's blocks and inlines as it goes.

use std::collections::{HashMap, HashSet};

use little_markup::{Block, Definition, Footnote, Inline, Item, Table, Target};

use crate::state::{Dest, Kind, MAX_DEPTH, State};
use crate::tree::{Node, Ref, Span};

/// How many substitutions a page may replace in all, so substitutions that
/// each refer to another twice can't make it grow without end.
const MAX_SUBSTITUTIONS: usize = 10_000;

/// Give the ids of each internal target to the heading after it, as
/// docutils' `PropagateTargets` does, through a chain of targets and past
/// problems; or, where the chain ends at an external or indirect target,
/// lead it where that one leads. A target whose ids move is left out; any
/// other stays, an anchor in its place.
pub(crate) fn propagate(state: &mut State, nodes: &mut [Node]) {
    // The places of the internal targets in a chain, and their indexes.
    let mut chain: Vec<(usize, usize)> = Vec::new();
    for at in 0..nodes.len() {
        let moved = match &mut nodes[at] {
            Node::Target(index) if state.targets[*index].dest == Dest::Here => {
                chain.push((at, *index));
                continue;
            }
            Node::Target(index) => {
                let dest = state.targets[*index].dest.clone();
                for &(_, from) in &chain {
                    state.targets[from].dest = dest.clone();
                }
                true
            }
            Node::Problem(_) => continue,
            Node::Heading { ids, .. } => {
                ids.extend(
                    chain
                        .iter()
                        .map(|&(_, from)| state.targets[from].id.clone()),
                );
                true
            }
            node => {
                children(node, &mut |nodes| propagate(state, nodes));
                false
            }
        };
        for (from, _) in chain.drain(..) {
            if moved {
                nodes[from] = Node::Nothing { prebib: true };
            }
        }
    }
}

/// Calls `f` with each run of blocks in `node`.
fn children(node: &mut Node, f: &mut impl FnMut(&mut [Node])) {
    match node {
        Node::Quote { blocks, .. } | Node::Alert { blocks, .. } => f(blocks),
        Node::Group(nodes) => f(nodes),
        Node::List { items, .. } => items.iter_mut().for_each(|item| f(item)),
        Node::Definitions(items) => items.iter_mut().for_each(|(_, blocks)| f(blocks)),
        Node::Fields(fields) => fields.iter_mut().for_each(|field| f(&mut field.body)),
        Node::Table(table) => {
            let header = table.header.iter_mut();
            for row in header.chain(table.rows.iter_mut()) {
                row.iter_mut().for_each(|cell| f(cell));
            }
        }
        _ => {}
    }
}

/// Where a reference leads, once it's resolved.
enum Resolved {
    Url(String),
    Id(String),
    /// The footnote or citation with this index.
    Footnote(usize),
}

/// Leads references to their targets, and writes the tree as the markup
/// library's blocks and inlines.
pub(crate) struct Resolver<'s> {
    state: &'s mut State,
    /// How many levels of section the document's title and subtitle took
    /// up, which those under them move up.
    pub promoted: usize,
    /// The footnotes numbered automatically without a label, and those
    /// with symbols, in order: what `[#]_` and `[*]_` refer to.
    auto: Vec<usize>,
    symbols: Vec<usize>,
    /// The footnote numbered automatically without a label that has each
    /// number, which `[3]_` can refer to.
    numbers: HashMap<String, usize>,
    /// Whether the anonymous references match the anonymous targets.
    anonymous: bool,
    /// Whether a mismatch of anonymous references, of `[#]_`s, or of
    /// `[*]_`s, or too many substitutions, has been warned of.
    warned: [bool; 4],
    /// The substitutions being replaced, the innermost last, and how many
    /// have been.
    expanding: Vec<String>,
    substituted: usize,
    /// The footnotes referred to by what's been resolved since they were
    /// last taken.
    referred: Vec<usize>,
}

impl<'s> Resolver<'s> {
    /// A resolver for the document `state` knows of, with its footnotes
    /// numbered as docutils numbers them: each numbered automatically takes
    /// the next number that isn't a name already.
    pub fn new(state: &'s mut State) -> Resolver<'s> {
        let mut auto = Vec::new();
        let mut symbols = Vec::new();
        let mut numbers = HashMap::new();
        let mut next = 1usize;
        for (index, footnote) in state.footnotes.iter().enumerate() {
            match &footnote.kind {
                Kind::Auto(label) => {
                    let number = loop {
                        let number = next.to_string();
                        next += 1;
                        if !state.names.contains_key(&number) {
                            break number;
                        }
                    };
                    if label.is_none() {
                        auto.push(index);
                        numbers.insert(number, index);
                    }
                }
                Kind::Symbol => symbols.push(index),
                Kind::Manual(_) | Kind::Citation(_) => {}
            }
        }
        let anonymous = state.anonymous_refs == state.anonymous.len();
        Resolver {
            state,
            promoted: 0,
            auto,
            symbols,
            numbers,
            anonymous,
            warned: [false; 4],
            expanding: Vec::new(),
            substituted: 0,
            referred: Vec::new(),
        }
    }

    /// Warn of `message`, on `line`.
    pub fn warn(&mut self, line: usize, message: String) {
        self.state.warn(line, message);
    }

    /// `nodes`, as blocks.
    pub fn blocks(&mut self, nodes: Vec<Node>) -> Vec<Block> {
        let mut blocks = Vec::new();
        for node in nodes {
            self.block(node, &mut blocks);
        }
        blocks
    }

    fn block(&mut self, node: Node, out: &mut Vec<Block>) {
        match node {
            Node::Para(spans) => out.push(Block::Paragraph(self.inlines(spans))),
            Node::Heading { depth, title, ids } => {
                let level = (depth + 1).saturating_sub(self.promoted).max(1);
                out.push(Block::Heading {
                    level: u8::try_from(level).unwrap_or(u8::MAX),
                    content: self.inlines(title),
                    ids,
                });
            }
            Node::Code { language, text } => out.push(Block::Code { language, text }),
            Node::Literal(spans, line) => {
                let mut text = String::new();
                if flatten(&self.inlines(spans), &mut text) {
                    let message = "wrote `.. parsed-literal::` as code, leaving out its markup, \
                                   as a code block can't hold any";
                    self.warn(line, message.to_string());
                }
                out.push(Block::Code {
                    language: String::new(),
                    text,
                });
            }
            Node::Math(tex) => out.push(Block::Math(tex)),
            Node::Quote {
                blocks,
                attribution,
            } => {
                let blocks = self.blocks(blocks);
                let attribution = self.inlines(attribution);
                out.push(Block::Quote {
                    blocks,
                    attribution,
                });
            }
            Node::Alert {
                kind,
                title,
                blocks,
            } => {
                let blocks = self.blocks(blocks);
                out.push(Block::Alert {
                    kind,
                    title,
                    blocks,
                });
            }
            Node::List { start, items } => {
                let items = items
                    .into_iter()
                    .map(|blocks| Item {
                        checked: None,
                        blocks: self.blocks(blocks),
                    })
                    .collect();
                out.push(Block::List { start, items });
            }
            Node::Definitions(items) => {
                let items = items
                    .into_iter()
                    .map(|(term, blocks)| Definition {
                        term: self.inlines(term),
                        blocks: self.blocks(blocks),
                    })
                    .collect();
                out.push(Block::Definitions(items));
            }
            Node::Fields(fields) => {
                let items = fields
                    .into_iter()
                    .map(|field| Definition {
                        term: self.inlines(field.name),
                        blocks: self.blocks(field.body),
                    })
                    .collect();
                out.push(Block::Definitions(items));
            }
            Node::Table(table) => {
                let header = table.header.map(|row| self.row(row));
                let rows = table.rows.into_iter().map(|row| self.row(row)).collect();
                out.push(Block::Table(Table {
                    line: Some(table.line),
                    align: Vec::new(),
                    header,
                    rows,
                }));
            }
            Node::Rule => out.push(Block::Rule),
            Node::Html(html, line) => out.push(Block::Html {
                html,
                line: Some(line),
            }),
            Node::Target(index) => {
                let target = &self.state.targets[index];
                if target.dest == Dest::Here {
                    out.push(Block::Anchor(target.id.clone()));
                }
            }
            Node::Anchor(id) => out.push(Block::Anchor(id)),
            Node::Nothing { .. } => {}
            Node::Group(nodes) | Node::Problem(nodes) => {
                for node in nodes {
                    self.block(node, out);
                }
            }
        }
    }

    fn row(&mut self, row: Vec<Vec<Node>>) -> Vec<Vec<Block>> {
        row.into_iter().map(|cell| self.blocks(cell)).collect()
    }

    /// `spans`, as inlines.
    pub fn inlines(&mut self, spans: Vec<Span>) -> Vec<Inline> {
        let mut out = Vec::new();
        // Whether the substitution before takes the whitespace after it.
        let mut trim = false;
        for span in spans {
            match span {
                Span::Substitution { name, raw, line } => {
                    trim = self.substitute(&name, &raw, line, &mut out);
                    continue;
                }
                Span::Text(text) if trim => out.push(Inline::Text(text.trim_start().to_string())),
                span => self.inline(span, &mut out),
            }
            trim = false;
        }
        out
    }

    fn inline(&mut self, span: Span, out: &mut Vec<Inline>) {
        let inline = match span {
            Span::Text(text) => Inline::Text(text),
            Span::Emphasis(spans) => Inline::Emphasis(self.inlines(spans)),
            Span::Strong(spans) => Inline::Strong(self.inlines(spans)),
            Span::Code(code) => Inline::Code(code),
            Span::Math(tex) => Inline::Math(tex),
            Span::Sub(spans) => Inline::Subscript(self.inlines(spans)),
            Span::Sup(spans) => Inline::Superscript(self.inlines(spans)),
            Span::Html(html, line) => Inline::Html {
                html,
                line: Some(line),
            },
            Span::Break => Inline::LineBreak,
            Span::Image { url, alt } => Inline::Image {
                url,
                alt,
                title: String::new(),
            },
            Span::Anchor(id) => Inline::Anchor(id),
            Span::Link {
                content,
                to,
                raw,
                line,
            } => return self.link(content, to, raw, line, out),
            Span::Footnote { key, raw, line } => match self.footnote(&key, &raw) {
                Ok(index) => self.footnote_ref(index),
                Err(why) => {
                    if let Some(why) = why {
                        self.warn(line, format!("kept `{raw}` as text, as {why}"));
                    }
                    Inline::Text(raw)
                }
            },
            Span::Substitution { name, raw, line } => {
                self.substitute(&name, &raw, line, out);
                return;
            }
        };
        out.push(inline);
    }

    /// A reference, `raw`, on `line`, to `to`, that says `content`.
    fn link(
        &mut self,
        content: Vec<Span>,
        to: Ref,
        raw: String,
        line: usize,
        out: &mut Vec<Inline>,
    ) {
        let resolved = match to {
            Ref::Url(url) => Ok(Resolved::Url(url)),
            Ref::Name(name) => self.named(&name).and_then(|index| self.follow(index)),
            Ref::Anonymous(_) if !self.anonymous => {
                if !std::mem::replace(&mut self.warned[0], true) {
                    let (refs, targets) = (self.state.anonymous_refs, self.state.anonymous.len());
                    let message = format!(
                        "kept the anonymous references as text, as there are {refs} of them \
                         but {targets} anonymous targets"
                    );
                    self.warn(line, message);
                }
                out.push(Inline::Text(raw));
                return;
            }
            Ref::Anonymous(at) => match self.state.anonymous.get(at) {
                Some(&index) => self.follow(index),
                None => Err("it has no anonymous target".to_string()),
            },
        };
        let target = match resolved {
            Ok(Resolved::Url(url)) => Target::Url(url),
            Ok(Resolved::Id(id)) => Target::Id(id),
            Ok(Resolved::Footnote(index)) => {
                let content = self.inlines(content);
                out.extend(content);
                let reference = self.footnote_ref(index);
                out.push(reference);
                return;
            }
            Err(why) if raw.is_empty() => {
                self.warn(line, format!("kept an image without its link, as {why}"));
                let content = self.inlines(content);
                out.extend(content);
                return;
            }
            Err(why) => {
                self.warn(line, format!("kept `{raw}` as text, as {why}"));
                out.push(Inline::Text(raw));
                return;
            }
        };
        out.push(Inline::Link {
            target,
            title: String::new(),
            content: self.inlines(content),
        });
    }

    /// The target `name` leads to.
    fn named(&self, name: &str) -> Result<usize, String> {
        match self.state.names.get(name) {
            None => Err(format!("no target is named `{name}`")),
            Some(found) => found
                .target
                .ok_or_else(|| format!("more than one target is named `{name}`")),
        }
    }

    /// Where the target `index` leads, through any indirect targets.
    fn follow(&self, mut index: usize) -> Result<Resolved, String> {
        let mut seen = HashSet::new();
        loop {
            let target = &self.state.targets[index];
            match &target.dest {
                Dest::Url(url) => return Ok(Resolved::Url(url.clone())),
                Dest::Here => return Ok(Resolved::Id(target.id.clone())),
                Dest::Footnote(footnote) => return Ok(Resolved::Footnote(*footnote)),
                Dest::Name(name) => {
                    if !seen.insert(index) {
                        return Err("its targets lead to each other in a loop".to_string());
                    }
                    index = self.named(name)?;
                }
            }
        }
    }

    /// The footnote the reference with `key`, written `raw`, refers to;
    /// else why not, if it's to be warned of.
    fn footnote(&mut self, key: &str, raw: &str) -> Result<usize, Option<String>> {
        for (prefix, slot) in [("# ", 1), ("* ", 2)] {
            let Some(number) = key.strip_prefix(prefix) else {
                continue;
            };
            let list = if slot == 1 { &self.auto } else { &self.symbols };
            let at = number.parse::<usize>().unwrap_or(0).checked_sub(1);
            if let Some(&index) = at.and_then(|at| list.get(at)) {
                return Ok(index);
            }
            if std::mem::replace(&mut self.warned[slot], true) {
                return Err(None);
            }
            let marker = &prefix[..1];
            let why =
                format!("there are more `[{marker}]_` references than `[{marker}]` footnotes");
            return Err(Some(why));
        }
        let (name, what) = match key.strip_prefix("cite:") {
            Some(name) => (name, "citation"),
            None => (key.strip_prefix('#').unwrap_or(key), "footnote"),
        };
        let label = raw
            .strip_prefix('[')
            .and_then(|raw| raw.strip_suffix("]_"))
            .unwrap_or(raw);
        match self.state.names.get(name).map(|found| found.target) {
            Some(Some(target)) => match self.state.targets[target].dest {
                Dest::Footnote(index) => Ok(index),
                _ => Err(Some(format!("no {what} is labelled `{label}`"))),
            },
            Some(None) => Err(Some(format!("more than one target is named `{name}`"))),
            None => match self.numbers.get(name) {
                Some(&index) => Ok(index),
                None => Err(Some(format!("no {what} is labelled `{label}`"))),
            },
        }
    }

    /// A reference to the footnote `index`.
    fn footnote_ref(&mut self, index: usize) -> Inline {
        self.referred.push(index);
        Inline::Footnote((index + 1).to_string())
    }

    /// Replace the substitution reference `raw`, to `name`, on `line`, in
    /// `out`: whether what replaces it takes the whitespace after it.
    fn substitute(&mut self, name: &str, raw: &str, line: usize, out: &mut Vec<Inline>) -> bool {
        let key = if self.state.substitutions.contains_key(name) {
            Some(name.to_string())
        } else {
            self.state
                .substitution_names
                .get(&name.to_lowercase())
                .cloned()
        };
        let why = match &key {
            None => Some(format!("no substitution is named `{name}`")),
            Some(key) if self.expanding.contains(key) => {
                Some("its definition refers back to it".to_string())
            }
            Some(_) if self.expanding.len() >= MAX_DEPTH => Some(format!(
                "substitutions refer to each other over {MAX_DEPTH} deep"
            )),
            Some(_) if self.substituted >= MAX_SUBSTITUTIONS => {
                if std::mem::replace(&mut self.warned[3], true) {
                    out.push(Inline::Text(raw.to_string()));
                    return false;
                }
                Some(format!(
                    "the page replaces over {MAX_SUBSTITUTIONS} substitutions"
                ))
            }
            Some(_) => None,
        };
        let Some(key) = key.filter(|_| why.is_none()) else {
            let why = why.unwrap_or_default();
            self.warn(line, format!("kept `{raw}` as text, as {why}"));
            out.push(Inline::Text(raw.to_string()));
            return false;
        };
        self.substituted += 1;
        let substitution = self.state.substitutions[&key].clone();
        if substitution.ltrim
            && let Some(Inline::Text(text)) = out.last_mut()
        {
            text.truncate(text.trim_end().len());
        }
        self.expanding.push(key);
        let inlines = self.inlines(substitution.spans);
        self.expanding.pop();
        out.extend(inlines);
        substitution.rtrim
    }

    /// The footnotes that what's been resolved refers to, directly or
    /// through other footnotes, as Markdown writes only those; warning of
    /// each other, as it's left out.
    pub fn footnotes(&mut self) -> Vec<Footnote> {
        let mut stack = std::mem::take(&mut self.referred);
        let mut resolved = Vec::new();
        for index in 0..self.state.footnotes.len() {
            let nodes = std::mem::take(&mut self.state.footnotes[index].blocks);
            let blocks = self.blocks(nodes);
            resolved.push((blocks, std::mem::take(&mut self.referred)));
        }
        let mut reached = vec![false; resolved.len()];
        while let Some(index) = stack.pop() {
            if !std::mem::replace(&mut reached[index], true) {
                stack.extend(&resolved[index].1);
            }
        }
        let mut footnotes = Vec::new();
        for (index, (blocks, _)) in resolved.into_iter().enumerate() {
            if reached[index] {
                let label = (index + 1).to_string();
                footnotes.push(Footnote { label, blocks });
                continue;
            }
            let footnote = &self.state.footnotes[index];
            let what = match footnote.kind {
                Kind::Citation(_) => "citation",
                _ => "footnote",
            };
            let message = format!(
                "left out {what} `[{}]`, as nothing refers to it",
                footnote.label
            );
            let line = footnote.line;
            self.warn(line, message);
        }
        footnotes
    }
}

/// The text of `inlines`, added to `out`, without their markup: whether
/// they had any.
fn flatten(inlines: &[Inline], out: &mut String) -> bool {
    let mut marked = false;
    for inline in inlines {
        match inline {
            Inline::Text(text) => {
                out.push_str(text);
                continue;
            }
            Inline::LineBreak => {
                out.push('\n');
                continue;
            }
            Inline::Code(text) | Inline::Math(text) => out.push_str(text),
            Inline::Emphasis(content)
            | Inline::Strong(content)
            | Inline::Strikethrough(content)
            | Inline::Underline(content)
            | Inline::Mark(content)
            | Inline::Superscript(content)
            | Inline::Subscript(content)
            | Inline::Link { content, .. } => {
                flatten(content, out);
            }
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::Footnote(label) => {
                out.push('[');
                out.push_str(label);
                out.push(']');
            }
            Inline::Html { .. } | Inline::Anchor(_) => {}
        }
        marked = true;
    }
    marked
}
