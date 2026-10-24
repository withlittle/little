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
use serde_json::Value;

/// Check the plugin's settings, `json`, as little gives them to it: it has
/// none, but the `read` every plugin has.
fn check_settings(json: &str) -> Result<(), String> {
    let settings: serde_json::Map<String, Value> = serde_json::from_str(json)
        .map_err(|_| format!("its settings must be a mapping, not {json}"))?;
    match settings.keys().find(|name| *name != "read") {
        Some(name) => Err(format!(
            "it has no setting `{name}`, only `read`, which it doesn't need: little gives it each \
             AsciiDoc file"
        )),
        None => Ok(()),
    }
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

#[cfg(target_arch = "wasm32")]
mod plugin {
    wit_bindgen::generate!({
        path: "../../wit/plugin.wit",
        inline: "
            package little:asciidoc;

            world asciidoc {
                import little:plugin/warnings@0.2.1;
                export little:plugin/setup@0.2.1;
                export little:plugin/formats@0.2.1;
            }
        ",
        world: "little:asciidoc/asciidoc",
        generate_all,
    });

    use exports::little::plugin::formats::{self, Part};
    use exports::little::plugin::setup;
    use little::plugin::warnings::warn;

    struct Plugin;

    impl setup::Guest for Plugin {
        fn setup(settings: String) -> Result<(), String> {
            super::check_settings(&settings)
        }
    }

    impl formats::Guest for Plugin {
        fn extensions() -> Vec<String> {
            vec!["adoc".to_string(), "asciidoc".to_string()]
        }

        fn read(page: String, text: String, markdown: String) -> Result<Vec<Part>, String> {
            let (parts, warnings) = super::read(&text, &markdown)?;
            for warning in &warnings {
                warn(&warning.display(&page));
            }
            Ok(parts
                .into_iter()
                .map(|part| match part {
                    super::Part::Markdown(markdown) => Part::Markdown(markdown),
                    super::Part::Html(html) => Part::Html(html),
                })
                .collect())
        }
    }

    export!(Plugin);
}

#[cfg(test)]
mod tests;
