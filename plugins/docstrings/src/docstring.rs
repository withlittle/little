//! Python's docstrings, in NumPy's, Google's, or Sphinx's style, which is
//! found for each docstring.

use ruff_python_parser::parse_expression;

use crate::model::{Item, Items, Section};

/// The sections of `docstring`, as Python writes it in its code.
pub fn parse(docstring: &str) -> Vec<Section> {
    let cleaned = clean(docstring);
    let lines: Vec<&str> = cleaned.lines().collect();
    if (0..lines.len()).any(|i| numpy_title(&lines, i).is_some()) {
        numpy(&lines)
    } else if lines.iter().any(|line| sphinx_field(line).is_some()) {
        sphinx(&lines)
    } else {
        google(&lines)
    }
}

/// The first paragraph of the text a docstring starts with, on one line.
pub fn summary(sections: &[Section]) -> String {
    match sections.first() {
        Some(Section::Text { title: None, text }) => {
            let paragraph = text.split("\n\n").next().unwrap_or_default();
            paragraph.split_whitespace().collect::<Vec<_>>().join(" ")
        }
        _ => String::new(),
    }
}

/// A docstring as Python's `inspect.cleandoc` tidies it: its tabs expanded,
/// the indentation its lines after the first share taken off, and blank
/// lines at either end taken away; and the space at the end of each line.
fn clean(docstring: &str) -> String {
    let lines: Vec<String> = (docstring.lines())
        .map(|line| expand_tabs(line).trim_end().to_string())
        .collect();
    let margin = (lines.iter().skip(1))
        .filter(|line| !line.is_empty())
        .map(|line| indent(line))
        .min()
        .unwrap_or(0);
    let mut cleaned: Vec<&str> = (lines.iter().enumerate())
        .map(|(i, line)| match i {
            0 => line.trim_start(),
            _ => line.get(margin..).unwrap_or_default(),
        })
        .collect();
    while cleaned.first().is_some_and(|line| line.is_empty()) {
        cleaned.remove(0);
    }
    while cleaned.last().is_some_and(|line| line.is_empty()) {
        cleaned.pop();
    }
    cleaned.join("\n")
}

/// `line` with each tab turned to the spaces up to the next multiple of 8
/// columns, as Python's `expandtabs` does.
fn expand_tabs(line: &str) -> String {
    let mut expanded = String::new();
    for c in line.chars() {
        match c {
            '\t' => {
                let spaces = 8 - expanded.chars().count() % 8;
                expanded.extend(std::iter::repeat_n(' ', spaces));
            }
            c => expanded.push(c),
        }
    }
    expanded
}

/// The spaces `line` starts with.
fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// `lines` without the indentation they share.
fn dedent<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    let margin = (lines.iter())
        .filter(|line| !line.trim().is_empty())
        .map(|line| indent(line))
        .min()
        .unwrap_or(0);
    (lines.iter())
        .map(|line| line.get(margin..).unwrap_or_default())
        .collect()
}

/// `lines` as text, without the indentation they share or blank lines at
/// either end.
fn block(lines: &[&str]) -> String {
    let lines = dedent(lines);
    let start = lines.iter().position(|line| !line.trim().is_empty());
    let end = lines.iter().rposition(|line| !line.trim().is_empty());
    match (start, end) {
        (Some(start), Some(end)) => lines[start..=end].join("\n"),
        _ => String::new(),
    }
}

/// `first`, the text after an item's name or type on its line, then the
/// lines after it.
fn joined(first: &str, rest: &str) -> String {
    match (first.trim(), rest) {
        ("", rest) => rest.to_string(),
        (first, "") => first.to_string(),
        (first, rest) => format!("{first}\n{rest}"),
    }
}

/// `text`, unless it's empty.
fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// What the section titled `title` lists, if it lists anything.
fn kind_of(title: &str) -> Option<Items> {
    Some(match title.to_lowercase().as_str() {
        "parameters" | "parameter" | "params" | "param" | "arguments" | "argument" | "args"
        | "arg" => Items::Parameters,
        "other parameters" | "other params" | "other arguments" | "other args"
        | "keyword arguments" | "keyword args" | "kwargs" => Items::OtherParameters,
        "attributes" | "attribute" => Items::Attributes,
        "returns" | "return" => Items::Returns,
        "yields" | "yield" => Items::Yields,
        "receives" | "receive" => Items::Receives,
        "raises" | "raise" | "exceptions" => Items::Raises,
        "warns" | "warn" => Items::Warns,
        _ => return None,
    })
}

