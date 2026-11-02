//! Readies a document to be written: drops what the site's settings leave
//! out, with a warning where something is lost, writes what they don't turn
//! on as something they do, and tidies its text, so each run of it can be
//! written as Markdown that reads back as it was.

use crate::Warnings;
use crate::model::{AlertKind, Block, Cell, Definition, Document, Inline, Item, Table, Target};
use crate::settings::Settings;
use std::collections::HashSet;

/// A kind of text set between delimiters, like emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Emphasis,
    Strong,
    Strikethrough,
    Underline,
    Mark,
    Superscript,
    Subscript,
}

impl Kind {
    /// The kind of `inline`, and what it holds, if it's set between
    /// delimiters.
    pub fn of(inline: &Inline) -> Option<(Kind, &[Inline])> {
        Some(match inline {
            Inline::Emphasis(c) => (Kind::Emphasis, c),
            Inline::Strong(c) => (Kind::Strong, c),
            Inline::Strikethrough(c) => (Kind::Strikethrough, c),
            Inline::Underline(c) => (Kind::Underline, c),
            Inline::Mark(c) => (Kind::Mark, c),
            Inline::Superscript(c) => (Kind::Superscript, c),
            Inline::Subscript(c) => (Kind::Subscript, c),
            _ => return None,
        })
    }

    pub fn split(inline: Inline) -> Result<(Kind, Vec<Inline>), Inline> {
        Ok(match inline {
            Inline::Emphasis(c) => (Kind::Emphasis, c),
            Inline::Strong(c) => (Kind::Strong, c),
            Inline::Strikethrough(c) => (Kind::Strikethrough, c),
            Inline::Underline(c) => (Kind::Underline, c),
            Inline::Mark(c) => (Kind::Mark, c),
            Inline::Superscript(c) => (Kind::Superscript, c),
            Inline::Subscript(c) => (Kind::Subscript, c),
            other => return Err(other),
        })
    }

    pub fn children_mut(inline: &mut Inline) -> Option<&mut Vec<Inline>> {
        match inline {
            Inline::Emphasis(c)
            | Inline::Strong(c)
            | Inline::Strikethrough(c)
            | Inline::Underline(c)
            | Inline::Mark(c)
            | Inline::Superscript(c)
            | Inline::Subscript(c) => Some(c),
            _ => None,
        }
    }

    pub fn wrap(self, children: Vec<Inline>) -> Inline {
        match self {
            Kind::Emphasis => Inline::Emphasis(children),
            Kind::Strong => Inline::Strong(children),
            Kind::Strikethrough => Inline::Strikethrough(children),
            Kind::Underline => Inline::Underline(children),
            Kind::Mark => Inline::Mark(children),
            Kind::Superscript => Inline::Superscript(children),
            Kind::Subscript => Inline::Subscript(children),
        }
    }

    /// What it's called in a warning.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Emphasis => "emphasis",
            Kind::Strong => "strong emphasis",
            Kind::Strikethrough => "strikethrough",
            Kind::Underline => "underlining",
            Kind::Mark => "highlighting",
            Kind::Superscript => "superscript",
            Kind::Subscript => "subscript",
        }
    }

    /// The setting that writes it in Markdown, if one does, but for emphasis.
    fn setting(self) -> Option<&'static str> {
        match self {
            Kind::Strikethrough => Some("strikethrough"),
            Kind::Mark => Some("mark"),
            Kind::Superscript => Some("superscript"),
            Kind::Subscript => Some("subscript"),
            Kind::Emphasis | Kind::Strong | Kind::Underline => None,
        }
    }

    fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// The bit of a link, among those of the kinds a run of text is within.
const LINK: u16 = 1 << 15;

pub(crate) struct Normalizer<'a> {
    settings: &'a Settings,
    defined: HashSet<String>,
    warnings: &'a mut Warnings,
    /// The anchors and footnotes in the link being read, which go before
    /// and after it, as a link can hold neither.
    hoisted: (Vec<Inline>, Vec<Inline>),
}

