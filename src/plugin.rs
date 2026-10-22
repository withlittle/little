//! Plugins: WebAssembly components, built for WASI 0.2, that export hooks
//! from `wit/plugin.wit`, each called at a step of the build. A build starts
//! each plugin the site names once, then, as it reaches each step, calls the
//! hook for it of each plugin that exports one, in the order the site names
//! them, with what the step made: the config, the theme's files, the
//! source's files, each page's text, which a plugin can read in a format of
//! its own, its code blocks, and its HTML, `site`, each `page`, each page as
//! the theme writes it, and the files the site is written with. What each
//! hook gives back is what the next plugin, then the build, goes on with.
//!
//! Each runs in wasmtime's sandbox, which reaches nothing little doesn't give
//! it: no network, no environment variables, and no files but the folders
//! its `read` names, which it can't change. What it prints goes to little's
//! stderr. So a site builds the same each time, its clock reads the time the
//! build started, and its random numbers are the same in each build.

use anyhow::{Context, Result, anyhow, bail};
use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde_yaml::Mapping;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::ops::Bound;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, SystemTime};
use time::OffsetDateTime;
use wasmtime::component::{Component, HasSelf, InstancePre, Linker, ResourceTable};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{FsPerms, HostWallClock, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::config::{self, Config, Wasm};
use crate::download;

/// little's side of the hooks in `wit/plugin.wit`, which wasmtime generates
/// as little is compiled.
mod bindings {
    wasmtime::component::bindgen!({
        path: "wit/plugin.wit",
        world: "hooks",
        additional_derives: [PartialEq],
    });
}

use bindings::exports::little::plugin as wit;
use wit::code_blocks::{Block, Output};
pub(crate) use wit::formats::{Code, Part};

/// The package of `wit/plugin.wit`, with its version, whose hooks little
/// calls. A plugin built for an earlier version with the same minor version,
/// like 0.2.0, exports hooks wasmtime finds as this version's.
const PACKAGE: &str = "little:plugin@0.2.1";

/// One engine compiles and runs every plugin.
static ENGINE: LazyLock<Engine> = LazyLock::new(Engine::default);

/// Each plugin compiled so far, by its file, so `little serve` compiles it
/// again only once it changes.
static COMPILED: LazyLock<Mutex<HashMap<PathBuf, Compiled>>> = LazyLock::new(Mutex::default);

/// A plugin compiled from its file, with when the file was changed, and its
/// size, then.
type Compiled = (Stamp, Pre);

/// When a file was changed, and its size.
type Stamp = (Option<SystemTime>, u64);

/// A plugin compiled, and linked to WASI, ready to start, with where its
/// hooks are.
#[derive(Clone)]
struct Pre {
    instance: InstancePre<State>,
    exports: Exports,
}

/// Declares `Exports`, where each hook a plugin exports is in it, and
/// `Hooks`, the hooks themselves, in a plugin that's started.
macro_rules! hooks {
    ($($hook:ident),*) => {
        /// Where each hook a plugin exports is in it: none for one it
        /// doesn't.
        #[derive(Clone)]
        struct Exports {
            $($hook: Option<wit::$hook::GuestIndices>,)*
        }

        /// The hooks a plugin that's started exports.
        struct Hooks {
            $($hook: Option<wit::$hook::Guest>,)*
        }

        impl Exports {
            /// The hooks `instance` exports, or why one isn't the hook in
            /// `wit/plugin.wit`.
            fn of(instance: &InstancePre<State>) -> Result<Exports> {
                Ok(Exports {
                    $($hook: {
                        let hook = stringify!($hook).replace('_', "-");
                        let (package, version) = PACKAGE.split_once('@').expect("it has a version");
                        let name = format!("{package}/{hook}@{version}");
                        match instance.component().get_export_index(None, &name) {
                            None => None,
                            Some(_) => Some(
                                wit::$hook::GuestIndices::new(instance)
                                    .map_err(|e| anyhow!("its `{hook}` hook isn't {PACKAGE}'s: {e:#}"))?,
                            ),
                        }
                    },)*
                })
            }

            /// Whether it exports any hook.
            fn any(&self) -> bool {
                false $(|| self.$hook.is_some())*
            }

            /// The hooks in `instance`, in `store`, or why one isn't the hook
            /// in `wit/plugin.wit`.
            fn load(
                &self,
                store: &mut Store<State>,
                instance: &wasmtime::component::Instance,
            ) -> Result<Hooks, String> {
                Ok(Hooks {
                    $($hook: match &self.$hook {
                        None => None,
                        Some(indices) => Some(indices.load(&mut *store, instance).map_err(|e| {
                            let hook = stringify!($hook).replace('_', "-");
                            format!("its `{hook}` hook isn't {PACKAGE}'s: {}", one_line(&format!("{e:#}")))
                        })?),
                    },)*
                })
            }
        }
    };
}

hooks!(
    setup,
    config,
    theme,
    files,
    source,
    formats,
    code_blocks,
    content,
    site,
    page,
    html,
    destination
);

/// A hook a plugin exports, which little calls only if it does.
fn hook<T>(hook: &Option<T>) -> &T {
    hook.as_ref()
        .expect("little calls only the hooks a plugin exports")
}

/// Something a plugin warns of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// The plugin's name, like `api`.
    pub plugin: String,
    pub warned: Warned,
}

/// What a plugin warns of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warned {
    /// What it says, with `warn`.
    Message(String),
    /// One of its hooks, like `source`, failed on `on`, what the hook was
    /// given, like `docs/guide.md`, so the build went on without its change.
    Failed {
        hook: &'static str,
        on: String,
        reason: String,
    },
}

impl Warning {
    /// That `plugin`'s `hook` failed on `on`, for `reason`.
    fn failed(plugin: &str, hook: &'static str, on: &str, reason: String) -> Warning {
        Warning {
            plugin: plugin.to_string(),
            warned: Warned::Failed {
                hook,
                on: on.to_string(),
                reason,
            },
        }
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let plugin = &self.plugin;
        match &self.warned {
            Warned::Message(message) => write!(f, "plugin `{plugin}`: {message}"),
            Warned::Failed { hook, on, reason } => {
                write!(
                    f,
                    "plugin `{plugin}`'s `{hook}` hook failed on {on}: {reason}"
                )
            }
        }
    }
}

/// A block a plugin couldn't render, which is left as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unrendered {
    /// The plugin's name, like `bob`.
    pub plugin: String,
    /// The line of its file the block starts on, if its file is Markdown:
    /// a block in a part a `formats` hook gives back has none.
    pub line: Option<usize>,
    /// Why, as the plugin says, or that it crashed.
    pub reason: String,
}

/// A file a hook is given, or gives back, like one of the theme's, and the
/// plugin whose hook last changed it, if one did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookedFile {
    /// Its path, like `_templates/default.html`.
    pub path: String,
    pub contents: Vec<u8>,
    /// The plugin, like `og`.
    pub plugin: Option<String>,
}

/// The plugins a build has started, the languages each renders, the
/// extensions of the files each reads, and what they've warned of.
#[derive(Default)]
pub struct Plugins {
    plugins: Vec<Running>,
    /// Each language a plugin renders, in lowercase, and which plugin.
    languages: HashMap<String, usize>,
    /// Each extension a plugin reads, without its `.`, and which plugin.
    extensions: HashMap<String, usize>,
    /// What they've warned of, and which hooks failed, since
    /// [`Plugins::take_warnings`].
    warnings: Vec<Warning>,
}

/// A plugin a build has started.
struct Running {
    name: String,
    pre: Pre,
    read: Vec<config::Read>,
    settings: String,
    /// The time its clock reads, from 1970.
    now: Duration,
    /// The plugin, unless it crashed.
    instance: Option<Instance>,
}

/// A plugin that's started, in the store that holds its memory.
struct Instance {
    store: Store<State>,
    hooks: Hooks,
}

/// What a plugin's store holds: what it can reach through WASI, and what it
/// warns of.
struct State {
    ctx: WasiCtx,
    table: ResourceTable,
    /// What it's warned of, with `warn`, since little last looked.
    warnings: Vec<String>,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl bindings::little::plugin::warnings::Host for State {
    fn warn(&mut self, message: String) {
        self.warnings.push(message);
    }
}

/// A clock stopped at the time the build started.
struct BuildTime(Duration);

impl HostWallClock for BuildTime {
    fn resolution(&self) -> Duration {
        Duration::from_nanos(1)
    }

