//! The reader: what it knows of the document it reads, as it goes, and what
//! it gathers, the page's footnotes and what to warn of.

use std::collections::{HashMap, HashSet};

use asciidoc_parser::blocks::{Block as Node, FindBlocks, IsBlock};
use asciidoc_parser::document::{Attribute, InterpretedValue};
use asciidoc_parser::inlines::InlineNode;
use asciidoc_parser::parser::{Fidelity, Transform};
use asciidoc_parser::warnings::{WarningSeverity, WarningType};
use asciidoc_parser::{Document as Parsed, Parser, Span};
use little_markup::{Block, Document, Footnote, Inline, Warning};

use crate::text;

/// The attributes the reader follows, as each is set where it's read.
const FOLLOWED: [&str; 4] = ["imagesdir", "source-language", "stem", "experimental"];

/// Marks the start of a title read on its own, so it's read as a paragraph,
/// not, say, as a list, if it starts with `* `: a character of Unicode's
/// private use area, which nothing in AsciiDoc reads.
const START: char = '\u{E000}';

/// The document in `source`, the AsciiDoc of a file, after the
/// frontmatter's `offset` lines, and what to warn of, in order of line.
pub(crate) fn read(source: &str, offset: usize) -> (Document, Vec<Warning>) {
    let parsed = Parser::default().parse(source);
    let mut reader = Reader::new(&parsed, source, offset);
    let document = reader.document();
    let mut warnings = reader.parser_warnings();
    warnings.append(&mut reader.warnings);
    warnings.sort_by_key(|warning| warning.line);
    (document, warnings)
}

/// What the reader knows of the document it reads, and what it gathers.
pub(crate) struct Reader<'a> {
    /// The document, as asciidoc-parser parsed it.
    pub(crate) doc: &'a Parsed<'a>,
    /// The lines of the frontmatter, before the document.
    offset: usize,
    /// The document's lines, as they're written.
    lines: Vec<&'a str>,
    /// What to warn of.
    warnings: Vec<Warning>,
    /// The page's footnotes, so far.
    pub(crate) footnotes: Vec<Footnote>,
    /// The label of each footnote with an id, for references to it.
    pub(crate) footnote_ids: HashMap<String, String>,
    /// The parsed lines where asciidoc-parser put a link in place of an
    /// include, which it doesn't follow.
    includes: HashSet<usize>,
    /// The document's attribute entries so far, each as a line of AsciiDoc,
    /// which a title read on its own is read after.
    entries: Vec<String>,
    /// The value of each attribute in [`FOLLOWED`] that's set, so far.
    values: HashMap<&'static str, String>,
    /// The line of the title read on its own, if one is, whose spans aren't
    /// in the document.
    title_line: Option<usize>,
    /// The line of the first `>` of the Markdown-style quote being read, if
    /// one is, whose blocks' spans are in its text without its `>`s, line
    /// for line, not in the document.
    pub(crate) quote_line: Option<usize>,
}

impl<'a> Reader<'a> {
    fn new(doc: &'a Parsed<'a>, source: &'a str, offset: usize) -> Self {
        let mut reader = Reader {
            doc,
            offset,
            lines: source.lines().collect(),
            warnings: Vec::new(),
            footnotes: Vec::new(),
            footnote_ids: HashMap::new(),
            includes: HashSet::new(),
            entries: Vec::new(),
            values: HashMap::new(),
            title_line: None,
            quote_line: None,
        };
        reader.find_includes();
        for attribute in doc.header().attributes() {
            reader.attribute(attribute);
        }
        reader
    }

    /// The document: its title, its description, its blocks and its
    /// footnotes. The author and revision lines are left out, as a theme
    /// has no place for them.
    fn document(&mut self) -> Document {
        let mut blocks = Vec::new();
        let header = self.doc.header();
        if let Some(source) = header.title_source() {
            let line = self.line(source);
            let content = self.title(source, line, header.title().unwrap_or_default());
            let ids = header.id().map(str::to_string).into_iter().collect();
            blocks.push(Block::Heading {
                level: 1,
                content,
                ids,
            });
        }
        let mut metadata = Vec::new();
        if let InterpretedValue::Value(description) = self.doc.attribute_value("description") {
            metadata.push(("description".to_string(), text::plain(&description)));
        }
        for node in self.doc.child_blocks() {
            self.block(node, &mut blocks);
        }
        Document {
            frontmatter: String::new(),
            metadata,
            blocks,
            footnotes: std::mem::take(&mut self.footnotes),
        }
    }

    /// The line of the file `span` starts on.
    pub(crate) fn line(&self, span: Span<'_>) -> usize {
        match (self.title_line, self.quote_line) {
            (Some(line), _) => line,
            (None, Some(first)) => first + span.line() - 1,
            (None, None) => self.doc.origin_of(span).line + self.offset,
        }
    }

    /// Warns of `message`, on `line` of the file.
    pub(crate) fn warn(&mut self, line: usize, message: impl Into<String>) {
        self.warnings.push(Warning::new(Some(line), message));
    }

