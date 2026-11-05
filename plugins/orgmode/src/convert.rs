//! The converter: the tree orgize reads an Org file into, element by
//! element, into a [`Document`], keeping what Org's HTML export shows.

use std::collections::{HashMap, HashSet};

use little_markup::{
    AlertKind, Block, Definition, Document, Footnote, Inline, Item, Kind, Target, Warning,
    each_inlines, plain_text,
};
use orgize::ast::{ExportBlock, Headline, ListItem, SourceBlock};
use orgize::rowan::WalkEvent;
use orgize::rowan::ast::AstNode;
use orgize::{ParseConfig, SyntaxElement, SyntaxKind, SyntaxNode};

use crate::keywords::{self, HeaderArgs, Keywords, Options, keyword_parts};
use crate::links;

/// The keywords that belong to the element after them, and those that
/// mean `NAME`, `RESULTS`, and `HEADER`, as `org-element` reads them.
const AFFILIATED: [&str; 13] = [
    "CAPTION", "NAME", "RESULTS", "HEADER", "PLOT", "DATA", "LABEL", "RESNAME", "SOURCE",
    "SRCNAME", "TBLNAME", "RESULT", "HEADERS",
];

/// The keywords that name an element, which a fuzzy link leads to.
const NAMES: [&str; 7] = [
    "NAME", "DATA", "LABEL", "RESNAME", "SOURCE", "SRCNAME", "TBLNAME",
];

/// The Org file `text`, the page's body, after the `lines_before` lines of
/// its frontmatter, as a [`Document`], and what's lost on the way, by line.
pub fn convert(text: &str, lines_before: usize) -> (Document, Vec<Warning>) {
    let root = parse_with(&keywords::base_config(), text);
    let options = keywords::options(&root);
    let root = if options.changed {
        parse_with(&options.config, text)
    } else {
        root
    };
    let keywords = Keywords::new(&root);
    let mut converter = Converter {
        text,
        lines: Lines::new(text, lines_before),
        header_args: keywords.header_args.clone(),
        options,
        keywords,
        targets: HashSet::new(),
        headlines: HashMap::new(),
        headline_ids: HashMap::new(),
        headline_keys: HashMap::new(),
        headings: HashMap::new(),
        base: 0,
        fixed_line: None,
        footnotes: Vec::new(),
        defined: HashSet::new(),
        anonymous: 0,
        skip: HashSet::new(),
        warnings: Vec::new(),
        macro_depth: 0,
    };
    converter.collect(&root);
    converter.footnote_definitions(&root);
    let mut blocks = Vec::new();
    if let Some(title) = converter.keywords.joined("TITLE") {
        let line = converter.lines.line(converter.keywords.title_at);
        let content = converter.snippet(&title, line);
        if !content.is_empty() {
            blocks.push(Block::Heading {
                level: 1,
                content,
                ids: Vec::new(),
            });
        }
    }
    converter.sections(&root, Pending::default(), &mut blocks);
    let mut footnotes = std::mem::take(&mut converter.footnotes);
    converter.retarget(&mut blocks);
    for footnote in &mut footnotes {
        converter.retarget(&mut footnote.blocks);
    }
    let metadata = converter
        .keywords
        .joined("DESCRIPTION")
        .map(|description| ("description".to_string(), description))
        .into_iter()
        .collect();
    let mut warnings = converter.warnings;
    warnings.sort_by_key(|warning| (warning.line.is_none(), warning.line));
    let document = Document {
        frontmatter: String::new(),
        metadata,
        blocks,
        footnotes,
    };
    (document, warnings)
}

/// The tree orgize reads `text` into, with `config`.
fn parse_with(config: &ParseConfig, text: &str) -> SyntaxNode {
    config.clone().parse(text).document().syntax().clone()
}

/// Where each line of the page's body starts, to give a warning its line
/// in the file.
pub(crate) struct Lines {
    starts: Vec<usize>,
    before: usize,
}

impl Lines {
    fn new(text: &str, before: usize) -> Lines {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(index, _)| index + 1));
        Lines { starts, before }
    }

    /// The line of the file, from 1, that `offset` in the body is on.
    pub(crate) fn line(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset) + self.before
    }
}

/// The affiliated keywords before an element, which it takes.
#[derive(Default)]
pub(crate) struct Pending {
    /// Its `#+CAPTION:`s' values, and the line of the first.
    caption: Vec<String>,
    caption_line: usize,
    /// Its `#+NAME:`, which a fuzzy link leads to.
    name: Option<String>,
    /// The arguments in its `#+HEADER:`s, for a block of code.
    header: Vec<String>,
    /// Its `#+ATTR_HTML:`s' values.
    html: Vec<String>,
}

impl Pending {
    fn add(&mut self, key: &str, value: String, line: usize) {
        match key {
            "CAPTION" => {
                if self.caption.is_empty() {
                    self.caption_line = line;
                }
                self.caption.push(value);
            }
            "HEADER" | "HEADERS" => self.header.push(value),
            "ATTR_HTML" => self.html.push(value),
            _ if NAMES.contains(&key) => self.name = Some(value),
            _ => {}
        }
    }
}

/// Whether the keyword `key`, in capitals, belongs to the element after it.
fn is_affiliated(key: &str) -> bool {
    AFFILIATED.contains(&key) || key.starts_with("ATTR_")
}