    fn now(&self) -> Duration {
        self.0
    }
}

impl Plugins {
    /// Start each of `plugins`, which the site at `site_dir` names, in turn,
    /// downloading and compiling it first if need be, with its clock at
    /// `now`, and set it up. Stops at the first that can't start, or that
    /// renders a language, or reads an extension, one before it does.
    pub fn start(
        plugins: &[config::Plugin],
        site_dir: &Path,
        now: OffsetDateTime,
    ) -> Result<Plugins> {
        let now = Duration::try_from(now - OffsetDateTime::UNIX_EPOCH).unwrap_or_default();
        let mut started = Plugins::default();
        for config::Plugin {
            name,
            wasm,
            read,
            settings,
        } in plugins
        {
            let file = match wasm {
                Wasm::File(file) => file.clone(),
                Wasm::Url(url) => download::plugin(site_dir, url)
                    .map_err(|e| anyhow!("plugin `{name}`'s `wasm` is `{url}`, but {e:#}"))?,
            };
            if !file.is_file() {
                bail!(
                    "plugin `{name}`'s `wasm` is {}, but there's no file there",
                    file.display()
                );
            }
            if let Some(read) = read.iter().find(|read| !read.folder.is_dir()) {
                bail!(
                    "plugin `{name}` reads {}, but there's no folder there",
                    read.folder.display()
                );
            }
            let pre = compile(&file)
                .with_context(|| format!("loading plugin `{name}` from {}", file.display()))?;
            let mut plugin = Running {
                name: name.clone(),
                pre,
                read: read.clone(),
                settings: settings.clone(),
                now,
                instance: None,
            };
            let cant_start = |reason| anyhow!("plugin `{name}` can't start: {reason}");
            plugin.start().map_err(cant_start)?;
            plugin.drain(&mut started.warnings);
            if plugin.pre.exports.code_blocks.is_some() {
                let languages = (plugin.call(&mut started.warnings, |store, hooks| {
                    hook(&hooks.code_blocks).call_languages(store).map(Ok)
                }))
                .map_err(cant_start)?;
                let index = started.plugins.len();
                for language in languages {
                    let language = language.to_lowercase();
                    if language.is_empty()
                        || language.contains(|c: char| c.is_whitespace() || c == ',')
                    {
                        bail!(
                            "plugin `{name}` renders `{language}`, but a language must be one \
                             word, like `bob`"
                        );
                    }
                    match started.languages.get(&language) {
                        Some(&other) if other != index => bail!(
                            "plugins `{}` and `{name}` both render `{language}`",
                            started.plugins[other].name
                        ),
                        _ => started.languages.insert(language, index),
                    };
                }
            }
            if plugin.pre.exports.formats.is_some() {
                let extensions = (plugin.call(&mut started.warnings, |store, hooks| {
                    hook(&hooks.formats).call_extensions(store).map(Ok)
                }))
                .map_err(cant_start)?;
                let index = started.plugins.len();
                for extension in extensions {
                    if extension.is_empty()
                        || extension.contains(|c: char| c.is_whitespace() || "./\\".contains(c))
                    {
                        bail!(
                            "plugin `{name}` reads `{extension}`, but an extension must be one \
                             word, without its `.`, like `ipynb`"
                        );
                    }
                    if extension == "md" {
                        bail!("plugin `{name}` reads `md`, but little reads Markdown itself");
                    }
                    match started.extensions.get(&extension) {
                        Some(&other) if other != index => bail!(
                            "plugins `{}` and `{name}` both read `{extension}`",
                            started.plugins[other].name
                        ),
                        _ => started.extensions.insert(extension, index),
                    };
                }
            }
            started.plugins.push(plugin);
        }
        Ok(started)
    }

    /// What the plugins have warned of, and which of their hooks failed,
    /// since this was last called, in the order they did.
    pub fn take_warnings(&mut self) -> Vec<Warning> {
        std::mem::take(&mut self.warnings)
    }

    /// The `config` of the site at `site_dir`, as each plugin's `config`
    /// hook changes it, in turn. Stops at the first that fails, or changes
    /// it so little can't use it.
    pub fn config(&mut self, mut config: Config, site_dir: &Path) -> Result<Config> {
        for plugin in &mut self.plugins {
            if plugin.pre.exports.config.is_none() {
                continue;
            }
            let name = plugin.name.clone();
            let values = (to_json(&config.values, "the config"))
                .and_then(|json| {
                    plugin.call(&mut self.warnings, |store, hooks| {
                        hook(&hooks.config).call_config(store, &json)
                    })
                })
                .and_then(|json| from_json(&json, "a config"))
                .map_err(|reason| anyhow!("plugin `{name}`'s `config` hook failed: {reason}"))?;
            config = (config.hooked(site_dir, values))
                .map_err(|e| anyhow!("plugin `{name}`'s `config` hook {e:#}"))?;
        }
        Ok(config)
    }

    /// The theme's `files`, as each plugin's `theme` hook changes them, in
    /// turn, each plugin's checked with `check`, which is given its name.
    /// Stops at the first that fails, or gives back a file at a path that
    /// isn't relative, two at one, or one where a folder is, or in a file.
    pub fn theme(
        &mut self,
        mut files: Vec<HookedFile>,
        mut check: impl FnMut(&str, &[HookedFile]) -> Result<()>,
    ) -> Result<Vec<HookedFile>> {
        for plugin in &mut self.plugins {
            if plugin.pre.exports.theme.is_none() {
                continue;
            }
            let name = plugin.name.clone();
            let (given, by): (Vec<_>, Vec<_>) = (files.into_iter())
                .map(
                    |HookedFile {
                         path,
                         contents,
                         plugin,
                     }| (wit::theme::File { path, contents }, plugin),
                )
                .unzip();
            let back = (plugin.call(&mut self.warnings, |store, hooks| {
                hook(&hooks.theme).call_theme(store, &given)
            }))
            .map_err(|reason| anyhow!("plugin `{name}`'s `theme` hook failed: {reason}"))?;
            let before: HashMap<&str, usize> = (given.iter().enumerate())
                .map(|(index, file)| (file.path.as_str(), index))
                .collect();
            let mut paths = BTreeSet::new();
            files = Vec::with_capacity(back.len());
            for wit::theme::File { path, contents } in back {
                let first_from = |from: &str| {
                    (paths.range::<str, _>(paths_from(from)).next()).map(String::as_str)
                };
                if let Some(why) = check_added(&path, "it gives back another there", first_from) {
                    bail!(
                        "plugin `{name}`'s `theme` hook gives back a file at `{path}`, but {why}"
                    );
                }
                paths.insert(path.clone());
                // A file it gives back as it was is still the one before's.
                let plugin = match before.get(path.as_str()) {
                    Some(&index) if given[index].contents == contents => by[index].clone(),
                    _ => Some(name.clone()),
                };
                files.push(HookedFile {
                    path,
                    contents,
                    plugin,
                });
            }
            files.sort_by(|a, b| a.path.cmp(&b.path));
            check(&name, &files)?;
        }
        Ok(files)
    }

    /// Change `files`, the source's files, by their paths, as each plugin's
    /// `files` hook says, in turn, making each it adds with `added`, from
    /// the plugin's name and the file's contents. Stops at the first that
    /// fails, or adds a file at a path that isn't relative, where one is,
    /// where a folder is, or in a file.
    pub fn files<T>(
        &mut self,
        files: &mut BTreeMap<String, T>,
        added: impl Fn(&str, Vec<u8>) -> T,
    ) -> Result<()> {
        for plugin in &mut self.plugins {
            if plugin.pre.exports.files.is_none() {
                continue;
            }
            let name = plugin.name.clone();
            let paths: Vec<String> = files.keys().cloned().collect();
            let wit::files::Changes { add, remove } = (plugin
                .call(&mut self.warnings, |store, hooks| {
                    hook(&hooks.files).call_files(store, &paths)
                }))
            .map_err(|reason| anyhow!("plugin `{name}`'s `files` hook failed: {reason}"))?;
            for path in &remove {
                files.remove(path);
            }
            let mut seen = HashSet::new();
            for wit::files::File { path, contents } in add {
                let there = if seen.insert(path.clone()) {
                    "there's one there, which it doesn't remove"
                } else {
                    "it adds another there"
                };
                let first_from = |from: &str| {
                    (files.range::<str, _>(paths_from(from)).next()).map(|(path, _)| path.as_str())
                };
                if let Some(why) = check_added(&path, there, first_from) {
                    bail!("plugin `{name}`'s `files` hook adds a file at `{path}`, but {why}");
                }
                files.insert(path, added(&name, contents));
            }
        }
        Ok(())
    }

    /// The `text` of `page`, its file from the site's folder, like
    /// `docs/guide.md`, as each plugin's `source` hook changes it, in turn.
    /// A hook that fails is warned of, naming `on`, and skipped.
    pub fn source(&mut self, page: &str, on: &str, text: String) -> String {
        self.text(
            "source",
            on,
            text,
            |exports| exports.source.is_some(),
            |store, hooks, text| hook(&hooks.source).call_source(store, page, text),
        )
    }

    /// The plugins, for the page built from `page`, its file from the site's
    /// folder, like `docs/guide.md`.
    pub fn on<'p>(&'p mut self, page: &'p str) -> PagePlugins<'p> {
        PagePlugins {
            plugins: Some(self),
            page,
        }
    }

    /// Whether a plugin's `formats` hook reads files with `extension`, like
    /// `ipynb`.
    pub fn reads(&self, extension: &str) -> bool {
        self.extensions.contains_key(extension)
    }

