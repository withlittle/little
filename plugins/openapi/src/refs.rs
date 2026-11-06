//! The files a document's `$ref`s lead to, and following them, within a
//! file and from one to another, by JSON pointers.

use std::collections::{BTreeMap, VecDeque};

use little_api_reference::pages::{join, parent};
use serde_json::Value;

use crate::files::parse;

/// Where a value is: the file it's in, as the plugin sees it, like
/// `api/openapi.yaml`, and its JSON pointer there, like
/// `/components/schemas/Pet`, or empty for the whole file.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Location {
    pub file: String,
    pub pointer: String,
}

/// A value, in the file it's in, from which its `$ref`s lead.
#[derive(Clone, Copy, Debug)]
pub struct Node<'a> {
    pub file: &'a str,
    pub value: &'a Value,
}

impl<'a> Node<'a> {
    /// The value at `key`, if it's an object with one.
    pub fn get(&self, key: &str) -> Option<Node<'a>> {
        let value = self.value.get(key)?;
        Some(Node { value, ..*self })
    }

    /// The text at `key`, if it's an object with text there.
    pub fn str(&self, key: &str) -> Option<&'a str> {
        self.value.get(key)?.as_str()
    }

    /// The text at `key`, if it's an object with text there that isn't
    /// blank.
    pub fn text(&self, key: &str) -> Option<&'a str> {
        self.str(key).filter(|text| !text.trim().is_empty())
    }

    /// Whether `key` is `true`.
    pub fn bool(&self, key: &str) -> bool {
        self.value.get(key).and_then(Value::as_bool) == Some(true)
    }

    /// The keys and values of an object, in order, or none.
    pub fn entries(&self) -> impl Iterator<Item = (&'a str, Node<'a>)> + use<'a> {
        let node = *self;
        let entries = node.value.as_object().into_iter().flatten();
        entries.map(move |(key, value)| (key.as_str(), Node { value, ..node }))
    }

    /// The items of a list, in order, or none.
    pub fn items(&self) -> impl Iterator<Item = Node<'a>> + use<'a> {
        let node = *self;
        let items = node.value.as_array().into_iter().flatten();
        items.map(move |value| Node { value, ..node })
    }

    /// The keys and values of the object at `key`, in order, or none.
    pub fn entries_at(&self, key: &str) -> impl Iterator<Item = (&'a str, Node<'a>)> + use<'a> {
        self.get(key).into_iter().flat_map(|node| node.entries())
    }

    /// The items of the list at `key`, in order, or none.
    pub fn items_at(&self, key: &str) -> impl Iterator<Item = Node<'a>> + use<'a> {
        self.get(key).into_iter().flat_map(|node| node.items())
    }

    /// The `$ref` of an object with one.
    pub fn reference(&self) -> Option<&'a str> {
        self.str("$ref")
    }
}

/// A `$ref` followed to what it leads to.
pub struct Followed<'a> {
    /// What it leads to, with no `$ref` of its own.
    pub node: Node<'a>,
    /// Where it led, and where each `$ref` there led, in order.
    pub chain: Vec<Location>,
    /// The first `summary` and `description` beside a `$ref` on the way,
    /// which go over what it leads to's, as OpenAPI 3.1's do.
    pub summary: Option<&'a str>,
    pub description: Option<&'a str>,
}

/// A document's files: its own, and each file its `$ref`s lead to, and
/// theirs, by path, as it's parsed, or why it can't be.
pub struct Files(BTreeMap<String, Result<Value, String>>);