/// Whether `node` ends in a blank line, so a keyword doesn't belong to the
/// element after it.
fn has_blank_line(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .any(|child| child.kind() == SyntaxKind::BLANK_LINE)
}

/// The number of blank lines `node` ends with.
fn blank_lines_after(node: &SyntaxNode) -> usize {
    let raw = node.to_string();
    let mut lines: Vec<&str> = raw.split('\n').collect();
    lines.pop();
    lines
        .iter()
        .rev()
        .take_while(|line| line.trim().is_empty())
        .count()
}

/// Where `node` starts, in the text it was read from.
pub(crate) fn start(node: &SyntaxNode) -> usize {
    node.text_range().start().into()
}

/// Where `node` ends, in the text it was read from.
pub(crate) fn end(node: &SyntaxNode) -> usize {
    node.text_range().end().into()
}

/// Whether Org's export leaves the headline out, with its subtree: one
/// that starts `COMMENT`, or is tagged `noexport`.
fn is_pruned(headline: &Headline) -> bool {
    let title = headline.title_raw();
    let title = title.trim_end();
    title == "COMMENT"
        || title.starts_with("COMMENT ")
        || title.starts_with("COMMENT\t")
        || headline.tags().any(|tag| tag == "noexport")
}

/// Whether the headline is Org's section of footnotes, which export reads
/// only for their definitions.
fn is_footnote_section(headline: &Headline) -> bool {
    headline.title_raw().trim() == "Footnotes"
}

/// The properties in the headline `node`'s drawer, each its name and value.
fn properties(node: &SyntaxNode) -> Vec<(String, String)> {
    node.children()
        .filter(|child| child.kind() == SyntaxKind::PROPERTY_DRAWER)
        .flat_map(|drawer| drawer.children())
        .filter(|child| child.kind() == SyntaxKind::NODE_PROPERTY)
        .filter_map(|property| {
            let raw = property.to_string();
            let line = raw.trim().strip_prefix(':')?;
            let (head, value) = match line.find([' ', '\t']) {
                Some(index) => (&line[..index], line[index..].trim()),
                None => (line, ""),
            };
            Some((head.strip_suffix(':')?.to_string(), value.to_string()))
        })
        .collect()
}

/// The raw text inside `node`, without its first and last children, its
/// delimiters.
pub(crate) fn inner_raw(node: &SyntaxNode) -> String {
    inner(node).iter().map(ToString::to_string).collect()
}

/// The children of `node`, but its first and last, its delimiters.
pub(crate) fn inner(node: &SyntaxNode) -> Vec<SyntaxElement> {
    let children: Vec<SyntaxElement> = node.children_with_tokens().collect();
    match children.len() {
        0..=2 => Vec::new(),
        length => children[1..length - 1].to_vec(),
    }
}

/// The text of the block `node`, without the commas that escape its lines.
fn block_value(node: &SyntaxNode) -> String {
    node.children()
        .filter(|child| child.kind() == SyntaxKind::BLOCK_CONTENT)
        .flat_map(|content| content.children_with_tokens())
        .filter_map(|child| child.into_token())
        .filter(|token| token.kind() == SyntaxKind::TEXT)
        .map(|token| token.text().to_string())
        .collect()
}

/// The name of a special block or a drawer, `node`.
fn block_name(node: &SyntaxNode) -> String {
    node.children()
        .find(|child| {
            matches!(
                child.kind(),
                SyntaxKind::BLOCK_BEGIN | SyntaxKind::DRAWER_BEGIN
            )
        })
        .into_iter()
        .flat_map(|begin| begin.children_with_tokens())
        .filter_map(|child| child.into_token())
        .filter(|token| token.kind() == SyntaxKind::TEXT)
        .map(|token| token.text().to_string())
        .find(|text| !text.starts_with("#+"))
        .unwrap_or_default()
}

/// `code` without the indentation all its lines share, as Org reads a
/// block of code, and without its last line break.
fn dedent(code: &str) -> String {
    let indent = code
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    let lines: Vec<&str> = code
        .lines()
        .map(|line| {
            let blank = line.len() - line.trim_start_matches([' ', '\t']).len();
            &line[blank.min(indent)..]
        })
        .collect();
    lines.join("\n")
}

/// The text of the fixed-width lines `node`, each without its `:` and the
/// space after it.
fn fixed_width(node: &SyntaxNode) -> String {
    let raw = node.to_string();
    let lines: Vec<&str> = raw
        .lines()
        .filter_map(|line| line.trim_start_matches([' ', '\t']).strip_prefix(':'))
        .map(|line| line.strip_prefix(' ').unwrap_or(line))
        .collect();
    lines.join("\n")
}

