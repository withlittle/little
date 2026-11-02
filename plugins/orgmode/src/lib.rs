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
use serde_json::Value;

/// Check the plugin's settings, `json`, as little gives them to it: it has
/// none, but the `read` every plugin has.
fn check_settings(json: &str) -> Result<(), String> {
    let settings: serde_json::Map<String, Value> = serde_json::from_str(json)
        .map_err(|_| format!("its settings must be a mapping, not {json}"))?;
    match settings.keys().find(|name| *name != "read") {
        Some(name) => Err(format!(
            "it has no setting `{name}`, only `read`, which it doesn't need: little gives it each \
             Org file"
        )),
        None => Ok(()),
    }
}

/// The parts of the page of the Org file `text`, in the Markdown that
/// `markdown`, the site's `markdown` settings, as JSON, allow, and what it
/// loses on the way, to warn of: the plugin's warnings, by line, then the
/// writer's.
fn read(text: &str, markdown: &str) -> Result<(Vec<Part>, Vec<Warning>), String> {
    let settings = Settings::from_json(markdown)?;
    let (frontmatter, body, lines_before) = match split_frontmatter(text) {
        Some((yaml, rest, lines)) => (yaml.to_string(), rest, lines),
        None => (String::new(), text, 0),
    };
    let (mut document, mut warnings) = convert::convert(body, lines_before);
    document.frontmatter = frontmatter;
    let (parts, written) = write(document, &settings);
    warnings.extend(written);
    Ok((parts, warnings))
}

#[cfg(target_arch = "wasm32")]
mod plugin {
    wit_bindgen::generate!({
        path: "../../wit/plugin.wit",
        inline: "
            package little:orgmode;

            world orgmode {
                import little:plugin/warnings@0.2.1;
                export little:plugin/setup@0.2.1;
                export little:plugin/formats@0.2.1;
            }
        ",
        world: "little:orgmode/orgmode",
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
            vec!["org".to_string()]
        }

        fn read(page: String, text: String, markdown: String) -> Result<Vec<Part>, String> {
            let (parts, warnings) = super::read(&text, &markdown)?;
            for warning in warnings {
                warn(&warning.display(&page));
            }
            Ok(parts
                .into_iter()
                .map(|part| match part {
                    little_markup::Part::Markdown(markdown) => Part::Markdown(markdown),
                    little_markup::Part::Html(html) => Part::Html(html),
                })
                .collect())
        }
    }

    export!(Plugin);
}

#[cfg(test)]
mod tests;
