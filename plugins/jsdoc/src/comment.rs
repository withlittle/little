//! Doc comments, `/** ... */`, as JSDoc and TSDoc write them: a
//! description, in Markdown, then block tags, like `@param`, and the
//! sections of a page they make.

use little_api_reference::markdown::fence_for;
use little_api_reference::model::{Item, Items, Section};

/// A doc comment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Comment {
    /// The text before its first tag, and that of its `@description`.
    pub description: String,
    /// Its block tags, in order.
    pub tags: Vec<Tag>,
}

/// A block tag of a doc comment, like `@param {string} name - Its name.`
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    /// Its name, without the `@`, like `param`.
    pub name: String,
    /// What follows its name, up to the next tag.
    pub text: String,
}

/// The tags that can end an `@example`, whose code may have lines that
/// start with a decorator, like `@Component({...})`.
const KNOWN: &[&str] = &[
    "abstract",
    "access",
    "alias",
    "arg",
    "argument",
    "async",
    "augments",
    "author",
    "callback",
    "class",
    "constant",
    "constructor",
    "copyright",
    "default",
    "defaultValue",
    "deprecated",
    "desc",
    "description",
    "enum",
    "example",
    "exception",
    "experimental",
    "extends",
    "file",
    "fileoverview",
    "function",
    "hidden",
    "ignore",
    "implements",
    "inheritDoc",
    "interface",
    "internal",
    "license",
    "memberof",
    "method",
    "module",
    "name",
    "namespace",
    "overload",
    "override",
    "overview",
    "package",
    "packageDocumentation",
    "param",
    "private",
    "prop",
    "property",
    "protected",
    "public",
    "readonly",
    "remarks",
    "return",
    "returns",
    "see",
    "since",
    "static",
    "summary",
    "template",
    "this",
    "throws",
    "todo",
    "type",
    "typeParam",
    "typedef",
    "typeparam",
    "version",
    "virtual",
    "yield",
    "yields",
];

/// The order a page shows labels in.
const LABELS: [&str; 9] = [
    "default",
    "abstract",
    "static",
    "protected",
    "readonly",
    "optional",
    "override",
    "async",
    "deprecated",
];

impl Comment {
    /// The comment whose text, between its `/*` and `*/`, is `content`.
    pub fn parse(content: &str) -> Comment {
        let lines = lines(content.strip_prefix('*').unwrap_or(content));
        let mut description: Vec<&str> = Vec::new();
        // Each tag's name, the rest of its first line, and its other lines.
        let mut tags: Vec<(&str, &str, Vec<&str>)> = Vec::new();
        let mut fence: Option<&str> = None;
        for &line in &lines {
            match fence {
                Some(open) if closing(line, open) => fence = None,
                Some(_) => {}
                None => {
                    if let Some((_, open, _)) = opening(line) {
                        fence = Some(open);
                    } else if let Some((name, rest)) = tag(line, tags.last().map(|tag| tag.0)) {
                        tags.push((name, rest.trim(), Vec::new()));
                        continue;
                    }
                }
            }
            match tags.last_mut() {
                Some((_, _, body)) => body.push(line),
                None => description.push(line),
            }
        }
        let tags: Vec<Tag> = (tags.into_iter())
            .map(|(name, first, body)| Tag {
                name: name.to_string(),
                text: trimmed(&format!("{first}\n{}", unindent(&body))),
            })
            .collect();
        let described = (tags.iter())
            .filter(|tag| matches!(tag.name.as_str(), "description" | "desc"))
            .map(|tag| tag.text.clone());
        let description: Vec<String> = std::iter::once(trimmed(&description.join("\n")))
            .chain(described)
            .filter(|text| !text.is_empty())
            .collect();
        Comment {
            description: description.join("\n\n"),
            tags,
        }
    }