/// What reads an Org file into a [`Document`], and what it's found so far.
pub(crate) struct Converter<'a> {
    /// The page's body, which every offset is into.
    pub(crate) text: &'a str,
    pub(crate) lines: Lines,
    pub(crate) options: Options,
    pub(crate) keywords: Keywords,
    /// The `header-args` the current headline's blocks of code have.
    pub(crate) header_args: HeaderArgs,
    /// The text of each target, radio target, and name, as
    /// [`links::words`] gives it, which a fuzzy link leads to.
    pub(crate) targets: HashSet<String>,
    /// Where each headline starts, by its title, as [`links::normalize`]
    /// gives it, the first of those with the same title.
    pub(crate) headlines: HashMap<String, usize>,
    /// Where each headline with an `ID` property starts, by its `ID`.
    pub(crate) headline_ids: HashMap<String, usize>,
    /// The title of the headline that starts at each offset.
    pub(crate) headline_keys: HashMap<usize, String>,
    /// The text of the heading the headline at each offset is, and its
    /// `CUSTOM_ID`, as the headline's converted.
    pub(crate) headings: HashMap<usize, (String, Option<String>)>,
    /// Where the text being read starts in the body.
    pub(crate) base: usize,
    /// The line of what's being read, when it's not in the body, like a
    /// macro's expansion.
    pub(crate) fixed_line: Option<usize>,
    pub(crate) footnotes: Vec<Footnote>,
    /// The labels of the footnotes defined so far.
    pub(crate) defined: HashSet<String>,
    /// The number of anonymous footnotes so far, `[fn::text]`.
    pub(crate) anonymous: usize,
    /// Where the elements a footnote's definition takes start, in the body.
    pub(crate) skip: HashSet<usize>,
    pub(crate) warnings: Vec<Warning>,
    /// The number of macros being expanded, one in another.
    pub(crate) macro_depth: usize,
}

impl Converter<'_> {
    /// The line of the file that `offset`, in the text being read, is on.
    pub(crate) fn line(&self, offset: usize) -> usize {
        self.fixed_line
            .unwrap_or_else(|| self.lines.line(self.base + offset))
    }

    /// Warn of `message`, on the line `offset`, in the text being read, is on.
    pub(crate) fn warn(&mut self, offset: usize, message: impl Into<String>) {
        let line = self.line(offset);
        self.warnings.push(Warning::new(Some(line), message));
    }

    /// The tree orgize reads `text` into, with the file's options.
    pub(crate) fn parse(&self, text: &str) -> SyntaxNode {
        parse_with(&self.options.config, text)
    }

    /// The blocks of the body from `from` to `to`, read on their own, with
    /// `pending`, the affiliated keywords before them.
    fn sub_elements(&mut self, from: usize, to: usize, pending: Pending) -> Vec<Block> {
        let root = self.parse(&self.text[from..to]);
        let saved = std::mem::replace(&mut self.base, from);
        let mut blocks = Vec::new();
        self.sections(&root, pending, &mut blocks);
        self.base = saved;
        blocks
    }

    /// The inlines of `text`, which isn't in the body, like a caption, or a
    /// macro's expansion, as [`Converter::inlines_of`] gives them, with
    /// each warning on `line`.
    pub(crate) fn snippet(&mut self, text: &str, line: usize) -> Vec<Inline> {
        let saved = self.fixed_line.replace(line);
        let inlines = self.inlines_of(text);
        self.fixed_line = saved;
        inlines
    }

    /// The inlines of `text`, read on its own, as if it were at
    /// [`Converter::base`]: the objects of each paragraph it reads as, and
    /// the text of any other element, joined with spaces.
    fn inlines_of(&mut self, text: &str) -> Vec<Inline> {
        let root = self.parse(text);
        let mut inlines = Vec::new();
        let elements = root.children().flat_map(|child| match child.kind() {
            SyntaxKind::SECTION => child.children().collect(),
            _ => vec![child],
        });
        for element in elements {
            let piece = if element.kind() == SyntaxKind::PARAGRAPH {
                let children: Vec<SyntaxElement> = element
                    .children_with_tokens()
                    .filter(|child| child.kind() != SyntaxKind::BLANK_LINE)
                    .collect();
                self.objects(&children, true)
            } else {
                vec![Inline::Text(element.to_string().trim().to_string())]
            };
            if !inlines.is_empty() && !piece.is_empty() {
                inlines.push(Inline::Text(" ".to_string()));
            }
            inlines.extend(piece);
        }
        trim_inlines(&mut inlines);
        inlines
    }

    /// Find the targets, names, and headlines that links lead to.
    fn collect(&mut self, root: &SyntaxNode) {
        let mut walk = root.preorder();
        while let Some(event) = walk.next() {
            let WalkEvent::Enter(node) = event else {
                continue;
            };
            match node.kind() {
                SyntaxKind::HEADLINE => {
                    let headline = Headline::cast(node.clone()).expect("a headline");
                    if is_pruned(&headline) || is_footnote_section(&headline) {
                        walk.skip_subtree();
                        continue;
                    }
                    let offset = start(&node);
                    let key = links::normalize(&headline.title_raw());
                    self.headlines.entry(key.clone()).or_insert(offset);
                    self.headline_keys.insert(offset, key);
                    for (name, value) in properties(&node) {
                        if name.eq_ignore_ascii_case("ID") {
                            self.headline_ids.entry(value).or_insert(offset);
                        }
                    }
                }
                SyntaxKind::TARGET | SyntaxKind::RADIO_TARGET => {
                    self.targets.insert(links::words(&inner_raw(&node)));
                }
                SyntaxKind::KEYWORD | SyntaxKind::AFFILIATED_KEYWORD => {
                    let (key, value) = keyword_parts(&node);
                    let attached =
                        node.kind() == SyntaxKind::AFFILIATED_KEYWORD || !has_blank_line(&node);
                    if attached && NAMES.contains(&key.to_ascii_uppercase().as_str()) {
                        self.targets.insert(links::words(&value));
                    }
                }
                _ => {}
            }
        }
    }

    /// Read each footnote's definition, `[fn:label] text`, wherever it is,
    /// with the elements after it that it takes, as Org does: up to the
    /// next definition or headline, or two blank lines.
    fn footnote_definitions(&mut self, root: &SyntaxNode) {
        let mut definitions = Vec::new();
        let mut walk = root.preorder();
        while let Some(event) = walk.next() {
            let WalkEvent::Enter(node) = event else {
                continue;
            };
            match node.kind() {
                SyntaxKind::HEADLINE => {
                    let headline = Headline::cast(node.clone()).expect("a headline");
                    if is_pruned(&headline) {
                        walk.skip_subtree();
                    }
                }
                SyntaxKind::FN_DEF => {
                    definitions.push(node);
                    walk.skip_subtree();
                }
                _ => {}
            }
        }
        for node in definitions {
            let mut to = end(&node);
            let mut taken = vec![start(&node)];
            if blank_lines_after(&node) < 2 {
                let mut next = node.next_sibling();
                while let Some(sibling) = next {
                    if matches!(sibling.kind(), SyntaxKind::FN_DEF | SyntaxKind::HEADLINE) {
                        break;
                    }
                    to = end(&sibling);
                    taken.push(start(&sibling));
                    if blank_lines_after(&sibling) >= 2 {
                        break;
                    }
                    next = sibling.next_sibling();
                }
            }
            self.footnote_definition(&node, to);
            self.skip.extend(taken);
        }
    }

    /// Define the footnote whose definition starts with `node`, and runs
    /// to `to`.
    fn footnote_definition(&mut self, node: &SyntaxNode, to: usize) {
        let mut label = String::new();
        let mut from = end(node);
        let mut colon = false;
        for child in node.children_with_tokens() {
            match child.kind() {
                SyntaxKind::COLON => colon = true,
                SyntaxKind::TEXT if colon && label.is_empty() => label = child.to_string(),
                SyntaxKind::R_BRACKET => {
                    from = child.text_range().end().into();
                    break;
                }
                _ => {}
            }
        }
        let blocks = self.sub_elements(self.base + from, self.base + to, Pending::default());
        self.define(label, blocks, start(node));
    }

    /// Define the footnote `label` as `blocks`, unless it's defined.
    pub(crate) fn define(&mut self, label: String, blocks: Vec<Block>, offset: usize) {
        if self.defined.insert(label.clone()) {
            self.footnotes.push(Footnote { label, blocks });
        } else {
            self.warn(
                offset,
                format!("left out a second definition of footnote `{label}`"),
            );
        }
    }
}

