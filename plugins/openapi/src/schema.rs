//! Writing schemas: what one is, in a few words, the notes on a value of
//! one, the table of an object's fields, and the section of each schema in
//! `components.schemas`.

use std::collections::HashMap;

use little_api_reference::pages::last;
use serde_json::Value;

use crate::markdown::{self, both, code, collapse, either, label, sentences, table};
use crate::page::{Target, Writer};
use crate::refs::{Files, Location, Node};

/// The columns of a table of fields.
pub const FIELDS: [&str; 4] = ["Field", "Type", "Required", "Description"];

/// How many `$ref`s deep a schema is followed into those it holds, which
/// can hold it.
const DEPTH: usize = 8;

/// The name of each schema in `components.schemas`, by where it is, and of
/// each place a `$ref` there leads, if it has no name of its own.
pub fn names<'a>(files: &'a Files, root: Node<'a>) -> HashMap<Location, &'a str> {
    let components = root.get("components");
    let schemas: Vec<(&str, Node)> = (components.into_iter())
        .flat_map(|components| components.entries_at("schemas"))
        .collect();
    let mut names = HashMap::new();
    for (name, _) in &schemas {
        names.insert(own(root.file, name), *name);
    }
    for (name, schema) in schemas {
        let Some(followed) = files.follow(schema, &mut Vec::new()) else {
            continue;
        };
        let mut current = name;
        for location in followed.chain {
            match names.get(&location).copied() {
                Some(named) => current = named,
                None => {
                    names.insert(location, current);
                }
            }
        }
    }
    names
}

/// Where the schema called `name` is in `components.schemas`, in `file`.
fn own(file: &str, name: &str) -> Location {
    let name = name.replace('~', "~0").replace('/', "~1");
    Location {
        file: file.to_string(),
        pointer: format!("/components/schemas/{name}"),
    }
}

/// `yes` if `required`, else `no`.
pub fn yes(required: bool) -> String {
    match required {
        true => "yes".into(),
        false => "no".into(),
    }
}

