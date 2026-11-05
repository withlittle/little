//! The tree the parser reads a file into: its blocks, as [`Node`]s, and
//! their text, as [`Span`]s, with references not yet led to their targets.

use little_markup::AlertKind;

/// A block of the document.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    /// A paragraph.
    Para(Vec<Span>),
    /// A section's title, at the top level only, at its depth, from 1, and
    /// with its ids.
    Heading {
        depth: usize,
        title: Vec<Span>,
        ids: Vec<String>,
    },
    /// A literal block, in a language, or none.
    Code { language: String, text: String },
    /// A parsed literal block: a literal block with inline markup, and its
    /// first line.
    Literal(Vec<Span>, usize),
    /// A formula on its own.
    Math(String),
    /// A block quote, and its attribution.
    Quote {
        blocks: Vec<Node>,
        attribution: Vec<Span>,
    },
    /// An admonition.
    Alert {
        kind: AlertKind,
        title: String,
        blocks: Vec<Node>,
    },
    /// A list, numbered from `start`, or with bullets.
    List {
        start: Option<u64>,
        items: Vec<Vec<Node>>,
    },
    /// A definition list or an option list: each term, and its definition.
    Definitions(Vec<(Vec<Span>, Vec<Node>)>),
    /// A field list, kept apart for the docinfo.
    Fields(Vec<Field>),
    /// A table.
    Table(Table),
    /// A transition.
    Rule,
    /// Raw HTML, and its line.
    Html(String, usize),
    /// The place of the target with this index, an internal one, whose ids
    /// go to the heading that follows, or here.
    Target(usize),
    /// An id, at this place.
    Anchor(String),
    /// Something that shows nothing, like a comment: `prebib` if docutils
    /// lets it come before the document's title.
    Nothing { prebib: bool },
    /// Blocks, in place, like a container's.
    Group(Vec<Node>),
    /// Blocks in place of what docutils reports as an error, like an
    /// unknown directive's source, as code: which, like a comment, may come
    /// before the document's title.
    Problem(Vec<Node>),
}

/// A field of a field list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Field {
    /// Its name, as text.
    pub name: Vec<Span>,
    /// Its body.
    pub body: Vec<Node>,
}

/// A table: its header row, if it has one, and its other rows, each a list
/// of cells.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Table {
    pub line: usize,
    pub header: Option<Vec<Vec<Node>>>,
    pub rows: Vec<Vec<Vec<Node>>>,
}

/// Text, and the inline markup in it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Span {
    Text(String),
    Emphasis(Vec<Span>),
    Strong(Vec<Span>),
    Code(String),
    Math(String),
    Sub(Vec<Span>),
    Sup(Vec<Span>),
    /// Raw HTML, and its line.
    Html(String, usize),
    Break,
    Image {
        url: String,
        alt: String,
    },
    /// A reference: what it says, where it leads, its text as written, for
    /// when it leads nowhere, and its line.
    Link {
        content: Vec<Span>,
        to: Ref,
        raw: String,
        line: usize,
    },
    /// An id, at this place.
    Anchor(String),
    /// A footnote or citation reference, by its key, with its text as
    /// written: the footnote's number, `#` and its label, `# N` for the Nth
    /// reference to a footnote numbered automatically, `* N` for the Nth
    /// with a symbol, or `cite:` and the citation's name.
    Footnote {
        key: String,
        raw: String,
        line: usize,
    },
    /// A substitution reference, by its name, as written.
    Substitution {
        name: String,
        raw: String,
        line: usize,
    },
}

/// Where a reference leads, before it's resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Ref {
    /// A URL.
    Url(String),
    /// The target with this name, normalized.
    Name(String),
    /// The anonymous target at this place in the order of anonymous
    /// references.
    Anonymous(usize),
}

impl Span {
    /// The text of `spans`, as docutils' `astext` gives it.
    pub fn text(spans: &[Span]) -> String {
        let mut text = String::new();
        for span in spans {
            span.push_text(&mut text);
        }
        text
    }

    fn push_text(&self, text: &mut String) {
        match self {
            Span::Text(s) | Span::Code(s) | Span::Math(s) => text.push_str(s),
            Span::Emphasis(spans) | Span::Strong(spans) | Span::Sub(spans) | Span::Sup(spans) => {
                spans.iter().for_each(|span| span.push_text(text));
            }
            Span::Link { content, .. } => content.iter().for_each(|span| span.push_text(text)),
            Span::Image { alt, .. } => text.push_str(alt),
            Span::Break => text.push('\n'),
            Span::Footnote { raw, .. } => text.push_str(raw),
            Span::Substitution { name, .. } => text.push_str(name),
            Span::Html(..) | Span::Anchor(_) => {}
        }
    }
}
