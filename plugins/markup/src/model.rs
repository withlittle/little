//! What a page holds, in no format in particular: what each plugin reads its
//! format into, and [`write`](crate::write) writes as Markdown.

/// A page: its frontmatter, its blocks, and its footnotes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    /// YAML frontmatter, as it's written between its `---` lines.
    pub frontmatter: String,
    /// What the format says of the page, like its `description`, each a key
    /// and its value, added to the frontmatter, unless it sets the key.
    pub metadata: Vec<(String, String)>,
    /// The page's blocks, in order.
    pub blocks: Vec<Block>,
    /// The page's footnotes, each written where it's first referred to.
    pub footnotes: Vec<Footnote>,
}

/// A footnote, and the label its references give.
#[derive(Debug, Clone, PartialEq)]
pub struct Footnote {
    /// The label that refers to it, unique on the page.
    pub label: String,
    /// What it says.
    pub blocks: Vec<Block>,
}

/// A block of a page.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// A paragraph.
    Paragraph(Vec<Inline>),
    /// A heading, at a level from 1, the page's title, written at 6 at most.
    Heading {
        /// Its level, from 1.
        level: u8,
        /// What it says.
        content: Vec<Inline>,
        /// The ids the format gives it, which [`Target::Id`] links to.
        ids: Vec<String>,
    },
    /// A block of code, in a language, or none.
    Code {
        /// The language, or empty.
        language: String,
        /// The code.
        text: String,
    },
    /// A formula on its own, in TeX.
    Math(String),
    /// A quote, and who said it, if the format says.
    Quote {
        /// What's quoted.
        blocks: Vec<Block>,
        /// Who said it, or empty.
        attribution: Vec<Inline>,
    },
    /// A note, a tip, or a warning, set apart from the text.
    Alert {
        /// Which kind it is.
        kind: AlertKind,
        /// Its title, or empty for its kind's.
        title: String,
        /// What it says.
        blocks: Vec<Block>,
    },
    /// A list, numbered from `start`, or with bullets.
    List {
        /// The number of the first item, or `None` for bullets.
        start: Option<u64>,
        /// Its items.
        items: Vec<Item>,
    },
    /// Terms, and what each means.
    Definitions(Vec<Definition>),
    /// A table.
    Table(Table),
    /// A thematic break.
    Rule,
    /// Raw HTML, kept where `raw_html` is on.
    Html {
        /// The HTML.
        html: String,
        /// Its line in the file, for a warning.
        line: Option<usize>,
    },
    /// An id that links can lead to, at this place on the page.
    Anchor(String),
}

/// An item of a list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Item {
    /// Whether it's a task, done or not, or `None` if it isn't one.
    pub checked: Option<bool>,
    /// What it says.
    pub blocks: Vec<Block>,
}

/// A term, and what it means.
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    /// The term.
    pub term: Vec<Inline>,
    /// What it means.
    pub blocks: Vec<Block>,
}

/// A table, of rows of cells, each of blocks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    /// Its line in the file, for a warning.
    pub line: Option<usize>,
    /// How each column is aligned.
    pub align: Vec<Align>,
    /// The row of headers, if it has one.
    pub header: Option<Vec<Cell>>,
    /// The rest of its rows.
    pub rows: Vec<Vec<Cell>>,
}

/// A cell of a table: its blocks, which Markdown holds only one paragraph
/// of, so more are flattened into one.
pub type Cell = Vec<Block>;

/// How a column of a table is aligned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    /// As the theme aligns it.
    #[default]
    None,
    /// To the left.
    Left,
    /// In the middle.
    Center,
    /// To the right.
    Right,
}

/// The kinds of alert GitHub has, which other formats' are led to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    /// Something to know.
    Note,
    /// Something that helps.
    Tip,
    /// Something not to miss.
    Important,
    /// Something to be careful of.
    Warning,
    /// Something that can go wrong.
    Caution,
}

impl AlertKind {
    /// The name GitHub's alerts give it, like `NOTE`.
    pub fn marker(self) -> &'static str {
        match self {
            AlertKind::Note => "NOTE",
            AlertKind::Tip => "TIP",
            AlertKind::Important => "IMPORTANT",
            AlertKind::Warning => "WARNING",
            AlertKind::Caution => "CAUTION",
        }
    }

    /// Its title, where the alert gives none, like `Note`.
    pub fn title(self) -> &'static str {
        match self {
            AlertKind::Note => "Note",
            AlertKind::Tip => "Tip",
            AlertKind::Important => "Important",
            AlertKind::Warning => "Warning",
            AlertKind::Caution => "Caution",
        }
    }
}

/// A run of text, or something in it.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    /// Text, as it's shown.
    Text(String),
    /// Emphasis, usually italic.
    Emphasis(Vec<Inline>),
    /// Strong emphasis, usually bold.
    Strong(Vec<Inline>),
    /// Text struck through.
    Strikethrough(Vec<Inline>),
    /// Underlined text, written in HTML, where `raw_html` is on.
    Underline(Vec<Inline>),
    /// Highlighted text.
    Mark(Vec<Inline>),
    /// Superscript.
    Superscript(Vec<Inline>),
    /// Subscript.
    Subscript(Vec<Inline>),
    /// Code, in a line.
    Code(String),
    /// A formula, in a line, in TeX.
    Math(String),
    /// A link.
    Link {
        /// Where it leads.
        target: Target,
        /// Its title, or empty.
        title: String,
        /// What it says, or empty for its target's heading, else the target.
        content: Vec<Inline>,
    },
    /// An image.
    Image {
        /// Its URL.
        url: String,
        /// The text that stands in for it.
        alt: String,
        /// Its title, or empty.
        title: String,
    },
    /// A reference to the footnote with this label.
    Footnote(String),
    /// A hard line break.
    LineBreak,
    /// Raw HTML, kept where `raw_html` is on.
    Html {
        /// The HTML.
        html: String,
        /// Its line in the file, for a warning.
        line: Option<usize>,
    },
    /// An id that links can lead to, at this place in the text.
    Anchor(String),
}

/// Where a link leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A URL, written as it is.
    Url(String),
    /// The heading or anchor with this id on the page.
    Id(String),
    /// The heading on the page whose text is this.
    Heading(String),
}

/// The text of `inlines`, as a reader sees it: their text, code, formulas,
/// links' text, and images' alt text, with each run of whitespace a space.
pub fn plain_text(inlines: &[Inline]) -> String {
    fn walk(inlines: &[Inline], out: &mut String) {
        for inline in inlines {
            match inline {
                Inline::Text(text) | Inline::Code(text) | Inline::Math(text) => out.push_str(text),
                Inline::Emphasis(children)
                | Inline::Strong(children)
                | Inline::Strikethrough(children)
                | Inline::Underline(children)
                | Inline::Mark(children)
                | Inline::Superscript(children)
                | Inline::Subscript(children)
                | Inline::Link {
                    content: children, ..
                } => walk(children, out),
                Inline::Image { alt, .. } => out.push_str(alt),
                Inline::LineBreak => out.push(' '),
                Inline::Footnote(_) | Inline::Html { .. } | Inline::Anchor(_) => {}
            }
        }
    }
    let mut out = String::new();
    walk(inlines, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
