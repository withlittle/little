//! A plugin for little's tests, which exports every hook, and does what its
//! settings say.
//!
//! In `setup`, it warns of its settings' `warn`, fails with `fail`, or
//! panics, for `panic`. It renders the languages they list in `languages`,
//! or `test`, and does what each block's first line says: `markdown` and
//! `html` give back the lines after it, `error` and `panic` fail with the
//! rest of the line, and the others report what the plugin can reach.
//!
//! Each other hook does what the settings under its name say, like
//! `"source": { "append": "!" }`, or nothing, without them. Under `only`,
//! they're for one page's file, or URL, else they're for every call. Then a
//! hook warns of `warn`, fails with `error`, or panics with `panic`, each
//! with `{arg}` in place of the page's file, or URL; else:
//!
//! - `source`, `content`, and `html` replace the first of `replace`, a pair,
//!   with the second, then add `append`, with `{arg}`, too;
//! - `config`, `site`, and `page` give back `give`, as it is, if it's
//!   there, else set the key `echo` names to the JSON they're given, then
//!   set each key in `merge`;
//! - `theme` and `files` leave out each file `remove` lists, then add each
//!   in `add`, `{ "path": …, "text": … }`, or `{ "path": …, "read": … }` for
//!   the file it reads, and one at `list` listing the paths they're given;
//! - `destination` adds each in `add`, and one at `list`, as they do.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

wit_bindgen::generate!({ path: "../../wit/plugin.wit", world: "hooks" });

use exports::little::plugin::code_blocks::{Block, Output};
use exports::little::plugin::{
    code_blocks, config, content, destination, files, html, page, setup, site, source, theme,
};
use little::plugin::warnings::warn;

static SETTINGS: Mutex<String> = Mutex::new(String::new());
static COUNT: AtomicU32 = AtomicU32::new(0);

struct Plugin;

/// The plugin's settings.
fn settings() -> Value {
    serde_json::from_str(&SETTINGS.lock().unwrap()).unwrap_or_default()
}

/// The settings for `hook`, if it's to do something with `arg`, the page's
/// file, or URL, once it's warned, failed, or panicked, as they say.
fn begin(hook: &str, arg: &str) -> Result<Option<Value>, String> {
    let settings = settings().get(hook).cloned().unwrap_or_default();
    let only = settings.get("only").and_then(Value::as_str);
    if settings.is_null() || only.is_some_and(|only| only != arg) {
        return Ok(None);
    }
    let say =
        |key: &str| (settings.get(key).and_then(Value::as_str)).map(|it| it.replace("{arg}", arg));
    if let Some(message) = say("warn") {
        warn(&message);
    }
    if let Some(reason) = say("error") {
        return Err(reason);
    }
    if let Some(message) = say("panic") {
        panic!("{message}");
    }
    Ok(Some(settings))
}

/// `text`, as `hook`'s settings change it.
fn text(hook: &str, arg: &str, mut text: String) -> Result<String, String> {
    let Some(settings) = begin(hook, arg)? else {
        return Ok(text);
    };
    if let Some(Value::Array(pair)) = settings.get("replace")
        && let [Value::String(from), Value::String(to)] = pair.as_slice()
    {
        text = text.replace(from.as_str(), to);
    }
    if let Some(append) = settings.get("append").and_then(Value::as_str) {
        text.push_str(&append.replace("{arg}", arg));
    }
    Ok(text)
}

/// `json`, an object, as `hook`'s settings change it.
fn object(hook: &str, json: String) -> Result<String, String> {
    let mut object: Map<String, Value> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let url = object
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(settings) = begin(hook, url)? else {
        return Ok(json);
    };
    if let Some(give) = settings.get("give").and_then(Value::as_str) {
        return Ok(give.to_string());
    }
    if let Some(key) = settings.get("echo").and_then(Value::as_str) {
        object.insert(key.to_string(), Value::String(json));
    }
    if let Some(Value::Object(merge)) = settings.get("merge") {
        object.extend(merge.clone());
    }
    serde_json::to_string(&object).map_err(|e| e.to_string())
}