/// Add the section of text titled `title`, if it has one, written in
/// `lines`, unless they're blank.
fn text(sections: &mut Vec<Section>, title: Option<&str>, lines: &[&str]) {
    let text = doctests(&block(lines));
    if !text.is_empty() {
        let title = title.map(str::to_string);
        sections.push(Section::Text { title, text });
    }
}

/// Add the section of `kind` that lists `items`, unless there are none.
fn items(sections: &mut Vec<Section>, kind: Items, items: Vec<Item>) {
    if !items.is_empty() {
        sections.push(Section::Items { kind, items });
    }
}

/// `text` with each example after `>>>`, up to a blank line, fenced as
/// Python's code.
fn doctests(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut written: Vec<String> = Vec::new();
    // The fence of a code block the docstring writes itself.
    let mut fence: Option<String> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str()) {
                fence = None;
            }
        } else if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let c = trimmed.chars().next().unwrap_or('`');
            fence = Some(trimmed.chars().take_while(|&d| d == c).collect());
        } else if trimmed.starts_with(">>>") {
            let margin = line.len() - trimmed.len();
            let end = (i..lines.len())
                .find(|&end| lines[end].trim().is_empty())
                .unwrap_or(lines.len());
            let example: Vec<&str> = (lines[i..end].iter())
                .map(|line| &line[margin.min(indent(line))..])
                .collect();
            let example = example.join("\n");
            let ticks = fence_for(&example);
            written.push(format!("{ticks}python\n{example}\n{ticks}"));
            i = end;
            continue;
        }
        written.push(line.to_string());
        i += 1;
    }
    written.join("\n")
}

