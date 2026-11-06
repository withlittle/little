//! A page, as little's plugins for other formats read it, and the Markdown
//! it's written as: only what the site's `markdown` settings turn on,
//! escaped, so it reads back as it was.
//!
//! A plugin reads its format into a [`Document`], and gives it to
//! [`write`], with the site's [`Settings`], for the parts of its page, and
//! what it lost on the way, to warn of.

mod block;
mod frontmatter;
mod inline;
mod model;
mod normalize;
mod settings;
mod slug;
mod targets;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
#[cfg(test)]
mod tests;

pub use frontmatter::split_frontmatter;
pub use model::*;
pub use settings::Settings;
pub use targets::each_inlines;

/// A part of a page, as the `formats` hook gives it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    /// Markdown, read on its own.
    Markdown(String),
    /// HTML, placed as it is.
    Html(String),
}

/// Something a page loses as it's written, to warn of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// The line of the file it's on, if it's known.
    pub line: Option<usize>,
    /// What's lost.
    pub message: String,
}

impl Warning {
    /// A warning of `message`, on `line`, if it's known.
    pub fn new(line: Option<usize>, message: impl Into<String>) -> Self {
        Warning {
            line,
            message: message.into(),
        }
    }

    /// The warning, as little shows it: the page's path, its line, if it's
    /// known, and what's lost.
    pub fn display(&self, page: &str) -> String {
        match self.line {
            Some(line) => format!("{page}:{line}: {}", self.message),
            None => format!("{page}: {}", self.message),
        }
    }
}

/// The warnings of a page, each once.
#[derive(Debug, Default)]
pub(crate) struct Warnings(pub Vec<Warning>);

impl Warnings {
    pub fn push(&mut self, line: Option<usize>, message: impl Into<String>) {
        let warning = Warning::new(line, message);
        if !self.0.contains(&warning) {
            self.0.push(warning);
        }
    }
}

/// Writes `document` as the parts of its page, in Markdown that `settings`
/// allow, with the warnings of what it loses: a part of HTML for each block
/// of raw HTML at its top, so it's placed whole, and one of Markdown for
/// what's around them, the first with its frontmatter.
pub fn write(mut document: Document, settings: &Settings) -> (Vec<Part>, Vec<Warning>) {
    let mut warnings = Warnings::default();
    targets::fill_links(&mut document);
    normalize::Normalizer::new(settings, &document, &mut warnings).document(&mut document);
    let targets = targets::Targets::new(&document, settings);
    let parts = block::Writer::new(settings, &targets, &document, &mut warnings).write();
    (parts, warnings.0)
}

/// Checks a plugin's settings, `json`, as little gives them to it. A plugin
/// that reads a format has none, but the `read` every plugin has. `noun`
/// names the format, as in "each AsciiDoc file".
pub fn check_settings(json: &str, noun: &str) -> Result<(), String> {
    let settings: serde_json::Map<String, serde_json::Value> = serde_json::from_str(json)
        .map_err(|_| format!("its settings must be a mapping, not {json}"))?;
    match settings.keys().find(|name| *name != "read") {
        Some(name) => Err(format!(
            "it has no setting `{name}`, only `read`, which it doesn't need: little gives it each \
             {noun} file"
        )),
        None => Ok(()),
    }
}

/// Exports the hooks of a plugin that reads a format, built for
/// WebAssembly: `setup`, which checks its settings, and `formats`, which
/// reads each file with one of `extensions`, like `"adoc"`.
///
/// Invoke it at the plugin's root, beside its `check_settings` and `read`,
/// which the hooks call. The plugin depends on wit-bindgen, as this library
/// doesn't, and is in `plugins/`, as `wit/plugin.wit` is read from two
/// folders above it. Each plugin's world has the same name, since little
/// finds hooks by their interfaces' names.
#[macro_export]
macro_rules! plugin {
    ($($extension:literal),+ $(,)?) => {
        #[cfg(target_arch = "wasm32")]
        mod plugin {
            ::wit_bindgen::generate!({
                path: "../../wit/plugin.wit",
                inline: "
                    package little:markup;

                    world markup {
                        import little:plugin/warnings@0.2.1;
                        export little:plugin/setup@0.2.1;
                        export little:plugin/formats@0.2.1;
                    }
                ",
                world: "little:markup/markup",
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
                    vec![$($extension.to_string()),+]
                }

                fn read(page: String, text: String, markdown: String) -> Result<Vec<Part>, String> {
                    let (parts, warnings) = super::read(&text, &markdown)?;
                    for warning in &warnings {
                        warn(&warning.display(&page));
                    }
                    Ok(parts
                        .into_iter()
                        .map(|part| match part {
                            $crate::Part::Markdown(markdown) => Part::Markdown(markdown),
                            $crate::Part::Html(html) => Part::Html(html),
                        })
                        .collect())
                }
            }

            export!(Plugin);
        }
    };
}
