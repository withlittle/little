//! Plugins that render fenced code blocks: WebAssembly components, built for
//! WASI 0.2, that export the interface in `wit/plugin.wit`. A build starts
//! each plugin the site names once, and gives it each block in its languages
//! as each page is read, before diagrams are drawn; the Markdown or HTML it
//! gives back takes the block's place.
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
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, SystemTime};
use time::OffsetDateTime;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{FsPerms, HostWallClock, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::config::{self, Wasm};
use crate::download;

mod bindings {
    wasmtime::component::bindgen!({ path: "wit/plugin.wit", world: "plugin" });
}

use bindings::exports::little::plugin::code_blocks::{Block, Output};
use bindings::{Plugin, PluginPre};

/// One engine compiles and runs every plugin.
static ENGINE: LazyLock<Engine> = LazyLock::new(Engine::default);

/// Each plugin compiled so far, by its file, so `little serve` compiles it
/// again only once it changes.
static COMPILED: LazyLock<Mutex<HashMap<PathBuf, Compiled>>> = LazyLock::new(Mutex::default);

/// A plugin compiled from its file, with when the file was changed, and its
/// size, then.
type Compiled = (Stamp, PluginPre<State>);

/// When a file was changed, and its size.
type Stamp = (Option<SystemTime>, u64);

/// A block a plugin couldn't render, which is left as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unrendered {
    /// The plugin's name, like `bob`.
    pub plugin: String,
    /// The line of its file the block starts on, if its file is Markdown: a
    /// notebook's lines are its cells'.
    pub line: Option<usize>,
    /// Why, as the plugin says, or that it crashed.
    pub reason: String,
}

/// The plugins a build has started, and the languages each renders.
#[derive(Default)]
pub struct Plugins {
    plugins: Vec<Running>,
    /// Each language a plugin renders, in lowercase, and which plugin.
    languages: HashMap<String, usize>,
}

/// A plugin a build has started.
struct Running {
    name: String,
    pre: PluginPre<State>,
    read: Vec<config::Read>,
    settings: String,
    /// The time its clock reads, from 1970.
    now: Duration,
    /// The plugin, unless it crashed, in the store that holds its memory.
    instance: Option<(Store<State>, Plugin)>,
}

