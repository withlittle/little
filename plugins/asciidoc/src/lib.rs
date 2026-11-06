//! A little plugin that builds a page from each AsciiDoc file, an `.adoc` or
//! `.asciidoc` file. It reads AsciiDoc itself, with asciidoc-parser, without
//! Asciidoctor or Ruby, into a [`little_markup::Document`], which
//! [`little_markup::write`] writes as the parts of its page, in Markdown, with
//! the extensions to it the site turns on.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod blocks;
mod inlines;
mod reader;
mod tables;
mod text;

use little_markup::{Part, Settings, Warning};

/// Check the plugin's settings, `json`, as little gives them to it: it has
/// none, but the `read` every plugin has.
fn check_settings(json: &str) -> Result<(), String> {
    little_markup::check_settings(json, "AsciiDoc")
}

/// The parts of the page of the AsciiDoc file `text`, with `markdown`, the
/// site's `markdown` settings, as JSON, and what to warn of, each with its
/// line in the file: the plugin's own, then those of the parts' writer.
/// YAML frontmatter, between `---` lines at the top, is the page's.
fn read(text: &str, markdown: &str) -> Result<(Vec<Part>, Vec<Warning>), String> {
    let settings = Settings::from_json(markdown)?;
    let (frontmatter, body, offset) =
        little_markup::split_frontmatter(text).unwrap_or(("", text, 0));
    let (mut document, mut warnings) = reader::read(body, offset);
    document.frontmatter = frontmatter.to_string();
    let (parts, written) = little_markup::write(document, &settings);
    warnings.extend(written);
    Ok((parts, warnings))
}

little_markup::plugin!("adoc", "asciidoc");

#[cfg(test)]
mod tests;
