//! Inlines: AsciiDoc's text and its markup, as little's.

use asciidoc_parser::document::RefType;
use asciidoc_parser::inlines::{
    Callout, CalloutGuard, CharRef, InlineNode, RawForm, RawOrigin, Ref, RefVariant, StemNotation,
    StyleVariant, UiKind,
};
use asciidoc_parser::{HasSpan, Span};
use little_markup::{Block, Footnote, Inline, Target};

use crate::reader::Reader;
use crate::text;

/// Macros that asciidoc-parser reads only where `:experimental:` is set.
const EXPERIMENTAL: [&str; 3] = ["kbd", "btn", "menu"];

impl Reader<'_> {
    /// `nodes`, as little's inlines.
    pub(crate) fn inlines(&mut self, nodes: &[InlineNode<'_>]) -> Vec<Inline> {
        let mut inlines = Vec::new();
        // Text, as HTML, as asciidoc-parser writes some of it, until it's
        // decoded into one `Inline::Text`.
        let mut run = String::new();
        for node in nodes {
            match node {
                InlineNode::Text { value, location } => {
                    self.check_macros(value, *location);
                    run.push_str(&text::escape(value));
                }
                InlineNode::CharRef { value, location } => match value {
                    CharRef::Special(c) => run.push_str(&text::escape(&c.to_string())),
                    CharRef::Replacement(html) => run.push_str(html),
                    CharRef::Entity(entity) => {
                        let name = entity.trim_start_matches('&').trim_end_matches(';');
                        if text::reference(name).is_some() {
                            run.push_str(entity);
                        } else {
                            // A name the plugin doesn't know is left to the
                            // browser, which knows them all.
                            flush(&mut run, &mut inlines);
                            let line = Some(self.line(*location));
                            inlines.push(Inline::Html {
                                html: entity.to_string(),
                                line,
                            });
                        }
                    }
                },
                InlineNode::Raw {
                    value,
                    form,
                    origin,
                    location,
                } => match (form, origin) {
                    (RawForm::Escaped, _) => run.push_str(&text::escape(value)),
                    (RawForm::AsIs, RawOrigin::Substitution) if !value.contains('<') => {
                        run.push_str(value)
                    }
                    _ if value.contains('<') => {
                        flush(&mut run, &mut inlines);
                        let line = Some(self.line(*location));
                        inlines.push(Inline::Html {
                            html: value.to_string(),
                            line,
                        });
                    }
                    _ => {
                        flush(&mut run, &mut inlines);
                        inlines.push(Inline::Text(text::decode(value)));
                    }
                },
                InlineNode::Callout(callout) => run.push_str(&text::escape(&marker(callout))),
                InlineNode::Image(image) if image.is_icon => {
                    run.push_str(&text::escape(&format!("[{}]", image.target)));
                }
                InlineNode::Ui(ui) => match &ui.kind {
                    UiKind::Keyboard(keys) => {
                        flush(&mut run, &mut inlines);
                        let keys: Vec<String> = keys.iter().map(|key| text::decode(key)).collect();
                        inlines.push(Inline::Code(keys.join("+")));
                    }
                    UiKind::Button(label) => {
                        flush(&mut run, &mut inlines);
                        inlines.push(Inline::Strong(vec![Inline::Text(text::decode(label))]));
                    }
                    UiKind::Menu {
                        menu,
                        submenus,
                        item,
                    } => {
                        let path: Vec<&str> = std::iter::once(menu.as_ref())
                            .chain(submenus.iter().map(AsRef::as_ref))
                            .chain(item.as_deref())
                            .collect();
                        run.push_str(&path.join(" &#8250; "));
                    }
                },
                InlineNode::IndexTerm(term) => {
                    if term.visible {
                        flush(&mut run, &mut inlines);
                        let children = self.inlines(&term.children);
                        inlines.extend(children);
                    }
                }
                node => {
                    flush(&mut run, &mut inlines);
                    self.node(node, &mut inlines);
                }
            }
        }
        flush(&mut run, &mut inlines);
        inlines
    }

    /// A node that isn't text, as little's inlines, added to `inlines`.
    fn node(&mut self, node: &InlineNode<'_>, inlines: &mut Vec<Inline>) {
        match node {
            InlineNode::Styled(styled) => {
                if let Some(id) = &styled.id {
                    inlines.push(Inline::Anchor(id.to_string()));
                }
                let mut children = self.inlines(&styled.children);
                let has_role = |role: &str| styled.roles.iter().any(|each| each.as_ref() == role);
                inlines.push(match styled.variant {
                    StyleVariant::Strong => Inline::Strong(children),
                    StyleVariant::Emphasis => Inline::Emphasis(children),
                    StyleVariant::Mark => Inline::Mark(children),
                    StyleVariant::Superscript => Inline::Superscript(children),
                    StyleVariant::Subscript => Inline::Subscript(children),
                    StyleVariant::Code => Inline::Code(flatten(&children)),
                    StyleVariant::Unquoted if has_role("underline") => Inline::Underline(children),
                    StyleVariant::Unquoted if has_role("line-through") => {
                        Inline::Strikethrough(children)
                    }
                    StyleVariant::DoubleQuote | StyleVariant::SingleQuote => {
                        let (open, close) = match styled.variant {
                            StyleVariant::DoubleQuote => ("\u{201c}", "\u{201d}"),
                            _ => ("\u{2018}", "\u{2019}"),
                        };
                        children.insert(0, Inline::Text(open.to_string()));
                        children.push(Inline::Text(close.to_string()));
                        inlines.extend(children);
                        return;
                    }
                    StyleVariant::Unquoted => {
                        inlines.extend(children);
                        return;
                    }
                });
            }
            InlineNode::Ref(reference) => match reference.variant {
                RefVariant::Link if self.is_include(reference.span()) => {}
                RefVariant::Link => {
                    let content = self.inlines(&reference.children);
                    inlines.push(Inline::Link {
                        target: Target::Url(reference.target.to_string()),
                        title: String::new(),
                        content,
                    });
                }
                RefVariant::Xref => inlines.push(self.xref(reference)),
            },
            InlineNode::Image(image) => {
                let folder = self.value("imagesdir");
                let url = text::in_folder(folder, &image.target);
                let alt = match &image.alt {
                    Some(alt) => alt.to_string(),
                    None => text::default_alt(&image.target),
                };
                let attribute = |name| image.attrs.named_attribute(name).map(|a| a.value());
                let title = attribute("title").map(text::plain).unwrap_or_default();
                let image_inline = Inline::Image { url, alt, title };
                inlines.push(match attribute("link") {
                    Some(link) => Inline::Link {
                        target: Target::Url(link.to_string()),
                        title: String::new(),
                        content: vec![image_inline],
                    },
                    None => image_inline,
                });
            }
            InlineNode::Footnote(footnote) if footnote.is_reference => {
                let id = footnote.id.as_deref().unwrap_or_default();
                match self.footnote_ids.get(id) {
                    Some(label) => inlines.push(Inline::Footnote(label.clone())),
                    None => {
                        let line = self.line(footnote.span());
                        self.warn(
                            line,
                            format!("left out a reference to footnote `{id}`, which isn't defined"),
                        );
                    }
                }
            }
            InlineNode::Footnote(footnote) => {
                let label = (self.footnotes.len() + 1).to_string();
                if let Some(id) = &footnote.id {
                    self.footnote_ids.insert(id.to_string(), label.clone());
                }
                // Holds its place, so a footnote inside it is numbered after it.
                self.footnotes.push(Footnote {
                    label: label.clone(),
                    blocks: Vec::new(),
                });
                let index = self.footnotes.len() - 1;
                let content = self.inlines(&footnote.children);
                self.footnotes[index].blocks = vec![Block::Paragraph(content)];
                inlines.push(Inline::Footnote(label));
            }
            InlineNode::Anchor(anchor) => inlines.push(Inline::Anchor(anchor.id.to_string())),
            InlineNode::Stem(stem) => {
                let source = stem
                    .source_text
                    .clone()
                    .unwrap_or_else(|| stem.value.to_string());
                match stem.notation {
                    StemNotation::LatexMath => inlines.push(Inline::Math(source)),
                    StemNotation::AsciiMath => {
                        let line = self.line(stem.span());
                        self.warn(
                            line,
                            format!(
                                "kept AsciiMath `{source}` as code, as little writes only TeX math"
                            ),
                        );
                        inlines.push(Inline::Code(source));
                    }
                }
            }
            InlineNode::LineBreak { .. } => inlines.push(Inline::LineBreak),
            _ => {}
        }
    }

    /// A cross-reference, as a link: to an id on the page, or, with a file
    /// in its target, to the file as it's written, which little leads to
    /// its page.
    fn xref(&mut self, reference: &Ref<'_>) -> Inline {
        let written = reference.target.as_ref();
        let catalog = self.doc.catalog();
        // asciidoc-parser derives a destination only for a reference to
        // another document, where it keeps the target as written.
        let target = if reference.derived.is_some() && !written.is_empty() {
            Target::Url(text::document(written))
        } else if written.ends_with(".adoc") || written.ends_with(".asciidoc") {
            // `<<other.adoc>>` is an id to Asciidoctor, which nothing has.
            Target::Url(written.to_string())
        } else if catalog.contains_id(written) {
            Target::Id(written.to_string())
        } else {
            match catalog.resolve_id(written) {
                Some(id) => Target::Id(id),
                None if written.contains(' ') => Target::Heading(written.to_string()),
                None => Target::Id(written.to_string()),
            }
        };
        let mut content = self.inlines(&reference.children);
        if content.is_empty()
            && let Target::Id(id) = &target
            && let Some(entry) = catalog.get_ref(id)
            && entry.ref_type != RefType::Section
            && let Some(reftext) = &entry.reftext
        {
            // A heading's text, the library gives the link itself.
            content = vec![Inline::Text(text::plain(reftext))];
        }
        Inline::Link {
            target,
            title: String::new(),
            content,
        }
    }

    /// Warns of each macro in `value`, text at `location`, that
    /// asciidoc-parser left as text, as it doesn't read it.
    fn check_macros(&mut self, value: &str, location: Span<'_>) {
        let found = text::macros(value);
        if found.is_empty() {
            return;
        }
        let first = self.line(location);
        for (at, written, name) in found {
            let line = first + value[..at].matches('\n').count();
            let message = if name == "include" {
                // As in a Markdown-style quote, which Asciidoctor doesn't
                // read includes in either.
                format!("kept `{written}` as text, as the plugin doesn't follow includes")
            } else if EXPERIMENTAL.contains(&name) {
                format!(
                    "kept `{written}` as text, as `{name}` needs `:experimental:`, which isn't set"
                )
            } else {
                format!("kept `{written}` as text, as `{name}` isn't a macro the plugin reads")
            };
            self.warn(line, message);
        }
    }
}