impl Files {
    /// The document in `file`, which holds `value`, and the files its
    /// `$ref`s lead to, read and parsed once each.
    pub fn load(file: &str, value: Value) -> Files {
        let mut files = BTreeMap::from([(file.to_string(), Ok(value))]);
        let mut queue = VecDeque::from([file.to_string()]);
        while let Some(file) = queue.pop_front() {
            let mut references = Vec::new();
            if let Some(Ok(value)) = files.get(&file) {
                collect(value, &mut references);
            }
            for reference in references {
                let Ok(Location { file: path, .. }) = locate(&file, &reference) else {
                    continue;
                };
                if files.contains_key(&path) {
                    continue;
                }
                let read =
                    std::fs::read(&path).map_err(|error| format!("can't read `{path}`: {error}"));
                let parsed = read.and_then(|bytes| {
                    parse(&path, &bytes).map_err(|error| format!("can't parse `{path}`: {error}"))
                });
                files.insert(path.clone(), parsed);
                queue.push_back(path);
            }
        }
        Files(files)
    }

    /// The whole of `file`, which [`Files::load`] was given.
    pub fn root(&self, file: &str) -> Node<'_> {
        let (file, value) = self
            .0
            .get_key_value(file)
            .expect("the document's file is loaded");
        let value = value.as_ref().expect("the document's file is parsed");
        Node { file, value }
    }

    /// What's at `location`, or why there's nothing.
    fn node(&self, location: &Location) -> Result<Node<'_>, String> {
        let Some((file, parsed)) = self.0.get_key_value(&location.file) else {
            return Err(format!("can't read `{}`", location.file));
        };
        let value = parsed.as_ref().map_err(Clone::clone)?;
        let value = value
            .pointer(&location.pointer)
            .ok_or_else(|| format!("`{}` has nothing at `{}`", location.file, location.pointer))?;
        Ok(Node { file, value })
    }

    /// What `node`'s `$ref` leads to, and the `$ref` there, if it has
    /// one, and so on; or none, warning why, if one can't be followed.
    pub fn follow<'a>(
        &'a self,
        node: Node<'a>,
        warnings: &mut Vec<String>,
    ) -> Option<Followed<'a>> {
        let mut followed = Followed {
            node,
            chain: Vec::new(),
            summary: None,
            description: None,
        };
        while let Some(reference) = followed.node.reference() {
            followed.summary = followed.summary.or(followed.node.text("summary"));
            followed.description = followed.description.or(followed.node.text("description"));
            let next = locate(followed.node.file, reference).and_then(|location| {
                if followed.chain.contains(&location) {
                    return Err("it leads back to itself".into());
                }
                Ok((self.node(&location)?, location))
            });
            match next {
                Ok((next, location)) => {
                    followed.chain.push(location);
                    followed.node = next;
                }
                Err(why) => {
                    let file = followed.node.file;
                    warnings.push(format!(
                        "can't follow `$ref: {reference}`, in `{file}`: {why}"
                    ));
                    return None;
                }
            }
        }
        Some(followed)
    }
}

/// Add each `$ref` in `value`, and in what it holds, to `references`.
fn collect(value: &Value, references: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get("$ref") {
                references.push(reference.clone());
            }
            object.values().for_each(|value| collect(value, references));
        }
        Value::Array(items) => items.iter().for_each(|value| collect(value, references)),
        _ => {}
    }
}

/// Where `reference`, a `$ref` in `file`, leads: a file, from `file`'s
/// folder, or `file` itself, and a JSON pointer in it, each
/// percent-decoded, as a URI's are; or why it can't be followed.
pub fn locate(file: &str, reference: &str) -> Result<Location, String> {
    let scheme = reference.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && (scheme.chars()).all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    });
    if scheme || reference.starts_with("//") {
        return Err("it's a URL, and only files are read".into());
    }
    let (path, fragment) = reference.split_once('#').unwrap_or((reference, ""));
    let (path, pointer) = (decode(path), decode(fragment));
    if !pointer.is_empty() && !pointer.starts_with('/') {
        return Err(
            "only a JSON pointer, like `#/components/schemas/Pet`, is read after `#`".into(),
        );
    }
    let file = match path.as_str() {
        "" => file.to_string(),
        path if path.starts_with('/') => normalize(path),
        path => normalize(&join(parent(file), path)),
    };
    Ok(Location { file, pointer })
}

