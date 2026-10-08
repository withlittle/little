//! A plugin for little's tests. It renders the languages its settings list
//! in `languages`, or `test`, and does what each block's first line says:
//! `markdown` and `html` give back the lines after it, `error` and `panic`
//! fail with the rest of the line, and the others report what the plugin
//! can reach.
//!
//! Built with the `pages` feature, it also adds the pages its settings list
//! in `pages`, each `{ "path": …, "markdown": … }`, or `{ "path": …,
//! "read": … }` for a page that's the file it reads, with the `warnings`
//! they list, or fails with `pages_error`, or panics with `pages_panic`.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use exports::little::plugin::code_blocks::{Block, Guest, Output};

#[cfg(not(feature = "pages"))]
wit_bindgen::generate!({ path: "../../wit/plugin.wit", world: "plugin" });
#[cfg(feature = "pages")]
wit_bindgen::generate!({ path: "../../wit/plugin.wit", world: "plugin-with-pages" });

static SETTINGS: Mutex<String> = Mutex::new(String::new());
static COUNT: AtomicU32 = AtomicU32::new(0);

struct Plugin;

impl Guest for Plugin {
    fn setup(settings: String) -> Result<Vec<String>, String> {
        let value: serde_json::Value =
            serde_json::from_str(&settings).map_err(|error| error.to_string())?;
        if let Some(reason) = value.get("fail").and_then(|reason| reason.as_str()) {
            return Err(reason.to_string());
        }
        if value.get("panic").is_some() {
            panic!("the plugin panicked in setup");
        }
        *SETTINGS.lock().unwrap() = settings;
        match value.get("languages") {
            Some(languages) => {
                serde_json::from_value(languages.clone()).map_err(|error| error.to_string())
            }
            None => Ok(vec!["test".to_string()]),
        }
    }

    fn render(block: Block) -> Result<Output, String> {
        let (line, rest) = block.text.split_once('\n').unwrap_or((&block.text, ""));
        let (command, argument) = line.split_once(' ').unwrap_or((line, ""));
        let text = |text: String| Ok(Output::Markdown(text));
        match command {
            "markdown" => text(rest.to_string()),
            "html" => Ok(Output::Html(rest.to_string())),
            "error" => Err(argument.to_string()),
            "panic" => panic!("{argument}"),
            "block" => text(format!(
                "{} | {} | {}",
                block.language, block.info, block.page
            )),
            "settings" => text(SETTINGS.lock().unwrap().clone()),
            "count" => text((COUNT.fetch_add(1, Ordering::Relaxed) + 1).to_string()),
            "read" => std::fs::read_to_string(argument)
                .map(Output::Markdown)
                .map_err(|error| error.to_string()),
            "write" => std::fs::write(argument, "written")
                .map(|()| Output::Markdown("written".to_string()))
                .map_err(|error| error.to_string()),
            "env" => text(format!(
                "{} variables and {} arguments",
                std::env::vars().count(),
                std::env::args().count()
            )),
            "time" => text(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|error| error.to_string())?
                    .as_secs()
                    .to_string(),
            ),
            "random" => text(RandomState::new().hash_one(0).to_string()),
            "print" => {
                println!("{argument}");
                text(String::new())
            }
            _ => Err(format!("there's no command `{command}`")),
        }
    }
}

#[cfg(feature = "pages")]
impl exports::little::plugin::pages::Guest for Plugin {
    fn pages() -> Result<exports::little::plugin::pages::Added, String> {
        use exports::little::plugin::pages::{Added, Page};

        let settings: serde_json::Value =
            serde_json::from_str(&SETTINGS.lock().unwrap()).map_err(|error| error.to_string())?;
        let setting = |key: &str| settings.get(key).and_then(|value| value.as_str());
        if let Some(reason) = setting("pages_error") {
            return Err(reason.to_string());
        }
        if let Some(message) = setting("pages_panic") {
            panic!("{message}");
        }
        let mut pages = Vec::new();
        for page in settings["pages"].as_array().into_iter().flatten() {
            let field = |key: &str| page.get(key).and_then(|value| value.as_str());
            let markdown = match field("read") {
                Some(file) => std::fs::read_to_string(file).map_err(|error| error.to_string())?,
                None => field("markdown").unwrap_or_default().to_string(),
            };
            pages.push(Page {
                path: field("path").unwrap_or_default().to_string(),
                markdown,
            });
        }
        let warnings = settings["warnings"].as_array().into_iter().flatten();
        Ok(Added {
            pages,
            warnings: warnings
                .filter_map(|warning| warning.as_str().map(str::to_string))
                .collect(),
        })
    }
}

export!(Plugin);