    /// The parts of `page`, its file from the site's folder, like
    /// `docs/analysis.ipynb`, as the `formats` hook of the plugin that reads
    /// its extension reads them from its `text`, with the site's `markdown`
    /// settings. Fails if no plugin reads it, the hook fails, or it gives
    /// back HTML, but the settings don't let raw HTML in.
    pub fn read(&mut self, page: &str, text: &str, markdown: &Mapping) -> Result<Vec<Part>> {
        let extension = Path::new(page).extension().and_then(|e| e.to_str());
        let Some(&index) = extension.and_then(|extension| self.extensions.get(extension)) else {
            bail!("no plugin reads {page}");
        };
        let plugin = &mut self.plugins[index];
        let name = plugin.name.clone();
        let parts = (to_json(markdown, "the `markdown` settings"))
            .and_then(|json| {
                plugin.call(&mut self.warnings, |store, hooks| {
                    hook(&hooks.formats).call_read(store, page, text, &json)
                })
            })
            .map_err(|reason| anyhow!("plugin `{name}`'s `formats` hook failed: {reason}"))?;
        let raw_html = markdown
            .get("raw_html")
            .and_then(serde_yaml::Value::as_bool);
        if raw_html != Some(true) && parts.iter().any(|part| matches!(part, Part::Html(_))) {
            bail!("plugin `{name}` gave back HTML, which needs `markdown.raw_html` to be `true`");
        }
        Ok(parts)
    }

    /// The `html` of `page`, as for [`Plugins::source`], as each plugin's
    /// `content` hook changes it, in turn.
    pub fn content(&mut self, page: &str, on: &str, html: String) -> String {
        self.text(
            "content",
            on,
            html,
            |exports| exports.content.is_some(),
            |store, hooks, html| hook(&hooks.content).call_content(store, page, html),
        )
    }

    /// `site`, as each plugin's `site` hook changes it, in turn, each
    /// plugin's checked with `check`, which is given its name. Stops at the
    /// first that fails.
    pub fn site(
        &mut self,
        mut site: Mapping,
        mut check: impl FnMut(&str, &Mapping) -> Result<()>,
    ) -> Result<Mapping> {
        for plugin in &mut self.plugins {
            if plugin.pre.exports.site.is_none() {
                continue;
            }
            let name = plugin.name.clone();
            site = (to_json(&site, "`site`"))
                .and_then(|json| {
                    plugin.call(&mut self.warnings, |store, hooks| {
                        hook(&hooks.site).call_site(store, &json)
                    })
                })
                .and_then(|json| from_json(&json, "a `site`"))
                .map_err(|reason| anyhow!("plugin `{name}`'s `site` hook failed: {reason}"))?;
            check(&name, &site)?;
        }
        Ok(site)
    }

    /// `page`, as each plugin's `page` hook changes it, in turn. A hook that
    /// fails is warned of, naming `on`, what the page is built from, and
    /// skipped.
    pub fn page(&mut self, on: &str, mut page: Mapping) -> Mapping {
        for plugin in &mut self.plugins {
            if plugin.pre.exports.page.is_none() {
                continue;
            }
            let changed = (to_json(&page, "the `page`"))
                .and_then(|json| {
                    plugin.call(&mut self.warnings, |store, hooks| {
                        hook(&hooks.page).call_page(store, &json)
                    })
                })
                .and_then(|json| from_json(&json, "a `page`"));
            match changed {
                Ok(changed) => page = changed,
                Err(reason) => {
                    (self.warnings).push(Warning::failed(&plugin.name, "page", on, reason))
                }
            }
        }
        page
    }

    /// The `html` the theme writes for the page at `url`, as a reader
    /// follows it, as each plugin's `html` hook changes it, in turn. A hook
    /// that fails is warned of, naming `on`, what the page is built from,
    /// and skipped.
    pub fn html(&mut self, url: &str, on: &str, html: String) -> String {
        self.text(
            "html",
            on,
            html,
            |exports| exports.html.is_some(),
            |store, hooks, html| hook(&hooks.html).call_html(store, url, html),
        )
    }

    /// The files each plugin's `destination` hook adds to those at `paths`,
    /// from a version's folder in the destination, in turn. Stops at the
    /// first that fails, or adds a file at a path that isn't relative,
    /// where one is, where a folder is, or in a file.
    pub fn destination(&mut self, mut paths: BTreeSet<String>) -> Result<Vec<HookedFile>> {
        let mut added = Vec::new();
        for plugin in &mut self.plugins {
            if plugin.pre.exports.destination.is_none() {
                continue;
            }
            let name = plugin.name.clone();
            let given: Vec<String> = paths.iter().cloned().collect();
            let files = (plugin.call(&mut self.warnings, |store, hooks| {
                hook(&hooks.destination).call_destination(store, &given)
            }))
            .map_err(|reason| anyhow!("plugin `{name}`'s `destination` hook failed: {reason}"))?;
            for wit::destination::File { path, contents } in files {
                let first_from = |from: &str| {
                    (paths.range::<str, _>(paths_from(from)).next()).map(String::as_str)
                };
                if let Some(why) = check_added(&path, "there's one there", first_from) {
                    bail!(
                        "plugin `{name}`'s `destination` hook adds a file at `{path}`, but {why}"
                    );
                }
                paths.insert(path.clone());
                added.push(HookedFile {
                    path,
                    contents,
                    plugin: Some(name.clone()),
                });
            }
        }
        Ok(added)
    }

    /// `text`, as the hook called `name` of each plugin that `exports` it
    /// changes it, in turn, with `call`. A hook that fails is warned of,
    /// naming `on`, and skipped.
    fn text(
        &mut self,
        name: &'static str,
        on: &str,
        mut text: String,
        exports: impl Fn(&Exports) -> bool,
        call: impl Fn(&mut Store<State>, &Hooks, &str) -> wasmtime::Result<Result<String, String>>,
    ) -> String {
        for plugin in &mut self.plugins {
            if !exports(&plugin.pre.exports) {
                continue;
            }
            match plugin.call(&mut self.warnings, |store, hooks| call(store, hooks, &text)) {
                Ok(changed) => text = changed,
                Err(reason) => {
                    (self.warnings).push(Warning::failed(&plugin.name, name, on, reason))
                }
            }
        }
        text
    }
}

/// `value` as JSON, for a hook, or why it can't be: it's `what`, like `the
/// config`.
fn to_json(value: &Mapping, what: &str) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| format!("little can't write {what} as JSON: {e}"))
}

/// The JSON object a hook gave back, or why it isn't one: it should be
/// `what`, like `a config`.
fn from_json(json: &str, what: &str) -> Result<Mapping, String> {
    serde_json::from_str(json)
        .map_err(|e| format!("it gave back {what} that isn't a JSON object: {e}"))
}

/// Why a file a hook adds can't be at `path`, if it can't: it's relative,
/// with `/` between its parts, so it can't be outside its folder, on any
/// system, as one with a drive, like `C:/`, would be on Windows.
fn bad_path(path: &str) -> Option<&'static str> {
    if path.starts_with('/') {
        Some("a file's path is relative, without a `/` before it")
    } else if (path.split('/')).any(|part| matches!(part, "" | "." | "..")) {
        Some("a file's path has `/` between its parts, and no part is empty, `.`, or `..`")
    } else if path.contains(['\\', ':']) {
        Some("a file's path has no `\\` or `:`, which mean something else on Windows")
    } else {
        None
    }
}

/// Why a hook can't add a file at `path`, beside the files `first_from`
/// finds, if it can't: its path is bad, as [`bad_path`] says, or there's
/// one there, which is `there`, or a disk couldn't have both, as one is
/// where a folder it's in would be, or one is in the folder that would be
/// where it is. `first_from` gives the first of their paths, in order, from
/// the one it's given.
fn check_added<'p>(
    path: &str,
    there: &str,
    first_from: impl Fn(&str) -> Option<&'p str>,
) -> Option<String> {
    if let Some(why) = bad_path(path) {
        return Some(why.to_string());
    }
    if first_from(path) == Some(path) {
        return Some(there.to_string());
    }
    for (end, _) in path.match_indices('/') {
        let folder = &path[..end];
        if first_from(folder) == Some(folder) {
            return Some(format!(
                "there's a file at `{folder}`, where its folder would be"
            ));
        }
    }
    // The paths in a folder sort together, just after it, with its `/`.
    let folder = format!("{path}/");
    (first_from(&folder))
        .filter(|file| file.starts_with(&folder))
        .map(|file| format!("there's a folder there, with `{file}` in it"))
}

/// The bounds of the paths, in order, from `from`.
fn paths_from(from: &str) -> (Bound<&str>, Bound<&str>) {
    (Bound::Included(from), Bound::Unbounded)
}

/// The plugins a build has started, for one page.
pub struct PagePlugins<'p> {
    plugins: Option<&'p mut Plugins>,
    page: &'p str,
}