    /// The text of each of its tags called one of `names`, in order.
    pub fn texts(&self, names: &'static [&'static str]) -> impl Iterator<Item = &str> {
        (self.tags.iter())
            .filter(move |tag| names.contains(&tag.name.as_str()))
            .map(|tag| tag.text.as_str())
    }

    /// Whether it has a tag called `name`.
    pub fn has(&self, name: &str) -> bool {
        self.tags.iter().any(|tag| tag.name == name)
    }

    /// Whether it hides what it's on: whether it has `@private`,
    /// `@internal`, `@ignore`, `@hidden`, or `@access private`.
    pub fn is_hidden(&self) -> bool {
        (self.tags.iter()).any(|tag| {
            matches!(
                tag.name.as_str(),
                "private" | "internal" | "ignore" | "hidden"
            ) || (tag.name == "access" && first_word(&tag.text) == "private")
        })
    }

    /// Whether it documents its module: whether it has `@module`, `@file`,
    /// `@fileoverview`, `@overview`, or `@packageDocumentation`.
    pub fn is_module(&self) -> bool {
        (self.tags.iter()).any(|tag| {
            matches!(
                tag.name.as_str(),
                "module" | "file" | "fileoverview" | "overview" | "packageDocumentation"
            )
        })
    }

    /// Whether it documents no code after it: its module, a `@typedef`, a
    /// `@callback`, or a licence.
    pub fn is_standalone(&self) -> bool {
        self.is_module()
            || (self.tags.iter()).any(|tag| {
                matches!(
                    tag.name.as_str(),
                    "typedef" | "callback" | "license" | "preserve" | "copyright"
                )
            })
    }

    /// The labels its tags give what it's on, like `deprecated`.
    pub fn labels(&self) -> Vec<&'static str> {
        let labels = (self.tags.iter()).filter_map(|tag| match tag.name.as_str() {
            "abstract" | "virtual" => Some("abstract"),
            "readonly" => Some("readonly"),
            "async" => Some("async"),
            "static" => Some("static"),
            "protected" => Some("protected"),
            "access" if first_word(&tag.text) == "protected" => Some("protected"),
            "override" => Some("override"),
            "deprecated" => Some("deprecated"),
            _ => None,
        });
        labels.collect()
    }

    /// What each of its `@param`s says, but those of a parameter's
    /// properties, like `options.name`.
    pub fn params(&self) -> Vec<Param> {
        let params = self.texts(&["param", "arg", "argument"]).map(param);
        params.filter(|param| !param.name.contains('.')).collect()
    }
}

/// `labels` in the order a page shows them, each once.
pub fn ordered<'a>(labels: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut ordered: Vec<&str> = Vec::new();
    for label in labels {
        if !ordered.contains(&label) {
            ordered.push(label);
        }
    }
    ordered.sort_by_key(|label| {
        let known = LABELS.iter().position(|known| known == label);
        known.unwrap_or(LABELS.len())
    });
    ordered.into_iter().map(String::from).collect()
}

/// The lines of a comment's `content`, after its `/**`: the first trimmed,
/// and the others without the `*` each starts with, if each does, or the
/// margin they share.
fn lines(content: &str) -> Vec<&str> {
    let mut lines = (content.split('\n')).map(|line| line.strip_suffix('\r').unwrap_or(line));
    let first = lines.next().unwrap_or_default().trim();
    let rest: Vec<&str> = lines.collect();
    let starred = (rest.iter())
        .filter(|line| !line.trim().is_empty())
        .all(|line| line.trim_start().starts_with('*'));
    let rest: Vec<&str> = (rest.iter())
        .map(|&line| match (line.trim().is_empty(), starred) {
            (true, _) => "",
            (false, true) => &line.trim_start()[1..],
            (false, false) => line,
        })
        .collect();
    let margin = margin(&rest);
    let rest = rest
        .into_iter()
        .map(|line| line.get(margin..).unwrap_or_default());
    std::iter::once(first).chain(rest).collect()
}

/// The margin of spaces and tabs that `lines` share, but blank ones.
fn margin(lines: &[&str]) -> usize {
    (lines.iter())
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0)
}