/// A callout in code, as it's written, like `// <1>`.
fn marker(callout: &Callout<'_>) -> String {
    match &callout.guard {
        CalloutGuard::LineComment(prefix) => format!("{prefix}<{}>", callout.number),
        CalloutGuard::Xml => format!("<!--{}-->", callout.number),
    }
}

/// Adds `run`, text as HTML, to `inlines`, decoded, and empties it.
fn flush(run: &mut String, inlines: &mut Vec<Inline>) {
    if !run.is_empty() {
        inlines.push(Inline::Text(text::decode(run)));
        run.clear();
    }
}

/// The text of `inlines`, without their markup, as code shows it.
pub(crate) fn flatten(inlines: &[Inline]) -> String {
    let mut flat = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text) | Inline::Code(text) | Inline::Math(text) => flat.push_str(text),
            Inline::Emphasis(children)
            | Inline::Strong(children)
            | Inline::Strikethrough(children)
            | Inline::Underline(children)
            | Inline::Mark(children)
            | Inline::Superscript(children)
            | Inline::Subscript(children)
            | Inline::Link {
                content: children, ..
            } => flat.push_str(&flatten(children)),
            Inline::Image { alt, .. } => flat.push_str(alt),
            Inline::Html { html, .. } => flat.push_str(&text::plain(html)),
            Inline::LineBreak => flat.push(' '),
            _ => {}
        }
    }
    flat
}