impl PagePlugins<'_> {
    /// No plugins at all.
    #[cfg(test)]
    pub fn none() -> PagePlugins<'static> {
        PagePlugins {
            plugins: None,
            page: "",
        }
    }

    /// Whether a file with `extension`, like `md`, is a page: a Markdown
    /// file, or a file a plugin's `formats` hook reads.
    pub fn is_page(&self, extension: &str) -> bool {
        extension == "md"
            || (self.plugins.as_deref()).is_some_and(|plugins| plugins.reads(extension))
    }

    /// Give each fenced code block in `blocks` whose language is a plugin's
    /// to that plugin, in turn, and put what it gives back in the block's
    /// place: Markdown, read with `options` into `arena`, or HTML. A block's
    /// line in its file is `lines_before` after its line in the document,
    /// or `None` for one in a part a `formats` hook gives back. Returns each
    /// block a plugin couldn't render, which is left as it was.
    pub fn render<'a>(
        &mut self,
        arena: &'a Arena<'a>,
        blocks: impl IntoIterator<Item = &'a AstNode<'a>>,
        options: &Options,
        lines_before: Option<usize>,
    ) -> Result<Vec<Unrendered>> {
        let Some(Plugins {
            plugins,
            languages,
            warnings,
            ..
        }) = self.plugins.as_deref_mut()
        else {
            return Ok(Vec::new());
        };
        // Each found first, so the Markdown a plugin gives back isn't given
        // to plugins again.
        let claimed: Vec<_> = (blocks.into_iter())
            .filter_map(|node| {
                let NodeValue::CodeBlock(block) = &node.data.borrow().value else {
                    return None;
                };
                let language = language(&block.info);
                let plugin = *languages.get(&language).filter(|_| block.fenced)?;
                let block = Block {
                    language,
                    info: block.info.clone(),
                    text: block.literal.clone(),
                    page: self.page.to_string(),
                };
                Some((node, plugin, block))
            })
            .collect();
        if claimed.is_empty() {
            return Ok(Vec::new());
        }

        // A plugin's footnotes would be numbered, and listed, apart from the
        // page's, as they're read on their own.
        let mut options = options.clone();
        options.extension.footnotes = false;
        options.extension.inline_footnotes = false;
        let mut unrendered = Vec::new();
        for (node, plugin, block) in claimed {
            let plugin = &mut plugins[plugin];
            let sourcepos = node.data.borrow().sourcepos;
            let line = lines_before.map(|before| before + sourcepos.start.line);
            let rendered = plugin.call(warnings, |store, hooks| {
                hook(&hooks.code_blocks).call_render(store, &block)
            });
            match rendered {
                Ok(Output::Markdown(markdown)) => {
                    let document = comrak::parse_document(arena, &markdown, &options);
                    // A warning for what's in it names the block.
                    for descendant in document.descendants() {
                        descendant.data.borrow_mut().sourcepos = sourcepos;
                    }
                    for child in document.children().collect::<Vec<_>>() {
                        node.insert_before(child);
                    }
                    node.detach();
                }
                Ok(Output::Html(html)) => {
                    if !options.render.r#unsafe {
                        let block = match line {
                            Some(line) => format!("the block on line {line}"),
                            None => "a block".to_string(),
                        };
                        bail!(
                            "plugin `{}` gave back HTML for {block}, which needs \
                             `markdown.raw_html` to be `true`",
                            plugin.name
                        );
                    }
                    node.data.borrow_mut().value = NodeValue::Raw(format!("{}\n", html.trim_end()));
                }
                Err(reason) => unrendered.push(Unrendered {
                    plugin: plugin.name.clone(),
                    line,
                    reason,
                }),
            }
        }
        Ok(unrendered)
    }
}

impl Running {
    /// Start the plugin afresh, in a sandbox of its own, and set it up, or
    /// say why it can't.
    fn start(&mut self) -> Result<(), String> {
        self.instance = None;
        let mut wasi = WasiCtxBuilder::new();
        // Before the folders are opened, which keep it.
        wasi.allow_blocking_current_thread(true)
            .stdout(std::io::stderr())
            .inherit_stderr()
            .allow_tcp(false)
            .allow_udp(false)
            .allow_ip_name_lookup(false)
            .wall_clock(BuildTime(self.now))
            .secure_random(StdRng::seed_from_u64(1))
            .insecure_random(StdRng::seed_from_u64(2))
            .insecure_random_seed(3);
        for read in &self.read {
            wasi.preopened_dir(&read.folder, &read.path, FsPerms::ReadOnly)
                .map_err(|e| format!("little couldn't open {}: {e}", read.folder.display()))?;
        }
        let state = State {
            ctx: wasi.build(),
            table: ResourceTable::new(),
            warnings: Vec::new(),
        };
        let mut store = Store::new(&ENGINE, state);
        let instance = (self.pre.instance.instantiate(&mut store)).map_err(|e| crashed(&e))?;
        let hooks = self.pre.exports.load(&mut store, &instance)?;
        if let Some(setup) = &hooks.setup {
            (setup.call_setup(&mut store, &self.settings)).map_err(|e| crashed(&e))??;
        }
        self.instance = Some(Instance { store, hooks });
        Ok(())
    }

    /// Call one of the plugin's hooks, with `call`, started again first if
    /// it crashed, and add what it warns of to `warnings`. Returns what the
    /// hook gives back, or why it can't.
    fn call<R>(
        &mut self,
        warnings: &mut Vec<Warning>,
        call: impl FnOnce(&mut Store<State>, &Hooks) -> wasmtime::Result<Result<R, String>>,
    ) -> Result<R, String> {
        if self.instance.is_none() {
            (self.start()).map_err(|reason| format!("it can't start again: {reason}"))?;
        }
        let Instance { store, hooks } = self.instance.as_mut().expect("the plugin started");
        let called = call(store, hooks);
        // What it warned of before it crashed, too.
        self.drain(warnings);
        called.unwrap_or_else(|e| {
            self.instance = None;
            Err(crashed(&e))
        })
    }

    /// Add what the plugin's warned of since this was last called to
    /// `warnings`.
    fn drain(&mut self, warnings: &mut Vec<Warning>) {
        let Some(Instance { store, .. }) = &mut self.instance else {
            return;
        };
        let messages = store.data_mut().warnings.drain(..);
        warnings.extend(messages.map(|message| Warning {
            plugin: self.name.clone(),
            warned: Warned::Message(message),
        }));
    }
}

/// The plugin in `file`, compiled, and linked to WASI, ready to start.
fn compile(file: &Path) -> Result<Pre> {
    let metadata = std::fs::metadata(file)?;
    let stamp = (metadata.modified().ok(), metadata.len());
    // Held while compiling, so a plugin is compiled once even if two builds
    // need it at once.
    let mut compiled = COMPILED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((when, pre)) = compiled.get(file)
        && *when == stamp
    {
        return Ok(pre.clone());
    }
    let wasm = std::fs::read(file)?;
    if wasm.starts_with(b"\0asm\x01\0\0\0") {
        bail!(
            "it's a WebAssembly module, but a plugin is a component, as Rust's `wasm32-wasip2` \
             target builds"
        );
    }
    if !wasm.starts_with(b"\0asm") {
        bail!("it isn't a WebAssembly file");
    }
    let component = Component::new(&ENGINE, &wasm)?;
    let mut linker = Linker::new(&ENGINE);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
    bindings::little::plugin::warnings::add_to_linker::<_, HasSelf<State>>(&mut linker, |state| {
        state
    })?;
    let instance = linker.instantiate_pre(&component)?;
    let exports = Exports::of(&instance)?;
    if !exports.any() {
        let ty = component.component_type();
        let names: Vec<_> = (ty.exports(&ENGINE))
            .map(|(name, _)| format!("`{name}`"))
            .collect();
        match names.as_slice() {
            [] => {
                bail!("it exports nothing, but a plugin exports little's hooks, from `{PACKAGE}`")
            }
            names => bail!(
                "it exports {}, but none of little's hooks, from `{PACKAGE}`",
                list(names)
            ),
        }
    }
    let pre = Pre { instance, exports };
    compiled.insert(file.to_path_buf(), (stamp, pre.clone()));
    Ok(pre)
}

/// Why a plugin crashed, from wasmtime, on one line, like `it crashed: wasm
/// trap: wasm `unreachable` instruction executed`, which a Rust plugin does
/// when it panics.
fn crashed(error: &wasmtime::Error) -> String {
    format!("it crashed: {}", one_line(&error.root_cause().to_string()))
}