impl<'a> Normalizer<'a> {
    pub fn new(settings: &'a Settings, document: &Document, warnings: &'a mut Warnings) -> Self {
        Normalizer {
            settings,
            defined: document.footnotes.iter().map(|f| f.label.clone()).collect(),
            warnings,
            hoisted: Default::default(),
        }
    }

    pub fn document(&mut self, document: &mut Document) {
        document.blocks = self.blocks(std::mem::take(&mut document.blocks));
        for footnote in &mut document.footnotes {
            footnote.blocks = self.blocks(std::mem::take(&mut footnote.blocks));
        }
    }

    fn blocks(&mut self, blocks: Vec<Block>) -> Vec<Block> {
        let mut out = Vec::new();
        for block in blocks {
            self.block(block, &mut out);
        }
        out
    }

    fn block(&mut self, block: Block, out: &mut Vec<Block>) {
        let settings = self.settings;
        match block {
            Block::Paragraph(inlines) => {
                let inlines = self.inlines(inlines);
                if inlines.iter().all(|i| matches!(i, Inline::Anchor(_))) {
                    out.extend(inlines.into_iter().filter_map(|i| match i {
                        Inline::Anchor(id) => Some(Block::Anchor(id)),
                        _ => None,
                    }));
                } else {
                    out.push(Block::Paragraph(inlines));
                }
            }
            Block::Heading {
                level,
                content,
                ids,
            } => out.push(Block::Heading {
                level: level.clamp(1, 6),
                content: self.inlines(content),
                ids,
            }),
            Block::Quote {
                blocks,
                attribution,
            } => out.push(Block::Quote {
                blocks: self.blocks(blocks),
                attribution: self.inlines(attribution),
            }),
            Block::Alert {
                kind,
                title,
                blocks,
            } => out.push(self.alert(kind, &title, blocks)),
            Block::List { start, items } => out.push(Block::List {
                start,
                items: items
                    .into_iter()
                    .map(|item| Item {
                        checked: item.checked,
                        blocks: self.blocks(item.blocks),
                    })
                    .collect(),
            }),
            Block::Definitions(definitions) => out.push(self.definitions(definitions)),
            Block::Table(table) => out.push(self.table(table)),
            Block::Html { html, line } => {
                if settings.raw_html {
                    out.push(Block::Html { html, line });
                } else {
                    self.warnings
                        .push(line, "left out raw HTML, as `raw_html` is off");
                }
            }
            Block::Math(tex) => {
                if !tex.trim().is_empty() {
                    out.push(Block::Math(tex));
                }
            }
            Block::Code { .. } | Block::Rule | Block::Anchor(_) => out.push(block),
        }
    }

    /// An alert, or, where `alerts` is off, a quote that starts with its
    /// title in bold.
    fn alert(&mut self, kind: AlertKind, title: &str, blocks: Vec<Block>) -> Block {
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut blocks = self.blocks(blocks);
        if self.settings.alerts {
            return Block::Alert {
                kind,
                title,
                blocks,
            };
        }
        let title = if title.is_empty() {
            kind.title().to_string()
        } else {
            title
        };
        blocks.insert(
            0,
            Block::Paragraph(vec![Inline::Strong(vec![Inline::Text(title)])]),
        );
        Block::Quote {
            blocks,
            attribution: Vec::new(),
        }
    }

