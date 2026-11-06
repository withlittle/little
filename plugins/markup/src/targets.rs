//! Where each link on a page leads: the `#fragment` of each heading, as
//! comrak works out its id, and of each anchor; and the text of a link
//! that has none, from the heading it leads to.

use crate::inline::html_pieces;
use crate::model::{Block, Document, Inline, Target, plain_text};
use crate::settings::Settings;
use crate::slug::slug;
use std::collections::{HashMap, HashSet};

/// Gives each link with no text that of the heading it leads to, else its
/// target, as the format would show it.
pub(crate) fn fill_links(document: &mut Document) {
    let mut by_text: HashMap<String, Vec<Inline>> = HashMap::new();
    let mut by_id: HashMap<String, Vec<Inline>> = HashMap::new();
    let mut add = |block: &Block| {
        if let Block::Heading { content, ids, .. } = block {
            by_text
                .entry(plain_text(content))
                .or_insert_with(|| content.clone());
            for id in ids {
                by_id.entry(id.clone()).or_insert_with(|| content.clone());
            }
        }
    };
    each_block(&document.blocks, &mut add);
    for footnote in &document.footnotes {
        each_block(&footnote.blocks, &mut add);
    }
    let mut fill = |inlines: &mut Vec<Inline>| fill(inlines, &by_text, &by_id);
    each_inlines(&mut document.blocks, &mut fill);
    for footnote in &mut document.footnotes {
        each_inlines(&mut footnote.blocks, &mut fill);
    }
}

/// Calls `f` with each block in `blocks`, and in those, in order, but for
/// those in tables, whose headings aren't headings in Markdown.
fn each_block<'a>(blocks: &'a [Block], f: &mut impl FnMut(&'a Block)) {
    for block in blocks {
        f(block);
        match block {
            Block::Quote { blocks, .. } | Block::Alert { blocks, .. } => each_block(blocks, f),
            Block::List { items, .. } => {
                for item in items {
                    each_block(&item.blocks, f);
                }
            }
            Block::Definitions(definitions) => {
                for definition in definitions {
                    each_block(&definition.blocks, f);
                }
            }
            _ => {}
        }
    }
}

/// Calls `f` with each block's own run of text in `blocks`, and in those,
/// in order.
pub fn each_inlines(blocks: &mut [Block], f: &mut impl FnMut(&mut Vec<Inline>)) {
    for block in blocks {
        match block {
            Block::Paragraph(inlines) => f(inlines),
            Block::Heading { content, .. } => f(content),
            Block::Quote {
                blocks,
                attribution,
            } => {
                each_inlines(blocks, f);
                f(attribution);
            }
            Block::Alert { blocks, .. } => each_inlines(blocks, f),
            Block::List { items, .. } => {
                for item in items {
                    each_inlines(&mut item.blocks, f);
                }
            }
            Block::Definitions(definitions) => {
                for definition in definitions {
                    f(&mut definition.term);
                    each_inlines(&mut definition.blocks, f);
                }
            }
            Block::Table(table) => {
                for row in table.header.iter_mut().chain(table.rows.iter_mut()) {
                    for cell in row {
                        each_inlines(cell, f);
                    }
                }
            }
            _ => {}
        }
    }
}

fn fill(
    inlines: &mut [Inline],
    by_text: &HashMap<String, Vec<Inline>>,
    by_id: &HashMap<String, Vec<Inline>>,
) {
    for inline in inlines {
        if let Inline::Link {
            target, content, ..
        } = inline
        {
            if !content.is_empty() {
                fill(content, by_text, by_id);
                continue;
            }
            let heading = match target {
                Target::Id(id) => by_id.get(id),
                Target::Heading(text) => by_text.get(&collapse(text)),
                Target::Url(_) => None,
            };
            *content = match heading {
                Some(heading) => link_text(heading),
                None => match target {
                    Target::Url(text) | Target::Id(text) | Target::Heading(text) => {
                        vec![Inline::Text(text.clone())]
                    }
                },
            };
        } else if let Inline::Styled(_, children) = inline {
            fill(children, by_text, by_id);
        }
    }
}

/// A heading's text, as a link to it shows it: without its footnotes and
/// anchors, which would be on the page twice, or its links.
fn link_text(inlines: &[Inline]) -> Vec<Inline> {
    let mut out = Vec::new();
    for inline in inlines {
        match inline {
            Inline::Footnote(_) | Inline::Anchor(_) => {}
            Inline::Link { content, .. } => out.extend(link_text(content)),
            Inline::Styled(kind, children) => out.push(Inline::Styled(*kind, link_text(children))),
            other => out.push(other.clone()),
        }
    }
    out
}

/// `text`, with each run of whitespace a space, and none at either end.
pub(crate) fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The `#fragment` of each heading and anchor on a page, once it's tidied.
#[derive(Debug, Default)]
pub(crate) struct Targets {
    /// The id comrak gives each heading, by where the heading is.
    slugs: HashMap<*const Block, String>,
    /// The `#fragment` of the first heading with each text.
    by_text: HashMap<String, String>,
    /// The `#fragment` of each id.
    by_id: HashMap<String, String>,
}

impl Targets {
    /// The targets of `document`'s headings and anchors, in the order comrak
    /// meets them: the page's, then its footnotes', in the order they're
    /// referred to. Where `raw_html` is off, an anchor can't be written, so
    /// a link to it leads to the heading before it instead.
    pub fn new(document: &Document, settings: &Settings) -> Targets {
        let footnotes: HashMap<&str, &[Block]> = document
            .footnotes
            .iter()
            .map(|footnote| (footnote.label.as_str(), footnote.blocks.as_slice()))
            .collect();
        let mut walk = Walk {
            targets: Targets::default(),
            taken: HashSet::new(),
            last: "#".to_string(),
            settings,
            notes: Vec::new(),
        };
        walk.blocks(&document.blocks);
        let mut index = 0;
        while let Some(label) = walk.notes.get(index) {
            if let Some(blocks) = footnotes.get(label) {
                walk.blocks(blocks);
            }
            index += 1;
        }
        walk.targets
    }

