//! A plugin for little's tests. It renders the languages its settings list
//! in `languages`, or `test`, and does what each block's first line says:
//! `markdown` and `html` give back the lines after it, `error` and `panic`
//! fail with the rest of the line, and the others report what the plugin
//! can reach.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use exports::little::plugin::code_blocks::{Block, Guest, Output};

wit_bindgen::generate!({ path: "../../wit/plugin.wit", world: "plugin" });

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

export!(Plugin);