/// `text`, with each run of whitespace in it, like a line break, a space.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `items`, in a list, like `a`, `a and b`, or `a, b, and c`.
fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [item] => item.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// A block's language: the first word of its info string, up to a comma, in
/// lowercase, as for highlighting, and as Mermaid's is found.
pub(crate) fn language(info: &str) -> String {
    let word = info.split_ascii_whitespace().next().unwrap_or_default();
    word.split(',').next().unwrap_or_default().to_lowercase()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::config::{Markdown, SiteUrl};
    use crate::parts::parse_parts;
    use crate::render::{Rendered, parse_document};
    use serde_json::json;
    use std::process::Command;
    use std::slice;
    use std::sync::OnceLock;

    /// When each test's site is built: the start of 2026.
    const NOW: i64 = 1_767_225_600;

    /// The plugin in `tests/plugin`, which exports every hook, built once,
    /// in `target/test-plugin`, for every test that needs it.
    pub(crate) fn test_plugin() -> &'static Path {
        static BUILT: OnceLock<PathBuf> = OnceLock::new();
        BUILT.get_or_init(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"));
            let target = root.join("target/test-plugin");
            let status = Command::new(env!("CARGO"))
                .args(["build", "--quiet", "--release", "--target", "wasm32-wasip2"])
                .arg("--manifest-path")
                .arg(root.join("tests/plugin/Cargo.toml"))
                .env("CARGO_TARGET_DIR", &target)
                .status()
                .expect("cargo runs");
            assert!(
                status.success(),
                "the test plugin doesn't build: is Rust's wasm32-wasip2 target installed? \
                 `rustup target add wasm32-wasip2` installs it"
            );
            target.join("wasm32-wasip2/release/little_test_plugin.wasm")
        })
    }

    /// The test plugin, as a site's config names it, with `settings`, as
    /// JSON, reading `read`.
    fn named(name: &str, settings: &str, read: &[config::Read]) -> config::Plugin {
        config::Plugin {
            name: name.to_string(),
            wasm: Wasm::File(test_plugin().to_path_buf()),
            read: read.to_vec(),
            settings: settings.to_string(),
        }
    }

    /// Start `plugins`, as a site's build would.
    fn start(plugins: &[config::Plugin]) -> Result<Plugins> {
        let now = OffsetDateTime::from_unix_timestamp(NOW).unwrap();
        Plugins::start(plugins, Path::new("."), now)
    }

    /// The test plugin, started with `settings`.
    fn started(settings: &str) -> Plugins {
        start(&[named("test", settings, &[])]).unwrap()
    }

    /// What `plugins` have warned of, as little shows it.
    fn warnings(plugins: &mut Plugins) -> Vec<String> {
        (plugins.take_warnings().iter())
            .map(Warning::to_string)
            .collect()
    }

    /// A component that exports an instance with nothing in it as each of
    /// `names`, as one built for another version of little's hooks might
    /// export them.
    fn exporting(names: &[&str]) -> Vec<u8> {
        let mut wasm = b"\0asm\x0d\0\x01\0".to_vec();
        let mut section = |id: u8, contents: Vec<u8>| {
            wasm.push(id);
            wasm.push(u8::try_from(contents.len()).unwrap());
            wasm.extend(contents);
        };
        let mut instances = vec![u8::try_from(names.len()).unwrap()];
        for _ in names {
            instances.extend([0x01, 0x00]);
        }
        section(0x05, instances);
        let mut exports = vec![u8::try_from(names.len()).unwrap()];
        for (index, name) in names.iter().enumerate() {
            exports.extend([0x00, u8::try_from(name.len()).unwrap()]);
            exports.extend(name.as_bytes());
            exports.extend([0x05, u8::try_from(index).unwrap(), 0x00]);
        }
        section(0x0b, exports);
        wasm
    }

    /// Render `md` as `docs/test.md`, with `markdown`, and `plugins`.
    fn render_with(md: &str, markdown: &Markdown, plugins: &mut Plugins) -> Result<Rendered> {
        let now = OffsetDateTime::from_unix_timestamp(NOW).unwrap();
        let url = SiteUrl::default();
        let mut plugins = plugins.on("docs/test.md");
        parse_document(md, "/", &(2..=6), &url, markdown, now, &mut plugins)
    }

    /// Render `md` with little's defaults, but highlighting, so the HTML is
    /// plain to compare, and `plugins`.
    fn render(md: &str, plugins: &mut Plugins) -> Rendered {
        let markdown = Markdown {
            highlight: None,
            ..Markdown::default()
        };
        render_with(md, &markdown, plugins).unwrap()
    }

    /// A fresh scratch folder holding `files`, each a (path, contents) pair.
    fn scratch(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("little-plugin-test-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for (file, contents) in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        root
    }

    #[test]
    fn a_plugin_renders_its_blocks_as_markdown_or_html() {
        let mut plugins = started("{}");
        let md = "# Title\n\n```test\nmarkdown\n## From the plugin\n\nSee [the guide](guide.md).\n\
                  ```\n\n```TEST {.wide}\nhtml\n<div class=\"wide\">Hi</div>\n```\n\n\
                  ```python\nprint(1)\n```\n";
        let r = render(md, &mut plugins);

        // Its Markdown is written as the page's is: its headings join the
        // page's table of contents, and its links are led within the site.
        let toc: Vec<_> = r.toc.iter().map(|item| item.anchor.as_str()).collect();
        assert_eq!(toc, ["#from-the-plugin"]);
        assert!(
            r.html
                .contains("<h2 id=\"from-the-plugin\">From the plugin"),
            "{}",
            r.html
        );
        assert!(
            r.html.contains(r#"<a href="/guide">the guide</a>"#),
            "{}",
            r.html
        );
        // Its HTML is written as it is, and other blocks are left alone.
        assert!(
            r.html.contains("<div class=\"wide\">Hi</div>\n"),
            "{}",
            r.html
        );
        assert!(
            r.html.contains("<code class=\"language-python\">print(1)"),
            "{}",
            r.html
        );
        assert!(!r.html.contains("markdown\n"), "{}", r.html);
        assert_eq!(r.unrendered_blocks, []);
    }

    #[test]
    fn a_plugins_html_needs_raw_html() {
        let mut plugins = started("{}");
        let markdown = Markdown {
            raw_html: false,
            mermaid: None,
            ..Markdown::default()
        };
        let md = "---\ntitle: Hi\n---\n```test\nhtml\n<div>Hi</div>\n```\n";
        let message = format!(
            "{:#}",
            render_with(md, &markdown, &mut plugins).err().unwrap()
        );
        assert_eq!(
            message,
            "plugin `test` gave back HTML for the block on line 4, which needs \
             `markdown.raw_html` to be `true`"
        );
    }

    #[test]
    fn a_plugins_markdown_is_drawn_but_not_given_to_plugins() {
        let mut plugins = started("{}");
        let md = "```test\nmarkdown\n~~~mermaid\nflowchart LR\n  A --> B\n~~~\n\n~~~test\ncount\n~~~\n```\n";
        let r = render(md, &mut plugins);
        assert!(r.html.starts_with("<svg id=\"mermaid-1\" "), "{}", r.html);
        assert!(
            r.html
                .contains("<pre><code class=\"language-test\">count\n</code></pre>"),
            "{}",
            r.html
        );
    }

    #[test]
    fn a_block_a_plugin_cant_render_is_left_as_it_was() {
        let mut plugins = started("{}");
        let md = "---\ntitle: Hi\n---\n```test\ncount\n```\n\n```test\nerror it is broken\n```\n\n\
                  ```test\npanic Oh no\n```\n\n```test\ncount\n```\n\n```test\nfly\n```\n";
        let r = render(md, &mut plugins);
        let unrendered = |line, reason: &str| Unrendered {
            plugin: "test".to_string(),
            line: Some(line),
            reason: reason.to_string(),
        };
        assert_eq!(
            r.unrendered_blocks,
            [
                unrendered(8, "it is broken"),
                unrendered(
                    12,
                    "it crashed: wasm trap: wasm `unreachable` instruction executed"
                ),
                unrendered(20, "there's no command `fly`"),
            ]
        );
        assert!(
            r.html.contains("error it is broken\n</code></pre>"),
            "{}",
            r.html
        );
        // A plugin that crashes starts afresh for the next block.
        assert_eq!(r.html.matches("<p>1</p>").count(), 2, "{}", r.html);
    }

    #[test]
    fn a_plugin_is_told_its_settings_and_each_block() {
        let settings = r#"{"languages":["test","Other"],"style":"fancy"}"#;
        let mut plugins = started(settings);
        let md = "```test\nsettings\n```\n\n```OTHER {width=10}\nblock\n```\n\n\
                  ```other,linenos\nblock\n```\n";
        let r = render(md, &mut plugins);
        let paragraphs: Vec<_> = (r.html.lines())
            .map(|line| line.replace("&quot;", "\""))
            .collect();
        assert_eq!(
            paragraphs,
            [
                format!("<p>{settings}</p>"),
                "<p>other | OTHER {width=10} | docs/test.md</p>".to_string(),
                "<p>other | other,linenos | docs/test.md</p>".to_string(),
            ]
        );
    }

    #[test]
    fn a_plugin_reaches_nothing_little_doesnt_give_it() {
        let mut plugins = started("{}");
        let md = "```test\nenv\n```\n\n```test\ntime\n```\n\n```test\nread Cargo.toml\n```\n\n\
                  ```test\nwrite written.txt\n```\n\n```test\nprint Hello\n```\n";
        let r = render(md, &mut plugins);
        // No environment variables or arguments, and a clock stopped at the
        // time the build started.
        assert_eq!(
            r.html,
            format!("<p>0 variables and 0 arguments</p>\n<p>{NOW}</p>\n")
                + "<pre><code class=\"language-test\">read Cargo.toml\n</code></pre>\n"
                + "<pre><code class=\"language-test\">write written.txt\n</code></pre>\n"
        );
        // No files, and what it prints goes to little's stderr.
        let reasons: Vec<_> = (r.unrendered_blocks.iter())
            .map(|block| block.reason.as_str())
            .collect();
        assert_eq!(reasons.len(), 2, "{reasons:?}");
        assert!(!Path::new("written.txt").exists());

        // Its random numbers are the same in each build.
        let random = |plugins: &mut Plugins| render("```test\nrandom\n```\n", plugins).html;
        assert_eq!(random(&mut plugins), random(&mut started("{}")));
    }

    #[test]
    fn a_plugin_reads_only_the_folders_the_site_lets_it() {
        let site = scratch(
            "read",
            &[
                ("site/data/hello.txt", "Hello from data"),
                ("site/secret.txt", "Secret"),
                ("shared/hello.txt", "Hello from shared"),
            ],
        );
        let read = |folder: &str, path: &str| config::Read {
            folder: site.join(folder),
            path: path.to_string(),
        };
        let read = [read("site/data", "data"), read("shared", "../shared")];
        let mut plugins = start(&[named("test", "{}", &read)]).unwrap();
        let md = "```test\nread data/hello.txt\n```\n\n```test\nread ../shared/hello.txt\n```\n\n\
                  ```test\nread data/../secret.txt\n```\n\n```test\nread secret.txt\n```\n\n\
                  ```test\nwrite data/hello.txt\n```\n";
        let r = render(md, &mut plugins);
        assert!(
            r.html
                .starts_with("<p>Hello from data</p>\n<p>Hello from shared</p>\n<pre>"),
            "{}",
            r.html
        );
        let lines: Vec<_> = (r.unrendered_blocks.iter())
            .map(|block| block.line)
            .collect();
        assert_eq!(
            lines,
            [Some(9), Some(13), Some(17)],
            "{:?}",
            r.unrendered_blocks
        );
        let hello = std::fs::read_to_string(site.join("site/data/hello.txt")).unwrap();
        assert_eq!(hello, "Hello from data");
        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn a_plugin_that_cant_start_stops_the_build() {
        let error = |plugins: &[config::Plugin]| format!("{:#}", start(plugins).err().unwrap());
        assert_eq!(
            error(&[named("test", r#"{"fail":"it needs a style"}"#, &[])]),
            "plugin `test` can't start: it needs a style"
        );
        assert_eq!(
            error(&[named("test", r#"{"panic":true}"#, &[])]),
            "plugin `test` can't start: it crashed: wasm trap: wasm `unreachable` instruction \
             executed"
        );
        for language in ["", "two words", "test,python"] {
            let settings = serde_json::json!({ "languages": [language] }).to_string();
            assert_eq!(
                error(&[named("test", &settings, &[])]),
                format!(
                    "plugin `test` renders `{language}`, but a language must be one word, like \
                     `bob`"
                )
            );
        }
        // A language is a plugin's alone, but it can say it twice.
        let twice = named("first", r#"{"languages":["test","TEST"]}"#, &[]);
        assert!(start(slice::from_ref(&twice)).is_ok());
        assert_eq!(
            error(&[twice, named("second", r#"{"languages":["Test"]}"#, &[])]),
            "plugins `first` and `second` both render `test`"
        );
        for extension in ["", ".ipynb", "two words", "notebooks/ipynb", "tar.gz"] {
            let settings = json!({ "extensions": [extension] }).to_string();
            assert_eq!(
                error(&[named("test", &settings, &[])]),
                format!(
                    "plugin `test` reads `{extension}`, but an extension must be one word, \
                     without its `.`, like `ipynb`"
                )
            );
        }
        assert_eq!(
            error(&[named("test", r#"{"extensions":["md"]}"#, &[])]),
            "plugin `test` reads `md`, but little reads Markdown itself"
        );
        // An extension is a plugin's alone, as it's written, but it can say
        // it twice.
        let reads = |name, extensions: &[&str]| {
            let settings = json!({ "languages": [], "extensions": extensions });
            named(name, &settings.to_string(), &[])
        };
        let twice = reads("first", &["test", "test"]);
        assert!(start(&[twice.clone(), reads("second", &["TEST"])]).is_ok());
        assert_eq!(
            error(&[twice, reads("second", &["test"])]),
            "plugins `first` and `second` both read `test`"
        );
    }

    #[test]
    fn a_plugin_reads_the_files_of_its_extensions() {
        let settings = json!({
            "extensions": ["test"],
            "formats": { "only": "docs/bad.test", "error": "can't read {arg}" },
        });
        let mut plugins = started(&settings.to_string());
        assert!(plugins.reads("test"));
        assert!(!plugins.reads("TEST"));
        assert!(!plugins.reads("md"));
        assert!(plugins.on("docs/a.md").is_page("test"));
        assert!(!plugins.on("docs/a.md").is_page("txt"));
        let markdown: Mapping = serde_yaml::from_str("raw_html: true\nautolink: true").unwrap();
        let text = json!([
            { "markdown": "# A" },
            { "html": "<p>B</p>" },
            { "code": { "language": "python", "text": "c" } },
            "given",
        ]);
        let parts = plugins
            .read("docs/a.test", &text.to_string(), &markdown)
            .unwrap();
        assert_eq!(
            parts,
            [
                Part::Markdown("# A".to_string()),
                Part::Html("<p>B</p>".to_string()),
                Part::Code(Code {
                    language: "python".to_string(),
                    text: "c".to_string(),
                }),
            ]
        );
        // It's given the page's file, and the `markdown` settings, as JSON.
        assert_eq!(
            warnings(&mut plugins),
            [r#"plugin `test`: docs/a.test {"raw_html":true,"autolink":true}"#]
        );

        let error = |plugins: &mut Plugins, page: &str, text: &str, markdown: &Mapping| {
            format!("{:#}", plugins.read(page, text, markdown).unwrap_err())
        };
        assert_eq!(
            error(&mut plugins, "docs/bad.test", "[]", &markdown),
            "plugin `test`'s `formats` hook failed: can't read docs/bad.test"
        );
        assert_eq!(
            error(&mut plugins, "docs/a.test", "[", &markdown),
            "plugin `test`'s `formats` hook failed: EOF while parsing a list at line 1 column 1"
        );
        assert_eq!(
            error(&mut plugins, "docs/a.txt", "[]", &markdown),
            "no plugin reads docs/a.txt"
        );
        let html = json!([{ "html": "<p>B</p>" }]).to_string();
        let no_html: Mapping = serde_yaml::from_str("raw_html: false").unwrap();
        assert_eq!(
            error(&mut plugins, "docs/a.test", &html, &no_html),
            "plugin `test` gave back HTML, which needs `markdown.raw_html` to be `true`"
        );
        // Without HTML, it needs nothing.
        let text = json!([{ "markdown": "A" }]).to_string();
        assert!(plugins.read("docs/a.test", &text, &no_html).is_ok());

        let settings = json!({ "extensions": ["test"], "formats": { "panic": "{arg}" } });
        let mut plugins = started(&settings.to_string());
        let message = error(&mut plugins, "docs/a.test", "[]", &markdown);
        assert!(
            message.starts_with("plugin `test`'s `formats` hook failed: it crashed: "),
            "{message}"
        );
    }

    #[test]
    fn markdown_parts_are_given_to_plugins_but_not_code_parts() {
        let mut plugins = started("{}");
        let markdown = |text: &str| Part::Markdown(text.to_string());
        let parts = vec![
            markdown("```test\ncount\n```"),
            Part::Code(Code {
                language: "test".to_string(),
                text: "count".to_string(),
            }),
            markdown("```test\nerror in a part\n```"),
        ];
        let options = Markdown {
            highlight: None,
            ..Markdown::default()
        };
        let now = OffsetDateTime::from_unix_timestamp(NOW).unwrap();
        let r = parse_parts(
            parts,
            "/",
            &(2..=6),
            &SiteUrl::default(),
            &options,
            now,
            &mut plugins.on("docs/analysis.test"),
        )
        .unwrap();
        assert_eq!(
            r.html,
            "<p>1</p>\n<pre><code class=\"language-test\">count\n</code></pre>\n\
             <pre><code class=\"language-test\">error in a part\n</code></pre>\n"
        );
        // A block's line would be its part's.
        assert_eq!(
            r.unrendered_blocks,
            [Unrendered {
                plugin: "test".to_string(),
                line: None,
                reason: "in a part".to_string(),
            }]
        );
    }

    #[test]
    fn a_plugin_must_be_a_component() {
        let site = scratch(
            "files",
            &[
                ("module.wasm", "\0asm\x01\0\0\0"),
                ("text.wasm", "Hello"),
                ("empty.wasm", "\0asm\x0d\0\x01\0"),
            ],
        );
        let plugin = |file: &str, read: &[config::Read]| config::Plugin {
            wasm: Wasm::File(site.join(file)),
            ..named("test", "{}", read)
        };
        let error = |plugin| format!("{:#}", start(&[plugin]).err().unwrap());
        let file = |file: &str| site.join(file).display().to_string();
        assert_eq!(
            error(plugin("missing.wasm", &[])),
            format!(
                "plugin `test`'s `wasm` is {}, but there's no file there",
                file("missing.wasm")
            )
        );
        assert_eq!(
            error(plugin("module.wasm", &[])),
            format!(
                "loading plugin `test` from {}: it's a WebAssembly module, but a plugin is a \
                 component, as Rust's `wasm32-wasip2` target builds",
                file("module.wasm")
            )
        );
        assert_eq!(
            error(plugin("text.wasm", &[])),
            format!(
                "loading plugin `test` from {}: it isn't a WebAssembly file",
                file("text.wasm")
            )
        );
        assert_eq!(
            error(plugin("empty.wasm", &[])),
            format!(
                "loading plugin `test` from {}: it exports nothing, but a plugin exports \
                 little's hooks, from `little:plugin@0.2.1`",
                file("empty.wasm")
            )
        );
        // A plugin built for another version of little's hooks.
        std::fs::write(
            site.join("old.wasm"),
            exporting(&[
                "little:plugin/code-blocks@0.1.0",
                "little:plugin/pages@0.1.0",
            ]),
        )
        .unwrap();
        assert_eq!(
            error(plugin("old.wasm", &[])),
            format!(
                "loading plugin `test` from {}: it exports `little:plugin/code-blocks@0.1.0` \
                 and `little:plugin/pages@0.1.0`, but none of little's hooks, from \
                 `little:plugin@0.2.1`",
                file("old.wasm")
            )
        );
        // One built for 0.2.0 is read as 0.2.1's, which only adds to it.
        std::fs::write(
            site.join("wrong.wasm"),
            exporting(&["little:plugin/source@0.2.0"]),
        )
        .unwrap();
        let message = error(plugin("wrong.wasm", &[]));
        let expected = format!(
            "loading plugin `test` from {}: its `source` hook isn't little:plugin@0.2.1's: ",
            file("wrong.wasm")
        );
        assert!(message.starts_with(&expected), "{message}");
        let missing = config::Read {
            folder: site.join("data"),
            path: "data".to_string(),
        };
        assert_eq!(
            error(named("test", "{}", &[missing])),
            format!(
                "plugin `test` reads {}, but there's no folder there",
                file("data")
            )
        );
        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn a_plugin_warns_from_any_hook() {
        let settings = json!({ "warn": "set up", "source": { "warn": "reading {arg}" } });
        let mut plugins = started(&settings.to_string());
        let text = plugins.source("docs/a.md", "docs/a.md", "# A\n".to_string());
        assert_eq!(text, "# A\n");
        assert_eq!(
            warnings(&mut plugins),
            ["plugin `test`: set up", "plugin `test`: reading docs/a.md"]
        );
        assert_eq!(warnings(&mut plugins), [] as [String; 0]);
    }

    #[test]
    fn a_hook_that_fails_is_warned_of_and_skipped() {
        let settings = json!({
            "warn": "set up",
            "source": { "only": "docs/a.md", "error": "can't read {arg}" },
            "html": { "only": "/a", "panic": "Oh no" },
            "page": { "only": "/b", "give": "[]" },
        });
        let mut plugins = started(&settings.to_string());
        plugins.take_warnings();
        let text = plugins.source("docs/a.md", "docs/a.md at v1.4", "# A\n".to_string());
        assert_eq!(text, "# A\n");
        let html = plugins.html("/a", "docs/a.md", "<p>A</p>".to_string());
        assert_eq!(html, "<p>A</p>");
        let page = Mapping::from_iter([("url".into(), "/b".into())]);
        assert_eq!(plugins.page("docs/b.md", page.clone()), page);
        assert_eq!(
            warnings(&mut plugins),
            [
                "plugin `test`'s `source` hook failed on docs/a.md at v1.4: can't read docs/a.md",
                "plugin `test`'s `html` hook failed on docs/a.md: it crashed: wasm trap: wasm \
                 `unreachable` instruction executed",
                // It starts again, after a crash, for the next hook.
                "plugin `test`: set up",
                "plugin `test`'s `page` hook failed on docs/b.md: it gave back a `page` that \
                 isn't a JSON object: invalid type: sequence, expected a YAML mapping at line 1 \
                 column 0",
            ]
        );
    }

    #[test]
    fn each_plugin_changes_what_the_one_before_gives_back() {
        let site = scratch("config", &[("little.yml", "title: Docs\n")]);
        let config = config::load(&site).unwrap();
        let first = json!({
            "config": { "merge": { "title": "First", "description": "Hooked" } },
            "source": { "append": " first" },
            "content": { "replace": ["A", "B"] },
            "site": { "merge": { "title": "Site" } },
            "page": { "merge": { "title": "Page" } },
            "html": { "append": " <!-- {arg} -->" },
        });
        let second = json!({
            "languages": [],
            "config": { "merge": { "title": "Second" } },
            "source": { "append": " second" },
            "content": { "replace": ["B", "C"] },
            "site": { "echo": "given" },
            "page": { "echo": "given" },
        });
        let mut plugins = start(&[
            named("first", &first.to_string(), &[]),
            named("second", &second.to_string(), &[]),
        ])
        .unwrap();

        let config = plugins.config(config, &site).unwrap();
        let value = |key: &str| config.values.get(key).and_then(serde_yaml::Value::as_str);
        assert_eq!(value("title"), Some("Second"));
        assert_eq!(value("description"), Some("Hooked"));
        let text = plugins.source("docs/a.md", "docs/a.md", "A".to_string());
        assert_eq!(text, "A first second");
        assert_eq!(
            plugins.content("docs/a.md", "docs/a.md", "A".to_string()),
            "C"
        );
        let mut checked = Vec::new();
        let site_value = Mapping::from_iter([("url".into(), "/".into())]);
        let hooked = (plugins.site(site_value, |name, site| {
            checked.push((name.to_string(), site.get("title").cloned()));
            Ok(())
        }))
        .unwrap();
        assert_eq!(
            hooked.get("given").and_then(serde_yaml::Value::as_str),
            Some(r#"{"url":"/","title":"Site"}"#)
        );
        assert_eq!(
            checked,
            [
                ("first".to_string(), Some("Site".into())),
                ("second".to_string(), Some("Site".into())),
            ]
        );
        let page = Mapping::from_iter([("url".into(), "/a".into())]);
        let page = plugins.page("docs/a.md", page);
        assert_eq!(
            page.get("given").and_then(serde_yaml::Value::as_str),
            Some(r#"{"url":"/a","title":"Page"}"#)
        );
        let html = plugins.html("/a", "docs/a.md", "<p>A</p>".to_string());
        assert_eq!(html, "<p>A</p> <!-- /a -->");
        assert_eq!(warnings(&mut plugins), [] as [String; 0]);
        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn a_plugins_config_hook_cant_change_what_little_reads_before_it_starts() {
        let site = scratch("config-errors", &[("little.yml", "title: Docs\n")]);
        let error = |settings: serde_json::Value| {
            let config = config::load(&site).unwrap();
            let mut plugins = started(&json!({ "config": settings }).to_string());
            format!("{:#}", plugins.config(config, &site).unwrap_err())
        };
        assert_eq!(
            error(json!({ "error": "no config" })),
            "plugin `test`'s `config` hook failed: no config"
        );
        assert_eq!(
            error(json!({ "give": "[]" })),
            "plugin `test`'s `config` hook failed: it gave back a config that isn't a JSON \
             object: invalid type: sequence, expected a YAML mapping at line 1 column 0"
        );
        assert_eq!(
            error(json!({ "merge": { "source": "./elsewhere" } })),
            "plugin `test`'s `config` hook changes `source`, but plugins can't change \
             `theme`, `plugins`, `source`, `destination`, `host`, or `port`"
        );
        let message = error(json!({ "merge": { "toc_max_depth": 9 } }));
        assert!(
            message.starts_with(
                "plugin `test`'s `config` hook gives back a config little can't use: "
            ),
            "{message}"
        );
        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn a_plugins_files_hook_adds_and_removes_the_sources_files() {
        let site = scratch("files-hook", &[("pkg/hello.md", "# Hello\n")]);
        let read = [config::Read {
            folder: site.join("pkg"),
            path: "pkg".to_string(),
        }];
        let settings = json!({ "files": {
            "remove": ["old.md", "missing.md", "guide.md"],
            "add": [
                { "path": "guide.md", "text": "# Guide, again\n" },
                { "path": "api/index.md", "read": "pkg/hello.md" },
            ],
            "list": "paths.txt",
        }});
        let mut plugins = start(&[named("api", &settings.to_string(), &read)]).unwrap();
        let mut files = BTreeMap::from_iter(
            ["guide.md", "old.md"].map(|path| (path.to_string(), "on disk".to_string())),
        );
        let added = |plugin: &str, contents: Vec<u8>| {
            format!("{plugin}: {}", String::from_utf8(contents).unwrap())
        };
        plugins.files(&mut files, added).unwrap();
        assert_eq!(
            files,
            BTreeMap::from_iter(
                [
                    ("api/index.md", "api: # Hello\n"),
                    ("guide.md", "api: # Guide, again\n"),
                    ("paths.txt", "api: guide.md\nold.md"),
                ]
                .map(|(path, origin)| (path.to_string(), origin.to_string()))
            )
        );

        let error = |files: serde_json::Value| {
            let mut plugins = started(&json!({ "files": files }).to_string());
            let mut on_disk = BTreeMap::from_iter([("guide.md".to_string(), ())]);
            format!("{:#}", plugins.files(&mut on_disk, |_, _| ()).unwrap_err())
        };
        let add = |paths: &[&str]| {
            let add: Vec<_> = (paths.iter())
                .map(|path| json!({ "path": path, "text": "" }))
                .collect();
            json!({ "add": add })
        };
        let message = |path: &str, why: &str| {
            format!("plugin `test`'s `files` hook adds a file at `{path}`, but {why}")
        };
        assert_eq!(
            error(json!({ "error": "no files" })),
            "plugin `test`'s `files` hook failed: no files"
        );
        assert_eq!(
            error(add(&["guide.md"])),
            message("guide.md", "there's one there, which it doesn't remove")
        );
        assert_eq!(
            error(add(&["api.md", "api.md"])),
            message("api.md", "it adds another there")
        );
        let relative = "a file's path is relative, without a `/` before it";
        assert_eq!(error(add(&["/api.md"])), message("/api.md", relative));
        let parts = "a file's path has `/` between its parts, and no part is empty, `.`, or `..`";
        for path in [
            "",
            "reference/",
            "reference//api.md",
            "./api.md",
            "reference/../../api.md",
        ] {
            assert_eq!(error(add(&[path])), message(path, parts));
        }
        // None is outside the source on Windows, either.
        let windows = "a file's path has no `\\` or `:`, which mean something else on Windows";
        for path in [
            "reference\\api.md",
            "C:/api.md",
            "C:api.md",
            "api.md:stream",
        ] {
            assert_eq!(error(add(&[path])), message(path, windows));
        }
        // Nor where a disk couldn't have it, beside the others.
        assert_eq!(
            error(add(&["guide.md/logo.png"])),
            message(
                "guide.md/logo.png",
                "there's a file at `guide.md`, where its folder would be"
            )
        );
        assert_eq!(
            error(add(&["api/index.md", "api"])),
            message("api", "there's a folder there, with `api/index.md` in it")
        );
        // But where one it removes was.
        let settings = json!({ "files": {
            "remove": ["guide.md"],
            "add": [{ "path": "guide.md/logo.png", "text": "" }],
        }});
        let mut plugins = started(&settings.to_string());
        let mut on_disk = BTreeMap::from_iter([("guide.md".to_string(), ())]);
        plugins.files(&mut on_disk, |_, _| ()).unwrap();
        assert_eq!(Vec::from_iter(on_disk.keys()), ["guide.md/logo.png"]);
        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn a_plugins_theme_hook_changes_the_themes_files() {
        let file = |path: &str, contents: &str, plugin: Option<&str>| HookedFile {
            path: path.to_string(),
            contents: contents.as_bytes().to_vec(),
            plugin: plugin.map(str::to_string),
        };
        let files = vec![
            file("_static/a.css", "a", None),
            file("_templates/default.html", "{{ content }}", None),
        ];
        let first = json!({ "theme": {
            "remove": ["_static/a.css"],
            "add": [
                { "path": "_static/a.css", "text": "changed" },
                { "path": "_templates/og.html", "text": "<meta>" },
            ],
            "list": "_static/paths.txt",
        }});
        // It gives back the files as it's given them.
        let second = json!({ "languages": [], "theme": {} });
        let mut plugins = start(&[
            named("first", &first.to_string(), &[]),
            named("second", &second.to_string(), &[]),
        ])
        .unwrap();
        let mut checked = Vec::new();
        let hooked = (plugins.theme(files.clone(), |name, files| {
            checked.push((name.to_string(), files.len()));
            Ok(())
        }))
        .unwrap();
        assert_eq!(
            hooked,
            [
                file("_static/a.css", "changed", Some("first")),
                file(
                    "_static/paths.txt",
                    "_static/a.css\n_templates/default.html",
                    Some("first")
                ),
                file("_templates/default.html", "{{ content }}", None),
                file("_templates/og.html", "<meta>", Some("first")),
            ]
        );
        assert_eq!(
            checked,
            [("first".to_string(), 4), ("second".to_string(), 4)]
        );

        let error = |theme: serde_json::Value| {
            let mut plugins = started(&json!({ "theme": theme }).to_string());
            let checked = plugins.theme(files.clone(), |name, _| bail!("{name} checked it"));
            format!("{:#}", checked.unwrap_err())
        };
        assert_eq!(error(json!({})), "test checked it");
        assert_eq!(
            error(json!({ "error": "no theme" })),
            "plugin `test`'s `theme` hook failed: no theme"
        );
        assert_eq!(
            error(json!({ "add": [{ "path": "_static/a.css", "text": "b" }] })),
            "plugin `test`'s `theme` hook gives back a file at `_static/a.css`, but it gives \
             back another there"
        );
        assert_eq!(
            error(json!({ "add": [{ "path": "/_static/b.css", "text": "b" }] })),
            "plugin `test`'s `theme` hook gives back a file at `/_static/b.css`, but a file's \
             path is relative, without a `/` before it"
        );
        assert_eq!(
            error(json!({ "add": [{ "path": "_static/a.css/b.css", "text": "b" }] })),
            "plugin `test`'s `theme` hook gives back a file at `_static/a.css/b.css`, but \
             there's a file at `_static/a.css`, where its folder would be"
        );
        assert_eq!(
            error(json!({ "add": [{ "path": "_static", "text": "b" }] })),
            "plugin `test`'s `theme` hook gives back a file at `_static`, but there's a folder \
             there, with `_static/a.css` in it"
        );
    }

    #[test]
    fn a_plugins_destination_hook_adds_files() {
        let first = json!({ "destination": {
            "add": [{ "path": "robots.txt", "text": "User-agent: *" }],
            "list": "paths.txt",
        }});
        let second = json!({ "languages": [], "destination": { "list": "seen.txt" } });
        let mut plugins = start(&[
            named("first", &first.to_string(), &[]),
            named("second", &second.to_string(), &[]),
        ])
        .unwrap();
        let paths = BTreeSet::from_iter(["index.html", "guide/index.html"].map(String::from));
        let added = plugins.destination(paths.clone()).unwrap();
        let added: Vec<_> = (added.iter())
            .map(|file| {
                let contents = String::from_utf8(file.contents.clone()).unwrap();
                (file.path.as_str(), contents, file.plugin.as_deref())
            })
            .collect();
        assert_eq!(
            added,
            [
                ("robots.txt", "User-agent: *".to_string(), Some("first")),
                (
                    "paths.txt",
                    "guide/index.html\nindex.html".to_string(),
                    Some("first")
                ),
                // Each plugin is given the files the ones before it add.
                (
                    "seen.txt",
                    "guide/index.html\nindex.html\npaths.txt\nrobots.txt".to_string(),
                    Some("second")
                ),
            ]
        );

        let settings = json!({ "destination": { "list": "index.html" } });
        let mut plugins = started(&settings.to_string());
        assert_eq!(
            format!("{:#}", plugins.destination(paths.clone()).unwrap_err()),
            "plugin `test`'s `destination` hook adds a file at `index.html`, but there's one \
             there"
        );
        let error = |path: &str| {
            let settings = json!({ "destination": { "add": [{ "path": path, "text": "" }] } });
            let mut plugins = started(&settings.to_string());
            format!("{:#}", plugins.destination(paths.clone()).unwrap_err())
        };
        assert_eq!(
            error("index.html/extra.txt"),
            "plugin `test`'s `destination` hook adds a file at `index.html/extra.txt`, but \
             there's a file at `index.html`, where its folder would be"
        );
        assert_eq!(
            error("guide"),
            "plugin `test`'s `destination` hook adds a file at `guide`, but there's a folder \
             there, with `guide/index.html` in it"
        );
        assert_eq!(
            error("C:/Users/x.bat"),
            "plugin `test`'s `destination` hook adds a file at `C:/Users/x.bat`, but a file's \
             path has no `\\` or `:`, which mean something else on Windows"
        );
    }

    #[test]
    fn a_list_reads_as_a_sentence() {
        let list = |items: &[&str]| {
            list(
                &items
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(list(&["a"]), "a");
        assert_eq!(list(&["a", "b"]), "a and b");
        assert_eq!(list(&["a", "b", "c"]), "a, b, and c");
    }
}