    /// The id comrak gives `heading`.
    pub fn slug(&self, heading: &Block) -> &str {
        self.slugs
            .get(&(heading as *const Block))
            .map_or("", String::as_str)
    }

    /// Where a link to `target` leads: a URL as it is; the `#fragment` of a
    /// heading or anchor; or, for one that isn't on the page, the id or
    /// text as a `#fragment`, which the link check warns of.
    pub fn resolve(&self, target: &Target) -> String {
        match target {
            Target::Url(url) => url.clone(),
            Target::Id(id) => self
                .by_id
                .get(id)
                .cloned()
                .unwrap_or_else(|| format!("#{id}")),
            Target::Heading(text) => {
                let text = collapse(text);
                self.by_text
                    .get(&text)
                    .cloned()
                    .unwrap_or_else(|| format!("#{text}"))
            }
        }
    }
}

struct Walk<'a> {
    targets: Targets,
    /// The ids given to the headings met.
    taken: HashSet<String>,
    /// The `#fragment` of the last heading met, or the top of the page.
    last: String,
    settings: &'a Settings,
    /// The labels of the footnotes referred to, in order.
    notes: Vec<&'a str>,
}

impl<'a> Walk<'a> {
    fn blocks(&mut self, blocks: &'a [Block]) {
        for block in blocks {
            match block {
                Block::Paragraph(inlines) => self.inlines(inlines),
                Block::Heading { content, ids, .. } => {
                    let mut text = String::new();
                    slug_text(content, self.settings.smart_punctuation, &mut text);
                    let slug = slug(&mut self.taken, &text);
                    let fragment = format!("#{slug}");
                    self.targets
                        .by_text
                        .entry(plain_text(content))
                        .or_insert_with(|| fragment.clone());
                    for id in ids {
                        self.targets
                            .by_id
                            .entry(id.clone())
                            .or_insert_with(|| fragment.clone());
                    }
                    self.targets.slugs.insert(block as *const Block, slug);
                    self.last = fragment;
                    self.inlines(content);
                }
                Block::Quote {
                    blocks,
                    attribution,
                } => {
                    self.blocks(blocks);
                    self.inlines(attribution);
                }
                Block::Alert { blocks, .. } => self.blocks(blocks),
                Block::List { items, .. } => {
                    for item in items {
                        self.blocks(&item.blocks);
                    }
                }
                Block::Definitions(definitions) => {
                    for definition in definitions {
                        self.inlines(&definition.term);
                        self.blocks(&definition.blocks);
                    }
                }
                Block::Table(table) => {
                    for row in table.header.iter().chain(&table.rows) {
                        for cell in row {
                            self.blocks(cell);
                        }
                    }
                }
                Block::Anchor(id) => self.anchor(id),
                Block::Code { .. } | Block::Math(_) | Block::Rule | Block::Html { .. } => {}
            }
        }
    }

    fn inlines(&mut self, inlines: &'a [Inline]) {
        for inline in inlines {
            match inline {
                Inline::Anchor(id) => self.anchor(id),
                Inline::Footnote(label) => {
                    if !self.notes.contains(&label.as_str()) {
                        self.notes.push(label);
                    }
                }
                Inline::Link { content, .. } | Inline::Styled(_, content) => self.inlines(content),
                _ => {}
            }
        }
    }

    fn anchor(&mut self, id: &str) {
        let fragment = if self.settings.raw_html {
            format!("#{id}")
        } else {
            self.last.clone()
        };
        self.targets.by_id.entry(id.to_string()).or_insert(fragment);
    }
}

/// The text comrak gives a heading its id from: that of its text, code,
/// formulas, and images, with each line break a space, and, with
/// `smart_punctuation`, without the runs of `-` it makes dashes of.
fn slug_text(inlines: &[Inline], smart: bool, out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => push_text(text, smart, out),
            Inline::Code(code) => out.push_str(&lines_as_spaces(code)),
            Inline::Math(tex) => out.push_str(lines_as_spaces(tex).trim()),
            Inline::Image { alt, .. } => push_text(alt, smart, out),
            Inline::Html { html, .. } => out.push_str(&html_text(html)),
            Inline::LineBreak => out.push(' '),
            Inline::Footnote(_) | Inline::Anchor(_) => {}
            Inline::Link { content, .. } | Inline::Styled(_, content) => {
                slug_text(content, smart, out)
            }
        }
    }
}

/// `text`, with each line ending a space, as code in a line is written.
pub(crate) fn lines_as_spaces(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

fn push_text(text: &str, smart: bool, out: &mut String) {
    if !smart {
        out.push_str(text);
        return;
    }
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '-' && chars.peek() == Some(&'-') {
            while chars.peek() == Some(&'-') {
                chars.next();
            }
            continue;
        }
        out.push(c);
    }
}

/// The text of raw HTML, without its tags or character references.
fn html_text(html: &str) -> String {
    let mut out = String::new();
    for (is_tag, piece) in html_pieces(html) {
        if is_tag {
            continue;
        }
        let mut rest = piece;
        while let Some(at) = rest.find('&') {
            out.push_str(&rest[..at]);
            let after = &rest[at + 1..];
            let name = after.find(';').filter(|&end| {
                end > 0
                    && end <= 32
                    && after[..end]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '#')
            });
            match name {
                Some(end) => rest = &after[end + 1..],
                None => {
                    out.push('&');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
    }
    lines_as_spaces(&out)
}