    /// The value of `name`, one of [`FOLLOWED`], if it's set here.
    pub(crate) fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Follows an attribute entry, for the titles read after it, and the
    /// attributes the reader follows.
    pub(crate) fn attribute(&mut self, attribute: &Attribute<'_>) {
        let name = attribute.name().data();
        let value = attribute.value();
        self.entries.push(match (value, attribute.raw_value()) {
            (InterpretedValue::Unset, _) => format!(":{name}!:"),
            (_, Some(raw)) => format!(":{name}: {}", raw.data()),
            (_, None) => format!(":{name}:"),
        });
        if let Some(followed) = FOLLOWED.iter().find(|followed| **followed == name) {
            match value {
                InterpretedValue::Value(value) => {
                    self.values.insert(followed, text::plain(value));
                }
                InterpretedValue::Set => {
                    self.values.insert(followed, String::new());
                }
                InterpretedValue::Unset => {
                    self.values.remove(followed);
                }
            }
        }
    }

    /// Finds each include, which asciidoc-parser, as it doesn't follow it,
    /// writes as a link to its file on a line of its own, and warns of it.
    fn find_includes(&mut self) {
        let doc = self.doc;
        for (index, parsed) in doc.span().data().lines().enumerate() {
            if !(parsed.starts_with("link:") && parsed.ends_with("[role=include]")) {
                continue;
            }
            let origin = doc.source_map().origin_at(index + 1, 1);
            if let Fidelity::Synthetic(Transform::SecureLinkRewrite)
            | Fidelity::Transformed(Transform::SecureLinkRewrite) = origin.fidelity
            {
                self.includes.insert(index + 1);
                let written = self
                    .lines
                    .get(origin.line - 1)
                    .map_or(parsed, |line| line.trim());
                self.warn(
                    origin.line + self.offset,
                    format!("left out `{written}`, as the plugin doesn't follow includes"),
                );
            }
        }
    }

    /// Whether `span` is on a line where an include was.
    pub(crate) fn is_include(&self, span: Span<'_>) -> bool {
        self.in_document() && self.includes.contains(&span.line())
    }

    /// Whether the spans being read are in the document, not in a title or
    /// a Markdown-style quote's text, where there are no includes.
    fn in_document(&self) -> bool {
        self.title_line.is_none() && self.quote_line.is_none()
    }

    /// `text`, the lines of a verbatim block that start on `span`'s line,
    /// but for those where an include was.
    pub(crate) fn without_includes(&self, span: Span<'_>, text: &str) -> String {
        if !self.in_document() {
            return text.to_string();
        }
        let first = span.line();
        let lines: Vec<&str> = text
            .split('\n')
            .enumerate()
            .filter(|(index, _)| !self.includes.contains(&(first + index)))
            .map(|(_, line)| line)
            .collect();
        lines.join("\n")
    }

    /// asciidoc-parser's warnings, but for those of little use to an author:
    /// its notes, and its guesses at references and footnotes, which little,
    /// and the reader, check themselves.
    fn parser_warnings(&self) -> Vec<Warning> {
        self.doc
            .warnings()
            .filter(|warning| warning.severity != WarningSeverity::Debug)
            .filter(|warning| {
                !matches!(
                    warning.warning,
                    WarningType::PossibleInvalidReference(_)
                        | WarningType::InvalidFootnoteReference(_)
                )
            })
            .map(|warning| {
                let line = self.doc.origin_of(warning.source).line + self.offset;
                let message = match &warning.warning {
                    // asciidoc-parser gives the level of the heading before,
                    // not the level it expected.
                    WarningType::SectionHeadingLevelSkipped(before, found) => format!(
                        "section heading level skipped (expected {}, found {found})",
                        before + 1
                    ),
                    other => other.to_string(),
                };
                Warning::new(Some(line), message)
            })
            .collect()
    }

    /// The content of a title, of the document or a section, which
    /// asciidoc-parser gives only as HTML: read again on its own, after the
    /// attribute entries before it, as a paragraph, or, if that fails, the
    /// text of `html`.
    pub(crate) fn title(&mut self, source: Span<'_>, line: usize, html: &str) -> Vec<Inline> {
        let mut text = self.entries.join("\n");
        text.push_str("\n\n");
        text.push(START);
        text.push_str(source.data());
        let parsed = Parser::default().parse(&text);
        let Some(nodes) = paragraph(parsed.child_blocks()) else {
            return vec![Inline::Text(text::plain(html))];
        };
        let outer = self.title_line.replace(line);
        let mut content = self.inlines(nodes);
        self.title_line = outer;
        if let Some(Inline::Text(first)) = content.first_mut()
            && let Some(rest) = first.strip_prefix(START)
        {
            *first = rest.to_string();
        }
        if matches!(content.first(), Some(Inline::Text(first)) if first.is_empty()) {
            content.remove(0);
        }
        content
    }
}

/// The inlines of the first paragraph in `blocks`, or in its preamble.
fn paragraph<'b>(mut blocks: impl Iterator<Item = &'b Node<'b>>) -> Option<&'b [InlineNode<'b>]> {
    blocks.find_map(|block| match block {
        Node::Preamble(preamble) => paragraph(preamble.child_blocks()),
        Node::Simple(simple) if simple.resolved_context().as_ref() == "paragraph" => {
            simple.inlines()
        }
        _ => None,
    })
}