impl Converter<'_> {
    /// Add the headline `node` to `blocks`, as a heading a level below its
    /// stars', since `#+TITLE:` is the first, with its section and the
    /// headlines under it, unless export leaves it out.
    fn headline(&mut self, node: &SyntaxNode, blocks: &mut Vec<Block>) {
        let headline = Headline::cast(node.clone()).expect("a headline");
        if is_pruned(&headline) || is_footnote_section(&headline) {
            return;
        }
        let saved = self.header_args.clone();
        let mut custom_id = None;
        for (name, value) in properties(node) {
            if name.eq_ignore_ascii_case("CUSTOM_ID") {
                custom_id = Some(value);
            } else if name.to_ascii_lowercase().starts_with("header-args") {
                self.header_args.set(&name, &value);
            }
        }
        let title: Vec<SyntaxElement> = headline.title().collect();
        let content = self.objects(&title, true);
        self.headings.insert(
            self.base + start(node),
            (plain_text(&content), custom_id.clone()),
        );
        blocks.push(Block::Heading {
            level: u8::try_from(headline.level() + 1).unwrap_or(u8::MAX),
            content,
            ids: custom_id.into_iter().collect(),
        });
        self.sections(node, Pending::default(), blocks);
        self.header_args = saved;
    }

    /// Add the blocks of the sections and headlines under `parent` to
    /// `blocks`, in order: a document's first section, then its headlines.
    /// The first section takes `pending`, the affiliated keywords before it.
    fn sections(&mut self, parent: &SyntaxNode, mut pending: Pending, blocks: &mut Vec<Block>) {
        for child in parent.children() {
            match child.kind() {
                SyntaxKind::SECTION => {
                    let nodes: Vec<SyntaxNode> = child.children().collect();
                    blocks.extend(self.elements(&nodes, std::mem::take(&mut pending)));
                }
                SyntaxKind::HEADLINE => self.headline(&child, blocks),
                _ => {}
            }
        }
    }

    /// The blocks of the elements `nodes`, the first with `pending`, the
    /// affiliated keywords before them.
    pub(crate) fn elements(&mut self, nodes: &[SyntaxNode], mut pending: Pending) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut index = 0;
        while index < nodes.len() {
            let node = &nodes[index];
            index += 1;
            if self.skip.contains(&(self.base + start(node))) {
                pending = Pending::default();
                continue;
            }
            if node.kind() == SyntaxKind::KEYWORD {
                let (key, value) = keyword_parts(node);
                let key = key.to_ascii_uppercase();
                if is_affiliated(&key) && !has_blank_line(node) {
                    pending.add(&key, value, self.line(start(node)));
                    continue;
                }
                pending = Pending::default();
                self.keyword(node, &key, value, nodes, &mut index, &mut blocks);
                continue;
            }
            let mut taken = std::mem::take(&mut pending);
            if let Some(name) = taken.name.take() {
                blocks.push(Block::Anchor(links::id_of(&name)));
            }
            self.element(node, nodes, &mut index, taken, &mut blocks);
        }
        blocks
    }

    /// Add what the keyword `node`, `#+KEY: value`, shows to `blocks`: the
    /// HTML of it and the `#+HTML:` lines after it, or nothing.
    fn keyword(
        &mut self,
        node: &SyntaxNode,
        key: &str,
        value: String,
        nodes: &[SyntaxNode],
        index: &mut usize,
        blocks: &mut Vec<Block>,
    ) {
        match key {
            "HTML" => {
                let mut html = value;
                let mut last = node;
                while !has_blank_line(last) {
                    match nodes.get(*index) {
                        Some(next)
                            if next.kind() == SyntaxKind::KEYWORD
                                && keyword_parts(next).0.eq_ignore_ascii_case("HTML") =>
                        {
                            html.push('\n');
                            html.push_str(&keyword_parts(next).1);
                            last = next;
                            *index += 1;
                        }
                        _ => break,
                    }
                }
                let line = self.line(start(node));
                blocks.push(Block::Html {
                    html,
                    line: Some(line),
                });
            }
            "INCLUDE" => self.warn(
                start(node),
                format!("left out `#+INCLUDE: {value}`, as the plugin doesn't include other files"),
            ),
            _ => {}
        }
    }

    /// The caption in `pending`, as an emphasised paragraph.
    fn caption(&mut self, pending: &Pending) -> Option<Block> {
        if pending.caption.is_empty() {
            return None;
        }
        let content = self.snippet(&pending.caption.join(" "), pending.caption_line);
        (!content.is_empty())
            .then(|| Block::Paragraph(vec![Inline::Styled(Kind::Emphasis, content)]))
    }

    /// The blocks of the elements in the block or drawer `node`.
    fn contents(&mut self, node: &SyntaxNode) -> Vec<Block> {
        let nodes: Vec<SyntaxNode> = node
            .children()
            .filter(|child| {
                matches!(
                    child.kind(),
                    SyntaxKind::BLOCK_CONTENT | SyntaxKind::DRAWER_CONTENT
                )
            })
            .flat_map(|content| content.children())
            .collect();
        self.elements(&nodes, Pending::default())
    }
}

