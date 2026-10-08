//! The Markdown of a page of the reference, from what it documents, in no
//! language in particular.

use std::collections::{HashMap, HashSet};

use crate::docstring::{fence_for, summary};
use crate::model::{Item, Items, Kind, Object, Section};

/// A module or package a package's page lists.
pub struct Listed {
    pub name: String,
    /// The relative link to its page, like `module.md`.
    pub link: String,
    /// The first paragraph of its docstring.
    pub summary: String,
}

/// The page of `module`, which is written in `file`, in `language`, and
/// lists `modules`, if it's a package.
pub fn page(module: &Object, file: &str, language: &str, modules: &[Listed]) -> String {
    let mut anchors = Anchors::default();
    anchors.anchor(&module.name);
    let mut ids = HashMap::new();
    assign(&module.members, "", &mut anchors, &mut ids);
    let page = Page {
        ids,
        file,
        language,
    };
    let mut out = format!("# {}\n\n", escape(&module.name));
    out += &labels(module);
    page.sections(&mut out, module);
    if !modules.is_empty() {
        let rows: Vec<Vec<String>> = (modules.iter())
            .map(|listed| {
                let name = format!("[{}]({})", code(&listed.name), listed.link);
                vec![name, listed.summary.clone()]
            })
            .collect();
        out += "**Modules:**\n\n";
        table(&mut out, &["Name", "Description"], &rows);
    }
    page.summaries(&mut out, module, "");
    for member in ordered(&module.members) {
        page.object(&mut out, member, "", 2);
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

/// What a page needs as it's written.
struct Page<'a> {
    /// The id of each object's heading, by its name, like `Solver.solve`.
    ids: HashMap<String, String>,
    file: &'a str,
    language: &'a str,
}

impl Page<'_> {
    /// Write `object`, whose name follows `prefix`, under a heading of
    /// `level`, and what it defines under it.
    fn object(&self, out: &mut String, object: &Object, prefix: &str, level: usize) {
        let name = format!("{prefix}{}", object.name);
        *out += &format!("{} {}\n\n", "#".repeat(level.min(6)), code(&name));
        *out += &labels(object);
        if let Some(signature) = &object.signature {
            self.code_block(out, signature);
        }
        self.sections(out, object);
        let prefix = format!("{name}.");
        self.summaries(out, object, &prefix);
        if let Some(source) = &object.source {
            let file = html(self.file);
            *out +=
                &format!("<details>\n<summary>Source code in <code>{file}</code></summary>\n\n");
            self.code_block(out, source);
            *out += "</details>\n\n";
        }
        for member in ordered(&object.members) {
            self.object(out, member, &prefix, level + 1);
        }
    }

    /// Write a block of `code` in the page's language.
    fn code_block(&self, out: &mut String, code: &str) {
        let fence = fence_for(code);
        *out += &format!("{fence}{}\n{code}\n{fence}\n\n", self.language);
    }

    /// Write the sections of `object`'s docstring.
    fn sections(&self, out: &mut String, object: &Object) {
        for section in &object.docs {
            match section {
                Section::Text { title: None, text } => *out += &format!("{text}\n\n"),
                Section::Text {
                    title: Some(title),
                    text,
                } => *out += &format!("**{}:**\n\n{text}\n\n", escape(title)),
                Section::Items { kind, items } => {
                    *out += &format!("**{}:**\n\n", kind.title());
                    let (header, rows) = listing(object, *kind, items);
                    table(out, &header, &rows);
                }
            }
        }
    }

    /// Write tables of what `object` defines, whose names follow `prefix`,
    /// each linked to its heading: its attributes, but where its docstring
    /// lists them, its classes, and its functions or methods.
    fn summaries(&self, out: &mut String, object: &Object, prefix: &str) {
        let listed = (object.docs.iter()).any(|section| {
            matches!(
                section,
                Section::Items {
                    kind: Items::Attributes,
                    ..
                }
            )
        });
        let functions = match object.kind {
            Kind::Class => "Methods",
            _ => "Functions",
        };
        let groups = [
            ("Attributes", Group::Attributes),
            ("Classes", Group::Classes),
            (functions, Group::Functions),
        ];
        for (title, group) in groups {
            if listed && group == Group::Attributes {
                continue;
            }
            let rows: Vec<Vec<String>> = (ordered(&object.members).into_iter())
                .filter(|member| Group::of(member) == group)
                .map(|member| {
                    let id = &self.ids[&format!("{prefix}{}", member.name)];
                    let name = format!("[{}](#{id})", code(&member.name));
                    vec![name, summary(&member.docs)]
                })
                .collect();
            if !rows.is_empty() {
                *out += &format!("**{title}:**\n\n");
                table(out, &["Name", "Description"], &rows);
            }
        }
    }
}