/// `text` with each `%` and two hex digits the byte they write.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                at += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// `path` without its `.` parts, or a `..` after a folder, or empty
/// parts, like `api/schemas/pet.yaml` for `api/./v1/../schemas//pet.yaml`;
/// `..`s that lead out of where it starts are kept.
pub fn normalize(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            ".." if absolute => {}
            part => parts.push(part),
        }
    }
    match (absolute, parts.join("/")) {
        (true, path) => format!("/{path}"),
        (false, path) if path.is_empty() => ".".into(),
        (false, path) => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(file: &str, pointer: &str) -> Location {
        Location {
            file: file.into(),
            pointer: pointer.into(),
        }
    }

    #[test]
    fn refs_lead_from_their_files_folder() {
        let at = |reference| locate("api/v1/openapi.yaml", reference);
        assert_eq!(
            at("#/components/schemas/Pet"),
            Ok(location("api/v1/openapi.yaml", "/components/schemas/Pet"))
        );
        assert_eq!(at("pet.yaml"), Ok(location("api/v1/pet.yaml", "")));
        assert_eq!(
            at("./schemas/pet.yaml#/Pet"),
            Ok(location("api/v1/schemas/pet.yaml", "/Pet"))
        );
        assert_eq!(
            at("../shared//a%20b.json#/x~1y/%7Bid%7D"),
            Ok(location("api/shared/a b.json", "/x~1y/{id}"))
        );
        assert_eq!(at("../../../x.yaml"), Ok(location("../x.yaml", "")));
        assert_eq!(at("/abs/../x.yaml"), Ok(location("/x.yaml", "")));
        assert_eq!(
            locate("openapi.yaml", "./x.yaml"),
            Ok(location("x.yaml", ""))
        );
        assert_eq!(
            locate("../specs/openapi.yaml", "../other/x.yaml"),
            Ok(location("../other/x.yaml", ""))
        );
        assert_eq!(normalize("a/.."), ".");
    }

    #[test]
    fn urls_and_other_fragments_arent_followed() {
        for reference in [
            "https://example.com/pet.yaml",
            "//example.com/pet.yaml",
            "urn:pet",
        ] {
            assert_eq!(
                locate("a.yaml", reference),
                Err("it's a URL, and only files are read".into())
            );
        }
        let error = locate("a.yaml", "#Pet").unwrap_err();
        assert!(error.starts_with("only a JSON pointer"), "{error}");
        assert_eq!(decode("%zz%4"), "%zz%4");
    }

    #[test]
    fn refs_are_followed_to_the_end_of_their_chain() {
        let value = serde_json::json!({
            "a": {"$ref": "#/b", "description": "A"},
            "b": {"$ref": "#/c", "description": "B"},
            "c": {"type": "string"},
            "loop": {"$ref": "#/loop"},
            "missing": {"$ref": "#/nothing"},
        });
        let files = Files::load("doc.json", value);
        let root = files.root("doc.json");
        let mut warnings = Vec::new();
        let followed = files.follow(root.get("a").unwrap(), &mut warnings).unwrap();
        assert_eq!(followed.node.str("type"), Some("string"));
        assert_eq!(
            followed.chain,
            [location("doc.json", "/b"), location("doc.json", "/c")]
        );
        assert_eq!(followed.description, Some("A"));
        assert!(
            files
                .follow(root.get("loop").unwrap(), &mut warnings)
                .is_none()
        );
        assert!(
            files
                .follow(root.get("missing").unwrap(), &mut warnings)
                .is_none()
        );
        assert_eq!(
            warnings,
            [
                "can't follow `$ref: #/loop`, in `doc.json`: it leads back to itself",
                "can't follow `$ref: #/nothing`, in `doc.json`: `doc.json` has nothing at `/nothing`",
            ]
        );
    }
}
