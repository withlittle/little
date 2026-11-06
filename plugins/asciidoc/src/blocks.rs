//! Blocks: AsciiDoc's sections, paragraphs, lists, and delimited blocks, as
//! little's.

use asciidoc_parser::blocks::{
    AdmonitionBlock, AdmonitionVariant, Block as Node, BreakType, FindBlocks, IsBlock, ListBlock,
    ListItemMarker, ListType, MediaBlock, MediaType, QuoteBlock, QuoteType, SectionBlock,
    SectionType, SimpleBlock, SimpleBlockStyle,
};
use asciidoc_parser::{HasSpan, Span};
use little_markup::{AlertKind, Block, Definition, Inline, Item, Kind, Target};

use crate::inlines;
use crate::reader::Reader;
use crate::text;

impl Reader<'_> {
    /// `node`, as little's blocks, added to `out`.
    pub(crate) fn block<'b>(&mut self, node: &'b Node<'b>, out: &mut Vec<Block>) {
        // Asciidoctor writes nothing of a comment.
        if node.resolved_context().as_ref() == "comment" || node.declared_style() == Some("comment")
        {
            return;
        }
        // A section's id is its heading's.
        if !matches!(node, Node::Section(_))
            && let Some(id) = node.id()
        {
            out.push(Block::Anchor(id.to_string()));
        }
        match node {
            Node::Section(section) => self.section(section, out),
            Node::Preamble(_) => self.children(node.child_blocks(), out),
            Node::CompoundDelimited(_) => {
                out.extend(self.caption(node));
                self.children(node.child_blocks(), out);
            }
            Node::Simple(simple) => {
                out.extend(self.caption(node));
                self.simple(node, simple, out);
            }
            Node::RawDelimited(raw) => {
                out.extend(self.caption(node));
                let content = raw.content();
                match node.resolved_context().as_ref() {
                    context @ ("listing" | "literal") => {
                        let text = inlines::verbatim(content.inlines());
                        let text = self.without_includes(content.original(), &text);
                        // Only a listing is source code, in a language.
                        let language = match context {
                            "listing" => self.language(node),
                            _ => String::new(),
                        };
                        out.push(Block::Code { language, text });
                    }
                    "stem" => self.stem(node, content.original(), out),
                    _ => {
                        let html = node.rendered_html_content().unwrap_or_default();
                        let html = self.without_includes(content.original(), html);
                        let line = Some(self.line(node.span()));
                        out.push(Block::Html { html, line });
                    }
                }
            }
            Node::Admonition(admonition) => self.admonition(node, admonition, out),
            Node::Quote(quote) => {
                out.extend(self.caption(node));
                self.quote(node, quote, out);
            }
            Node::List(list) => {
                out.extend(self.caption(node));
                self.list(list, out);
            }
            Node::Media(media) => self.media(node, media, out),
            Node::Table(table) => {
                out.extend(self.caption(node));
                self.table(table, out);
            }
            Node::Break(rule) if rule.type_() == BreakType::Thematic => out.push(Block::Rule),
            Node::DocumentAttribute(attribute) => self.attribute(attribute),
            // A page break, which a web page hasn't, and a table of contents,
            // which the theme writes itself.
            _ => {}
        }
    }

    /// Each of `nodes`, as little's blocks, added to `out`.
    pub(crate) fn children<'b>(
        &mut self,
        nodes: impl Iterator<Item = &'b Node<'b>>,
        out: &mut Vec<Block>,
    ) {
        for node in nodes {
            self.block(node, out);
        }
    }

    /// The title of `node`, after its caption, like `Table 1. `, as a
    /// paragraph in emphasis, as Asciidoctor shows it, if it has one.
    fn caption<'b>(&mut self, node: &'b Node<'b>) -> Option<Block> {
        let title = node.title_content()?;
        let mut content = Vec::new();
        if let Some(caption) = node.caption() {
            content.push(Inline::Text(text::plain(caption)));
        }
        content.extend(self.inlines(title.inlines()));
        Some(Block::Paragraph(vec![Inline::Styled(
            Kind::Emphasis,
            content,
        )]))
    }

    /// A section: its heading, a level below AsciiDoc's, as `=` is the
    /// page's title, then its blocks.
    fn section<'b>(&mut self, section: &'b SectionBlock<'b>, out: &mut Vec<Block>) {
        let source = section.section_title_source();
        let line = self.line(source);
        let mut content = self.title(source, line, section.section_title());
        // An appendix's caption, like `Appendix A: `, or the section's
        // number, where `:sectnums:` is set.
        let number = match (section.caption(), section.section_number()) {
            (Some(caption), _) => Some(text::plain(caption)),
            (None, Some(number)) if section.section_type() != SectionType::Discrete => {
                Some(format!("{number}. "))
            }
            _ => None,
        };
        if let Some(number) = number {
            content.insert(0, Inline::Text(number));
        }
        out.push(Block::Heading {
            level: u8::try_from(section.level() + 1).unwrap_or(u8::MAX),
            content,
            ids: section.id().map(str::to_string).into_iter().collect(),
        });
        self.children(section.child_blocks(), out);
    }
}