/// `lines`, without the margin they share, on lines of their own.
fn unindent(lines: &[&str]) -> String {
    let margin = margin(lines);
    let lines: Vec<&str> = (lines.iter())
        .map(|line| line.get(margin..).unwrap_or_default())
        .collect();
    lines.join("\n")
}

/// The name of the tag that `line` starts, if it starts one, and the rest
/// of the line. In an `@example`, the tag `within`, only a known tag's
/// name starts one, and not one followed by `(`, as a decorator's is.
fn tag<'a>(line: &'a str, within: Option<&str>) -> Option<(&'a str, &'a str)> {
    let rest = line.trim_start_matches(' ');
    if line.len() - rest.len() > 3 {
        return None;
    }
    let rest = rest.strip_prefix('@')?;
    let length = (rest.find(|c: char| !c.is_ascii_alphanumeric())).unwrap_or(rest.len());
    let (name, after) = rest.split_at(length);
    let example = within == Some("example") && (!KNOWN.contains(&name) || after.starts_with('('));
    (name.starts_with(|c: char| c.is_ascii_alphabetic()) && !example).then_some((name, after))
}

/// The first word of `text`, or nothing.
fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or_default()
}

/// What a `@param` or `@property` says of a parameter or property.
#[derive(Debug, Default, PartialEq)]
pub struct Param {
    /// Its name, without the `...` of a rest parameter.
    pub name: String,
    /// Its type, if the tag gives it, like `string` for `{string}`.
    pub ty: Option<String>,
    /// Whether it's optional, `[name]`, or `{string=}`.
    pub optional: bool,
    /// Its default, from `[name=default]`.
    pub default: Option<String>,
    pub description: String,
}

/// What the `text` of a `@param` or `@property` says, like
/// `{string} [name="x"] - Its name.`
pub fn param(text: &str) -> Param {
    let (ty, rest) = braced(text);
    let mut param = Param::default();
    param.ty = ty.map(|ty| match ty.strip_suffix('=') {
        Some(ty) => {
            param.optional = true;
            ty.trim_end().to_string()
        }
        None => ty,
    });
    let rest = rest.trim_start();
    let rest = match bracketed(rest) {
        Some((inner, rest)) => {
            param.optional = true;
            let (name, default) = inner.split_once('=').unwrap_or((inner, ""));
            param.name = name.trim().to_string();
            param.default = Some(default.trim().to_string()).filter(|default| !default.is_empty());
            rest
        }
        None => {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            param.name = rest[..end].to_string();
            &rest[end..]
        }
    };
    param.name = param.name.trim_start_matches("...").to_string();
    param.description = description(rest);
    param
}

/// The type in braces that `text` starts with, like `string` for
/// `{string}`, with its whitespace collapsed, and what's after it; a
/// `{@link}` isn't a type.
pub fn braced(text: &str) -> (Option<String>, &str) {
    let text = text.trim_start();
    if !text.starts_with('{') || text.starts_with("{@") {
        return (None, text);
    }
    let mut depth = 0;
    for (index, c) in text.char_indices() {
        match c {
            '{' => depth += 1,
            '}' if depth == 1 => return (Some(collapse(&text[1..index])), &text[index + 1..]),
            '}' => depth -= 1,
            _ => {}
        }
    }
    (None, text)
}

/// What's in the brackets that `text` starts with, like `name="x"` for
/// `[name="x"]`, and what's after them.
fn bracketed(text: &str) -> Option<(&str, &str)> {
    let mut depth = 0;
    for (index, c) in text.char_indices() {
        match c {
            '[' => depth += 1,
            ']' if depth == 1 => return Some((&text[1..index], &text[index + 1..])),
            ']' => depth -= 1,
            _ if index == 0 => return None,
            _ => {}
        }
    }
    None
}

/// `text` with each run of whitespace in it a space.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The description after a tag's name or type, without the `-` it may
/// start with.
pub fn description(text: &str) -> String {
    let text = text.trim_start();
    let text = match text.strip_prefix('-') {
        Some(rest) if rest.is_empty() || rest.starts_with(char::is_whitespace) => rest,
        _ => text,
    };
    text.trim().to_string()
}

