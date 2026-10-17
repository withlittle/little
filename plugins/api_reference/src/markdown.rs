//! The Markdown of a page of the reference, from what it documents, in no
//! language in particular.

use std::collections::{HashMap, HashSet};

use crate::model::{Item, Items, Kind, Object, Section};

/// The language a reference documents, as its pages write it.
#[derive(Clone, Copy, Debug)]
pub struct Language {
    /// The language of its code blocks, like `python`.
    pub name: &'static str,
    /// What goes between a name and the name of what it defines, like `.`
    /// in `Solver.solve`.
    pub separator: &'static str,
}

/// A module, or something a module re-exports, that a page lists.
pub struct Listed {
    pub name: String,
    /// The relative link to it, like `module.md`, if it has one.
    pub link: Option<String>,
    /// The first paragraph of its docs.
    pub summary: String,
}

/// The page of `module`, which is written in `file`, in `language`, with
/// each of `lists` that isn't empty under its title, like `Modules`.
pub fn page(
    module: &Object,
    file: &str,
    language: &Language,
    lists: &[(&str, &[Listed])],
) -> String {
    let page = Page {
        ids: ids(module, language),
        file,
        language,
    };
    let mut out = format!("# {}\n\n", escape(&module.name));
    out += &labels(module);
    page.sections(&mut out, module);
    for (title, listed) in lists.iter().filter(|(_, listed)| !listed.is_empty()) {
        let rows: Vec<Vec<String>> = (listed.iter())
            .map(|listed| {
                let name = match &listed.link {
                    Some(link) => format!("[{}]({link})", code(&listed.name)),
                    None => code(&listed.name),
                };
                vec![name, listed.summary.clone()]
            })
            .collect();
        out += &format!("**{}:**\n\n", escape(title));
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

/// `page` with `title` in its frontmatter, which the site calls it in its
/// nav, breadcrumbs, browser tab, and search, in place of its heading.
pub fn titled(title: &str, page: &str) -> String {
    // A string in JSON is one in YAML too, quoted, so a `:`, `#`, or quote
    // in the title reads back as it is.
    let title = serde_json::Value::from(title);
    format!("---\ntitle: {title}\n---\n\n{page}")
}

/// The id comrak gives the heading of each object on the page of `module`,
/// in `language`, by the name the heading shows, like `Solver.solve`.
pub fn ids(module: &Object, language: &Language) -> HashMap<String, String> {
    let mut anchors = Anchors::default();
    anchors.anchor(&module.name);
    let mut ids = HashMap::new();
    assign(&module.members, "", language, &mut anchors, &mut ids);
    ids
}

/// What a page needs as it's written.
struct Page<'a> {
    /// The id of each object's heading, by its name, like `Solver.solve`.
    ids: HashMap<String, String>,
    file: &'a str,
    language: &'a Language,
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
        let prefix = format!("{name}{}", self.language.separator);
        self.summaries(out, object, &prefix);
        if let Some(source) = &object.source {
            let file = html(object.file.as_deref().unwrap_or(self.file));
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
        *out += &format!("{fence}{}\n{code}\n{fence}\n\n", self.language.name);
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
    /// each linked to its heading, a table for each group of what it
    /// defines, but for its attributes, where its docstring lists them.
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
        let members = ordered(&object.members);
        for members in members.chunk_by(|a, b| Group::of(a) == Group::of(b)) {
            let group = Group::of(members[0]);
            if group == Group::Modules || (listed && group == Group::Attributes) {
                continue;
            }
            let rows: Vec<Vec<String>> = (members.iter())
                .map(|member| {
                    let id = &self.ids[&format!("{prefix}{}", member.name)];
                    let name = format!("[{}](#{id})", code(&member.name));
                    vec![name, summary(&member.docs)]
                })
                .collect();
            *out += &format!("**{}:**\n\n", group.title(object.kind));
            table(out, &["Name", "Description"], &rows);
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
        Items::OtherParameters | Items::Attributes | Items::Fields => {
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
        Items::Raises | Items::Warns | Items::Throws => {
            let rows = (items.iter())
                .map(|item| vec![ty(item.ty.as_deref()), item.description.clone()])
                .collect();
            (vec!["Type", "Description"], rows)
        }
        Items::Variants => {
            let rows = (items.iter())
                .map(|item| vec![name(item), item.description.clone()])
                .collect();
            (vec!["Name", "Description"], rows)
        }
        Items::TypeParameters => {
            let rows = (items.iter())
                .map(|item| {
                    let mut written = item.name.clone().unwrap_or_default();
                    if let Some(constraint) = &item.ty {
                        written += &format!(" extends {constraint}");
                    }
                    if let Some(default) = &item.default {
                        written += &format!(" = {default}");
                    }
                    vec![code(&written), item.description.clone()]
                })
                .collect();
            (vec!["Name", "Description"], rows)
        }
        Items::Properties | Items::Members => {
            let defaults = items.iter().any(|item| item.default.is_some());
            let rows = (items.iter())
                .map(|item| {
                    let mut row = vec![name(item)];
                    if kind == Items::Properties {
                        row.push(ty(item.ty.as_deref()));
                    }
                    if defaults {
                        row.push(ty(item.default.as_deref()));
                    }
                    row.push(item.description.clone());
                    row
                })
                .collect();
            let header = match (kind, defaults) {
                (Items::Properties, true) => vec!["Name", "Type", "Default", "Description"],
                (Items::Properties, false) => vec!["Name", "Type", "Description"],
                (_, true) => vec!["Name", "Value", "Description"],
                (_, false) => vec!["Name", "Description"],
            };
            (header, rows)
        }
    }
}

/// Where a page lists an object, in the order it lists them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Attributes,
    Properties,
    Classes,
    Interfaces,
    Macros,
    Structs,
    Enums,
    Constants,
    Variables,
    Statics,
    Traits,
    AssociatedTypes,
    AssociatedConstants,
    Functions,
    TypeAliases,
    Unions,
    Modules,
}

impl Group {
    fn of(object: &Object) -> Group {
        match object.kind {
            Kind::Attribute | Kind::Property => Group::Attributes,
            Kind::Class => Group::Classes,
            Kind::Function | Kind::Method => Group::Functions,
            Kind::Package | Kind::Module | Kind::Crate => Group::Modules,
            Kind::Struct => Group::Structs,
            Kind::Enum => Group::Enums,
            Kind::Union => Group::Unions,
            Kind::Trait => Group::Traits,
            Kind::TypeAlias => Group::TypeAliases,
            Kind::AssociatedType => Group::AssociatedTypes,
            Kind::Constant => Group::Constants,
            Kind::AssociatedConstant => Group::AssociatedConstants,
            Kind::Static => Group::Statics,
            Kind::Macro => Group::Macros,
            Kind::Interface => Group::Interfaces,
            Kind::Variable => Group::Variables,
            Kind::Field => Group::Properties,
        }
    }

    /// Its title, in a table of what an object of `kind` defines: its
    /// functions are methods if it's a type.
    fn title(self, kind: Kind) -> &'static str {
        match self {
            Group::Attributes => "Attributes",
            Group::Properties => "Properties",
            Group::Classes => "Classes",
            Group::Interfaces => "Interfaces",
            Group::Variables => "Variables",
            Group::Macros => "Macros",
            Group::Structs => "Structs",
            Group::Enums => "Enums",
            Group::Constants => "Constants",
            Group::Statics => "Statics",
            Group::Traits => "Traits",
            Group::AssociatedTypes => "Associated Types",
            Group::AssociatedConstants => "Associated Constants",
            Group::Functions => match kind {
                Kind::Class
                | Kind::Struct
                | Kind::Enum
                | Kind::Union
                | Kind::Trait
                | Kind::Interface => "Methods",
                _ => "Functions",
            },
            Group::TypeAliases => "Type Aliases",
            Group::Unions => "Unions",
            Group::Modules => "Modules",
        }
    }
}

/// `members` in the order a page shows them, by group, then by name.
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
    language: &Language,
    anchors: &mut Anchors,
    ids: &mut HashMap<String, String>,
) {
    for member in ordered(members) {
        let name = format!("{prefix}{}", member.name);
        ids.insert(name.clone(), anchors.anchor(&name));
        let prefix = format!("{name}{}", language.separator);
        assign(&member.members, &prefix, language, anchors, ids);
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

/// The first paragraph of the text an object's docs start with, on one
/// line.
pub fn summary(sections: &[Section]) -> String {
    match sections.first() {
        Some(Section::Text { title: None, text }) => {
            let paragraph = text.split("\n\n").next().unwrap_or_default();
            paragraph.split_whitespace().collect::<Vec<_>>().join(" ")
        }
        _ => String::new(),
    }
}

/// The fence for a code block of `code`: three backticks, or more than it
/// has in a row.
pub fn fence_for(code: &str) -> String {
    let longest = (code.split(|c| c != '`')).map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Parameter;

    const PYTHON: Language = Language {
        name: "python",
        separator: ".",
    };
    const RUST: Language = Language {
        name: "rust",
        separator: "::",
    };

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
            link: Some("sub/README.md".to_string()),
            summary: "A package.".to_string(),
        }];
        let page = page(&module, "pkg/solving.py", &PYTHON, &[("Modules", &listed)]);
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
    fn a_rust_page_lists_its_types_their_methods_and_what_it_re_exports() {
        let mut module = object("config", Kind::Module, "The config.");
        let mut config = object("Config", Kind::Struct, "A site's config.");
        config.signature = Some(
            "pub struct Config {\n    pub source: PathBuf,\n    /* private fields */\n}".into(),
        );
        config.docs.push(Section::Items {
            kind: Items::Fields,
            items: vec![Item {
                name: Some("source".to_string()),
                ty: Some("PathBuf".to_string()),
                description: "Its source.".to_string(),
                ..Item::default()
            }],
        });
        config.docs.push(Section::Text {
            title: Some("Implements".to_string()),
            text: "`Clone`, `Debug`".to_string(),
        });
        config.source = Some("pub struct Config {}".to_string());
        let mut load = object("load", Kind::Function, "Load it.");
        load.signature = Some("pub fn load() -> Config".to_string());
        load.source = Some("pub fn load() -> Config {}".to_string());
        load.file = Some("src/load.rs".to_string());
        config.members.push(load);
        let mut kind = object("Kind", Kind::Enum, "What it is.");
        kind.docs.push(Section::Items {
            kind: Items::Variants,
            items: vec![Item {
                name: Some("Site".to_string()),
                description: "A site.".to_string(),
                ..Item::default()
            }],
        });
        module.members.push(kind);
        module.members.push(config);
        let re_exports = [Listed {
            name: "Extern".to_string(),
            link: None,
            summary: String::new(),
        }];
        let lists: [(&str, &[Listed]); 2] = [("Modules", &[]), ("Re-exports", &re_exports)];
        assert_eq!(ids(&module, &RUST)["Config::load"], "configload");
        assert_eq!(
            page(&module, "src/config.rs", &RUST, &lists),
            "# config

`module`

The config.

**Re-exports:**

| Name | Description |
| --- | --- |
| `Extern` |  |

**Structs:**

| Name | Description |
| --- | --- |
| [`Config`](#config-1) | A site's config. |

**Enums:**

| Name | Description |
| --- | --- |
| [`Kind`](#kind) | What it is. |

## `Config`

`struct`

```rust
pub struct Config {
    pub source: PathBuf,
    /* private fields */
}
```

A site's config.

**Fields:**

| Name | Type | Description |
| --- | --- | --- |
| `source` | `PathBuf` | Its source. |

**Implements:**

`Clone`, `Debug`

**Methods:**

| Name | Description |
| --- | --- |
| [`load`](#configload) | Load it. |

<details>
<summary>Source code in <code>src/config.rs</code></summary>

```rust
pub struct Config {}
```

</details>

### `Config::load`

`function`

```rust
pub fn load() -> Config
```

Load it.

<details>
<summary>Source code in <code>src/load.rs</code></summary>

```rust
pub fn load() -> Config {}
```

</details>

## `Kind`

`enum`

What it is.

**Variants:**

| Name | Description |
| --- | --- |
| `Site` | A site. |
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
    fn a_type_parameter_shows_its_constraint_and_default() {
        let items = [Item {
            name: Some("T".to_string()),
            ty: Some("string".to_string()),
            default: Some("\"a\"".to_string()),
            description: "The key.".to_string(),
        }];
        let function = object("f", Kind::Function, "");
        let (header, rows) = listing(&function, Items::TypeParameters, &items);
        assert_eq!(header, ["Name", "Description"]);
        assert_eq!(rows, [["`T extends string = \"a\"`", "The key."]]);
    }

    #[test]
    fn properties_and_members_show_defaults_and_values_only_if_one_has_them() {
        let property = |name: &str, default: Option<&str>| Item {
            name: Some(name.to_string()),
            ty: Some("number".to_string()),
            default: default.map(str::to_string),
            ..Item::default()
        };
        let alias = object("Options", Kind::TypeAlias, "");
        let items = [property("a", None), property("b?", Some("1"))];
        let (header, rows) = listing(&alias, Items::Properties, &items);
        assert_eq!(header, ["Name", "Type", "Default", "Description"]);
        assert_eq!(rows[0], ["`a`", "`number`", "", ""]);
        assert_eq!(rows[1], ["`b?`", "`number`", "`1`", ""]);
        let (header, _) = listing(&alias, Items::Properties, &items[..1]);
        assert_eq!(header, ["Name", "Type", "Description"]);
        let enumeration = object("Color", Kind::Enum, "");
        let (header, rows) = listing(&enumeration, Items::Members, &items);
        assert_eq!(header, ["Name", "Value", "Description"]);
        assert_eq!(rows[1], ["`b?`", "`1`", ""]);
        let (header, rows) = listing(&enumeration, Items::Members, &items[..1]);
        assert_eq!(header, ["Name", "Description"]);
        assert_eq!(rows[0], ["`a`", ""]);
    }

    #[test]
    fn properties_come_first_and_an_interfaces_functions_are_methods() {
        let mut interface = object("Shape", Kind::Interface, "");
        interface.members = vec![
            object("area", Kind::Method, "Its area."),
            object("sides", Kind::Field, "Its sides."),
        ];
        let mut module = object("shapes", Kind::Module, "");
        module.members = vec![
            object("count", Kind::Variable, "A count."),
            interface,
            object("Square", Kind::Class, "A square."),
        ];
        let page = page(&module, "shapes.ts", &PYTHON, &[]);
        let order: Vec<&str> = (page.lines())
            .filter(|line| line.starts_with("**") || line.starts_with('#'))
            .collect();
        assert_eq!(
            order,
            [
                "# shapes",
                "**Classes:**",
                "**Interfaces:**",
                "**Variables:**",
                "## `Square`",
                "## `Shape`",
                "**Properties:**",
                "**Methods:**",
                "### `Shape.sides`",
                "### `Shape.area`",
                "## `count`",
            ]
        );
        assert!(page.contains("`property`\n\nIts sides."));
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

    #[test]
    fn a_summary_is_the_first_paragraph_on_one_line() {
        let docs = [Section::Text {
            title: None,
            text: "A summary\nover two lines.\n\nMore.".to_string(),
        }];
        assert_eq!(summary(&docs), "A summary over two lines.");
        let titled = [Section::Text {
            title: Some("Notes".to_string()),
            text: "Not a summary.".to_string(),
        }];
        assert_eq!(summary(&titled), "");
        assert_eq!(summary(&[]), "");
    }

    #[test]
    fn a_title_goes_in_frontmatter_quoted() {
        assert_eq!(
            titled("API reference", "# my-lib\n"),
            "---\ntitle: \"API reference\"\n---\n\n# my-lib\n"
        );
        assert_eq!(
            titled("\"Core\": #1 \\ API", "# core\n"),
            "---\ntitle: \"\\\"Core\\\": #1 \\\\ API\"\n---\n\n# core\n"
        );
    }

    #[test]
    fn a_fence_is_longer_than_the_backticks_in_its_code() {
        assert_eq!(fence_for("x = 1"), "```");
        assert_eq!(fence_for("s = '```'"), "````");
    }
}