    /// Terms and what they mean, or, where Markdown can't hold them, a list
    /// with each term in bold, and what it means below it.
    fn definitions(&mut self, definitions: Vec<Definition>) -> Block {
        let definitions: Vec<Definition> = definitions
            .into_iter()
            .map(|d| Definition {
                term: self.inlines(d.term),
                blocks: self.blocks(d.blocks),
            })
            .collect();
        let whole = definitions
            .iter()
            .all(|d| !d.term.is_empty() && !d.blocks.is_empty());
        if (self.settings.description_lists && whole) || self.settings.raw_html {
            return Block::Definitions(definitions);
        }
        let items = definitions
            .into_iter()
            .map(|d| {
                let mut blocks = Vec::new();
                if !d.term.is_empty() {
                    blocks.push(Block::Paragraph(self.inlines(vec![Inline::Strong(d.term)])));
                }
                blocks.extend(d.blocks);
                Item {
                    checked: None,
                    blocks,
                }
            })
            .collect();
        Block::List { start: None, items }
    }

    /// A table whose cells hold a line each, or, where `tables` is off, a
    /// list of its rows.
    fn table(&mut self, table: Table) -> Block {
        let mut flattened = false;
        let header = table.header.map(|row| self.row(row, &mut flattened));
        let rows: Vec<Vec<Cell>> = table
            .rows
            .into_iter()
            .map(|row| self.row(row, &mut flattened))
            .collect();
        if flattened {
            self.warnings.push(
                table.line,
                "wrote each cell of a table on a line, as a cell in Markdown holds no more",
            );
        }
        if self.settings.tables {
            return Block::Table(Table {
                line: table.line,
                align: table.align,
                header,
                rows,
            });
        }
        self.warnings
            .push(None, "wrote tables as lists, as `tables` is off");
        let header = header.map(|row| (row, true));
        let items = header
            .into_iter()
            .chain(rows.into_iter().map(|row| (row, false)))
            .map(|(row, bold)| {
                let mut inlines = Vec::new();
                for (index, cell) in row.into_iter().enumerate() {
                    if index > 0 {
                        inlines.push(Inline::Text(" | ".to_string()));
                    }
                    let text = match cell.into_iter().next() {
                        Some(Block::Paragraph(text)) => text,
                        _ => Vec::new(),
                    };
                    if bold {
                        inlines.push(Inline::Strong(text));
                    } else {
                        inlines.extend(text);
                    }
                }
                let inlines = self.inlines(inlines);
                Item {
                    checked: None,
                    blocks: if inlines.is_empty() {
                        Vec::new()
                    } else {
                        vec![Block::Paragraph(inlines)]
                    },
                }
            })
            .collect();
        Block::List { start: None, items }
    }

    fn row(&mut self, row: Vec<Cell>, flattened: &mut bool) -> Vec<Cell> {
        row.into_iter()
            .map(|cell| {
                let (inlines, flat) = flatten(cell);
                *flattened |= flat;
                let inlines = self.inlines(inlines);
                if inlines.is_empty() {
                    Vec::new()
                } else {
                    vec![Block::Paragraph(inlines)]
                }
            })
            .collect()
    }

    /// Tidies a block's own run of text, like a paragraph's.
    fn inlines(&mut self, inlines: Vec<Inline>) -> Vec<Inline> {
        let mut out = self.children(inlines, 0);
        trim(&mut out);
        out
    }

    /// Tidies a run of text within the kinds in `within`.
    fn children(&mut self, inlines: Vec<Inline>, within: u16) -> Vec<Inline> {
        let mut out = Vec::new();
        for inline in inlines {
            self.inline(inline, within, &mut out);
        }
        trim_breaks(&mut out);
        out
    }