/// The type parameters the `text` of a `@template` or `@typeParam` names,
/// like `{string} K, V - The key's and the value's types.`: the type in
/// braces constrains the first.
pub fn template(text: &str) -> Vec<Item> {
    let (constraint, mut rest) = braced(text);
    let mut items: Vec<Item> = Vec::new();
    loop {
        rest = rest.trim_start();
        let (name, default, after) = match bracketed(rest) {
            Some((inner, after)) => {
                let (name, default) = inner.split_once('=').unwrap_or((inner, ""));
                let default = Some(default.trim().to_string()).filter(|text| !text.is_empty());
                (name.trim(), default, after)
            }
            None => {
                let end = (rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')))
                    .unwrap_or(rest.len());
                (&rest[..end], None, &rest[end..])
            }
        };
        if name.is_empty() {
            break;
        }
        items.push(Item {
            name: Some(name.to_string()),
            ty: constraint.clone().filter(|_| items.is_empty()),
            default,
            description: String::new(),
        });
        rest = after.trim_start();
        match rest.strip_prefix(',') {
            Some(after) => rest = after,
            None => break,
        }
    }
    let description = description(rest);
    for item in &mut items {
        item.description = description.clone();
    }
    items
}

/// What the code says of what a comment is on, for the sections it makes.
#[derive(Debug, Default)]
pub struct Context {
    /// The language of its examples, like `ts`.
    pub language: &'static str,
    /// Whether the code gives its type, so a `@type` isn't shown.
    pub typed: bool,
    /// Its type parameters, from the code, each with the constraint and
    /// default that win over a `@template`'s.
    pub type_parameters: Vec<Item>,
    /// The properties of the type it defines, from the code.
    pub properties: Vec<Item>,
    /// The members of the enum it defines, with their values.
    pub members: Vec<Item>,
}

/// The sections of a page that `comment` makes, on what `context`
/// describes: its description, split at its headings, then a section for
/// each kind of tag that makes one, in the order a page shows them.
pub fn sections(comment: &Comment, context: &Context) -> Vec<Section> {
    let mut sections = headed(&comment.description);
    for text in comment.texts(&["deprecated"]) {
        titled(&mut sections, "Deprecated", text);
    }
    for text in comment.texts(&["remarks"]) {
        titled(&mut sections, "Remarks", text);
    }
    let mut type_parameters: Vec<Item> = (comment.texts(&["template", "typeParam", "typeparam"]))
        .flat_map(template)
        .collect();
    for item in &mut type_parameters {
        if let Some(code) = (context.type_parameters.iter()).find(|code| code.name == item.name) {
            item.ty = code.ty.clone().or(item.ty.take());
            item.default = code.default.clone().or(item.default.take());
        }
    }
    listed(&mut sections, Items::TypeParameters, type_parameters);
    let parameters = (comment.texts(&["param", "arg", "argument"])).map(|text| {
        let param = param(text);
        Item {
            name: Some(param.name),
            ty: param.ty,
            default: param.default,
            description: param.description,
        }
    });
    listed(&mut sections, Items::Parameters, parameters.collect());
    listed(
        &mut sections,
        Items::Properties,
        properties(comment, &context.properties),
    );
    if (context.members.iter()).any(|member| !member.description.is_empty()) {
        listed(&mut sections, Items::Members, context.members.clone());
    }
    listed(
        &mut sections,
        Items::Returns,
        typed(comment.texts(&["returns", "return"])),
    );
    listed(
        &mut sections,
        Items::Yields,
        typed(comment.texts(&["yields", "yield"])),
    );
    listed(
        &mut sections,
        Items::Throws,
        typed(comment.texts(&["throws", "exception"])),
    );
    if !context.typed {
        let ty = comment.texts(&["type"]).find_map(|text| braced(text).0);
        if let Some(ty) = ty {
            titled(&mut sections, "Type", &code(&ty));
        }
    }
    let defaults = comment.texts(&["default", "defaultValue", "defaultvalue"]);
    for text in defaults.filter(|text| !text.is_empty()) {
        let text = match text.contains('`') {
            true => text.to_string(),
            false => code(text),
        };
        titled(&mut sections, "Default", &text);
    }
    let examples: Vec<String> = (comment.texts(&["example"]))
        .map(|text| example(text, context.language))
        .filter(|text| !text.is_empty())
        .collect();
    let title = if examples.len() == 1 {
        "Example"
    } else {
        "Examples"
    };
    titled(&mut sections, title, &examples.join("\n\n"));
    for text in comment.texts(&["since"]) {
        titled(&mut sections, "Since", text);
    }
    let see: Vec<String> = (comment.texts(&["see"]))
        .filter(|text| !text.is_empty())
        .map(|text| format!("- {}", see(text).replace('\n', "\n  ")))
        .collect();
    titled(&mut sections, "See Also", &see.join("\n"));
    sections
}

/// The properties `comment`'s `@property`s list, and, if they list any or
/// one of them has a description, those of the type it's on, from `code`:
/// the code's first, in its order, with their types from it, then the
/// others. An optional property's name ends with `?`.
fn properties(comment: &Comment, code: &[Item]) -> Vec<Item> {
    let documented: Vec<Item> = (comment.texts(&["property", "prop"]))
        .map(|text| {
            let param = param(text);
            Item {
                name: Some(param.name + if param.optional { "?" } else { "" }),
                ty: param.ty,
                default: param.default,
                description: param.description,
            }
        })
        .collect();
    let bare = |item: &Item| {
        let name = item.name.as_deref().unwrap_or_default();
        name.trim_end_matches('?').to_string()
    };
    let described = code.iter().any(|item| !item.description.is_empty());
    if documented.is_empty() && !described {
        return Vec::new();
    }
    let mut properties: Vec<Item> = Vec::new();
    for item in code {
        let mut item = item.clone();
        if let Some(doc) = documented.iter().find(|doc| bare(doc) == bare(&item)) {
            item.ty = item.ty.or_else(|| doc.ty.clone());
            item.default = item.default.or_else(|| doc.default.clone());
            if !doc.description.is_empty() {
                item.description = doc.description.clone();
            }
        }
        properties.push(item);
    }
    let named: Vec<String> = code.iter().map(bare).collect();
    properties.extend((documented.into_iter()).filter(|doc| !named.contains(&bare(doc))));
    properties
}

/// What each of `texts`, of a `@returns`, `@yields`, or `@throws`, says:
/// the type in braces it starts with, if it has one, and a description.
fn typed<'a>(texts: impl Iterator<Item = &'a str>) -> Vec<Item> {
    let items = texts.map(|text| {
        let (ty, rest) = braced(text);
        Item {
            name: None,
            ty,
            default: None,
            description: description(rest),
        }
    });
    items
        .filter(|item| item.ty.is_some() || !item.description.is_empty())
        .collect()
}