/// The text of `nodes`, a verbatim block's, as it's written, but for what
/// its substitutions change, like an attribute reference, with each callout
/// as it's written.
pub(crate) fn verbatim(nodes: &[InlineNode<'_>]) -> String {
    fn html(nodes: &[InlineNode<'_>], out: &mut String) {
        for node in nodes {
            match node {
                InlineNode::Text { value, .. } => out.push_str(&text::escape(value)),
                InlineNode::CharRef { value, .. } => match value {
                    CharRef::Special(c) => out.push_str(&text::escape(&c.to_string())),
                    CharRef::Replacement(replacement) => out.push_str(replacement),
                    CharRef::Entity(entity) => out.push_str(entity),
                },
                InlineNode::Raw {
                    value,
                    form: RawForm::Escaped,
                    ..
                } => out.push_str(&text::escape(value)),
                InlineNode::Raw { value, .. } => out.push_str(value),
                InlineNode::Callout(callout) => out.push_str(&text::escape(&marker(callout))),
                InlineNode::Styled(styled) => html(&styled.children, out),
                InlineNode::Ref(reference) if !reference.children.is_empty() => {
                    html(&reference.children, out)
                }
                InlineNode::LineBreak { .. } => {}
                node => out.push_str(&text::escape(node.span().data())),
            }
        }
    }
    let mut out = String::new();
    html(nodes, &mut out);
    text::decode(&out)
}