impl Converter<'_> {
    /// Add the blocks the element `node`, the one before `nodes[index]`,
    /// shows to `blocks`, with `pending`, its affiliated keywords.
    fn element(
        &mut self,
        node: &SyntaxNode,
        nodes: &[SyntaxNode],
        index: &mut usize,
        pending: Pending,
        blocks: &mut Vec<Block>,
    ) {
        let offset = start(node);
        match node.kind() {
            SyntaxKind::PARAGRAPH => self.paragraph(node, pending, blocks),
            SyntaxKind::SOURCE_BLOCK => self.source_block(node, nodes, index, pending, blocks),
            SyntaxKind::EXAMPLE_BLOCK | SyntaxKind::FIXED_WIDTH | SyntaxKind::TABLE_EL => {
                let text = match node.kind() {
                    SyntaxKind::EXAMPLE_BLOCK => dedent(&block_value(node)),
                    SyntaxKind::FIXED_WIDTH => fixed_width(node),
                    _ => {
                        self.warn(
                            offset,
                            "kept a table.el table as code, as the plugin reads only Org's own \
                             tables",
                        );
                        own_text(node).trim_end().to_string()
                    }
                };
                blocks.extend(self.caption(&pending));
                blocks.push(Block::Code {
                    language: String::new(),
                    text,
                });
            }
            SyntaxKind::QUOTE_BLOCK => {
                let inner = self.contents(node);
                blocks.push(Block::Quote {
                    blocks: inner,
                    attribution: Vec::new(),
                });
            }
            SyntaxKind::CENTER_BLOCK => blocks.extend(self.contents(node)),
            SyntaxKind::VERSE_BLOCK => self.verse(node, blocks),
            SyntaxKind::SPECIAL_BLOCK => {
                let kind = match block_name(node).to_ascii_lowercase().as_str() {
                    "note" => Some(AlertKind::Note),
                    "tip" | "hint" => Some(AlertKind::Tip),
                    "important" => Some(AlertKind::Important),
                    "warning" => Some(AlertKind::Warning),
                    "caution" | "danger" | "error" => Some(AlertKind::Caution),
                    _ => None,
                };
                let inner = self.contents(node);
                match kind {
                    Some(kind) => blocks.push(Block::Alert {
                        kind,
                        title: String::new(),
                        blocks: inner,
                    }),
                    None => blocks.extend(inner),
                }
            }
            SyntaxKind::EXPORT_BLOCK => {
                let block = ExportBlock::cast(node.clone()).expect("an export block");
                if block.ty().is_some_and(|ty| ty.eq_ignore_ascii_case("html")) {
                    let html = block.value().trim_end_matches(['\n', '\r']).to_string();
                    let line = self.line(offset);
                    blocks.push(Block::Html {
                        html,
                        line: Some(line),
                    });
                }
            }
            SyntaxKind::DRAWER => {
                if block_name(node).eq_ignore_ascii_case("RESULTS") {
                    blocks.extend(self.contents(node));
                }
            }
            SyntaxKind::BABEL_CALL => {
                let raw = node.to_string();
                let call = raw.split_once(':').map_or("", |(_, call)| call).trim();
                self.warn(
                    offset,
                    format!("left out `#+CALL: {call}`, as the plugin doesn't run code"),
                );
            }
            SyntaxKind::COMMENT
            | SyntaxKind::COMMENT_BLOCK
            | SyntaxKind::PROPERTY_DRAWER
            | SyntaxKind::PLANNING
            | SyntaxKind::CLOCK => {}
            SyntaxKind::RULE => blocks.push(Block::Rule),
            SyntaxKind::LATEX_ENVIRONMENT => {
                blocks.push(Block::Math(own_text(node).trim().to_string()))
            }
            SyntaxKind::LIST => self.list(node, blocks),
            SyntaxKind::ORG_TABLE => {
                blocks.extend(self.caption(&pending));
                let table = self.table(node);
                blocks.push(table);
            }
            SyntaxKind::FN_DEF => self.footnote_definition(node, end(node)),
            _ => {
                let raw = node.to_string().trim().to_string();
                if !raw.is_empty() {
                    blocks.push(Block::Paragraph(vec![Inline::Text(raw)]));
                }
            }
        }
    }

    /// Add what the block of code `node` shows to `blocks`, as its
    /// `:exports` says: its code, its saved results, both, or neither; and
    /// take its results from `nodes`, after it.
    fn source_block(
        &mut self,
        node: &SyntaxNode,
        nodes: &[SyntaxNode],
        index: &mut usize,
        pending: Pending,
        blocks: &mut Vec<Block>,
    ) {
        let block = SourceBlock::cast(node.clone()).expect("a block of code");
        let language = block.language().map(|l| l.to_string()).unwrap_or_default();
        let mut args = self.header_args.of(&language);
        for header in &pending.header {
            args.push(' ');
            args.push_str(header);
        }
        if let Some(parameters) = block.parameters() {
            args.push(' ');
            args.push_str(&parameters);
        }
        let exports = keywords::exports(&args);
        let shows_results = matches!(exports.as_str(), "results" | "both");
        let shows_code = !matches!(exports.as_str(), "results" | "none");
        let mut results = None;
        let is_results = |node: &SyntaxNode| {
            node.kind() == SyntaxKind::KEYWORD
                && matches!(
                    keyword_parts(node).0.to_ascii_uppercase().as_str(),
                    "RESULTS" | "RESULT"
                )
        };
        if let Some(keyword) = nodes.get(*index).filter(|next| is_results(next)) {
            let from = *index + 1;
            let mut to = from;
            if !has_blank_line(keyword) {
                while let Some(next) = nodes.get(to) {
                    to += 1;
                    let affiliated = next.kind() == SyntaxKind::KEYWORD
                        && is_affiliated(&keyword_parts(next).0.to_ascii_uppercase())
                        && !has_blank_line(next);
                    if !affiliated {
                        break;
                    }
                }
            }
            *index = to;
            if shows_results {
                results = Some(self.elements(&nodes[from..to], Pending::default()));
            }
        }
        if shows_code {
            blocks.extend(self.caption(&pending));
            blocks.push(Block::Code {
                language,
                text: dedent(&block.value()),
            });
        }
        if shows_results {
            match results {
                Some(results) => blocks.extend(results),
                None => self.warn(
                    start(node),
                    "left out the block's results, as none are saved, and the plugin doesn't run \
                     code",
                ),
            }
        }
    }
}

