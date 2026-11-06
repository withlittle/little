//! What a page of an API reference documents, and its Markdown, in no
//! language in particular, for the plugins that build one from a language's
//! code, like `pydoc`, from Python's; and the settings and pages they have
//! alike.

pub mod markdown;
pub mod model;
pub mod pages;
pub mod settings;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod text;

/// The glue between a plugin and little's `wit/plugin.wit`: a module that
/// exports the `setup` and `files` hooks, and imports `warnings`.
///
/// `setup` keeps the site's settings, read with the plugin's
/// `Settings::parse`. Then `files` adds the pages `document` gives back for
/// them, and warns of what it warns of. `document` is a
/// `fn(&Settings) -> Result<Added, String>`, and an error from it stops the
/// build.
///
/// The plugin calls it at the root of its crate, where its `Settings` are.
/// It's built only for WebAssembly, so the plugin's tests run without it.
/// The plugin depends on `wit-bindgen`, which reads `wit/plugin.wit` from
/// the plugin's folder in `plugins/`.
#[macro_export]
macro_rules! plugin {
    ($document:expr) => {
        #[cfg(target_arch = "wasm32")]
        mod plugin {
            use std::sync::Mutex;

            // `document`, and what it names, are found from the root of the
            // plugin's crate.
            use super::*;

            ::wit_bindgen::generate!({
                path: "../../wit/plugin.wit",
                inline: "
                    package little:api-reference;

                    world api-reference {
                        import little:plugin/warnings@0.2.1;
                        export little:plugin/setup@0.2.1;
                        export little:plugin/files@0.2.1;
                    }
                ",
                world: "little:api-reference/api-reference",
                generate_all,
            });

            use exports::little::plugin::files::{self, Changes, File};
            use exports::little::plugin::setup;
            use little::plugin::warnings::warn;

            /// What the site set, from `setup`.
            static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

            struct Plugin;

            impl setup::Guest for Plugin {
                fn setup(settings: String) -> Result<(), String> {
                    *SETTINGS.lock().unwrap() = Some(Settings::parse(&settings)?);
                    Ok(())
                }
            }

            impl files::Guest for Plugin {
                fn files(_: Vec<String>) -> Result<Changes, String> {
                    let document: fn(&Settings) -> Result<$crate::pages::Added, String> =
                        $document;
                    let settings = SETTINGS.lock().unwrap();
                    let added = document(settings.as_ref().expect("little calls `setup` first"))?;
                    let add = added.files(warn).into_iter();
                    Ok(Changes {
                        add: add
                            .map(|(path, contents)| File { path, contents })
                            .collect(),
                        remove: Vec::new(),
                    })
                }
            }

            export!(Plugin);
        }
    };
}