/// The Markdown of an `@example`'s `text`: its `<caption>`, if it has one,
/// then its code, in a code block in `language`, unless it has its own.
fn example(text: &str, language: &str) -> String {
    let caption = (text.strip_prefix("<caption>")).and_then(|rest| rest.split_once("</caption>"));
    let (caption, code) = match caption {
        Some((caption, code)) => (caption.trim(), trimmed(code)),
        None => ("", text.to_string()),
    };
    let fenced = code.split('\n').any(|line| opening(line).is_some());
    let code = match fenced || code.is_empty() {
        true => code,
        false => {
            let fence = fence_for(&code);
            format!("{fence}{language}\n{code}\n{fence}")
        }
    };
    let parts: Vec<&str> = [caption, &code]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();
    parts.join("\n\n")
}

/// The Markdown of a `@see`'s `text`: a link to the URL it starts with, or
/// to the name it starts with, if that's all it has, or a `-` follows it;
/// or else its text.
fn see(text: &str) -> String {
    let text = text.trim();
    if text.contains("{@link") {
        return text.to_string();
    }
    let (word, rest) = text.split_at(text.find(char::is_whitespace).unwrap_or(text.len()));
    let rest = rest.trim_start();
    if word.starts_with("http://") || word.starts_with("https://") {
        return format!("<{word}> {rest}").trim_end().to_string();
    }
    let name = (word.chars()).all(|c| c.is_alphanumeric() || "_$.#~:/-@'\"()".contains(c));
    match name && (rest.is_empty() || rest.starts_with('-')) {
        true => format!("{{@link {word}}} {rest}").trim_end().to_string(),
        false => text.to_string(),
    }
}