/// The header and rows of the table of `items`, in a section of `kind` of
/// `object`'s docstring.
fn listing(object: &Object, kind: Items, items: &[Item]) -> (Vec<&'static str>, Vec<Vec<String>>) {
    let name = |item: &Item| item.name.as_deref().map(code).unwrap_or_default();
    let ty = |ty: Option<&str>| ty.map(code).unwrap_or_default();
    match kind {
        Items::Parameters => {
            let rows = (items.iter())
                .map(|item| {
                    let written = item.name.as_deref().unwrap_or_default();
                    let parameter = (object.parameters.iter())
                        .find(|parameter| parameter.name == written.trim_start_matches('*'));
                    let annotation =
                        parameter.and_then(|parameter| parameter.annotation.as_deref());
                    let default = match parameter {
                        Some(parameter) => match &parameter.default {
                            Some(default) => code(default),
                            None => "*required*".to_string(),
                        },
                        None => item.default.as_deref().map(code).unwrap_or_default(),
                    };
                    let ty = ty(annotation.or(item.ty.as_deref()));
                    vec![name(item), ty, item.description.clone(), default]
                })
                .collect();
            (vec!["Name", "Type", "Description", "Default"], rows)
        }
        Items::OtherParameters | Items::Attributes => {
            let rows = (items.iter())
                .map(|item| {
                    let typed = (object.types.iter())
                        .find(|(name, _)| item.name.as_ref() == Some(name))
                        .filter(|_| kind == Items::Attributes)
                        .map(|(_, ty)| ty.as_str());
                    let ty = ty(typed.or(item.ty.as_deref()));
                    vec![name(item), ty, item.description.clone()]
                })
                .collect();
            (vec!["Name", "Type", "Description"], rows)
        }
        Items::Returns | Items::Yields | Items::Receives => {
            let named = items.iter().any(|item| item.name.is_some());
            let annotation = match (kind, items) {
                (Items::Returns, [_]) => object.returns.as_deref(),
                _ => None,
            };
            let rows = (items.iter())
                .map(|item| {
                    let ty = ty(annotation.or(item.ty.as_deref()));
                    let row = vec![ty, item.description.clone()];
                    match named {
                        true => [vec![name(item)], row].concat(),
                        false => row,
                    }
                })
                .collect();
            let header = match named {
                true => vec!["Name", "Type", "Description"],
                false => vec!["Type", "Description"],
            };
            (header, rows)
        }
        Items::Raises | Items::Warns => {
            let rows = (items.iter())
                .map(|item| vec![ty(item.ty.as_deref()), item.description.clone()])
                .collect();
            (vec!["Type", "Description"], rows)
        }
    }
}

/// Where a page lists an object.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Attributes,
    Classes,
    Functions,
    Modules,
}

impl Group {
    fn of(object: &Object) -> Group {
        match object.kind {
            Kind::Attribute | Kind::Property => Group::Attributes,
            Kind::Class => Group::Classes,
            Kind::Function | Kind::Method => Group::Functions,
            Kind::Package | Kind::Module => Group::Modules,
        }
    }
}

/// `members` in the order a page shows them: attributes, classes, then
/// functions, each in order of name.
fn ordered(members: &[Object]) -> Vec<&Object> {
    let mut ordered: Vec<&Object> = members.iter().collect();
    ordered.sort_by(|a, b| (Group::of(a), &a.name).cmp(&(Group::of(b), &b.name)));
    ordered
}