/// The fence for a code block of `code`: three backticks, or more than it
/// has in a row.
pub fn fence_for(code: &str) -> String {
    let longest = (code.split(|c| c != '`')).map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

/// `text` split at each `separator` outside brackets and quotes.
fn top_level(text: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut quote = None;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c) if c == separator && depth == 0 => {
                parts.push(&text[start..i]);
                start = i + c.len_utf8();
            }
            (None, '\'' | '"') => quote = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

/// A type, as a docstring writes it, without `optional`, and the default
/// value it gives after `default`, as NumPy's style does, like
/// `int, default 0`.
fn type_and_default(ty: &str) -> (Option<String>, Option<String>) {
    let mut kept = Vec::new();
    let mut default = None;
    for part in top_level(ty, ',') {
        let part = part.trim();
        if part == "optional" {
            continue;
        }
        if let Some(rest) = part.strip_prefix("default")
            && (rest.is_empty() || rest.starts_with([' ', ':', '=']))
        {
            default = non_empty(rest.trim_start_matches([' ', ':', '=']));
            continue;
        }
        if !part.is_empty() {
            kept.push(part);
        }
    }
    (non_empty(&kept.join(", ")), default)
}

/// An item, from `head`, the line that starts it, like `name : type`, and
/// `description`, the lines after it.
fn item(kind: Items, head: &str, description: String) -> Item {
    match kind {
        Items::Raises | Items::Warns => {
            let (ty, rest) = head.split_once(':').unwrap_or((head, ""));
            Item {
                ty: non_empty(ty),
                description: joined(rest, &description),
                ..Item::default()
            }
        }
        // A value written without a name, by its type, like `dict[str, int]:`
        // or `nx.Graph: A graph.`, is read as Napoleon reads the first.
        Items::Returns | Items::Yields | Items::Receives
            if (head.split_once(':'))
                .is_none_or(|(name, ty)| !is_name(name.trim()) || ty.trim().is_empty()) =>
        {
            let (ty, text) = head.split_once(':').unwrap_or((head, ""));
            Item {
                ty: non_empty(ty),
                description: joined(text, &description),
                ..Item::default()
            }
        }
        _ => {
            let (name, ty, text) = parenthesized(head).unwrap_or_else(|| {
                let (name, ty) = head.split_once(':').unwrap_or((head, ""));
                (name, ty, "")
            });
            let (ty, default) = type_and_default(ty);
            Item {
                name: non_empty(name),
                ty,
                default,
                description: joined(text, &description),
            }
        }
    }
}

/// The name, type, and text on `head`, if it's written as in Google's style,
/// like `name (type): text` or `name (type)`, as some docstrings in NumPy's
/// style are.
fn parenthesized(head: &str) -> Option<(&str, &str, &str)> {
    let (name, inside) = head.split_once('(')?;
    let ty = top_level(inside, ')')[0];
    let text = match inside.get(ty.len() + 1..)?.trim() {
        "" => "",
        rest => rest.strip_prefix(':')?,
    };
    let name = name.trim();
    is_name(name).then_some((name, ty, text))
}

/// Whether `text` is a parameter's or attribute's name, like `x`, `**kwargs`,
/// or `\*args`, as Sphinx needs it written.
fn is_name(text: &str) -> bool {
    let name = text.trim_start_matches(['*', '\\']);
    !name.is_empty() && name.chars().all(|c| c == '_' || c.is_alphanumeric())
}

/// Whether `head`, a line that starts an item in NumPy's style, starts with
/// the names it documents, like `x`, `x1, x2 : float`, or `x (float):`, and
/// isn't a section's title in Google's style, like `Returns:`.
fn starts_with_names(head: &str) -> bool {
    if parenthesized(head).is_some() {
        return true;
    }
    let title = (head.strip_suffix(':'))
        .is_some_and(|title| GOOGLE.contains(&title.to_lowercase().as_str()));
    let names = head.split_once(':').map_or(head, |(names, _)| names);
    !title && names.split(',').all(|name| is_name(name.trim()))
}

/// Where the items in `body`, a section of `kind` in NumPy's style, end: in
/// a section of names, at a paragraph that doesn't start with one, which
/// goes on after the section, as the docstring's text.
fn end_of_items(kind: Items, body: &[&str]) -> usize {
    if !matches!(
        kind,
        Items::Parameters | Items::OtherParameters | Items::Attributes
    ) {
        return body.len();
    }
    let lines = dedent(body);
    (1..lines.len())
        .find(|&i| {
            let line = lines[i];
            lines[i - 1].is_empty()
                && !line.is_empty()
                && !line.starts_with(' ')
                && !starts_with_names(line)
        })
        .unwrap_or(body.len())
}

/// `lines` split into items: each starts on a line that isn't indented, and
/// goes on in the indented lines after it.
fn entries<'a>(lines: &[&'a str]) -> Vec<(&'a str, String)> {
    let lines = dedent(lines);
    let mut entries: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in lines {
        if !line.is_empty() && !line.starts_with(' ') {
            entries.push((line, Vec::new()));
        } else if let Some((_, rest)) = entries.last_mut() {
            rest.push(line);
        }
    }
    (entries.into_iter())
        .map(|(head, rest)| (head, block(&rest)))
        .collect()
}

/// The title of the section in NumPy's style that starts on line `i`, if
/// one does, underlined with dashes, without a colon after it, as Sphinx's
/// Napoleon reads it.
fn numpy_title<'a>(lines: &[&'a str], i: usize) -> Option<&'a str> {
    let dashes = |line: &str| line.len() >= 3 && line.bytes().all(|b| b == b'-');
    let title = lines[i];
    let underline = lines.get(i + 1)?;
    (!title.is_empty() && !title.starts_with(' ') && !dashes(title) && dashes(underline))
        .then_some(title.trim().trim_end_matches(':').trim_end())
}

/// A docstring in NumPy's style.
fn numpy(lines: &[&str]) -> Vec<Section> {
    let starts: Vec<usize> = (0..lines.len())
        .filter(|&i| numpy_title(lines, i).is_some())
        .collect();
    let mut sections = Vec::new();
    text(&mut sections, None, &lines[..starts[0]]);
    for (n, &start) in starts.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(lines.len());
        let title = numpy_title(lines, start).unwrap_or_default();
        let body = &lines[start + 2..end];
        match kind_of(title) {
            Some(kind) => {
                let (body, after) = body.split_at(end_of_items(kind, body));
                let listed = (entries(body).into_iter())
                    .map(|(head, description)| item(kind, head, description))
                    .flat_map(split_names)
                    .collect();
                items(&mut sections, kind, listed);
                text(&mut sections, None, after);
            }
            None => text(&mut sections, Some(title), body),
        }
    }
    sections
}

