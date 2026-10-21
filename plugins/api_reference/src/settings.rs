//! The settings the plugins share, as little gives them to a plugin: a
//! JSON object, from its entry under `plugins` in a site's `little.yml`.

use serde_json::{Map, Value};

/// The settings in `json`, by name.
pub fn mapping(json: &str) -> Result<Map<String, Value>, String> {
    match serde_json::from_str(json) {
        Ok(Value::Object(settings)) => Ok(settings),
        _ => Err(format!("its settings must be a mapping, not {json}")),
    }
}

/// The folders `read` names, as the plugin sees them, like `src`.
pub fn folders(read: &Value) -> Vec<String> {
    let folders = read.as_array().into_iter().flatten();
    folders
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}

/// The one folder in `read`, which holds `code`, like `the package's code`,
/// for a plugin that documents one `thing`, like a `package`; none, or more
/// than one, is an error.
pub fn one_folder(read: Vec<String>, thing: &str, code: &str) -> Result<String, String> {
    match <[String; 1]>::try_from(read) {
        Ok([folder]) => Ok(folder),
        Err(read) if read.is_empty() => Err(format!(
            "name the folder {code} is in, in `read`, like `read: ./src`"
        )),
        Err(_) => Err(format!(
            "it documents one {thing}, in one folder, so `read` names one; for each other \
             {thing}, add another entry under `plugins`, with another name and the same `wasm`"
        )),
    }
}

/// `title`: what the site calls `page`, like `the crate's page`.
pub fn title(title: &Value, page: &str) -> Result<String, String> {
    let text = title.as_str().filter(|title| !title.trim().is_empty());
    text.map(String::from).ok_or_else(|| {
        format!("`title` must be what the site calls {page}, like `title: API reference`")
    })
}

/// `url`: the folder the plugin's pages go in, as [`folder`] reads it.
pub fn url(url: &Value) -> Result<String, String> {
    (url.as_str().and_then(folder)).ok_or_else(|| {
        "`url` must be the folder its pages go in, like `/api`, with no part `.` or `..`".into()
    })
}

/// `url` as a folder from the source's, with `/` between its parts, or none
/// if a part is empty, `.`, or `..`.
pub fn folder(url: &str) -> Option<String> {
    let url = url.trim_matches('/');
    let parts_ok =
        (url.split('/')).all(|part| !matches!(part, "" | "." | "..") && !part.contains('\\'));
    (url.is_empty() || parts_ok).then(|| url.to_string())
}