/// Give the heading of each of `members`, whose names follow `prefix`, and
/// of what they define, the id comrak gives it, in the order the page
/// shows them.
fn assign(
    members: &[Object],
    prefix: &str,
    anchors: &mut Anchors,
    ids: &mut HashMap<String, String>,
) {
    for member in ordered(members) {
        let name = format!("{prefix}{}", member.name);
        ids.insert(name.clone(), anchors.anchor(&name));
        assign(&member.members, &format!("{name}."), anchors, ids);
    }
}

/// The ids of a page's headings so far, made as comrak makes them.
#[derive(Default)]
struct Anchors(HashSet<String>);

impl Anchors {
    /// The id of the next heading, whose text is `text`: its letters,
    /// numbers, `-`, and `_`, in lower case, with `-` for each space, and a
    /// number after it if an earlier heading has it.
    fn anchor(&mut self, text: &str) -> String {
        let id: String = (text.to_lowercase().chars())
            .filter(|&c| c.is_alphanumeric() || matches!(c, '_' | '-' | ' '))
            .map(|c| if c == ' ' { '-' } else { c })
            .collect();
        let mut anchor = id.clone();
        let mut n = 0;
        while self.0.contains(&anchor) {
            n += 1;
            anchor = format!("{id}-{n}");
        }
        self.0.insert(anchor.clone());
        anchor
    }
}

/// The line of `object`'s labels, like `` `class` `dataclass` ``.
fn labels(object: &Object) -> String {
    let labels: Vec<String> = std::iter::once(object.kind.label())
        .chain(object.labels.iter().map(String::as_str))
        .map(code)
        .collect();
    format!("{}\n\n", labels.join(" "))
}

/// A table, with `header`, of `rows`, with each cell on one line, and its
/// pipes escaped.
fn table(out: &mut String, header: &[&str], rows: &[Vec<String>]) {
    *out += &format!("| {} |\n", header.join(" | "));
    *out += &format!("|{}\n", " --- |".repeat(header.len()));
    for row in rows {
        let cells: Vec<String> = row.iter().map(|text| cell(text)).collect();
        *out += &format!("| {} |\n", cells.join(" | "));
    }
    *out += "\n";
}

/// `text` on one line, with each `|` escaped, for a cell of a table, where
/// GitHub's Markdown takes `\|` as `|`, even in code.
fn cell(text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text.replace('|', "\\|")
}

/// `text` in code.
fn code(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let ticks = "`".repeat(longest + 1);
    let pad = if longest > 0 { " " } else { "" };
    format!("{ticks}{pad}{text}{pad}{ticks}")
}

/// `text` with the characters Markdown might read as markup escaped.
fn escape(text: &str) -> String {
    let mut escaped = String::new();
    for c in text.chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#' | '|'
        ) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// `text` with the characters HTML reads as markup escaped.