    fn inline(&mut self, inline: Inline, within: u16, out: &mut Vec<Inline>) {
        let inline = match Kind::split(inline) {
            Ok((kind, children)) => return self.delimited(kind, children, within, out),
            Err(inline) => inline,
        };
        let settings = self.settings;
        match inline {
            Inline::Text(text) => push(out, Inline::Text(text.replace(['\t', '\n', '\r'], " "))),
            Inline::Code(code) => {
                if !code.is_empty() {
                    push(out, Inline::Code(code));
                }
            }
            Inline::Math(tex) => {
                if !tex.trim().is_empty() {
                    push(out, Inline::Math(tex));
                }
            }
            Inline::Footnote(label) => {
                if !settings.footnotes {
                    self.warnings
                        .push(None, "left out footnotes, as `footnotes` is off");
                } else if !self.defined.contains(&label) {
                    self.warnings.push(
                        None,
                        format!("left out a reference to footnote `{label}`, which isn't defined"),
                    );
                } else if within & LINK != 0 {
                    self.hoisted.1.push(Inline::Footnote(label));
                } else {
                    push(out, Inline::Footnote(label));
                }
            }
            Inline::Anchor(id) => {
                if within & LINK != 0 {
                    self.hoisted.0.push(Inline::Anchor(id));
                } else {
                    push(out, Inline::Anchor(id));
                }
            }
            Inline::Html { html, line } => {
                if settings.raw_html {
                    push(out, Inline::Html { html, line });
                } else {
                    self.warnings
                        .push(line, "left out raw HTML, as `raw_html` is off");
                }
            }
            Inline::Link {
                target,
                title,
                content,
            } => {
                if within & LINK != 0 {
                    for child in content {
                        self.inline(child, within, out);
                    }
                    return;
                }
                let mut content = self.children(content, within | LINK);
                let (anchors, footnotes) = std::mem::take(&mut self.hoisted);
                for anchor in anchors {
                    push(out, anchor);
                }
                if content.is_empty() {
                    content.push(Inline::Text(match &target {
                        Target::Url(text) | Target::Id(text) | Target::Heading(text) => {
                            text.clone()
                        }
                    }));
                }
                let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
                push(
                    out,
                    Inline::Link {
                        target,
                        title,
                        content,
                    },
                );
                for footnote in footnotes {
                    push(out, footnote);
                }
            }
            Inline::Image { url, alt, title } => push(
                out,
                Inline::Image {
                    url,
                    alt: alt.split_whitespace().collect::<Vec<_>>().join(" "),
                    title: title.split_whitespace().collect::<Vec<_>>().join(" "),
                },
            ),
            Inline::LineBreak => push(out, Inline::LineBreak),
            _ => unreachable!("delimited inlines are handled above"),
        }
    }

    /// Text set between delimiters, spliced into its parent if it's within
    /// its own kind, or the site can't write its kind, with the whitespace,
    /// line breaks, and anchors at its edges set outside it.
    fn delimited(&mut self, kind: Kind, children: Vec<Inline>, within: u16, out: &mut Vec<Inline>) {
        let writable = self.writable(kind);
        if !writable {
            let message = match kind.setting() {
                Some(setting) => format!(
                    "left out {}, as `{setting}` and `raw_html` are off",
                    kind.name()
                ),
                None => format!("left out {}, as `raw_html` is off", kind.name()),
            };
            self.warnings.push(None, message);
        }
        if within & kind.bit() != 0 || !writable {
            for child in children {
                self.inline(child, within, out);
            }
            return;
        }
        let mut children = self.children(children, within | kind.bit());
        let (before, after) = hoist(&mut children);
        for inline in before {
            push(out, inline);
        }
        if !children.is_empty() {
            push(out, kind.wrap(children));
        }
        for inline in after {
            push(out, inline);
        }
    }

    fn writable(&self, kind: Kind) -> bool {
        let settings = self.settings;
        match kind {
            Kind::Emphasis | Kind::Strong => true,
            Kind::Strikethrough => settings.strikethrough || settings.raw_html,
            Kind::Underline => settings.raw_html,
            Kind::Mark => settings.mark || settings.raw_html,
            Kind::Superscript => settings.superscript || settings.raw_html,
            Kind::Subscript => settings.subscript || settings.raw_html,
        }
    }
}

