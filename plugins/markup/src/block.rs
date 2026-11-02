//! Writes a page's blocks as Markdown, each set apart from the one before
//! it, so none runs into another, and nested in those that hold it; and the
//! footnotes each part of the page refers to, at its end.

use crate::frontmatter::frontmatter;
use crate::inline::{Context, attribute};
use crate::model::{AlertKind, Align, Block, Cell, Definition, Document, Inline, Item, Table};
use crate::settings::Settings;
use crate::targets::Targets;
use crate::{Part, Warnings};
use std::collections::HashMap;

/// Writes a page as the parts of its page.
pub(crate) struct Writer<'a> {
    pub(crate) settings: &'a Settings,
    pub(crate) targets: &'a Targets,
    pub(crate) warnings: &'a mut Warnings,
    document: &'a Document,
    /// The blocks of each footnote, by its label.
    footnotes: HashMap<&'a str, &'a [Block]>,
    /// The name of each footnote the part being written refers to, by its
    /// label.
    names: HashMap<String, String>,
    /// The labels of the footnotes the part being written refers to, in the
    /// order it first does.
    order: Vec<String>,
    /// The number of footnotes named on the page so far.
    counter: usize,
}

/// What a block is written as, which says how it sits beside others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A paragraph.
    Paragraph,
    /// A list, which another of its kind just after it would join, unless
    /// their markers differ, and which can start on the line after a
    /// paragraph if it `interrupts`.
    List {
        ordered: bool,
        alternate: bool,
        interrupts: bool,
    },
    /// Anything else.
    Other,
}

/// A block, written as Markdown.
struct Chunk {
    text: String,
    shape: Shape,
}

impl Chunk {
    fn other(text: String) -> Chunk {
        Chunk {
            text,
            shape: Shape::Other,
        }
    }
}

impl<'a> Writer<'a> {
    pub(crate) fn new(
        settings: &'a Settings,
        targets: &'a Targets,
        document: &'a Document,
        warnings: &'a mut Warnings,
    ) -> Self {
        Writer {
            settings,
            targets,
            warnings,
            document,
            footnotes: document
                .footnotes
                .iter()
                .map(|footnote| (footnote.label.as_str(), footnote.blocks.as_slice()))
                .collect(),
            names: HashMap::new(),
            order: Vec::new(),
            counter: 0,
        }
    }