/// What a plugin's store holds: what it can reach through WASI.
struct State {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
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
    /// renders a language one before it does.
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
            let languages = (plugin.start())
                .map_err(|reason| anyhow!("plugin `{name}` can't start: {reason}"))?;
            let index = started.plugins.len();
            for language in languages {
                let language = language.to_lowercase();
                if language.is_empty() || language.contains(|c: char| c.is_whitespace() || c == ',')
                {
                    bail!(
                        "plugin `{name}` renders `{language}`, but a language must be one word, \
                         like `bob`"
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
            started.plugins.push(plugin);
        }
        Ok(started)
    }

    /// The plugins, for the page built from `page`, its file from the site's
    /// folder, like `docs/guide.md`.
    pub fn on<'p>(&'p mut self, page: &'p str) -> PagePlugins<'p> {
        PagePlugins {
            plugins: Some(self),
            page,
        }
    }
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

    /// Give each fenced code block in `blocks` whose language is a plugin's
    /// to that plugin, in turn, and put what it gives back in the block's
    /// place: Markdown, read with `options` into `arena`, or HTML. A block's
    /// line in its file is `lines_before` after its line in the document,
    /// or `None` for a notebook's. Returns each block a plugin couldn't
    /// render, which is left as it was.
    pub fn render<'a>(
        &mut self,
        arena: &'a Arena<'a>,
        blocks: impl IntoIterator<Item = &'a AstNode<'a>>,
        options: &Options,
        lines_before: Option<usize>,
    ) -> Result<Vec<Unrendered>> {
        let Some(plugins) = self.plugins.as_deref_mut() else {
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
                let plugin = *plugins.languages.get(&language).filter(|_| block.fenced)?;
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
            let plugin = &mut plugins.plugins[plugin];
            let sourcepos = node.data.borrow().sourcepos;
            let line = lines_before.map(|before| before + sourcepos.start.line);
            match plugin.render(&block) {
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
    /// Start the plugin afresh, in a sandbox of its own, and set it up.
    /// Returns the languages it renders, or why it can't start.
    fn start(&mut self) -> Result<Vec<String>, String> {
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
        };
        let mut store = Store::new(&ENGINE, state);
        let plugin = self.pre.instantiate(&mut store).map_err(|e| crashed(&e))?;
        let languages = (plugin.little_plugin_code_blocks())
            .call_setup(&mut store, &self.settings)
            .map_err(|e| crashed(&e))??;
        self.instance = Some((store, plugin));
        Ok(languages)
    }

    /// The plugin's rendering of `block`, or why it can't. After a crash,
    /// it's started again for the next block.
    fn render(&mut self, block: &Block) -> Result<Output, String> {
        if self.instance.is_none() {
            (self.start()).map_err(|reason| format!("it can't start again: {reason}"))?;
        }
        let (store, plugin) = self.instance.as_mut().expect("the plugin started");
        match plugin.little_plugin_code_blocks().call_render(store, block) {
            Ok(rendered) => rendered,
            Err(e) => {
                self.instance = None;
                Err(crashed(&e))
            }
        }
    }
}

/// The plugin in `file`, compiled, and linked to WASI, ready to start.
fn compile(file: &Path) -> Result<PluginPre<State>> {
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
    let pre = PluginPre::new(linker.instantiate_pre(&component)?)?;
    compiled.insert(file.to_path_buf(), (stamp, pre.clone()));
    Ok(pre)
}

/// Why a plugin crashed, from wasmtime, on one line, like `it crashed: wasm
/// trap: wasm `unreachable` instruction executed`, which a Rust plugin does
/// when it panics.
fn crashed(error: &wasmtime::Error) -> String {
    let reason = error.root_cause().to_string();
    format!(
        "it crashed: {}",
        reason.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

/// A block's language: the first word of its info string, up to a comma, in
/// lowercase, as for highlighting.
fn language(info: &str) -> String {
    let word = info.split_ascii_whitespace().next().unwrap_or_default();
    word.split(',').next().unwrap_or_default().to_lowercase()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::config::{Markdown, SiteUrl};
    use crate::notebook::parse_notebook;
    use crate::render::{Rendered, parse_document};
    use std::process::Command;
    use std::slice;
    use std::sync::OnceLock;

    /// When each test's site is built: the start of 2026.
    const NOW: i64 = 1_767_225_600;

    /// The plugin in `tests/plugin`, built once for every test that needs
    /// it.
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
    }

    #[test]
    fn a_plugin_must_be_a_component() {
        let site = scratch(
            "files",
            &[("module.wasm", "\0asm\x01\0\0\0"), ("text.wasm", "Hello")],
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
    fn a_notebooks_markdown_is_given_to_plugins_but_not_its_code() {
        let mut plugins = started("{}");
        let notebook = serde_json::json!({
            "cells": [
                { "cell_type": "markdown", "metadata": {}, "source": "```test\ncount\n```" },
                {
                    "cell_type": "code",
                    "execution_count": 1,
                    "metadata": {},
                    "source": "count",
                    "outputs": [{
                        "output_type": "display_data",
                        "metadata": {},
                        "data": { "text/markdown": "```test\nerror in an output\n```" }
                    }]
                }
            ],
            "metadata": { "kernelspec": { "language": "test", "name": "test" } },
            "nbformat": 4,
            "nbformat_minor": 5
        });
        let markdown = Markdown {
            highlight: None,
            ..Markdown::default()
        };
        let now = OffsetDateTime::from_unix_timestamp(NOW).unwrap();
        let r = parse_notebook(
            &notebook.to_string(),
            "/",
            &(2..=6),
            &SiteUrl::default(),
            &markdown,
            now,
            &mut plugins.on("docs/analysis.ipynb"),
        )
        .unwrap();
        assert_eq!(
            r.html,
            "<p>1</p>\n<pre><code class=\"language-test\">count\n</code></pre>\n\
             <pre><code class=\"language-test\">error in an output\n</code></pre>\n"
        );
        // A block's line would be its cell's.
        assert_eq!(
            r.unrendered_blocks,
            [Unrendered {
                plugin: "test".to_string(),
                line: None,
                reason: "in an output".to_string(),
            }]
        );
    }
}
