//! Makes the tree of a file a document, as docutils' transforms do once
//! it's read: the title of a lone section is the document's title, and that
//! of a lone subsection after it the subtitle, as `DocTitle` makes them; a
//! field list after them is the docinfo, as `DocInfo` makes it; and each
//! reference is led to its target, with [`crate::resolve`].

use little_markup::{Block, Document, Inline, Warning};

use crate::Clock;
use crate::body;
use crate::lines;
use crate::resolve::{Resolver, propagate};
use crate::state::State;
use crate::text::normalize;
use crate::tree::{Field, Node, Span};

/// The bibliographic fields of the docinfo, by their names, normalized,
/// and their labels.
const FIELDS: [(&str, &str); 12] = [
    ("author", "Author"),
    ("authors", "Authors"),
    ("organization", "Organization"),
    ("address", "Address"),
    ("contact", "Contact"),
    ("version", "Version"),
    ("revision", "Revision"),
    ("status", "Status"),
    ("date", "Date"),
    ("copyright", "Copyright"),
    ("dedication", "Dedication"),
    ("abstract", "Abstract"),
];

/// The document of the reStructuredText `rest`, below its YAML
/// `frontmatter`, built at `clock`, and the warnings of what's left out or
/// changed, in the order of their lines.
pub(crate) fn document(rest: &str, frontmatter: &str, clock: Clock) -> (Document, Vec<Warning>) {
    let mut state = State::new(clock);
    let lines = lines::split(rest);
    let mut nodes = body::parse(&mut state, &lines, true);
    propagate(&mut state, &mut nodes);
    let title = promote(&mut nodes, 1);
    let subtitle = title.as_ref().and_then(|_| promote(&mut nodes, 2));
    let docinfo = docinfo(&mut nodes);
    let mut resolver = Resolver::new(&mut state);
    resolver.promoted = usize::from(title.is_some()) + usize::from(subtitle.is_some());
    let mut blocks = Vec::new();
    if let Some((title, ids)) = title {
        let content = resolver.inlines(title);
        blocks.push(Block::Heading {
            level: 1,
            content,
            ids,
        });
    }
    if let Some((subtitle, ids)) = subtitle {
        let mut content: Vec<Inline> = ids.into_iter().map(Inline::Anchor).collect();
        content.push(Inline::Emphasis(resolver.inlines(subtitle)));
        blocks.push(Block::Paragraph(content));
    }
    blocks.extend(resolver.blocks(docinfo));
    blocks.extend(resolver.blocks(nodes));
    let footnotes = resolver.footnotes();
    let mut warnings = std::mem::take(&mut state.warnings);
    warnings.sort_by_key(|warning| warning.line);
    let document = Document {
        frontmatter: frontmatter.to_string(),
        metadata: std::mem::take(&mut state.metadata),
        blocks,
        footnotes,
    };
    (document, warnings)
}

/// Whether docutils lets `node` come before the document's title and its
/// docinfo, as it lets what shows nothing, raw output, and errors.
fn prebib(node: &Node) -> bool {
    matches!(
        node,
        Node::Nothing { prebib: true }
            | Node::Target(_)
            | Node::Anchor(_)
            | Node::Html(..)
            | Node::Problem(_)
    )
}

/// The title of the lone section at `depth`, and its ids, taken out of
/// `nodes`, if it's the first block that docutils doesn't let come before a
/// title, and no other section at its depth, or above, follows it.
fn promote(nodes: &mut Vec<Node>, depth: usize) -> Option<(Vec<Span>, Vec<String>)> {
    let at = nodes.iter().position(|node| !prebib(node))?;
    let lone = matches!(nodes[at], Node::Heading { depth: found, .. } if found == depth)
        && !nodes[at + 1..]
            .iter()
            .any(|node| matches!(node, Node::Heading { depth: found, .. } if *found <= depth));
    match lone.then(|| nodes.remove(at)) {
        Some(Node::Heading { title, ids, .. }) => Some((title, ids)),
        _ => None,
    }
}