/// `item`, or, if it names more than one, like `x1, x2`, an item for each,
/// as NumPy's style lets those of the same type share a description.
fn split_names(item: Item) -> Vec<Item> {
    let names: Vec<&str> = match &item.name {
        Some(names) if names.contains(',') => (names.split(','))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect(),
        _ => return vec![item],
    };
    if !names.iter().all(|name| is_name(name)) {
        return vec![item];
    }
    (names.into_iter())
        .map(|name| Item {
            name: Some(name.to_string()),
            ..item.clone()
        })
        .collect()
}

/// The titles of sections in Google's style.
const GOOGLE: &[&str] = &[
    "args",
    "arguments",
    "attention",
    "attributes",
    "caution",
    "danger",
    "error",
    "example",
    "examples",
    "hint",
    "important",
    "keyword args",
    "keyword arguments",
    "methods",
    "note",
    "notes",
    "other parameters",
    "parameters",
    "params",
    "raises",
    "receives",
    "references",
    "return",
    "returns",
    "see also",
    "tip",
    "todo",
    "warning",
    "warnings",
    "warns",
    "yield",
    "yields",
];

/// The title of the section in Google's style that starts on line `i`, if
/// one does: a title it knows, then a colon, then indented lines.
fn google_title<'a>(lines: &[&'a str], i: usize) -> Option<&'a str> {
    let line = lines[i];
    let title = line.strip_suffix(':')?;
    if line.starts_with(' ') || !GOOGLE.contains(&title.to_lowercase().as_str()) {
        return None;
    }
    let next = lines[i + 1..].iter().find(|line| !line.is_empty())?;
    next.starts_with(' ').then_some(title)
}

/// A docstring in Google's style, or with no sections at all.
fn google(lines: &[&str]) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < lines.len() {
        let Some(title) = google_title(lines, i) else {
            i += 1;
            continue;
        };
        text(&mut sections, None, &lines[start..i]);
        let end = (i + 1..lines.len())
            .find(|&end| !lines[end].is_empty() && !lines[end].starts_with(' '))
            .unwrap_or(lines.len());
        let body = &lines[i + 1..end];
        match kind_of(title) {
            Some(kind) => items(&mut sections, kind, google_items(kind, body)),
            None => text(&mut sections, Some(title), body),
        }
        (start, i) = (end, end);
    }
    text(&mut sections, None, &lines[start..]);
    sections
}

/// The items of a section of `kind` in Google's style, in `body`.
fn google_items(kind: Items, body: &[&str]) -> Vec<Item> {
    if let Items::Returns | Items::Yields | Items::Receives = kind {
        let text = block(body);
        if text.is_empty() {
            return Vec::new();
        }
        let (first, rest) = text.split_once('\n').unwrap_or((&text, ""));
        let item = match type_prefix(first) {
            Some((ty, description)) => Item {
                ty: Some(ty),
                description: joined(description, &block(&rest.lines().collect::<Vec<_>>())),
                ..Item::default()
            },
            None => Item {
                description: text.clone(),
                ..Item::default()
            },
        };
        return vec![item];
    }
    (entries(body).into_iter())
        .map(|(head, rest)| match kind {
            Items::Raises | Items::Warns => item(kind, head, rest),
            _ => match google_item(head) {
                Some((name, ty, description)) => {
                    let (ty, default) = ty.map(type_and_default).unwrap_or_default();
                    Item {
                        name: Some(name.to_string()),
                        ty,
                        default,
                        description: joined(description, &rest),
                    }
                }
                None => Item {
                    name: non_empty(head),
                    description: rest,
                    ..Item::default()
                },
            },
        })
        .collect()
}