impl Reader<'_> {
    /// A paragraph, or a block of code, a formula, or HTML, written as one.
    fn simple<'b>(
        &mut self,
        node: &'b Node<'b>,
        simple: &'b SimpleBlock<'b>,
        out: &mut Vec<Block>,
    ) {
        let content = simple.content();
        let source = content.original();
        match (simple.style(), node.resolved_context().as_ref()) {
            (SimpleBlockStyle::Literal, _) => {
                let text = inlines::verbatim(content.inlines());
                out.push(Block::Code {
                    language: String::new(),
                    text: self.without_includes(source, &text),
                });
            }
            // As Asciidoctor does, but asciidoc-parser doesn't, a source or
            // listing paragraph is read as it's written, so `*` stays `*`.
            (SimpleBlockStyle::Listing | SimpleBlockStyle::Source, _) => {
                let language = self.language(node);
                let text = self.without_includes(source, source.data());
                out.push(Block::Code { language, text });
            }
            (_, "stem") => self.stem(node, source, out),
            (_, "pass") => {
                let html = self.without_includes(source, source.data());
                let line = Some(self.line(node.span()));
                out.push(Block::Html { html, line });
            }
            _ => {
                let content = self.inlines(content.inlines());
                if !content.is_empty() {
                    out.push(Block::Paragraph(content));
                }
            }
        }
    }

    /// The language of a block of code: its source style's, like
    /// `[source,rust]`'s, else `source-language`'s, or another style it
    /// has, like `[mermaid]`, which little may draw.
    fn language<'b>(&self, node: &'b Node<'b>) -> String {
        match node.declared_style() {
            Some("listing") => String::new(),
            Some(style) if style != "source" => style.to_string(),
            _ => node
                .attrlist()
                .and_then(|attrlist| attrlist.named_or_positional_attribute("language", 2))
                .map(|language| language.value())
                .or_else(|| self.value("source-language"))
                .unwrap_or_default()
                .to_string(),
        }
    }

    /// A formula on its own, at `source`: TeX, where it's written in LaTeX,
    /// else code, as little writes only TeX, warned of.
    fn stem<'b>(&mut self, node: &'b Node<'b>, source: Span<'b>, out: &mut Vec<Block>) {
        let text = self.without_includes(source, source.data().trim_end());
        let tex = match node.declared_style() {
            Some("latexmath") => true,
            Some("asciimath") => false,
            _ => matches!(self.value("stem"), Some("latexmath" | "latex" | "tex")),
        };
        if tex {
            out.push(Block::Math(text));
        } else {
            let line = self.line(node.span());
            self.warn(
                line,
                "kept an AsciiMath formula as code, as little writes only TeX math",
            );
            out.push(Block::Code {
                language: "asciimath".to_string(),
                text,
            });
        }
    }

    /// An admonition, as the alert of its kind, with its title, or its
    /// label, where `:note-caption:` or the like changes it.
    fn admonition<'b>(
        &mut self,
        node: &'b Node<'b>,
        admonition: &'b AdmonitionBlock<'b>,
        out: &mut Vec<Block>,
    ) {
        let kind = match admonition.variant() {
            AdmonitionVariant::Note => AlertKind::Note,
            AdmonitionVariant::Tip => AlertKind::Tip,
            AdmonitionVariant::Important => AlertKind::Important,
            AdmonitionVariant::Caution => AlertKind::Caution,
            AdmonitionVariant::Warning => AlertKind::Warning,
        };
        let title = match node.title() {
            Some(title) => text::plain(title),
            None if admonition.label() != kind.title() => text::plain(admonition.label()),
            None => String::new(),
        };
        let mut blocks = Vec::new();
        match admonition.content() {
            Some(content) => {
                let content = self.inlines(content.inlines());
                if !content.is_empty() {
                    blocks.push(Block::Paragraph(content));
                }
            }
            None => self.children(admonition.child_blocks(), &mut blocks),
        }
        out.push(Block::Alert {
            kind,
            title,
            blocks,
        });
    }

    /// A quote, or a verse, which keeps its lines, and who said it, and
    /// where.
    fn quote<'b>(&mut self, node: &'b Node<'b>, quote: &'b QuoteBlock<'b>, out: &mut Vec<Block>) {
        let mut blocks = Vec::new();
        match quote.content() {
            Some(content) => {
                let content = self.inlines(content.inlines());
                if quote.type_() == QuoteType::Verse {
                    blocks = verse(content);
                } else if !content.is_empty() {
                    blocks.push(Block::Paragraph(content));
                }
            }
            None => {
                // A Markdown-style quote's blocks are read from its lines
                // without their `>`s, so their lines are counted from its
                // first, after its title and attributes.
                let outer = self.quote_line;
                let first = node.span().data().lines().enumerate().find(|(_, line)| {
                    !(line.trim().is_empty()
                        || line.starts_with(['.', '['])
                        || line.starts_with("//"))
                });
                if let Some((index, line)) = first
                    && line.starts_with('>')
                {
                    self.quote_line = Some(self.line(node.span()) + index);
                }
                self.children(quote.blocks().iter(), &mut blocks);
                self.quote_line = outer;
            }
        }
        let mut attribution = Vec::new();
        if let Some(author) = quote.attribution() {
            attribution.push(Inline::Text(text::plain(author)));
        }
        if let Some(work) = quote.citetitle() {
            if !attribution.is_empty() {
                attribution.push(Inline::Text(", ".to_string()));
            }
            attribution.push(Inline::Styled(
                Kind::Emphasis,
                vec![Inline::Text(text::plain(work))],
            ));
        }
        out.push(Block::Quote {
            blocks,
            attribution,
        });
    }

    /// A list: of bullets, or tasks; numbered, as a callout list is; or of
    /// terms, each with what it means, or nothing, where the next term
    /// shares it.
    fn list<'b>(&mut self, list: &'b ListBlock<'b>, out: &mut Vec<Block>) {
        let items = list.child_blocks().filter_map(|node| match node {
            Node::ListItem(item) => Some(item),
            _ => None,
        });
        if list.type_() == ListType::Description {
            let mut definitions = Vec::new();
            for item in items {
                let term = match item.list_item_marker() {
                    ListItemMarker::DefinedTerm { term, .. } => self.inlines(term.inlines()),
                    _ => Vec::new(),
                };
                let mut blocks = Vec::new();
                self.children(item.child_blocks(), &mut blocks);
                definitions.push(Definition { term, blocks });
            }
            out.push(Block::Definitions(definitions));
            return;
        }
        let start = match list.type_() {
            ListType::Unordered => None,
            ListType::Ordered => Some(list.start().map_or(1, |start| start.max(0) as u64)),
            _ => Some(1),
        };
        let mut list_items = Vec::new();
        for item in items {
            let mut blocks = Vec::new();
            self.children(item.child_blocks(), &mut blocks);
            list_items.push(Item {
                checked: item.checkbox(),
                blocks,
            });
        }
        out.push(Block::List {
            start,
            items: list_items,
        });
    }

    /// An image, in a paragraph of its own, in `:imagesdir:`, with its
    /// title after it, as Asciidoctor shows it; or a video or a sound, in
    /// HTML, as Asciidoctor writes it.
    fn media<'b>(&mut self, node: &'b Node<'b>, media: &'b MediaBlock<'b>, out: &mut Vec<Block>) {
        let target = media.resolved_target();
        let url = text::in_folder(self.value("imagesdir"), target);
        let attributes = media.macro_attrlist();
        let html = match (
            media.type_(),
            attributes.nth_attribute(1).map(|a| a.value()),
        ) {
            (MediaType::Image, _) => {
                let alt = attributes
                    .named_or_positional_attribute("alt", 1)
                    .map(|alt| alt.value().to_string())
                    .filter(|alt| !alt.is_empty())
                    .unwrap_or_else(|| text::default_alt(target));
                let mut image = Inline::Image {
                    url,
                    alt,
                    title: String::new(),
                };
                if let Some(link) = attributes.named_attribute("link") {
                    image = Inline::Link {
                        target: Target::Url(link.value().to_string()),
                        title: String::new(),
                        content: vec![image],
                    };
                }
                out.push(Block::Paragraph(vec![image]));
                out.extend(self.caption(node));
                return;
            }
            (MediaType::Video, Some("youtube")) => format!(
                "<iframe src=\"https://www.youtube.com/embed/{}\" allowfullscreen></iframe>",
                text::escape(target)
            ),
            (MediaType::Video, Some("vimeo")) => format!(
                "<iframe src=\"https://player.vimeo.com/video/{}\" allowfullscreen></iframe>",
                text::escape(target)
            ),
            (MediaType::Video, _) => {
                format!("<video src=\"{}\" controls></video>", text::escape(&url))
            }
            (MediaType::Audio, _) => {
                format!("<audio src=\"{}\" controls></audio>", text::escape(&url))
            }
        };
        out.extend(self.caption(node));
        let line = Some(self.line(node.span()));
        out.push(Block::Html { html, line });
    }
}

/// A verse's `inlines`, as paragraphs, one after each blank line, with a
/// line break at the end of each line.
fn verse(inlines: Vec<Inline>) -> Vec<Block> {
    let mut paragraphs = Vec::new();
    let mut paragraph = Vec::new();
    // The line ends since the last of the verse's text.
    let mut ends = 0;
    let mut add = |inline: Inline, ends: &mut usize, paragraph: &mut Vec<Inline>| {
        if *ends > 1 && !paragraph.is_empty() {
            paragraphs.push(Block::Paragraph(std::mem::take(paragraph)));
        } else if *ends == 1 && !paragraph.is_empty() {
            paragraph.push(Inline::LineBreak);
        }
        *ends = 0;
        paragraph.push(inline);
    };
    for inline in inlines {
        let Inline::Text(text) = inline else {
            add(inline, &mut ends, &mut paragraph);
            continue;
        };
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                ends += 1;
            }
            if !line.is_empty() {
                add(Inline::Text(line.to_string()), &mut ends, &mut paragraph);
            }
        }
    }
    if !paragraph.is_empty() {
        paragraphs.push(Block::Paragraph(paragraph));
    }
    paragraphs
}