impl Converter<'_> {
    /// Add the paragraph `node` to `blocks`, with `pending`, its affiliated
    /// keywords: its text, with each formula on its own, `\[…\]` or
    /// `$$…$$`, a block between paragraphs, as Markdown has no other place
    /// for one, and its caption after it, if it's an image alone.
    fn paragraph(&mut self, node: &SyntaxNode, mut pending: Pending, blocks: &mut Vec<Block>) {
        let affiliated: Vec<SyntaxNode> = node
            .children()
            .filter(|child| child.kind() == SyntaxKind::AFFILIATED_KEYWORD)
            .collect();
        // orgize leaves the text after `#+ATTR_…:` lines unread, so it's
        // read again on its own.
        if let Some(last) = affiliated.last() {
            for keyword in &affiliated {
                let (key, value) = keyword_parts(keyword);
                let line = self.line(start(keyword));
                pending.add(&key.to_ascii_uppercase(), value, line);
            }
            let (from, to) = (self.base + end(last), self.base + end(node));
            blocks.extend(self.sub_elements(from, to, pending));
            return;
        }
        let mut out = Vec::new();
        let mut run = Vec::new();
        for child in node.children_with_tokens() {
            if child.kind() == SyntaxKind::BLANK_LINE {
                continue;
            }
            match child.as_node().and_then(display_math) {
                Some(tex) => {
                    self.push_paragraph(&mut run, &mut out);
                    out.push(Block::Math(tex));
                }
                None => run.push(child),
            }
        }
        self.push_paragraph(&mut run, &mut out);
        if let Some(alt) = alt(&pending.html) {
            set_alt(&mut out, alt);
        }
        let image =
            matches!(out.as_slice(), [Block::Paragraph(inlines)] if is_image_alone(inlines));
        blocks.extend(out);
        if image {
            blocks.extend(self.caption(&pending));
        }
    }

    /// Add the paragraph of the objects `run` to `out`, unless it shows
    /// nothing, and empty `run`.
    fn push_paragraph(&mut self, run: &mut Vec<SyntaxElement>, out: &mut Vec<Block>) {
        if run.is_empty() {
            return;
        }
        let inlines = self.objects(run, true);
        run.clear();
        let shown = inlines.iter().any(|inline| match inline {
            Inline::Text(text) => !text.trim().is_empty(),
            Inline::LineBreak => false,
            _ => true,
        });
        if shown {
            out.push(Block::Paragraph(inlines));
        }
    }

    /// Add the list `node` to `blocks`: numbered, if its first item is,
    /// from its counter, `[@N]`; terms and what they mean, if its first
    /// item has a term, `- term :: text`; or else with bullets.
    fn list(&mut self, node: &SyntaxNode, blocks: &mut Vec<Block>) {
        let items: Vec<ListItem> = node.children().filter_map(ListItem::cast).collect();
        let Some(first) = items.first() else {
            return;
        };
        let ordered = first.bullet().starts_with(|c: char| c.is_ascii_digit());
        let descriptive = !ordered && tag_of(first).is_some();
        let number = first
            .counter()
            .and_then(|counter| counter.parse::<u64>().ok())
            .unwrap_or(1);
        let mut list = Vec::new();
        let mut definitions = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if ordered
                && index > 0
                && let Some(counter) = item.counter()
                && counter.parse::<u64>().ok() != number.checked_add(index as u64)
            {
                self.warn(
                    start(item.syntax()),
                    format!(
                        "left out the counter `[@{}]`, as Markdown numbers a list only from its \
                         start",
                        &*counter
                    ),
                );
            }
            let checkbox = item.checkbox().map(|checkbox| checkbox.to_string());
            let tag = tag_of(item).map(|tag| {
                let elements: Vec<SyntaxElement> = tag
                    .children_with_tokens()
                    .filter(|child| child.kind() != SyntaxKind::COLON2)
                    .collect();
                let mut inlines = self.objects(&elements, true);
                trim_inlines(&mut inlines);
                inlines
            });
            let content: Vec<SyntaxNode> = item
                .syntax()
                .children()
                .filter(|child| child.kind() == SyntaxKind::LIST_ITEM_CONTENT)
                .flat_map(|content| content.children())
                .collect();
            let mut inner = self.elements(&content, Pending::default());
            if descriptive {
                let mut term = Vec::new();
                if let Some(checkbox) = checkbox {
                    term.push(Inline::Text(format!("[{checkbox}] ")));
                }
                term.extend(tag.unwrap_or_else(|| vec![Inline::Text("(no term)".to_string())]));
                definitions.push(Definition {
                    term,
                    blocks: inner,
                });
                continue;
            }
            // A term in a list that isn't of terms stays in its item, as
            // orgize keeps one in a numbered list.
            if let Some(mut lead) = tag {
                lead.push(Inline::Text(" :: ".to_string()));
                match inner.first_mut() {
                    Some(Block::Paragraph(inlines)) => {
                        trim_inlines(inlines);
                        lead.append(inlines);
                        *inlines = lead;
                    }
                    _ => inner.insert(0, Block::Paragraph(lead)),
                }
            }
            list.push(Item {
                checked: checkbox.map(|checkbox| checkbox == "X"),
                blocks: inner,
            });
        }
        blocks.push(if descriptive {
            Block::Definitions(definitions)
        } else {
            Block::List {
                start: ordered.then_some(number),
                items: list,
            }
        });
    }

    /// Add the verse block `node` to `blocks`: a paragraph for each stanza,
    /// with a line break after each of its lines, and each line's
    /// indentation kept, as spaces that don't break.
    fn verse(&mut self, node: &SyntaxNode, blocks: &mut Vec<Block>) {
        let Some(content) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::BLOCK_CONTENT)
        else {
            return;
        };
        let raw = content.to_string();
        let (saved, from) = (self.base, self.base + start(&content));
        let mut stanza = Vec::new();
        let mut offset = 0;
        for line in raw.split_inclusive('\n') {
            let at = offset;
            offset += line.len();
            let text = line.trim_end_matches(['\n', '\r']);
            if text.trim().is_empty() {
                if !stanza.is_empty() {
                    blocks.push(Block::Paragraph(std::mem::take(&mut stanza)));
                }
                continue;
            }
            let indent = text.len() - text.trim_start_matches([' ', '\t']).len();
            if !stanza.is_empty() {
                stanza.push(Inline::LineBreak);
            }
            if indent > 0 {
                stanza.push(Inline::Text("\u{a0}".repeat(indent)));
            }
            self.base = from + at + indent;
            let inlines = self.inlines_of(&text[indent..]);
            stanza.extend(inlines);
        }
        self.base = saved;
        if !stanza.is_empty() {
            blocks.push(Block::Paragraph(stanza));
        }
    }
}

