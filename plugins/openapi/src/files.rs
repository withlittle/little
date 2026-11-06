//! Finding the OpenAPI documents in the folders the plugin reads, and
//! parsing a file of JSON or YAML.

use std::collections::BTreeMap;

use little_api_reference::pages::{join, names};
use serde_json::Value;

use crate::markdown::{code, either};
use crate::refs::normalize;
use crate::yaml;

/// An OpenAPI 3 document.
pub struct Document {
    /// Its file, as the plugin sees it, like `api/openapi.yaml`, without
    /// `.` parts.
    pub file: String,
    /// Its page's path, from the source's folder, like `openapi.md`.
    pub page: String,
    /// What it holds.
    pub value: Value,
}

/// What the file at `path` holds, as JSON, if its extension is `.json`, or
/// else as YAML, or why it can't be parsed.
pub fn parse(path: &str, bytes: &[u8]) -> Result<Value, String> {
    let bytes = bytes.strip_prefix("\u{feff}".as_bytes()).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| "it isn't UTF-8".to_string())?;
    match path.to_ascii_lowercase().ends_with(".json") {
        true => serde_json::from_str(text).map_err(|error| error.to_string()),
        false => yaml::parse(text),
    }
}

/// The OpenAPI 3 documents in `read`, the folders the plugin reads, and in
/// the folders in them, each with its page in `url`, in order of their
/// pages; or an error, if two would be the same page.
pub fn find(
    read: &[String],
    url: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<Document>, String> {
    let mut documents: BTreeMap<String, Document> = BTreeMap::new();
    let mut searched = Vec::new();
    for folder in read {
        let mut paths = Vec::new();
        if visit(folder, "", &mut paths, warnings).is_none() {
            continue;
        }
        searched.push(code(folder));
        for (path, stem) in paths {
            // As a `$ref` back to it leads, from another file.
            let file = normalize(&join(folder, &path));
            let Some(value) = read_document(&file, warnings) else {
                continue;
            };
            let page = join(url, &format!("{stem}.md"));
            if let Some(other) = documents.get(&page) {
                return Err(format!(
                    "`{}` and `{file}` would both be the page `{page}`: rename or move one",
                    other.file
                ));
            }
            documents.insert(page.clone(), Document { file, page, value });
        }
    }
    if documents.is_empty() && !searched.is_empty() {
        warnings.push(format!(
            "there's no OpenAPI 3 document in {}: a `.json`, `.yaml`, or `.yml` file whose \
             `openapi` is like `3.1.0`",
            either(&searched)
        ));
    }
    Ok(documents.into_values().collect())
}

/// Add the path of each `.json`, `.yaml`, and `.yml` file in `folder`, in
/// `root`, and in the folders in it, but `node_modules` and those whose
/// names start with `.`, to `paths`, each from `root`, with that path
/// without its extension; or none, warning why, if `folder` can't be read.
fn visit(
    root: &str,
    folder: &str,
    paths: &mut Vec<(String, String)>,
    warnings: &mut Vec<String>,
) -> Option<()> {
    let path = match folder {
        "" => root.to_string(),
        folder => join(root, folder),
    };
    let names = names(&path)
        .map_err(|warning| warnings.push(warning))
        .ok()?;
    for name in names {
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }
        let inner = join(folder, &name);
        let Ok(metadata) = std::fs::metadata(join(&path, &name)) else {
            continue;
        };
        if metadata.is_dir() {
            visit(root, &inner, paths, warnings);
        } else if let Some(stem) = stem(&name) {
            let stem = join(folder, stem);
            paths.push((inner, stem));
        }
    }
    Some(())
}

/// `name` without its extension, if it's `.json`, `.yaml`, or `.yml`,
/// whatever its case.
fn stem(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    let known = ["json", "yaml", "yml"]
        .iter()
        .any(|known| extension.eq_ignore_ascii_case(known));
    (known && !stem.is_empty()).then_some(stem)
}

/// What `file` holds, if it's an OpenAPI 3 document, with an `openapi`
/// like `3.1.0`; else none, warning of a document of another version, or
/// of a file that mentions `openapi` or `swagger` but can't be read.
fn read_document(file: &str, warnings: &mut Vec<String>) -> Option<Value> {
    let bytes = std::fs::read(file)
        .map_err(|error| warnings.push(format!("can't read `{file}`: {error}")))
        .ok()?;
    let mentions = |word: &[u8]| bytes.windows(word.len()).any(|window| window == word);
    if !mentions(b"openapi") && !mentions(b"swagger") {
        return None;
    }
    let value = parse(file, &bytes)
        .map_err(|error| warnings.push(format!("can't parse `{file}`: {error}")))
        .ok()?;
    let version = |key| match value.get(key)? {
        Value::String(version) => Some(version.clone()),
        Value::Number(version) => Some(version.to_string()),
        _ => None,
    };
    match (version("openapi"), version("swagger")) {
        (Some(version), _) if version == "3" || version.starts_with("3.") => Some(value),
        (Some(version), _) => {
            warnings.push(format!(
                "`{file}` is in OpenAPI {version}, but it reads only OpenAPI 3, like `3.1.0`"
            ));
            None
        }
        (None, Some(version)) => {
            warnings.push(format!(
                "`{file}` is in Swagger {version}, which it doesn't read: convert it to OpenAPI 3 first"
            ));
            None
        }
        (None, None) => None,
    }
}