/// The name, type, and description on the line that starts an item in
/// Google's style, like `name (type): description`.
fn google_item(head: &str) -> Option<(&str, Option<&str>, &str)> {
    let end = head.find(['(', ':'])?;
    let name = head[..end].trim();
    if name.is_empty() || name.contains(char::is_whitespace) {
        return None;
    }
    let rest = &head[end..];
    let (ty, rest) = match rest.strip_prefix('(') {
        Some(inside) => {
            let close = top_level(inside, ')').first()?.len();
            (Some(&inside[..close]), inside.get(close + 1..)?)
        }
        None => (None, rest),
    };
    let description = rest.trim_start().strip_prefix(':')?;
    Some((name, ty, description.trim()))
}

/// The type before the first colon of `line`, where a value returned in
/// Google's style may give it, and the text after it, if what's before it
/// is a Python expression.
fn type_prefix(line: &str) -> Option<(String, &str)> {
    let parts = top_level(line, ':');
    let [ty, ..] = parts.as_slice() else {
        return None;
    };
    if parts.len() < 2 || ty.trim().is_empty() || parse_expression(ty.trim()).is_err() {
        return None;
    }
    Some((ty.trim().to_string(), line[ty.len() + 1..].trim()))
}

/// The fields of Sphinx's style that the plugin reads.
const SPHINX: &[&str] = &[
    "arg",
    "argument",
    "cvar",
    "except",
    "exception",
    "ivar",
    "key",
    "keyword",
    "param",
    "parameter",
    "raise",
    "raises",
    "return",
    "returns",
    "rtype",
    "type",
    "var",
    "vartype",
    "yield",
    "yields",
    "ytype",
];

/// The field, its argument, and its text, on `line`, if it starts a field
/// in Sphinx's style, like `:param x: The x.`.
fn sphinx_field(line: &str) -> Option<(&str, &str, &str)> {
    let (inside, text) = line.strip_prefix(':')?.split_once(':')?;
    let (field, argument) = inside.split_once(' ').unwrap_or((inside, ""));
    SPHINX
        .contains(&field)
        .then(|| (field, argument.trim(), text.trim()))
}

/// A docstring in Sphinx's style.
fn sphinx(lines: &[&str]) -> Vec<Section> {
    let mut prose = Vec::new();
    let mut fields: Vec<(&str, &str, Vec<&str>)> = Vec::new();
    for &line in lines {
        if let Some((field, argument, text)) = sphinx_field(line) {
            fields.push((field, argument, vec![text]));
        } else if let Some((_, _, rest)) = fields.last_mut()
            && (line.is_empty() || line.starts_with(' '))
        {
            rest.push(line.trim());
        } else {
            prose.push(line);
        }
    }
    let mut parameters = Vec::new();
    let mut others = Vec::new();
    let mut attributes = Vec::new();
    let mut raises = Vec::new();
    let mut returns = Item::default();
    let mut yields = Item::default();
    for (field, argument, text) in fields {
        let description = block(&text);
        match field {
            "param" | "parameter" | "arg" | "argument" | "key" | "keyword" => {
                let (ty, name) = match argument.rsplit_once(' ') {
                    Some((ty, name)) => (type_and_default(ty), name),
                    None => ((None, None), argument),
                };
                let list = match field {
                    "key" | "keyword" => &mut others,
                    _ => &mut parameters,
                };
                list.push(Item {
                    name: non_empty(name),
                    ty: ty.0,
                    default: ty.1,
                    description,
                });
            }
            "type" => {
                let list = match others.iter().any(|item| named(item, argument)) {
                    true => &mut others,
                    false => &mut parameters,
                };
                typed(list, argument, &description);
            }
            "var" | "ivar" | "cvar" => attributes.push(Item {
                name: non_empty(argument),
                description,
                ..Item::default()
            }),
            "vartype" => typed(&mut attributes, argument, &description),
            "returns" | "return" => returns.description = description,
            "rtype" => returns.ty = non_empty(&description),
            "yields" | "yield" => yields.description = description,
            "ytype" => yields.ty = non_empty(&description),
            _ => raises.push(Item {
                ty: non_empty(argument),
                description,
                ..Item::default()
            }),
        }
    }
    let mut sections = Vec::new();
    text(&mut sections, None, &prose);
    items(&mut sections, Items::Parameters, parameters);
    items(&mut sections, Items::OtherParameters, others);
    items(&mut sections, Items::Attributes, attributes);
    for (kind, item) in [(Items::Returns, returns), (Items::Yields, yields)] {
        if item != Item::default() {
            sections.push(Section::Items {
                kind,
                items: vec![item],
            });
        }
    }
    items(&mut sections, Items::Raises, raises);
    sections
}