/// The docinfo, if the first block that docutils doesn't let come before
/// a title is a field list, taken out of `nodes`: a description list of its
/// fields, each bibliographic one by its label, then its dedication and its
/// abstract, as topics.
fn docinfo(nodes: &mut Vec<Node>) -> Vec<Node> {
    let at = nodes.iter().position(|node| !prebib(node));
    let Some(at) = at.filter(|&at| matches!(nodes[at], Node::Fields(_))) else {
        return Vec::new();
    };
    let Node::Fields(fields) = nodes.remove(at) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    let mut topics: [Vec<Node>; 2] = Default::default();
    for field in fields {
        let name = normalize(&Span::text(&field.name));
        let found = FIELDS.iter().find(|(key, _)| *key == name);
        let Some(&(key, label)) = found.filter(|_| !field.body.is_empty()) else {
            items.push(generic(field));
            continue;
        };
        let term = vec![Span::Text(label.to_string())];
        match key {
            "authors" => match authors(&field.body) {
                Some(body) => items.push((term, body)),
                None => items.push(generic(field)),
            },
            "dedication" | "abstract" => {
                let topic = &mut topics[usize::from(key == "abstract")];
                if !topic.is_empty() {
                    items.push(generic(field));
                    continue;
                }
                topic.push(Node::Para(vec![Span::Strong(term)]));
                topic.extend(field.body);
            }
            _ => match <[Node; 1]>::try_from(field.body) {
                Ok([Node::Para(mut spans)]) => {
                    clean(&mut spans);
                    if key == "address" {
                        spans = breaks(spans);
                    }
                    items.push((term, vec![Node::Para(spans)]));
                }
                Ok([node]) => items.push((field.name, vec![node])),
                Err(body) => items.push((field.name, body)),
            },
        }
    }
    let mut docinfo = Vec::new();
    if !items.is_empty() {
        docinfo.push(Node::Definitions(items));
    }
    docinfo.extend(topics.into_iter().flatten());
    docinfo
}

/// A field of the docinfo as it's written, with the RCS keywords in its
/// body cleaned, if it's a paragraph.
fn generic(mut field: Field) -> (Vec<Span>, Vec<Node>) {
    if let [Node::Para(spans)] = field.body.as_mut_slice() {
        clean(spans);
    }
    (field.name, field.body)
}

/// The authors in the body of an `Authors` field, a paragraph each, as
/// docutils finds them: in one paragraph, split at each `;`, else each
/// `,`; in a bullet list, an item each; or in paragraphs, one each.
fn authors(body: &[Node]) -> Option<Vec<Node>> {
    let names: Vec<Vec<Span>> = match body {
        [Node::Para(spans)] => {
            let text = Span::text(spans);
            let names = [';', ',']
                .into_iter()
                .map(|separator| text.split(separator).collect::<Vec<_>>())
                .find(|names| names.len() > 1)
                .unwrap_or_else(|| vec![text.as_str()]);
            names
                .into_iter()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(|name| vec![Span::Text(name.to_string())])
                .collect()
        }
        [Node::List { start: None, items }] => items
            .iter()
            .map(|item| match item.as_slice() {
                [Node::Para(spans)] => Some(spans.clone()),
                _ => None,
            })
            .collect::<Option<_>>()?,
        _ => body
            .iter()
            .filter(|node| !matches!(node, Node::Nothing { prebib: true }))
            .map(|node| match node {
                Node::Para(spans) => Some(spans.clone()),
                _ => None,
            })
            .collect::<Option<_>>()?,
    };
    (!names.is_empty()).then(|| names.into_iter().map(Node::Para).collect())
}

/// `spans`, with a line break at each line's end, as docutils keeps an
/// address's lines.
fn breaks(spans: Vec<Span>) -> Vec<Span> {
    let mut out = Vec::new();
    for span in spans {
        let Span::Text(text) = span else {
            out.push(span);
            continue;
        };
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                out.push(Span::Break);
            }
            if !line.is_empty() {
                out.push(Span::Text(line.to_string()));
            }
        }
    }
    out
}

/// Clean the RCS keywords in `spans`, if they're only text, as docutils
/// cleans them in the docinfo.
fn clean(spans: &mut Vec<Span>) {
    if spans.iter().all(|span| matches!(span, Span::Text(_)))
        && let Some(cleaned) = clean_rcs(&Span::text(spans))
    {
        *spans = vec![Span::Text(cleaned)];
    }
}