/// `text` in code.
fn code(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let ticks = "`".repeat(longest + 1);
    let pad = if longest > 0 { " " } else { "" };
    format!("{ticks}{pad}{text}{pad}{ticks}")
}

/// `text` without the blank lines it starts with, or the whitespace it ends
/// with.
pub fn trimmed(text: &str) -> String {
    let first = text.len() - text.trim_start().len();
    let start = text[..first].rfind('\n').map_or(0, |newline| newline + 1);
    text[start..].trim_end().to_string()
}

/// The indent, fence, and info string of `line`, if it opens a fenced code
/// block.
fn opening(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.trim_start_matches(' ');
    let indent = &line[..line.len() - rest.len()];
    let mark = rest.chars().next().filter(|&c| c == '`' || c == '~')?;
    let length = rest.len() - rest.trim_start_matches(mark).len();
    let (fence, info) = rest.split_at(length);
    let opens = indent.len() <= 3 && length >= 3 && !(mark == '`' && info.contains('`'));
    opens.then(|| (indent, fence, info.trim()))
}

/// Whether `line` closes the code block `fence` opened.
fn closing(line: &str, fence: &str) -> bool {
    let rest = line.trim_start_matches(' ');
    let mark = fence.chars().next().unwrap_or('`');
    let length = rest.len() - rest.trim_start_matches(mark).len();
    line.len() - rest.len() <= 3 && length >= fence.len() && rest[length..].trim().is_empty()
}

/// The sections of a description, `text`: what's before its first
/// heading, untitled, then each heading's, titled by it, but those with no
/// text.
fn headed(text: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut title = None;
    let mut lines = Vec::new();
    let mut fence: Option<&str> = None;
    for line in text.split('\n') {
        match fence {
            Some(open) if closing(line, open) => fence = None,
            Some(_) => {}
            None => {
                if let Some((_, open, _)) = opening(line) {
                    fence = Some(open);
                } else if let Some(heading) = heading(line) {
                    push(&mut sections, title.replace(heading), &lines.join("\n"));
                    lines.clear();
                    continue;
                }
            }
        }
        lines.push(line);
    }
    push(&mut sections, title, &lines.join("\n"));
    sections
}

/// The title of the heading `line` is, if it's one, like `Notes` for
/// `## Notes`.
fn heading(line: &str) -> Option<String> {
    let rest = line.trim_start_matches(' ');
    let level = rest.len() - rest.trim_start_matches('#').len();
    let text = &rest[level..];
    let starts = line.len() - rest.len() <= 3 && (1..=6).contains(&level);
    if !starts || !(text.is_empty() || text.starts_with([' ', '\t'])) {
        return None;
    }
    let title = text.trim();
    let unclosed = title.trim_end_matches('#');
    let title = match unclosed.is_empty() || unclosed.ends_with([' ', '\t']) {
        true => unclosed.trim_end(),
        false => title,
    };
    (!title.is_empty()).then(|| title.to_string())
}

/// Push the section titled `title`, of `text`, to `sections`, if it has
/// text.
fn push(sections: &mut Vec<Section>, title: Option<String>, text: &str) {
    let text = trimmed(text);
    if !text.is_empty() {
        sections.push(Section::Text { title, text });
    }
}

/// Push the section titled `title`, of `text`, to `sections`, if it has
/// text.
fn titled(sections: &mut Vec<Section>, title: &str, text: &str) {
    push(sections, Some(title.to_string()), text);
}

