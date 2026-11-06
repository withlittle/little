//! A little plugin that adds a page of reference for each HTTP API in an
//! OpenAPI 3 document, in JSON or YAML, in the folders it reads.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod files;
mod markdown;
mod page;
mod refs;
mod schema;
#[cfg(test)]
mod tests;
mod yaml;

use std::collections::HashSet;

use little_api_reference::pages::Added;
use little_api_reference::settings;

use crate::refs::Files;

/// What a site sets for the plugin, in its `little.yml`.
#[derive(Debug, PartialEq)]
struct Settings {
    /// The folders the documents are in, as the plugin sees them, like
    /// `api`.
    read: Vec<String>,
    /// The folder their pages go in, from the source's, like `api`, or
    /// empty for the source's own.
    url: String,
}

impl Settings {
    /// The settings in `json`, as little gives them to the plugin.
    fn parse(json: &str) -> Result<Settings, String> {
        let mut read = Vec::new();
        let mut url = String::new();
        for (name, value) in settings::mapping(json)? {
            match name.as_str() {
                "read" => read = settings::folders(&value),
                "url" => url = settings::url(&value)?,
                _ => {
                    return Err(format!(
                        "it has no setting `{name}`, only `read`, the folders its OpenAPI \
                         documents are in, and `url`, the folder its pages go in"
                    ));
                }
            }
        }
        if read.is_empty() {
            return Err(
                "name the folders its OpenAPI documents are in, in `read`, like `read: api`".into(),
            );
        }
        Ok(Settings { read, url })
    }
}

/// A page for each OpenAPI 3 document in the folders `settings` reads; or
/// why there are none.
fn document(settings: &Settings) -> Result<Added, String> {
    let mut warnings = Vec::new();
    let documents = files::find(&settings.read, &settings.url, &mut warnings)?;
    let mut pages = Vec::new();
    for document in documents {
        let files = Files::load(&document.file, document.value);
        let (markdown, page_warnings) = page::write(&files, &document.file);
        warnings.extend(page_warnings);
        pages.push((document.page, markdown));
    }
    let mut seen = HashSet::new();
    warnings.retain(|warning| seen.insert(warning.clone()));
    Ok(Added { pages, warnings })
}

little_api_reference::plugin!(document);