/// What matches a pattern at the start of some text: the length of the
/// match, and what replaces it.
type Pattern = fn(&str) -> Option<(usize, String)>;

/// `text`, with each RCS keyword in it cleaned, as docutils cleans them,
/// with the first of its patterns that matches: `$Date: 2002/08/20
/// 13:52:58 $` as `2002-08-20`, `$RCSfile: name,v $` as `name`, and
/// `$Keyword: value $` as `value`; or none, if none matches.
fn clean_rcs(text: &str) -> Option<String> {
    let patterns: [Pattern; 3] = [rcs_date, rcs_file, rcs_keyword];
    patterns
        .into_iter()
        .find_map(|pattern| replace_all(text, pattern))
}

/// `text`, with each match of `pattern` replaced, if it has one: `pattern`
/// gives the length of a match at the start of the text it's given, which
/// is at a `$`, and what replaces it.
fn replace_all(text: &str, pattern: Pattern) -> Option<String> {
    let mut out = String::new();
    let mut rest = text;
    let mut found = false;
    while let Some(at) = rest.find('$') {
        if let Some((length, replacement)) = pattern(&rest[at..]) {
            out.push_str(&rest[..at]);
            out.push_str(&replacement);
            rest = &rest[at + length..];
            found = true;
        } else {
            out.push_str(&rest[..=at]);
            rest = &rest[at + 1..];
        }
    }
    out.push_str(rest);
    found.then_some(out)
}

/// `text` after `prefix`, which it starts with, whatever their case.
fn strip_prefix_ignoring_case<'t>(text: &'t str, prefix: &str) -> Option<&'t str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// `\$Date: (\d{4})[-/](\d\d)[-/](\d\d)[ T][\d:]+[^$]* \$`, whatever its
/// case, as `Y-M-D`.
fn rcs_date(text: &str) -> Option<(usize, String)> {
    const PREFIX: &str = "$date: ";
    let rest = strip_prefix_ignoring_case(text, PREFIX)?;
    let bytes = rest.as_bytes();
    let digits = |from: usize, to: usize| {
        bytes
            .get(from..to)
            .is_some_and(|digits| digits.iter().all(u8::is_ascii_digit))
    };
    let separator = |at: usize| matches!(bytes.get(at), Some(b'-' | b'/'));
    let date = digits(0, 4) && separator(4) && digits(5, 7) && separator(7) && digits(8, 10);
    if !date
        || !matches!(bytes.get(10), Some(b' ' | b'T' | b't'))
        || !matches!(bytes.get(11), Some(b'0'..=b'9' | b':'))
    {
        return None;
    }
    let end = 12 + rest[12..].find('$')?;
    let cleaned = format!("{}-{}-{}", &rest[..4], &rest[5..7], &rest[8..10]);
    (bytes[end - 1] == b' ').then_some((PREFIX.len() + end + 1, cleaned))
}

/// `\$RCSfile: (.+),v \$`, whatever its case, as its name.
fn rcs_file(text: &str) -> Option<(usize, String)> {
    const PREFIX: &str = "$rcsfile: ";
    let rest = strip_prefix_ignoring_case(text, PREFIX)?;
    let line = &rest[..rest.find('\n').unwrap_or(rest.len())];
    let end = line
        .to_ascii_lowercase()
        .rfind(",v $")
        .filter(|&end| end > 0)?;
    Some((PREFIX.len() + end + 4, line[..end].to_string()))
}

/// `\$[a-zA-Z]+: (.+) \$`, as its value.
fn rcs_keyword(text: &str) -> Option<(usize, String)> {
    let letters = text[1..]
        .bytes()
        .take_while(u8::is_ascii_alphabetic)
        .count();
    let rest = text[1 + letters..]
        .strip_prefix(": ")
        .filter(|_| letters > 0)?;
    let line = &rest[..rest.find('\n').unwrap_or(rest.len())];
    let end = line.rfind(" $").filter(|&end| end > 0)?;
    Some((1 + letters + 2 + end + 2, line[..end].to_string()))
}