/// Whether `item` is called `name`.
fn named(item: &Item, name: &str) -> bool {
    item.name.as_deref() == Some(name)
}

/// Give the item called `name` in `list` the type `ty`, as Sphinx's style
/// gives it in a field of its own, adding the item if there isn't one.
fn typed(list: &mut Vec<Item>, name: &str, ty: &str) {
    let (ty, default) = type_and_default(ty);
    match list.iter_mut().find(|item| named(item, name)) {
        Some(item) => (item.ty, item.default) = (ty, item.default.take().or(default)),
        None => list.push(Item {
            name: non_empty(name),
            ty,
            default,
            ..Item::default()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(text: &str) -> Section {
        Section::Text {
            title: None,
            text: text.to_string(),
        }
    }

    fn titled(title: &str, text: &str) -> Section {
        Section::Text {
            title: Some(title.to_string()),
            text: text.to_string(),
        }
    }

    fn listed(name: Option<&str>, ty: Option<&str>, description: &str) -> Item {
        Item {
            name: name.map(str::to_string),
            ty: ty.map(str::to_string),
            default: None,
            description: description.to_string(),
        }
    }

    #[test]
    fn a_docstring_is_cleaned_as_python_cleans_it() {
        let docstring = "\n    Summary.\n\n\tTabbed.\n      Indented.  \n    ";
        assert_eq!(clean(docstring), "Summary.\n\n    Tabbed.\n  Indented.");
        assert_eq!(clean("First line.\n    Second."), "First line.\nSecond.");
    }

    #[test]
    fn a_docstring_without_sections_is_text() {
        let sections = parse("Summary.\n\n    More, with *Markdown*.\n    ");
        assert_eq!(sections, [text("Summary.\n\nMore, with *Markdown*.")]);
        assert_eq!(summary(&sections), "Summary.");
        assert_eq!(parse("   "), []);
        assert_eq!(
            summary(&parse("A summary\nover two lines.\n\nMore.")),
            "A summary over two lines."
        );
    }

    #[test]
    fn numpy_sections_are_read() {
        let docstring = "
            Solve a problem.

            Parameters
            ----------
            problem : Problem
                The problem,
                over two lines.
            tolerance: float, optional
                How close.
            steps : int, default 10
                How many.
            verbose
                Whether to talk.
            x1, x2 : float
                The ends.
            *args
                The rest.

            Returns
            -------
            Solution
                The solution.

            Raises
            ------
            ValueError:
                If it can't.
            TypeError
                If it's wrong.

            Notes
            -----
            Some notes.

            Examples
            --------
            Like so:

            >>> solve(problem)
            Solution()
            ";
        let mut steps = listed(Some("steps"), Some("int"), "How many.");
        steps.default = Some("10".to_string());
        assert_eq!(
            parse(docstring),
            [
                text("Solve a problem."),
                Section::Items {
                    kind: Items::Parameters,
                    items: vec![
                        listed(
                            Some("problem"),
                            Some("Problem"),
                            "The problem,\nover two lines."
                        ),
                        listed(Some("tolerance"), Some("float"), "How close."),
                        steps,
                        listed(Some("verbose"), None, "Whether to talk."),
                        listed(Some("x1"), Some("float"), "The ends."),
                        listed(Some("x2"), Some("float"), "The ends."),
                        listed(Some("*args"), None, "The rest."),
                    ]
                },
                Section::Items {
                    kind: Items::Returns,
                    items: vec![listed(None, Some("Solution"), "The solution.")]
                },
                Section::Items {
                    kind: Items::Raises,
                    items: vec![
                        listed(None, Some("ValueError"), "If it can't."),
                        listed(None, Some("TypeError"), "If it's wrong."),
                    ]
                },
                titled("Notes", "Some notes."),
                titled(
                    "Examples",
                    "Like so:\n\n```python\n>>> solve(problem)\nSolution()\n```"
                ),
            ]
        );
    }

    #[test]
    fn numpy_returns_may_be_named() {
        let docstring =
            "Split.\n\nReturns\n-------\nhead : str\n    The head.\ntail : str\n    The tail.";
        assert_eq!(
            parse(docstring)[1],
            Section::Items {
                kind: Items::Returns,
                items: vec![
                    listed(Some("head"), Some("str"), "The head."),
                    listed(Some("tail"), Some("str"), "The tail."),
                ]
            }
        );
    }

    #[test]
    fn a_numpy_title_may_have_a_colon_after_it() {
        let docstring =
            "Do.\n\nParameters:\n-----------\nx : int\n    An x.\n\nUsage:\n------\nLike so.";
        assert_eq!(
            parse(docstring)[1..],
            [
                Section::Items {
                    kind: Items::Parameters,
                    items: vec![listed(Some("x"), Some("int"), "An x.")],
                },
                titled("Usage", "Like so."),
            ]
        );
    }

    #[test]
    fn numpy_items_may_be_written_as_in_googles_style() {
        let docstring = "Run.

            Parameters
            ----------
            problem (Problem):
                The problem.
            score (Callable[[str], float] | None):
                How to score.
            shots (int | None): How many,
                at least one.
            ";
        assert_eq!(
            parse(docstring)[1],
            Section::Items {
                kind: Items::Parameters,
                items: vec![
                    listed(Some("problem"), Some("Problem"), "The problem."),
                    listed(
                        Some("score"),
                        Some("Callable[[str], float] | None"),
                        "How to score."
                    ),
                    listed(
                        Some("shots"),
                        Some("int | None"),
                        "How many,\nat least one."
                    ),
                ]
            }
        );
    }

    #[test]
    fn numpy_items_are_split_only_into_names() {
        let docstring =
            "Move.\n\nParameters\n----------\nx, y : float\n    Where.\nx[0], x[1] : int\n    Odd.";
        assert_eq!(
            parse(docstring)[1],
            Section::Items {
                kind: Items::Parameters,
                items: vec![
                    listed(Some("x"), Some("float"), "Where."),
                    listed(Some("y"), Some("float"), "Where."),
                    listed(Some("x[0], x[1]"), Some("int"), "Odd."),
                ]
            }
        );
    }

    #[test]
    fn a_numpy_section_of_names_ends_at_a_paragraph_without_one() {
        let items = Section::Items {
            kind: Items::Parameters,
            items: vec![listed(Some("data"), Some("list"), "The data.")],
        };
        let docstring = "Fit.

            Parameters
            ----------
            data : list
                The data.

            Returns (slope, intercept), fitted
            to the data.
            ";
        assert_eq!(
            parse(docstring)[1..],
            [
                items.clone(),
                text("Returns (slope, intercept), fitted\nto the data.")
            ]
        );
        let docstring = "Fit.

            Parameters
            ----------
            data : list
                The data.

            Returns:
            int:
                The fit.
            ";
        assert_eq!(
            parse(docstring)[1..],
            [items, text("Returns:\nint:\n    The fit.")]
        );
    }

    #[test]
    fn a_numpy_value_without_a_name_is_read_by_its_type() {
        let docstring = "Count.\n\nReturns\n-------\ndict[str, tuple[int, float]]:\n    The counts.\nnx.Graph: A graph,\n    drawn.";
        assert_eq!(
            parse(docstring)[1],
            Section::Items {
                kind: Items::Returns,
                items: vec![
                    listed(None, Some("dict[str, tuple[int, float]]"), "The counts."),
                    listed(None, Some("nx.Graph"), "A graph,\ndrawn."),
                ]
            }
        );
    }

    #[test]
    fn google_sections_are_read() {
        let docstring = "
            Solve a problem.

            Args:
                problem (Problem): The problem,
                    over two lines.
                tolerance (float, optional): How close.
                mapping (dict[str, tuple(int, int)]): A mapping.
                verbose: Whether to talk.

            Returns:
                Solution: The solution.

            Raises:
                ValueError: If it can't.

            More text.

            Note:
                A note.

            Example:
                >>> solve(problem)
            ";
        assert_eq!(
            parse(docstring),
            [
                text("Solve a problem."),
                Section::Items {
                    kind: Items::Parameters,
                    items: vec![
                        listed(
                            Some("problem"),
                            Some("Problem"),
                            "The problem,\nover two lines."
                        ),
                        listed(Some("tolerance"), Some("float"), "How close."),
                        listed(
                            Some("mapping"),
                            Some("dict[str, tuple(int, int)]"),
                            "A mapping."
                        ),
                        listed(Some("verbose"), None, "Whether to talk."),
                    ]
                },
                Section::Items {
                    kind: Items::Returns,
                    items: vec![listed(None, Some("Solution"), "The solution.")]
                },
                Section::Items {
                    kind: Items::Raises,
                    items: vec![listed(None, Some("ValueError"), "If it can't.")]
                },
                text("More text."),
                titled("Note", "A note."),
                titled("Example", "```python\n>>> solve(problem)\n```"),
            ]
        );
    }

    #[test]
    fn a_google_return_without_a_type_is_described() {
        let docstring = "Check.\n\nReturns:\n    True if it should stop: it's done.\n    Or False.";
        assert_eq!(
            parse(docstring)[1],
            Section::Items {
                kind: Items::Returns,
                items: vec![listed(
                    None,
                    None,
                    "True if it should stop: it's done.\nOr False."
                )]
            }
        );
    }

    #[test]
    fn a_google_title_without_indented_lines_after_it_is_text() {
        let docstring = "Summary.\n\nNote:\nnot a section.";
        assert_eq!(
            parse(docstring),
            [text("Summary.\n\nNote:\nnot a section.")]
        );
    }

    #[test]
    fn sphinx_fields_are_read() {
        let docstring = "
            Solve a problem.

            :param problem: The problem,
                over two lines.
            :type problem: Problem
            :param float tolerance: How close.
            :keyword verbose: Whether to talk.
            :ivar steps: How many.
            :vartype steps: int
            :returns: The solution.
            :rtype: Solution
            :raises ValueError: If it can't.
            ";
        assert_eq!(
            parse(docstring),
            [
                text("Solve a problem."),
                Section::Items {
                    kind: Items::Parameters,
                    items: vec![
                        listed(
                            Some("problem"),
                            Some("Problem"),
                            "The problem,\nover two lines."
                        ),
                        listed(Some("tolerance"), Some("float"), "How close."),
                    ]
                },
                Section::Items {
                    kind: Items::OtherParameters,
                    items: vec![listed(Some("verbose"), None, "Whether to talk.")]
                },
                Section::Items {
                    kind: Items::Attributes,
                    items: vec![listed(Some("steps"), Some("int"), "How many.")]
                },
                Section::Items {
                    kind: Items::Returns,
                    items: vec![listed(None, Some("Solution"), "The solution.")]
                },
                Section::Items {
                    kind: Items::Raises,
                    items: vec![listed(None, Some("ValueError"), "If it can't.")]
                },
            ]
        );
    }

    #[test]
    fn examples_are_fenced_but_code_the_docstring_fences_is_not() {
        let docstring =
            "Use it.\n\n```python\n>>> as written\n```\n\n    >>> x = 1\n    >>> x\n    1\n\nDone.";
        assert_eq!(
            parse(docstring),
            [text(
                "Use it.\n\n```python\n>>> as written\n```\n\n```python\n>>> x = 1\n>>> x\n1\n```\n\nDone."
            )]
        );
    }

    #[test]
    fn a_type_loses_optional_and_gives_its_default() {
        assert_eq!(
            type_and_default("int, optional"),
            (Some("int".into()), None)
        );
        assert_eq!(
            type_and_default("{'a', 'b'}, default 'a'"),
            (Some("{'a', 'b'}".into()), Some("'a'".into()))
        );
        assert_eq!(
            type_and_default("int, default: 3"),
            (Some("int".into()), Some("3".into()))
        );
        assert_eq!(
            type_and_default("defaultdict"),
            (Some("defaultdict".into()), None)
        );
        assert_eq!(type_and_default(""), (None, None));
    }

    #[test]
    fn a_fence_is_longer_than_the_backticks_in_its_code() {
        assert_eq!(fence_for("x = 1"), "```");
        assert_eq!(fence_for("s = '```'"), "````");
    }
}