/// Push the section that lists `items`, of `kind`, to `sections`, if it
/// lists any.
fn listed(sections: &mut Vec<Section>, kind: Items, items: Vec<Item>) {
    if !items.is_empty() {
        sections.push(Section::Items { kind, items });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(name: &str, text: &str) -> Tag {
        Tag {
            name: name.to_string(),
            text: text.to_string(),
        }
    }

    fn titles(sections: &[Section]) -> Vec<String> {
        let titles = sections.iter().map(|section| match section {
            Section::Text { title, .. } => title.clone().unwrap_or_default(),
            Section::Items { kind, .. } => kind.title().to_string(),
        });
        titles.collect()
    }

    #[test]
    fn a_comment_has_a_description_then_its_tags() {
        let comment = Comment::parse(
            "*\n * Adds two numbers.\n *\n * Even  big ones.\n * @param {number} a - The first.\n \
             * @param {number} [b=1] The second,\n *   if any.\n * @returns {number} Their sum.\n ",
        );
        assert_eq!(comment.description, "Adds two numbers.\n\nEven  big ones.");
        assert_eq!(
            comment.tags,
            [
                tag("param", "{number} a - The first."),
                tag("param", "{number} [b=1] The second,\nif any."),
                tag("returns", "{number} Their sum."),
            ]
        );
        assert_eq!(
            Comment::parse("* @type {number} ").tags,
            [tag("type", "{number}")]
        );
        let unstarred = Comment::parse("*\n  Adds.\n\n  @since 2.0\n");
        assert_eq!(unstarred.description, "Adds.");
        assert_eq!(unstarred.tags, [tag("since", "2.0")]);
    }

    #[test]
    fn code_blocks_and_decorators_in_examples_dont_start_tags() {
        let comment = Comment::parse(
            "*\n * Uses a decorator:\n * ```ts\n * @Component({})\n * class A {}\n * ```\n \
             * @example\n * @Injectable()\n * class B {}\n * @since 1.0\n ",
        );
        let description = "Uses a decorator:\n```ts\n@Component({})\nclass A {}\n```";
        assert_eq!(comment.description, description);
        let example = tag("example", "@Injectable()\nclass B {}");
        assert_eq!(comment.tags, [example, tag("since", "1.0")]);
        let described = Comment::parse("* Adds.\n * @description More.\n ");
        assert_eq!(described.description, "Adds.\n\nMore.");
    }

    #[test]
    fn a_param_has_a_type_a_name_a_default_and_a_description() {
        let named = param("{string} [name=\"x\"] - Its name.");
        let expected = Param {
            name: "name".to_string(),
            ty: Some("string".to_string()),
            optional: true,
            default: Some("\"x\"".to_string()),
            description: "Its name.".to_string(),
        };
        assert_eq!(named, expected);
        let rest = param("...values - The rest.");
        assert_eq!((rest.name.as_str(), rest.ty), ("values", None));
        let optional = param("{number=} size");
        assert_eq!(
            (optional.ty.as_deref(), optional.optional),
            (Some("number"), true)
        );
        let nested = param("{{ x: number,\n y: number }} point -1 is none.");
        assert_eq!(nested.ty.as_deref(), Some("{ x: number, y: number }"));
        assert_eq!(nested.description, "-1 is none.");
    }

    #[test]
    fn a_template_names_type_parameters() {
        let items = template("{string} K, V - The keys' and values' types.");
        let names: Vec<_> = items.iter().map(|item| item.name.as_deref()).collect();
        assert_eq!(names, [Some("K"), Some("V")]);
        assert_eq!(items[0].ty.as_deref(), Some("string"));
        assert_eq!(items[1].ty, None);
        assert_eq!(items[1].description, "The keys' and values' types.");
        let defaulted = template("[T=string] A type.");
        assert_eq!(defaulted[0].default.as_deref(), Some("string"));
        assert_eq!(defaulted[0].description, "A type.");
    }

    #[test]
    fn a_comment_makes_sections_in_a_pages_order() {
        let comment = Comment::parse(
            "*\n * Finds things.\n *\n * ## Notes\n *\n * Slowly.\n * @see find\n \
             * @since 1.0\n * @example\n * find(1)\n * @throws {Error} If lost.\n \
             * @returns The things.\n * @param what - What to find.\n * @template T - Its type.\n \
             * @remarks Rarely used.\n * @deprecated Use `seek`.\n * @type {Finder}\n ",
        );
        let context = Context {
            language: "ts",
            type_parameters: vec![Item {
                name: Some("T".to_string()),
                ty: Some("object".to_string()),
                ..Item::default()
            }],
            ..Context::default()
        };
        let sections = sections(&comment, &context);
        let expected = [
            "",
            "Notes",
            "Deprecated",
            "Remarks",
            "Type Parameters",
            "Parameters",
            "Returns",
            "Throws",
            "Type",
            "Example",
            "Since",
            "See Also",
        ];
        assert_eq!(titles(&sections), expected);
        let Section::Items { items, .. } = &sections[4] else {
            panic!("not a list: {:?}", sections[4]);
        };
        assert_eq!(items[0].ty.as_deref(), Some("object"));
        let text = |index: usize| match &sections[index] {
            Section::Text { text, .. } => text.clone(),
            section => panic!("not text: {section:?}"),
        };
        assert_eq!(text(8), "`Finder`");
        assert_eq!(text(9), "```ts\nfind(1)\n```");
        assert_eq!(text(11), "- {@link find}");
        let typed = Context {
            typed: true,
            ..Context::default()
        };
        assert!(!titles(&super::sections(&comment, &typed)).contains(&"Type".to_string()));
    }

    #[test]
    fn properties_come_from_the_code_and_the_comment() {
        let comment = Comment::parse(
            "*\n * @property {string} name - Its name.\n * @property {number} [size] Its size.\n ",
        );
        let code = vec![Item {
            name: Some("name".to_string()),
            ty: Some("Name".to_string()),
            ..Item::default()
        }];
        let listed = properties(&comment, &code);
        let names: Vec<_> = listed.iter().map(|item| item.name.as_deref()).collect();
        assert_eq!(names, [Some("name"), Some("size?")]);
        assert_eq!(listed[0].ty.as_deref(), Some("Name"));
        assert_eq!(listed[0].description, "Its name.");
        assert!(properties(&Comment::default(), &code).is_empty());
    }

    #[test]
    fn examples_and_see_alsos_are_written_as_markdown() {
        let captioned = example("<caption>Adding.</caption>\nadd(1, 2)", "js");
        assert_eq!(captioned, "Adding.\n\n```js\nadd(1, 2)\n```");
        let fenced = "```sh\nnpm install\n```";
        assert_eq!(example(fenced, "js"), fenced);
        assert_eq!(see("Point#x - Its x."), "{@link Point#x} - Its x.");
        assert_eq!(
            see("https://jsdoc.app Its docs."),
            "<https://jsdoc.app> Its docs."
        );
        assert_eq!(see("the docs"), "the docs");
        assert_eq!(
            see("[docs](https://jsdoc.app)"),
            "[docs](https://jsdoc.app)"
        );
    }

    #[test]
    fn tags_give_labels_or_hide_what_theyre_on() {
        let comment = Comment::parse("*\n * @deprecated\n * @access protected\n * @virtual\n ");
        assert_eq!(comment.labels(), ["deprecated", "protected", "abstract"]);
        let labels = ordered(["async", "deprecated", "default", "async", "accessor"]);
        assert_eq!(labels, ["default", "async", "deprecated", "accessor"]);
        assert!(Comment::parse("* @internal ").is_hidden());
        assert!(Comment::parse("* @access private ").is_hidden());
        assert!(!comment.is_hidden());
        assert!(Comment::parse("* @module util ").is_standalone());
        assert!(Comment::parse("* @typedef {object} Point ").is_standalone());
    }
}