/// Adds `inline` to the end of `out`, joined with the inline there if
/// they're text, code, formulas, or of the same kind of delimited text,
/// which Markdown would read wrongly side by side.
fn push(out: &mut Vec<Inline>, inline: Inline) {
    let join = match (out.last(), &inline) {
        (_, Inline::Text(text)) if text.is_empty() => return,
        (Some(Inline::Text(_)), Inline::Text(_))
        | (Some(Inline::Code(_)), Inline::Code(_))
        | (Some(Inline::Math(_)), Inline::Math(_)) => true,
        (Some(last), inline) => {
            matches!((Kind::of(last), Kind::of(inline)), (Some((a, _)), Some((b, _))) if a == b)
        }
        (None, _) => false,
    };
    if !join {
        out.push(inline);
        return;
    }
    match (out.last_mut().expect("joined to the last"), inline) {
        (Inline::Text(last), Inline::Text(text)) | (Inline::Code(last), Inline::Code(text)) => {
            last.push_str(&text);
        }
        (Inline::Math(last), Inline::Math(tex)) => {
            last.push(' ');
            last.push_str(&tex);
        }
        (last, inline) => {
            let into = Kind::children_mut(last).expect("delimited");
            if let Ok((_, children)) = Kind::split(inline) {
                for child in children {
                    push(into, child);
                }
            }
        }
    }
}

/// Takes the whitespace, line breaks, and anchors from either end of
/// `children`, which can't be at the edge of delimited text in Markdown.
fn hoist(children: &mut Vec<Inline>) -> (Vec<Inline>, Vec<Inline>) {
    let mut before = Vec::new();
    loop {
        match children.first_mut() {
            Some(Inline::Text(text)) => {
                let cut = text.len() - text.trim_start().len();
                before.push(Inline::Text(text.drain(..cut).collect()));
                if !text.is_empty() {
                    break;
                }
                children.remove(0);
            }
            Some(Inline::LineBreak | Inline::Anchor(_)) => before.push(children.remove(0)),
            _ => break,
        }
    }
    let mut after = Vec::new();
    loop {
        match children.last_mut() {
            Some(Inline::Text(text)) => {
                let kept = text.trim_end().len();
                after.push(Inline::Text(text.split_off(kept)));
                if !text.is_empty() {
                    break;
                }
                children.pop();
            }
            Some(Inline::LineBreak | Inline::Anchor(_)) => {
                after.extend(children.pop());
            }
            _ => break,
        }
    }
    after.reverse();
    (before, after)
}

/// Trims the spaces beside each line break, which Markdown drops.
fn trim_breaks(inlines: &mut Vec<Inline>) {
    for index in 0..inlines.len() {
        if inlines[index] != Inline::LineBreak {
            continue;
        }
        if let Some(Inline::Text(text)) = index.checked_sub(1).and_then(|i| inlines.get_mut(i)) {
            text.truncate(text.trim_end_matches(' ').len());
        }
        if let Some(Inline::Text(text)) = inlines.get_mut(index + 1) {
            let cut = text.len() - text.trim_start_matches(' ').len();
            text.drain(..cut);
        }
    }
    inlines.retain(|inline| !matches!(inline, Inline::Text(text) if text.is_empty()));
}

/// Trims the spaces and line breaks at either end of a block's text, past
/// any anchors, as Markdown drops them.
fn trim(inlines: &mut Vec<Inline>) {
    let mut index = 0;
    while index < inlines.len() {
        match &mut inlines[index] {
            Inline::Anchor(_) => index += 1,
            Inline::LineBreak => {
                inlines.remove(index);
            }
            Inline::Text(text) => {
                let cut = text.len() - text.trim_start_matches(' ').len();
                text.drain(..cut);
                if !text.is_empty() {
                    break;
                }
                inlines.remove(index);
            }
            _ => break,
        }
    }
    let mut end = inlines.len();
    while end > 0 {
        match &mut inlines[end - 1] {
            Inline::Anchor(_) => end -= 1,
            Inline::LineBreak => {
                inlines.remove(end - 1);
                end -= 1;
            }
            Inline::Text(text) => {
                text.truncate(text.trim_end_matches(' ').len());
                if !text.is_empty() {
                    break;
                }
                inlines.remove(end - 1);
                end -= 1;
            }
            _ => break,
        }
    }
}