impl Converter<'_> {
    /// Lead each link to a headline in `blocks`, whose target is the
    /// placeholder [`Converter::headline_link`] gave it, to its heading, by
    /// its `CUSTOM_ID`, or else its text.
    fn retarget(&self, blocks: &mut [Block]) {
        each_inlines(blocks, &mut |inlines| self.retarget_inlines(inlines));
    }

    fn retarget_inlines(&self, inlines: &mut [Inline]) {
        for inline in inlines {
            match inline {
                Inline::Link {
                    target, content, ..
                } => {
                    if let Target::Id(id) = target
                        && let Some(offset) = id.strip_prefix('\u{0}')
                        && let Ok(offset) = offset.parse::<usize>()
                    {
                        *target = match self.headings.get(&offset) {
                            Some((_, Some(id))) => Target::Id(id.clone()),
                            Some((text, None)) => Target::Heading(text.clone()),
                            None => Target::Heading(
                                self.headline_keys.get(&offset).cloned().unwrap_or_default(),
                            ),
                        };
                    }
                    self.retarget_inlines(content);
                }
                Inline::Styled(_, children) => self.retarget_inlines(children),
                _ => {}
            }
        }
    }

    /// The target of a link to the headline at `offset` in the body, a
    /// placeholder until every headline's heading is known.
    pub(crate) fn headline_link(offset: usize) -> Target {
        Target::Id(format!("\u{0}{offset}"))
    }
}