/// The files `settings` add: each in `add`, then one at `list`, listing
/// `paths`.
fn added(settings: &Value, paths: &[String]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut added = Vec::new();
    for file in settings
        .get("add")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let field = |key: &str| file.get(key).and_then(Value::as_str);
        let contents = match field("read") {
            Some(read) => std::fs::read(read).map_err(|e| e.to_string())?,
            None => field("text").unwrap_or_default().as_bytes().to_vec(),
        };
        added.push((field("path").unwrap_or_default().to_string(), contents));
    }
    if let Some(list) = settings.get("list").and_then(Value::as_str) {
        added.push((list.to_string(), paths.join("\n").into_bytes()));
    }
    Ok(added)
}

/// The paths `settings` remove.
fn removed(settings: &Value) -> Vec<String> {
    let remove = settings.get("remove").and_then(Value::as_array);
    (remove.into_iter().flatten())
        .filter_map(|path| path.as_str().map(str::to_string))
        .collect()
}

impl setup::Guest for Plugin {
    fn setup(settings: String) -> Result<(), String> {
        let value: Value = serde_json::from_str(&settings).map_err(|e| e.to_string())?;
        if let Some(message) = value.get("warn").and_then(Value::as_str) {
            warn(message);
        }
        if let Some(reason) = value.get("fail").and_then(Value::as_str) {
            return Err(reason.to_string());
        }
        if value.get("panic").is_some() {
            panic!("the plugin panicked in setup");
        }
        *SETTINGS.lock().unwrap() = settings;
        Ok(())
    }
}

impl config::Guest for Plugin {
    fn config(config: String) -> Result<String, String> {
        object("config", config)
    }
}

impl theme::Guest for Plugin {
    fn theme(files: Vec<theme::File>) -> Result<Vec<theme::File>, String> {
        let Some(settings) = begin("theme", "")? else {
            return Ok(files);
        };
        let paths: Vec<String> = files.iter().map(|file| file.path.clone()).collect();
        let remove = removed(&settings);
        let mut files: Vec<_> = (files.into_iter())
            .filter(|file| !remove.contains(&file.path))
            .collect();
        for (path, contents) in added(&settings, &paths)? {
            files.push(theme::File { path, contents });
        }
        Ok(files)
    }
}

impl files::Guest for Plugin {
    fn files(paths: Vec<String>) -> Result<files::Changes, String> {
        let Some(settings) = begin("files", "")? else {
            return Ok(files::Changes {
                add: Vec::new(),
                remove: Vec::new(),
            });
        };
        let add = added(&settings, &paths)?;
        Ok(files::Changes {
            add: (add.into_iter())
                .map(|(path, contents)| files::File { path, contents })
                .collect(),
            remove: removed(&settings),
        })
    }
}

impl source::Guest for Plugin {
    fn source(page: String, source: String) -> Result<String, String> {
        text("source", &page, source)
    }
}

impl code_blocks::Guest for Plugin {
    fn languages() -> Vec<String> {
        match settings().get("languages") {
            Some(languages) => {
                serde_json::from_value(languages.clone()).expect("`languages` lists strings")
            }
            None => vec!["test".to_string()],
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

impl content::Guest for Plugin {
    fn content(page: String, html: String) -> Result<String, String> {
        text("content", &page, html)
    }
}

impl site::Guest for Plugin {
    fn site(site: String) -> Result<String, String> {
        object("site", site)
    }
}

impl page::Guest for Plugin {
    fn page(page: String) -> Result<String, String> {
        object("page", page)
    }
}

impl html::Guest for Plugin {
    fn html(url: String, html: String) -> Result<String, String> {
        text("html", &url, html)
    }
}

impl destination::Guest for Plugin {
    fn destination(paths: Vec<String>) -> Result<Vec<destination::File>, String> {
        let Some(settings) = begin("destination", "")? else {
            return Ok(Vec::new());
        };
        let add = added(&settings, &paths)?;
        Ok((add.into_iter())
            .map(|(path, contents)| destination::File { path, contents })
            .collect())
    }
}

export!(Plugin);