    /// The parts of the page: one of HTML for each block of raw HTML at its
    /// top, and one of Markdown for each run of blocks around them, but the
    /// first, which holds the page's frontmatter, even if it has no blocks.
    pub(crate) fn write(mut self) -> Vec<Part> {
        let document = self.document;
        let mut first = Some(frontmatter(&document.frontmatter, &document.metadata));
        let mut parts = Vec::new();
        let runs = document
            .blocks
            .split_inclusive(|block| matches!(block, Block::Html { .. }));
        for run in runs {
            let (blocks, html) = match run.split_last() {
                Some((Block::Html { html, .. }, rest)) => (rest, Some(html)),
                _ => (run, None),
            };
            let mut text = first.take().unwrap_or_default();
            let body = self.part(blocks);
            if !body.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&body);
                text.push('\n');
            }
            if !text.is_empty() {
                parts.push(Part::Markdown(text));
            }
            if let Some(html) = html {
                parts.push(Part::Html(html.clone()));
            }
        }
        if let Some(frontmatter) = first.filter(|frontmatter| !frontmatter.is_empty()) {
            parts.push(Part::Markdown(frontmatter));
        }
        parts
    }

    /// `blocks`, and the footnotes they refer to, at the end, as Markdown
    /// that's read on its own.
    fn part(&mut self, blocks: &'a [Block]) -> String {
        let mut texts: Vec<String> = self.blocks(blocks).into_iter().map(|c| c.text).collect();
        let mut index = 0;
        while let Some(label) = self.order.get(index).cloned() {
            let name = self.names[&label].clone();
            let blocks = self.footnotes.get(label.as_str()).copied();
            let body = join(&self.blocks(blocks.unwrap_or_default()));
            texts.push(if body.is_empty() {
                format!("[^{name}]:")
            } else {
                format!("[^{name}]:\n{}", prefix(&body, "    ", "    "))
            });
            index += 1;
        }
        self.names.clear();
        self.order.clear();
        texts.join("\n\n")
    }

    /// The name the footnote with `label` is written with in the part being
    /// written: the next number on the page, where the part first refers to
    /// it, as each part is read on its own, so needs its own definition of
    /// it, and names are unique on the page.
    pub(crate) fn footnote_name(&mut self, label: &str) -> String {
        if let Some(name) = self.names.get(label) {
            return name.clone();
        }
        self.counter += 1;
        let name = self.counter.to_string();
        self.names.insert(label.to_string(), name.clone());
        self.order.push(label.to_string());
        name
    }

    /// `blocks`, each written as Markdown, but for those that write nothing,
    /// with each list in the other markers from a list of its kind before it.
    fn blocks(&mut self, blocks: &'a [Block]) -> Vec<Chunk> {
        let mut chunks: Vec<Chunk> = Vec::new();
        for block in blocks {
            let before = chunks.last().map(|chunk| chunk.shape);
            let chunk = self.block(block, before);
            if !chunk.text.is_empty() {
                chunks.push(chunk);
            }
        }
        chunks
    }

    fn block(&mut self, block: &'a Block, before: Option<Shape>) -> Chunk {
        match block {
            Block::Paragraph(inlines) => Chunk {
                text: self.inlines(inlines, Context::Paragraph),
                shape: Shape::Paragraph,
            },
            Block::Heading {
                level,
                content,
                ids,
            } => Chunk::other(self.heading(block, *level, content, ids)),
            Block::Code { language, text } => Chunk::other(fence(language, text)),
            Block::Math(tex) => {
                if !self.settings.math {
                    self.warnings
                        .push(None, "wrote formulas as code, as `math` is off");
                }
                Chunk::other(fence("math", tex.trim()))
            }
            Block::Quote {
                blocks,
                attribution,
            } => Chunk::other(self.quote(blocks, attribution)),
            Block::Alert {
                kind,
                title,
                blocks,
            } => Chunk::other(self.alert(*kind, title, blocks)),
            Block::List { start, items } => self.list(*start, items, before),
            Block::Definitions(definitions) => Chunk::other(self.definitions(definitions)),
            Block::Table(table) => Chunk::other(self.table(table)),
            Block::Rule => Chunk::other("___".to_string()),
            Block::Html { html, .. } => Chunk::other(nested_html(html)),
            Block::Anchor(id) if self.settings.raw_html => {
                Chunk::other(format!("<div id=\"{}\"></div>", attribute(id)))
            }
            Block::Anchor(_) => Chunk::other(String::new()),
        }
    }

    /// A heading, at its level, after an anchor for each of its ids that
    /// isn't the one comrak gives it, where `raw_html` is on.
    fn heading(
        &mut self,
        heading: &Block,
        level: u8,
        content: &[Inline],
        ids: &[String],
    ) -> String {
        let mut out = "#".repeat(usize::from(level));
        out.push(' ');
        if self.settings.raw_html {
            let targets = self.targets;
            let slug = targets.slug(heading);
            let mut written: Vec<&str> = Vec::new();
            for id in ids {
                if id != slug && !written.contains(&id.as_str()) {
                    out.push_str(&format!("<a id=\"{}\"></a>", attribute(id)));
                    written.push(id);
                }
            }
        }
        self.inlines_into(&mut out, content, Context::Heading);
        if out.ends_with(' ') {
            out.pop();
        }
        out
    }

    /// A quote, with who said it below it, after a dash.
    fn quote(&mut self, blocks: &'a [Block], attribution: &[Inline]) -> String {
        let mut texts: Vec<String> = self.blocks(blocks).into_iter().map(|c| c.text).collect();
        if !attribution.is_empty() {
            let mut line = "— ".to_string();
            self.inlines_into(&mut line, attribution, Context::Paragraph);
            texts.push(line);
        }
        prefix(&texts.join("\n\n"), "> ", "> ")
    }

    /// One of GitHub's alerts, with its title, if it has its own, which
    /// comrak reads backslash escapes and character references in.
    fn alert(&mut self, kind: AlertKind, title: &str, blocks: &'a [Block]) -> String {
        let mut out = format!("> [!{}]", kind.marker());
        if !title.is_empty() {
            out.push(' ');
            out.push_str(
                &title
                    .replace('\\', "\\\\")
                    .replace('&', "&amp;")
                    .replace(['\n', '\r'], " "),
            );
        }
        let body = join(&self.blocks(blocks));
        if !body.is_empty() {
            out.push('\n');
            out.push_str(&prefix(&body, "> ", "> "));
        }
        out
    }

    /// A list, in the other markers from a list of its kind just before it,
    /// which it would otherwise join; tight if each of its items is.
    fn list(&mut self, start: Option<u64>, items: &'a [Item], before: Option<Shape>) -> Chunk {
        let ordered = start.is_some();
        let alternate = matches!(
            before,
            Some(Shape::List { ordered: o, alternate: false, .. }) if o == ordered
        );
        let mut bodies = Vec::new();
        for item in items {
            let mut chunks = self.blocks(&item.blocks);
            if let Some(checked) = item.checked {
                let task = match (self.settings.task_lists, checked) {
                    (true, false) => "[ ]",
                    (true, true) => "[x]",
                    (false, false) => "\\[ \\]",
                    (false, true) => "\\[x\\]",
                };
                match chunks.first_mut() {
                    Some(chunk) if chunk.shape == Shape::Paragraph => {
                        chunk.text = format!("{task} {}", chunk.text);
                    }
                    _ => chunks.insert(
                        0,
                        Chunk {
                            text: task.to_string(),
                            shape: Shape::Paragraph,
                        },
                    ),
                }
            }
            bodies.push(chunks);
        }
        let tight = bodies.iter().all(|chunks| match chunks.as_slice() {
            [] | [_] => true,
            [first, second] => {
                first.shape == Shape::Paragraph
                    && matches!(
                        second.shape,
                        Shape::List {
                            interrupts: true,
                            ..
                        }
                    )
            }
            _ => false,
        });
        let separator = if tight { "\n" } else { "\n\n" };
        let mut texts = Vec::new();
        for (index, chunks) in bodies.iter().enumerate() {
            let marker = match start {
                None if alternate => "*".to_string(),
                None => "-".to_string(),
                Some(start) => {
                    let number = start.saturating_add(index as u64).min(999_999_999);
                    format!("{number}{}", if alternate { ')' } else { '.' })
                }
            };
            let body: Vec<&str> = chunks.iter().map(|chunk| chunk.text.as_str()).collect();
            let indent = " ".repeat(marker.len() + 1);
            texts.push(prefix(
                &body.join(separator),
                &format!("{marker} "),
                &indent,
            ));
        }
        Chunk {
            text: texts.join(separator),
            shape: Shape::List {
                ordered,
                alternate,
                interrupts: bodies.first().is_some_and(|chunks| !chunks.is_empty())
                    && start.is_none_or(|start| start == 1),
            },
        }
    }

    /// Terms, and what each means, as a description list, where Markdown
    /// can hold them, else in HTML; tight if each means a paragraph.
    fn definitions(&mut self, definitions: &'a [Definition]) -> String {
        let mut written = Vec::new();
        for definition in definitions {
            let term = self.inlines(&definition.term, Context::Paragraph);
            let chunks = self.blocks(&definition.blocks);
            let paragraph = matches!(chunks.as_slice(), [chunk] if chunk.shape == Shape::Paragraph);
            written.push((term, join(&chunks), paragraph));
        }
        let whole = written
            .iter()
            .all(|(term, body, _)| !term.is_empty() && !body.is_empty());
        if self.settings.description_lists && whole {
            let tight = written.iter().all(|(_, _, paragraph)| *paragraph);
            let between = if tight { "\n" } else { "\n\n" };
            let texts: Vec<String> = written
                .iter()
                .map(|(term, body, _)| format!("{term}{between}{}", prefix(body, ": ", "  ")))
                .collect();
            return texts.join("\n\n");
        }
        let mut out = "<dl>".to_string();
        for (term, body, _) in &written {
            if !term.is_empty() {
                out.push_str(&format!("\n<dt>\n\n{term}\n\n</dt>"));
            }
            if !body.is_empty() {
                out.push_str(&format!("\n<dd>\n\n{body}\n\n</dd>"));
            }
        }
        out.push_str("\n</dl>");
        out
    }

    /// A table, with a row of headers, empty if it has none, as Markdown's
    /// tables need one, and as many columns as its longest row.
    fn table(&mut self, table: &'a Table) -> String {
        let columns = table
            .header
            .iter()
            .chain(&table.rows)
            .map(Vec::len)
            .max()
            .unwrap_or(0);
        if columns == 0 {
            return String::new();
        }
        let mut out = self.row(table.header.as_deref().unwrap_or_default(), columns);
        out.push_str("\n|");
        for column in 0..columns {
            out.push_str(match table.align.get(column).copied().unwrap_or_default() {
                Align::None => " --- |",
                Align::Left => " :-- |",
                Align::Center => " :-: |",
                Align::Right => " --: |",
            });
        }
        for row in &table.rows {
            out.push('\n');
            out.push_str(&self.row(row, columns));
        }
        out
    }

    /// A row of a table, with an empty cell for each it doesn't have.
    fn row(&mut self, cells: &'a [Cell], columns: usize) -> String {
        let mut out = "|".to_string();
        for column in 0..columns {
            out.push(' ');
            if let Some([Block::Paragraph(inlines)]) = cells.get(column).map(Vec::as_slice) {
                self.inlines_into(&mut out, inlines, Context::Cell);
            }
            out.push_str(" |");
        }
        out
    }
}