/// The types `schema`'s `type` names.
fn types<'a>(schema: Node<'a>) -> Vec<&'a str> {
    match schema.value.get("type") {
        Some(Value::String(kind)) => vec![kind.as_str()],
        Some(Value::Array(kinds)) => kinds.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

/// The types `schema`, which has no `type`, can be, from its other
/// keywords, or none, if it can be anything.
fn inferred(schema: Node) -> Vec<&'static str> {
    let value = schema.value;
    let values: Vec<&Value> = match (value.get("enum"), value.get("const")) {
        (Some(Value::Array(values)), _) => values.iter().collect(),
        (_, Some(value)) => vec![value],
        _ => Vec::new(),
    };
    if !values.is_empty() {
        let mut kinds = Vec::new();
        for value in values {
            let kind = match value {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
        return kinds;
    }
    let has = |keys: &[&str]| keys.iter().any(|key| value.get(key).is_some());
    let groups: [(&[&str], &str); 4] = [
        (
            &[
                "properties",
                "additionalProperties",
                "patternProperties",
                "required",
                "minProperties",
                "maxProperties",
            ],
            "object",
        ),
        (
            &[
                "items",
                "prefixItems",
                "minItems",
                "maxItems",
                "uniqueItems",
                "contains",
            ],
            "array",
        ),
        (&["minLength", "maxLength", "pattern"], "string"),
        (
            &[
                "minimum",
                "maximum",
                "exclusiveMinimum",
                "exclusiveMaximum",
                "multipleOf",
            ],
            "number",
        ),
    ];
    let kind = groups
        .iter()
        .find(|(keys, _)| has(keys))
        .map(|(_, kind)| *kind);
    kind.into_iter().collect()
}

/// How many of `thing` there can be, from `min` to `max`, like `1 to 64
/// characters`, or none, if there's no limit.
fn count(min: Option<u64>, max: Option<u64>, thing: &str) -> Option<String> {
    let things = |n: u64| match n {
        1 => thing.to_string(),
        _ => format!("{thing}s"),
    };
    match (min.filter(|min| *min > 0), max) {
        (Some(min), Some(max)) if min == max => Some(format!("Exactly {min} {}.", things(min))),
        (Some(min), Some(max)) => Some(format!("{min} to {max} {}.", things(max))),
        (Some(min), None) => Some(format!("At least {min} {}.", things(min))),
        (None, Some(max)) => Some(format!("At most {max} {}.", things(max))),
        (None, None) => None,
    }
}

impl<'a> Writer<'a> {
    /// The name of the first place in `chain` that has one.
    fn named(&self, chain: &[Location]) -> Option<&'a str> {
        chain
            .iter()
            .find_map(|location| self.names.get(location).copied())
    }

    /// A link to the section of the schema called `name`.
    fn schema_link(&self, name: &str) -> String {
        self.target_link(code(name), Target::Schema(name.to_string()))
    }

    /// What `schema` is, in a few words, like `` `string(date-time)` ``, or
    /// a link to its section, if it has a name.
    pub fn kind(&mut self, schema: Node<'a>) -> String {
        let kinds = self.kinds(schema, &mut Vec::new());
        either(&kinds)
    }

    /// What `schema` can be, each in a few words, with `stack` the places
    /// the `$ref`s it's in led, so one that holds itself isn't followed
    /// again.
    fn kinds(&mut self, schema: Node<'a>, stack: &mut Vec<Location>) -> Vec<String> {
        let Some(reference) = schema.reference() else {
            let mut kinds = self.own_kinds(schema, stack);
            if schema.bool("nullable") && !kinds.contains(&code("null")) {
                kinds.push(code("null"));
            }
            return kinds;
        };
        let Some(followed) = self.follow(schema) else {
            return vec![code(reference)];
        };
        if let Some(name) = self.named(&followed.chain) {
            return vec![self.schema_link(name)];
        }
        let looped = followed
            .chain
            .iter()
            .any(|location| stack.contains(location));
        if looped || stack.len() >= DEPTH {
            return vec![code(&unnamed(followed.chain.last()))];
        }
        stack.extend(followed.chain.last().cloned());
        let kinds = self.kinds(followed.node, stack);
        stack.pop();
        kinds
    }

    /// What `schema`, which isn't a `$ref`, can be, but `null`, if it's
    /// 3.0's `nullable`.
    fn own_kinds(&mut self, schema: Node<'a>, stack: &mut Vec<Location>) -> Vec<String> {
        let Some(object) = schema.value.as_object() else {
            return vec!["any".into()];
        };
        for key in ["oneOf", "anyOf"] {
            let mut kinds = Vec::new();
            for member in schema.items_at(key) {
                for kind in self.kinds(member, stack) {
                    if !kinds.contains(&kind) {
                        kinds.push(kind);
                    }
                }
            }
            if kinds.iter().any(|kind| kind != "any") {
                return kinds;
            }
        }
        if object.contains_key("allOf") {
            let mut parts = Vec::new();
            for member in schema.items_at("allOf") {
                let part = either(&self.kinds(member, stack));
                if part != "any" && !parts.contains(&part) {
                    parts.push(part);
                }
            }
            if object.contains_key("properties") && !parts.contains(&code("object")) {
                parts.push(code("object"));
            }
            match parts.len() {
                0 => {}
                1 => return parts,
                _ => return vec![both(&parts)],
            }
        }
        let mut types = types(schema);
        if !object.contains_key("type") {
            types = inferred(schema);
        }
        let mut kinds = Vec::new();
        for kind in types {
            let kind = self.typed(schema, kind, stack);
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
        if kinds.is_empty() {
            kinds.push("any".into());
        }
        kinds
    }

    /// What `schema` is, as the type `kind`: an array of what, a map of
    /// what, or the type, with its format.
    fn typed(&mut self, schema: Node<'a>, kind: &str, stack: &mut Vec<Location>) -> String {
        match kind {
            "array" => match schema.get("items").filter(|items| items.value.is_object()) {
                Some(items) => format!("array of {}", either(&self.kinds(items, stack))),
                None => code("array"),
            },
            "object" => {
                let values = schema
                    .get("additionalProperties")
                    .filter(|values| values.value.is_object());
                match values {
                    Some(values) if schema.get("properties").is_none() => {
                        format!("map of {}", either(&self.kinds(values, stack)))
                    }
                    _ => code("object"),
                }
            }
            "string" | "number" | "integer" => match schema.text("format") {
                Some(format) => code(&format!("{kind}({})", format.trim())),
                None => code(kind),
            },
            kind => code(kind),
        }
    }

    /// What `nodes`, a schema, and what it leads to, if it has no name, say
    /// of a value of it, each a sentence: if `all`, whether `owner`, a
    /// parameter or header, or it, is deprecated, and `description`; then
    /// whether it's read- or write-only, the values it can be, its default,
    /// and its limits; and, if `all`, its examples.
    pub fn facts(
        &mut self,
        nodes: &[Node<'a>],
        owner: Option<Node<'a>>,
        description: Option<&str>,
        all: bool,
    ) -> Vec<String> {
        let get = |key: &str| nodes.iter().find_map(|node| node.value.get(key));
        let number = |key: &str| get(key).filter(|value| value.is_number());
        let size = |key: &str| get(key).and_then(Value::as_u64);
        let mut facts = Vec::new();
        let deprecated = owner
            .iter()
            .chain(nodes)
            .any(|node| node.bool("deprecated"));
        if all && deprecated {
            facts.push("Deprecated.".into());
        }
        if get("readOnly") == Some(&Value::Bool(true)) {
            facts.push("Read-only.".into());
        }
        if get("writeOnly") == Some(&Value::Bool(true)) {
            facts.push("Write-only.".into());
        }
        if let Some(description) = description.filter(|_| all) {
            facts.push(collapse(description));
        }
        if let Some(values) = get("enum").and_then(Value::as_array) {
            facts.extend(one_of(values, ""));
        } else if let Some(value) = get("const") {
            facts.push(format!("Always {}.", markdown::value(value)));
        }
        let items = get("items").filter(|items| items.get("$ref").is_none());
        if let Some(values) = items
            .and_then(|items| items.get("enum"))
            .and_then(Value::as_array)
        {
            facts.extend(one_of(values, "Each is "));
        }
        if let Some(default) = get("default") {
            facts.push(format!("Defaults to {}.", markdown::value(default)));
        }
        for (bound, exclusive, at, past) in [
            ("minimum", "exclusiveMinimum", "At least", "More than"),
            ("maximum", "exclusiveMaximum", "At most", "Less than"),
        ] {
            if let Some(Value::Number(limit)) = get(exclusive) {
                facts.push(format!("{past} {}.", code(&limit.to_string())));
            }
            if let Some(limit) = number(bound) {
                let words = match get(exclusive) == Some(&Value::Bool(true)) {
                    true => past,
                    false => at,
                };
                facts.push(format!("{words} {}.", code(&limit.to_string())));
            }
        }
        facts.extend(count(size("minLength"), size("maxLength"), "character"));
        if let Some(pattern) = get("pattern").and_then(Value::as_str) {
            facts.push(format!("Matches {}.", code(pattern)));
        }
        facts.extend(count(size("minItems"), size("maxItems"), "item"));
        if get("uniqueItems") == Some(&Value::Bool(true)) {
            facts.push("Its items are unique.".into());
        }
        if let Some(multiple) = number("multipleOf") {
            facts.push(format!("A multiple of {}.", code(&multiple.to_string())));
        }
        if all {
            let examples = self.scalar_examples(nodes, owner);
            match examples.as_slice() {
                [] => {}
                [example] => facts.push(format!("Example: {example}.")),
                examples => facts.push(format!("Examples: {}.", both(examples))),
            }
        }
        facts
    }

    /// The examples of a value of `nodes`, which `owner` has, each in code,
    /// if they're neither objects nor arrays: `owner`'s own, else those of
    /// `nodes`.
    fn scalar_examples(&mut self, nodes: &[Node<'a>], owner: Option<Node<'a>>) -> Vec<String> {
        let mut values: Vec<&Value> = Vec::new();
        if let Some(owner) = owner {
            if let Some(example) = owner.value.get("example") {
                values.push(example);
            } else {
                for (_, example) in owner.entries_at("examples") {
                    let Some(example) = self.resolve(example) else {
                        continue;
                    };
                    let value = example
                        .value
                        .get("value")
                        .or(example.value.get("dataValue"));
                    values.extend(value);
                }
            }
        }
        if values.is_empty() {
            let get = |key: &str| nodes.iter().find_map(|node| node.value.get(key));
            match (get("example"), get("examples")) {
                (Some(example), _) => values.push(example),
                (None, Some(Value::Array(examples))) => values.extend(examples),
                _ => {}
            }
        }
        (values.into_iter())
            .filter(|value| !value.is_object() && !value.is_array())
            .map(markdown::value)
            .collect()
    }

    /// The notes on a value of `schema`, which `owner`, a parameter or
    /// header, has, if it's one's, as sentences, with `over`, the
    /// description beside its `$ref`, over its own.
    pub fn notes(
        &mut self,
        schema: Option<Node<'a>>,
        owner: Option<Node<'a>>,
        over: Option<&'a str>,
    ) -> String {
        let mut nodes = Vec::new();
        let mut beside = None;
        if let Some(schema) = schema {
            nodes.push(schema);
            let followed = schema.reference().and_then(|_| self.follow(schema));
            if let Some(followed) =
                followed.filter(|followed| self.named(&followed.chain).is_none())
            {
                nodes.push(followed.node);
                beside = followed.description;
            }
        }
        let description = (over.or(owner.and_then(|owner| owner.text("description"))))
            .or(nodes.iter().find_map(|node| node.text("description")))
            .or(beside);
        let facts = self.facts(&nodes, owner, description, true);
        sentences(&facts)
    }

    /// The schema `schema` is, if it's written where it's used: if it's not
    /// a `$ref`, or it's one to a schema with no name, or any, if it's a
    /// `member` of an `allOf`, but not one in `stack`; with where it is, if
    /// a `$ref` led there.
    pub fn expand(
        &mut self,
        schema: Node<'a>,
        stack: &[Location],
        member: bool,
    ) -> Option<(Node<'a>, Option<Location>)> {
        if schema.reference().is_none() {
            return Some((schema, None));
        }
        let followed = self.follow(schema)?;
        if !member && self.named(&followed.chain).is_some() {
            return None;
        }
        let looped = followed
            .chain
            .iter()
            .any(|location| stack.contains(location));
        if looped || stack.len() >= DEPTH {
            return None;
        }
        Some((followed.node, followed.chain.last().cloned()))
    }

    /// The rows of a table of the fields of `schema`, if it's written where
    /// it's used.
    pub fn field_rows(&mut self, schema: Node<'a>) -> Vec<Vec<String>> {
        let mut rows = Vec::new();
        if let Some((node, location)) = self.expand(schema, &[], false) {
            let mut stack: Vec<Location> = location.into_iter().collect();
            self.rows(node, "", &mut stack, &mut rows);
        }
        rows
    }

    /// Add a row for each field of `schema`, an object, or of its items, if
    /// it's an array, to `rows`, each after `path`, like `owner`, and for
    /// each field of those written where they're used, in turn.
    fn rows(
        &mut self,
        schema: Node<'a>,
        path: &str,
        stack: &mut Vec<Location>,
        rows: &mut Vec<Vec<String>>,
    ) {
        let array = types(schema).contains(&"array")
            || (schema.get("items").is_some() && schema.get("properties").is_none());
        if array {
            let items = schema.get("items").filter(|items| items.value.is_object());
            if let Some((items, location)) =
                items.and_then(|items| self.expand(items, stack, false))
            {
                self.within(location, stack, |writer, stack| {
                    writer.rows(items, &format!("{path}[]"), stack, rows);
                });
            }
            return;
        }
        for (name, property, required) in self.properties(schema, stack) {
            let field = match path {
                "" => name.to_string(),
                path => format!("{path}.{name}"),
            };
            let kind = either(&self.kinds(property, stack));
            let notes = self.notes(Some(property), None, None);
            rows.push(vec![code(&field), kind, yes(required), notes]);
            if let Some((inner, location)) = self.expand(property, stack, false) {
                self.within(location, stack, |writer, stack| {
                    writer.rows(inner, &field, stack, rows);
                });
            }
        }
    }

    /// Do `then`, with `location`, if there's one, on `stack`.
    fn within(
        &mut self,
        location: Option<Location>,
        stack: &mut Vec<Location>,
        then: impl FnOnce(&mut Self, &mut Vec<Location>),
    ) {
        let pushed = location.is_some();
        stack.extend(location);
        then(self, stack);
        if pushed {
            stack.pop();
        }
    }

    /// The fields of `schema`, an object, with those of the members of its
    /// `allOf` first, each once, and whether it's required.
    fn properties(
        &mut self,
        schema: Node<'a>,
        stack: &mut Vec<Location>,
    ) -> Vec<(&'a str, Node<'a>, bool)> {
        let mut fields = Vec::new();
        let mut required = Vec::new();
        self.gather(schema, stack, &mut fields, &mut required);
        (fields.into_iter())
            .map(|(name, node)| (name, node, required.contains(&name)))
            .collect()
    }

    /// Add the fields of `schema`, and of the members of its `allOf`, first,
    /// to `fields`, each once, and the names of those it requires to
    /// `required`.
    fn gather(
        &mut self,
        schema: Node<'a>,
        stack: &mut Vec<Location>,
        fields: &mut Vec<(&'a str, Node<'a>)>,
        required: &mut Vec<&'a str>,
    ) {
        for member in schema.items_at("allOf") {
            if let Some((member, location)) = self.expand(member, stack, true) {
                self.within(location, stack, |writer, stack| {
                    writer.gather(member, stack, fields, required);
                });
            }
        }
        for (name, property) in schema.entries_at("properties") {
            if !fields.iter().any(|(other, _)| *other == name) {
                fields.push((name, property));
            }
        }
        for name in schema
            .items_at("required")
            .filter_map(|name| name.value.as_str())
        {
            if !required.contains(&name) {
                required.push(name);
            }
        }
    }

    /// `## Schemas`, with each schema in `components.schemas`.
    pub fn schemas(&mut self) {
        let components = self.root.get("components");
        let schemas: Vec<(&str, Node)> = (components.into_iter())
            .flat_map(|components| components.entries_at("schemas"))
            .collect();
        if schemas.is_empty() {
            return;
        }
        self.heading(2, "Schemas", None);
        for (name, schema) in schemas {
            self.heading(3, &code(name), Some(Target::Schema(name.to_string())));
            self.schema(name, schema);
        }
    }

    /// The section of `schema`, called `name`: its type, its description,
    /// its fields, and its examples; or a link to another schema, if it's
    /// that one.
    fn schema(&mut self, name: &str, schema: Node<'a>) {
        let mut stack = vec![own(self.root.file, name)];
        let (node, beside) = match schema.reference() {
            None => (schema, None),
            Some(reference) => {
                let Some(followed) = self.follow(schema) else {
                    self.line(&label("Type", "", &code(reference)));
                    return;
                };
                let other = (followed.chain.iter()).find_map(|location| {
                    self.names
                        .get(location)
                        .copied()
                        .filter(|other| *other != name)
                });
                if let Some(other) = other {
                    if schema.bool("deprecated") {
                        self.line("**Deprecated.**");
                    }
                    let link = self.schema_link(other);
                    self.line(&label("Type", "", &link));
                    self.description(followed.description, 3);
                    return;
                }
                stack.extend(followed.chain);
                (followed.node, followed.description)
            }
        };
        if node.bool("deprecated") || schema.bool("deprecated") {
            self.line("**Deprecated.**");
        }
        let mut parts = vec![either(&self.kinds(node, &mut stack))];
        parts.extend(self.facts(&[node], None, None, false));
        self.line(&label("Type", "", &sentences(&parts)));
        self.description(beside.or(node.text("description")), 3);
        let mut rows = Vec::new();
        self.rows(node, "", &mut stack, &mut rows);
        if !rows.is_empty() {
            table(&mut self.out, &FIELDS, &rows);
        }
        let examples: Vec<&Value> = match (node.value.get("example"), node.value.get("examples")) {
            (Some(example), _) => vec![example],
            (None, Some(Value::Array(examples))) => examples.iter().collect(),
            _ => Vec::new(),
        };
        let bold = match examples.len() {
            0 => return,
            1 => "Example",
            _ => "Examples",
        };
        if examples
            .iter()
            .all(|example| !example.is_object() && !example.is_array())
        {
            let values: Vec<String> = examples.into_iter().map(markdown::value).collect();
            self.line(&label(bold, "", &both(&values)));
        } else {
            self.line(&label(bold, "", ""));
            for example in examples {
                self.example_block(example, None);
            }
        }
    }
}

/// What to call the schema at `location`, which has no name, where it's
/// not written out again: the last part of its pointer, like `Node` for
/// `/$defs/Node`, else its file's name.
fn unnamed(location: Option<&Location>) -> String {
    let Some(location) = location else {
        return String::new();
    };
    match location
        .pointer
        .rsplit('/')
        .next()
        .filter(|part| !part.is_empty())
    {
        Some(part) => part.replace("~1", "/").replace("~0", "~"),
        None => last(&location.file).to_string(),
    }
}

/// That a value is one of `values`, after `before`, like `Each is `.
fn one_of(values: &[Value], before: &str) -> Option<String> {
    let values: Vec<String> = values.iter().map(markdown::value).collect();
    let sentence = match values.as_slice() {
        [] => return None,
        [value] => format!("always {value}."),
        values => format!("one of {}.", either(values)),
    };
    Some(match before {
        "" => capitalized(&sentence),
        before => format!("{before}{sentence}"),
    })
}

/// `text` with a capital at its start.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