fn html(text: &str) -> String {
    (text.replace('&', "&amp;"))
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Parameter;

    fn object(name: &str, kind: Kind, docs: &str) -> Object {
        let mut object = Object::new(name, kind);
        if !docs.is_empty() {
            object.docs = vec![Section::Text {
                title: None,
                text: docs.to_string(),
            }];
        }
        object
    }

    #[test]
    fn ids_are_made_as_comrak_makes_them() {
        let mut anchors = Anchors::default();
        assert_eq!(anchors.anchor("solver"), "solver");
        assert_eq!(anchors.anchor("Solver"), "solver-1");
        assert_eq!(anchors.anchor("Solver.solve_it"), "solversolve_it");
        assert_eq!(anchors.anchor("A b-c"), "a-b-c");
        assert_eq!(anchors.anchor("solver"), "solver-2");
    }

    #[test]
    fn a_page_lists_and_links_what_its_module_defines() {
        let mut module = object("solving", Kind::Module, "Solves things.\n\nWell.");
        let mut solver = object("Solver", Kind::Class, "Solves.\n\nProblems.");
        solver.labels.push("dataclass".to_string());
        solver.signature = Some("Solver(tolerance: float = 0.1)".to_string());
        solver.parameters.push(Parameter {
            name: "tolerance".to_string(),
            annotation: Some("float".to_string()),
            default: Some("0.1".to_string()),
        });
        solver.docs.push(Section::Items {
            kind: Items::Parameters,
            items: vec![Item {
                name: Some("tolerance".to_string()),
                ty: Some("int".to_string()),
                default: None,
                description: "How close: | x |\nis.".to_string(),
            }],
        });
        solver.source = Some("def __init__(self, tolerance: float = 0.1):\n    pass".to_string());
        let mut solve = object("solve", Kind::Method, "Solve it.");
        solve.signature = Some("solve() -> bool".to_string());
        solve.returns = Some("bool".to_string());
        solve.docs.push(Section::Items {
            kind: Items::Returns,
            items: vec![Item {
                description: "Whether it did.".to_string(),
                ..Item::default()
            }],
        });
        solver.members.push(solve);
        module
            .members
            .push(object("solve", Kind::Function, "Solve anything."));
        module.members.push(solver);
        let listed = [Listed {
            name: "sub".to_string(),
            link: "sub/README.md".to_string(),
            summary: "A package.".to_string(),
        }];
        let page = page(&module, "pkg/solving.py", "python", &listed);
        assert_eq!(
            page,
            "# solving

`module`

Solves things.

Well.

**Modules:**

| Name | Description |
| --- | --- |
| [`sub`](sub/README.md) | A package. |

**Classes:**

| Name | Description |
| --- | --- |
| [`Solver`](#solver) | Solves. |

**Functions:**

| Name | Description |
| --- | --- |
| [`solve`](#solve) | Solve anything. |

## `Solver`

`class` `dataclass`

```python
Solver(tolerance: float = 0.1)
```

Solves.

Problems.

**Parameters:**

| Name | Type | Description | Default |
| --- | --- | --- | --- |
| `tolerance` | `float` | How close: \\| x \\| is. | `0.1` |

**Methods:**

| Name | Description |
| --- | --- |
| [`solve`](#solversolve) | Solve it. |

<details>
<summary>Source code in <code>pkg/solving.py</code></summary>

```python
def __init__(self, tolerance: float = 0.1):
    pass
```

</details>

### `Solver.solve`

`method`

```python
solve() -> bool
```

Solve it.

**Returns:**

| Type | Description |
| --- | --- |
| `bool` | Whether it did. |

## `solve`

`function`

Solve anything.
"
        );
    }

    #[test]
    fn a_parameter_not_in_the_signature_has_the_docstrings_default() {
        let mut function = object("f", Kind::Function, "");
        function.parameters.push(Parameter {
            name: "args".to_string(),
            annotation: None,
            default: Some("()".to_string()),
        });
        let items = [
            Item {
                name: Some("*args".to_string()),
                ty: Some("int".to_string()),
                ..Item::default()
            },
            Item {
                name: Some("key".to_string()),
                default: Some("1".to_string()),
                ..Item::default()
            },
        ];
        let (_, rows) = listing(&function, Items::Parameters, &items);
        assert_eq!(rows[0], ["`*args`", "`int`", "", "`()`"]);
        assert_eq!(rows[1], ["`key`", "", "", "`1`"]);
    }

    #[test]
    fn an_attribute_has_the_type_it_is_annotated_with() {
        let mut class = object("C", Kind::Class, "");
        class.types.push(("x".to_string(), "float".to_string()));
        let items = [
            Item {
                name: Some("x".to_string()),
                ty: Some("int".to_string()),
                ..Item::default()
            },
            Item {
                name: Some("y".to_string()),
                ty: Some("str".to_string()),
                ..Item::default()
            },
        ];
        let (header, rows) = listing(&class, Items::Attributes, &items);
        assert_eq!(header, ["Name", "Type", "Description"]);
        assert_eq!(rows, [["`x`", "`float`", ""], ["`y`", "`str`", ""]]);
    }

    #[test]
    fn code_is_fenced_by_more_backticks_than_it_has() {
        assert_eq!(code("x"), "`x`");
        assert_eq!(code("a`b"), "`` a`b ``");
    }

    #[test]
    fn markup_is_escaped() {
        assert_eq!(escape("linear_systems"), "linear\\_systems");
        assert_eq!(html("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
    }
}