/// A fenced code block, in more backticks than any run of them in `text`,
/// in `language`, without the backticks and line breaks it can't hold.
fn fence(language: &str, text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat(longest.max(2) + 1);
    let language: String = language
        .chars()
        .filter(|c| !matches!(c, '`' | '\n' | '\r'))
        .collect();
    let mut out = format!("{fence}{}\n{text}", language.trim());
    if !text.is_empty() && !text.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&fence);
    out
}

/// Raw HTML in a block, in a `<div>`, so it's one HTML block, whatever its
/// lines start with, with a comment in place of each blank line, which
/// would end it.
fn nested_html(html: &str) -> String {
    let lines: Vec<&str> = html.lines().collect();
    let mut out = "<div>\n".to_string();
    let start = lines.iter().position(|line| !line.trim().is_empty());
    let end = lines.iter().rposition(|line| !line.trim().is_empty());
    if let (Some(start), Some(end)) = (start, end) {
        for line in &lines[start..=end] {
            out.push_str(if line.trim().is_empty() {
                "<!-- -->"
            } else {
                line
            });
            out.push('\n');
        }
    }
    out.push_str("</div>");
    out
}

/// `text`, with `first` before its first line and `rest` before each
/// other, but on a blank line, which gets no trailing space.
fn prefix(text: &str, first: &str, rest: &str) -> String {
    let mut out = String::new();
    for (index, line) in text.split('\n').enumerate() {
        let prefix = if index == 0 {
            first
        } else {
            out.push('\n');
            rest
        };
        if line.is_empty() {
            out.push_str(prefix.trim_end());
        } else {
            out.push_str(prefix);
            out.push_str(line);
        }
    }
    out
}

/// Blocks, each set apart from the one before it by a blank line.
fn join(chunks: &[Chunk]) -> String {
    let texts: Vec<&str> = chunks.iter().map(|chunk| chunk.text.as_str()).collect();
    texts.join("\n\n")
}