/// What a cell of a table holds, as one line of text, and whether it held
/// more than a paragraph, so had to be flattened: each paragraph, item, or
/// line of code on a line of its own.
fn flatten(cell: Cell) -> (Vec<Inline>, bool) {
    if let [] | [Block::Paragraph(_)] = cell.as_slice() {
        let inlines = match cell.into_iter().next() {
            Some(Block::Paragraph(inlines)) => inlines,
            _ => Vec::new(),
        };
        return (inlines, false);
    }
    let flat = cell
        .iter()
        .filter(|b| !matches!(b, Block::Anchor(_)))
        .count()
        > 1
        || cell
            .iter()
            .any(|b| !matches!(b, Block::Paragraph(_) | Block::Anchor(_)));
    let mut lines = Vec::new();
    for block in cell {
        lines_of(block, &mut lines);
    }
    let mut out = Vec::new();
    for line in lines.into_iter().filter(|line| !line.is_empty()) {
        let anchors = line.iter().all(|i| matches!(i, Inline::Anchor(_)));
        if !out.is_empty() && !anchors {
            out.push(Inline::LineBreak);
        }
        out.extend(line);
    }
    (out, flat)
}

fn lines_of(block: Block, lines: &mut Vec<Vec<Inline>>) {
    match block {
        Block::Paragraph(inlines) => lines.push(inlines),
        Block::Heading { content, .. } => lines.push(vec![Inline::Strong(content)]),
        Block::Code { text, .. } => {
            lines.extend(
                text.lines()
                    .map(|line| vec![Inline::Code(line.to_string())]),
            );
        }
        Block::Math(tex) => lines.push(vec![Inline::Math(tex)]),
        Block::Quote {
            blocks,
            attribution,
        } => {
            for block in blocks {
                lines_of(block, lines);
            }
            if !attribution.is_empty() {
                let mut line = vec![Inline::Text("— ".to_string())];
                line.extend(attribution);
                lines.push(line);
            }
        }
        Block::Alert {
            kind,
            title,
            blocks,
        } => {
            let title = if title.trim().is_empty() {
                kind.title().to_string()
            } else {
                title
            };
            lines.push(vec![Inline::Strong(vec![Inline::Text(title)])]);
            for block in blocks {
                lines_of(block, lines);
            }
        }
        Block::List { start, items } => {
            for (index, item) in items.into_iter().enumerate() {
                let mut marker = match start {
                    Some(start) => format!("{}. ", start.saturating_add(index as u64)),
                    None => "• ".to_string(),
                };
                marker.push_str(match item.checked {
                    Some(true) => "☑ ",
                    Some(false) => "☐ ",
                    None => "",
                });
                let mut item_lines = Vec::new();
                for block in item.blocks {
                    lines_of(block, &mut item_lines);
                }
                match item_lines.first_mut() {
                    Some(first) => first.insert(0, Inline::Text(marker)),
                    None => item_lines.push(vec![Inline::Text(marker)]),
                }
                lines.extend(item_lines);
            }
        }
        Block::Definitions(definitions) => {
            for definition in definitions {
                lines.push(vec![Inline::Strong(definition.term)]);
                for block in definition.blocks {
                    lines_of(block, lines);
                }
            }
        }
        Block::Table(table) => {
            for row in table.header.into_iter().chain(table.rows) {
                let mut line = Vec::new();
                for (index, cell) in row.into_iter().enumerate() {
                    if index > 0 {
                        line.push(Inline::Text(" | ".to_string()));
                    }
                    line.extend(flatten(cell).0);
                }
                lines.push(line);
            }
        }
        Block::Rule => {}
        Block::Html { html, line } => lines.push(vec![Inline::Html { html, line }]),
        Block::Anchor(id) => lines.push(vec![Inline::Anchor(id)]),
    }
}
