//! A little plugin that builds a page from each Org mode file, an `.org`
//! file, as Org's own HTML export shows it, without Emacs: it reads the
//! file with orgize, and converts what Org would export into a
//! [`little_markup::Document`], which [`little_markup::write`] writes as
//! the parts of its page, in Markdown, with the extensions to it the site
//! turns on. Nothing in the file is run, so a block of code shows the
//! results saved with it.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod convert;
mod inline;
mod keywords;
mod links;
mod table;

use little_markup::{Part, Settings, Warning, split_frontmatter, write};

/// Check the plugin's settings, `json`, as little gives them to it: it has
/// none, but the `read` every plugin has.
fn check_settings(json: &str) -> Result<(), String> {
    little_markup::check_settings(json, "Org")
}

/// The parts of the page of the Org file `text`, in the Markdown that
/// `markdown`, the site's `markdown` settings, as JSON, allow, and what it
/// loses on the way, to warn of: the plugin's warnings, by line, then the
/// writer's.
fn read(text: &str, markdown: &str) -> Result<(Vec<Part>, Vec<Warning>), String> {
    let settings = Settings::from_json(markdown)?;
    let (frontmatter, body, lines_before) = split_frontmatter(text).unwrap_or(("", text, 0));
    let (mut document, mut warnings) = convert::convert(body, lines_before);
    document.frontmatter = frontmatter.to_string();
    let (parts, written) = write(document, &settings);
    warnings.extend(written);
    Ok((parts, warnings))
}

little_markup::plugin!("org");

#[cfg(test)]
mod tests;