/// The TeX of the formula `node`, if it's one on its own, `\[…\]` or
/// `$$…$$`.
fn display_math(node: &SyntaxNode) -> Option<String> {
    if node.kind() != SyntaxKind::LATEX_FRAGMENT {
        return None;
    }
    let raw = node.to_string();
    let inner = raw
        .strip_prefix("\\[")
        .and_then(|rest| rest.strip_suffix("\\]"))
        .or_else(|| raw.strip_prefix("$$")?.strip_suffix("$$"))?;
    Some(inner.trim().to_string())
}

/// The `:alt` of an image, from the values of its paragraph's
/// `#+ATTR_HTML:`s, as `org-export-read-attribute` reads them, the last
/// line's first.
fn alt(lines: &[String]) -> Option<String> {
    let words: Vec<&str> = lines
        .iter()
        .rev()
        .flat_map(|line| line.split_whitespace())
        .collect();
    let is_key = |word: &str| {
        word.strip_prefix(':').is_some_and(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
    };
    let at = words.iter().position(|word| *word == ":alt")?;
    let value: Vec<&str> = words[at + 1..]
        .iter()
        .take_while(|word| !is_key(word))
        .copied()
        .collect();
    let value = value.join(" ");
    let value = match value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        Some(quoted) => quoted.to_string(),
        None => value,
    };
    (!value.is_empty()).then_some(value)
}

/// Give `alt` to the first image in `blocks`, if the first link or image
/// in them is one, or is a link around one, as Org's HTML export gives a
/// paragraph's `#+ATTR_HTML:` to its first link.
fn set_alt(blocks: &mut [Block], alt: String) {
    fn first(inlines: &mut [Inline]) -> Option<&mut Inline> {
        for inline in inlines {
            match inline {
                Inline::Link { .. } | Inline::Image { .. } => return Some(inline),
                Inline::Styled(_, children) => {
                    if let Some(found) = first(children) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }
    let found = blocks.iter_mut().find_map(|block| match block {
        Block::Paragraph(inlines) => first(inlines),
        _ => None,
    });
    let image = match found {
        Some(Inline::Link { content, .. }) => content
            .iter_mut()
            .find(|inline| matches!(inline, Inline::Image { .. })),
        found => found,
    };
    if let Some(Inline::Image { alt: old, .. }) = image {
        *old = alt;
    }
}

/// Whether `inlines` are an image alone, or a link around one, but for
/// spaces, as Org's HTML export finds a figure.
fn is_image_alone(inlines: &[Inline]) -> bool {
    let shown = |inline: &&Inline| !matches!(inline, Inline::Text(text) if text.trim().is_empty());
    let shown: Vec<&Inline> = inlines.iter().filter(shown).collect();
    match shown.as_slice() {
        [Inline::Image { .. }] => true,
        [Inline::Link { content, .. }] => is_image_alone(content),
        _ => false,
    }
}

/// The term of the list item `item`, `- term :: text`, if it has one.
fn tag_of(item: &ListItem) -> Option<SyntaxNode> {
    item.syntax()
        .children()
        .find(|child| child.kind() == SyntaxKind::LIST_ITEM_TAG)
}

/// The text of `node`, without the affiliated keywords before it, or the
/// blank lines after it.
fn own_text(node: &SyntaxNode) -> String {
    node.children_with_tokens()
        .filter(|child| {
            !matches!(
                child.kind(),
                SyntaxKind::AFFILIATED_KEYWORD | SyntaxKind::BLANK_LINE
            )
        })
        .map(|child| child.to_string())
        .collect()
}

/// `inlines` without the spaces at either end.
pub(crate) fn trim_inlines(inlines: &mut Vec<Inline>) {
    while let Some(Inline::Text(text)) = inlines.first_mut() {
        let trimmed = text.trim_start().to_string();
        if !trimmed.is_empty() {
            *text = trimmed;
            break;
        }
        inlines.remove(0);
    }
    while let Some(Inline::Text(text)) = inlines.last_mut() {
        let trimmed = text.trim_end().to_string();
        if !trimmed.is_empty() {
            *text = trimmed;
            break;
        }
        inlines.pop();
    }
}
