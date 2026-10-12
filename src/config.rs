//! The 3-level cascading config: little -> theme -> site, each a `little.yml`.
//! Each level deep-merges over the previous, so the *last* writer wins.
//! little's own file is compiled in; the theme's and the site's are optional.
//!
//! `theme` chooses the theme level: `true` for little's built-in theme,
//! `false` for none, the path to a theme's folder, or the URL of a theme's
//! archive (see `download`). A theme can set it too, to build on another
//! theme, whose settings go beneath its own: a site whose theme builds on
//! another cascades little -> that theme -> its theme -> site.
//!
//! `source` and `destination` name the folders a site is built from and
//! into. Any level can set them, but they're always relative to the site.
//!
//! `toc_min_depth` and `toc_max_depth` are the levels of heading, from 1 for
//! `#` to 6, that each page's table of contents lists.
//!
//! `host` and `port` are where `little serve` serves the site, unless its
//! `--host` and `--port` say otherwise.
//!
//! `url` is the address the site is published at, like
//! `https://example.github.io/my-project`. Its path, `/my-project`, goes
//! before each URL little gives the theme, and `little serve` serves the site
//! under it too.
//!
//! `repo` is the address of the repository the site's source is in, like
//! `https://github.com/withlittle/little`, and `branch` the branch each page
//! links to its file on there.
//!
//! `search` is whether each build indexes the site for a search box (see
//! `search`): little leaves it off, and a theme with a search box turns it on.
//!
//! `markdown` holds a setting for each extension to CommonMark, like
//! `tables`, that turns it on or off: little turns on GitHub's, and a theme
//! or a site can change any of them. `math`, also GitHub's, chooses how math
//! is written, `mermaid` how diagrams are drawn, and `highlight` how code is.
//!
//! `mermaid` holds Mermaid's own config, in Mermaid's own names, like `theme`
//! and `themeVariables`, which each diagram little draws starts from: little
//! sets none, so a theme can choose how diagrams look, and a site change it.
//!
//! `plugins` names each plugin and its `.wasm` file (see `plugin`), by a path
//! from the `little.yml` that names it, or an `https://` URL, with the
//! plugin's own settings beside it. They cascade plugin by plugin and setting
//! by setting, and `false` turns a theme's plugin off. Only the site can set
//! `read`, the folders a plugin can read.
//!
//! `versions` lists the versions of the site, in the order its menu lists
//! them, each with the `title` readers see, the `url` it's published at, one
//! of them at `/`, and the git `ref` it's built from, but for one, at most,
//! that's built from the site's files as they are (see `site`). A version
//! from a ref has its own config, in the `little.yml` it has there, but the
//! site's themes, and the settings that are the whole site's, like `url`
//! (see `Config::for_version`).

use anyhow::{Context, Result, anyhow, bail};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde_yaml::{Mapping, Value};
use std::borrow::Cow;
use std::io::ErrorKind;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use crate::download;
use crate::theme::{self, Location};

/// The config file a theme and a site may each provide.
pub const CONFIG_FILE: &str = "little.yml";

/// A site's merged config, and what it chose.
#[derive(Debug)]
pub struct Config {
    /// The merged cascade: what `little config` prints, and what the theme
    /// sees as `site`.
    pub values: Mapping,
    /// The theme the site chose, then each theme it builds on in turn. Empty
    /// for `theme: false`.
    pub themes: Vec<Location>,
    /// The folder of Markdown files and notebooks the site is built from.
    pub source_dir: PathBuf,
    /// The folder the site is built into, which every build wipes.
    pub destination_dir: PathBuf,
    /// The levels of heading each page's table of contents lists, from
    /// `toc_min_depth` to `toc_max_depth`.
    pub toc_depths: RangeInclusive<u8>,
    /// The host `little serve` serves at, like `127.0.0.1`.
    pub host: String,
    /// The port `little serve` serves at, like 8080.
    pub port: u16,
    /// Where the site is published, from `url`.
    pub url: SiteUrl,
    /// The repository the site's source is in, from `repo` and `branch`, if
    /// it sets `repo`.
    pub repo: Option<Repo>,
    /// Whether a build indexes the site for search, from `search`.
    pub search: bool,
    /// The extensions to CommonMark each page is written with, from
    /// `markdown`.
    pub markdown: Markdown,
    /// The plugins that render code blocks, from `plugins`, in the order
    /// they're named.
    pub plugins: Vec<Plugin>,
    /// The versions of the site, from `versions`, in the order its menu lists
    /// them: none for a site that has just the one.
    pub versions: Vec<Version>,
    /// little's defaults, with the settings of each theme over them: what a
    /// version of the site from a git ref puts its own config over.
    beneath: Mapping,
}

/// The settings a version of a site from a git ref takes from the site, not
/// from its own little.yml: where the whole site is built into, published,
/// and served, which theme it's in, and which versions it has.
const SITE_WIDE: [&str; 7] = [
    "theme",
    "url",
    "destination",
    "repo",
    "host",
    "port",
    "versions",
];

/// A version of a site, from `versions`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    /// Its title, like `1.4 (Latest)`, which readers see.
    pub title: String,
    /// Its path from the root of the site, from its `url`, like `/1.4`, which
    /// is also the folder of the destination it's built into, or empty, for
    /// the version at `/`.
    pub path: String,
    /// The git ref it's built from, like `v1.4`, or none, for the site's
    /// files as they are.
    pub git_ref: Option<String>,
}

impl Version {
    /// The folders, one in another, of the destination it's built into,
    /// from its `path`: none for the version at `/`.
    pub fn folders(&self) -> impl Iterator<Item = &str> {
        self.path.split('/').filter(|folder| !folder.is_empty())
    }
}

/// A plugin a site uses, from `plugins`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plugin {
    /// Its name, like `bob`.
    pub name: String,
    /// Its `.wasm` file.
    pub wasm: Wasm,
    /// The folders it can read, from `read`.
    pub read: Vec<Read>,
    /// Its settings, all but `wasm`, as a JSON object, for its `setup`.
    pub settings: String,
}

/// Where a plugin's `.wasm` file is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Wasm {
    /// A file, at its path from the `little.yml` that names it.
    File(PathBuf),
    /// An `https://` URL, which a build downloads once (see `download`).
    Url(String),
}

/// A folder a plugin can read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    /// The folder, from the site's folder.
    pub folder: PathBuf,
    /// Its path as the plugin sees it, as the site's little.yml writes it,
    /// like `src`.
    pub path: String,
}

/// Where a site is published, from its `url`. A site that doesn't set one is
/// at the root of whichever domain serves it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SiteUrl {
    /// The scheme and host, like `https://example.github.io`; empty if the
    /// site doesn't set `url`.
    pub origin: String,
    /// The path the site is under, like `/my-project`, with no `/` at the
    /// end; empty for a site at the root of its domain.
    pub path: String,
}

impl SiteUrl {
    /// `url` as the browser should follow it: a URL from the root of the
    /// site, like `/guide/`, gets the site's path, as `/my-project/guide/`.
    /// Any other URL, like `https://…`, `//…`, `#…`, or `guide/`, is left as
    /// it is.
    pub fn link<'a>(&self, url: &'a str) -> Cow<'a, str> {
        if self.path.is_empty() || !from_the_root(url) {
            return Cow::Borrowed(url);
        }
        Cow::Owned(format!("{}{url}", self.path))
    }

    /// `url`, which already has the site's path, with the site's scheme and
    /// host before it, as a feed, a sitemap, or a link preview needs. Left as
    /// it is if the site doesn't set `url`, or `url` isn't from the root.
    pub fn absolute<'a>(&self, url: &'a str) -> Cow<'a, str> {
        if self.origin.is_empty() || !from_the_root(url) {
            return Cow::Borrowed(url);
        }
        Cow::Owned(format!("{}{url}", self.origin))
    }
}

/// Whether `url` starts at the root of the domain, like `/guide/`, rather
/// than at another domain, like `//example.com/`, or the current page.
fn from_the_root(url: &str) -> bool {
    url.starts_with('/') && !url.starts_with("//")
}

/// The repository a site's source is in, from `repo`, and the branch its
/// pages link to their files on, from `branch`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    /// Its address, like `https://github.com/withlittle/little`, with no `/`
    /// at the end.
    pub url: String,
    /// The branch, like `main`.
    pub branch: String,
}

/// Where to view a file in a site's repository, and where to edit it.
#[derive(Debug, PartialEq, Eq)]
pub struct FileUrls {
    pub source: String,
    pub edit: String,
}

/// What's encoded in each part of a file's path in a URL: all but letters,
/// digits, `-`, `.`, `_`, and `~`, so a name with a space or a `#` in it
/// can't break the URL.
const PATH_PART: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

impl Repo {
    /// Where to view and to edit the file at `path` in the repository, like
    /// `docs/guide/install.md`, on its branch. little knows only GitHub's and
    /// GitLab's addresses, so a repository anywhere else has none.
    pub fn file_urls(&self, path: &Path) -> Option<FileUrls> {
        let host = self.url.split_once("://")?.1.split('/').next()?;
        let (source, edit) = if host.eq_ignore_ascii_case("github.com") {
            ("blob", "edit")
        } else if host.eq_ignore_ascii_case("gitlab.com") {
            ("-/blob", "-/edit")
        } else {
            return None;
        };
        let branch = self.branch.split('/').map(Cow::Borrowed);
        let file = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy());
        let parts: Vec<String> = branch
            .chain(file)
            .map(|part| utf8_percent_encode(&part, PATH_PART).to_string())
            .collect();
        let rest = parts.join("/");
        Some(FileUrls {
            source: format!("{}/{source}/{rest}", self.url),
            edit: format!("{}/{edit}/{rest}", self.url),
        })
    }
}

/// Which extensions to CommonMark each page is written with, how its math is
/// written, how its diagrams are drawn, and which classes its code is
/// highlighted with, from the settings under `markdown`, each named for what
/// an author writes, as `src/little.yml` describes them; and how its diagrams
/// look, from `mermaid`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Markdown {
    // As on GitHub.
    pub tables: bool,
    pub strikethrough: bool,
    pub autolinks: bool,
    pub task_lists: bool,
    pub footnotes: bool,
    pub alerts: bool,
    pub emoji: bool,
    pub raw_html: bool,
    /// How math is written, if it's found.
    pub math: Option<Math>,
    /// How a Mermaid block is drawn, if it's drawn rather than left as code.
    pub mermaid: Option<Mermaid>,
    // Beyond GitHub.
    pub smart_punctuation: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub mark: bool,
    pub insert: bool,
    pub description_lists: bool,
    pub inline_footnotes: bool,
    pub fenced_divs: bool,
    pub multiline_blockquotes: bool,
    pub hard_breaks: bool,
    pub cjk_friendly_emphasis: bool,
    /// The classes code is highlighted with, if it's highlighted.
    pub highlight: Option<Highlight>,
}

/// How math is written: as MathML, which browsers show as it is, or as its
/// TeX, marked with `data-math-style` for a theme's script to render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Math {
    MathML,
    Tex,
}

/// How a Mermaid block is drawn: as an `<svg>`, as little builds the page,
/// with Mermaid's config from `mermaid`, as JSON, as Mermaid reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mermaid {
    Svg(serde_json::Value),
}

/// A set of classes for highlighted code: Pygments', like `k` for a keyword,
/// or GitHub's, like `pl-k`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Highlight {
    Pygments,
    GitHub,
}

impl Default for Markdown {
    /// little's own settings, from `src/little.yml`.
    fn default() -> Self {
        markdown(&builtin_defaults()).expect("src/little.yml sets each Markdown extension")
    }
}

/// little's built-in defaults: level 1 of the cascade. They live in
/// `src/little.yml`, compiled into the binary so little stays a single file
/// with nothing to install alongside it.
fn builtin_defaults() -> Mapping {
    parse_config(include_str!("little.yml")).expect("src/little.yml holds settings")
}

/// Follow the chain of themes from the one the site chooses, then merge the
/// cascade.
pub fn load(site_dir: &Path) -> Result<Config> {
    let defaults = builtin_defaults();
    let site_file = site_dir.join(CONFIG_FILE);
    let site_name = site_file.display().to_string();
    let mut site_cfg = read_config(&site_file, &site_name)?;
    long_form_plugins(&mut site_cfg, &site_name, site_dir, true)?;

    let mut next = match site_cfg.get("theme").or(defaults.get("theme")) {
        Some(value) => choose_theme(value, site_dir, &site_name, site_dir)?,
        None => None,
    };
    let mut themes = Vec::new();
    let mut theme_cfgs = Vec::new();
    // Each theme folder's real path, to catch themes that build on each other
    // in a loop however their paths are written.
    let mut seen = Vec::new();
    while let Some(location) = next {
        let cfg = match location.folder() {
            None => {
                next = None; // the built-in theme builds on nothing
                parse_config(theme::builtin::CONFIG).expect("theme/little.yml holds settings")
            }
            Some(dir) => {
                let real = dir
                    .canonicalize()
                    .with_context(|| format!("reading {}", dir.display()))?;
                if seen.contains(&real) {
                    let chain: Vec<String> = themes
                        .iter()
                        .chain([&location])
                        .map(ToString::to_string)
                        .collect();
                    bail!(
                        "themes build on each other in a loop: {}",
                        chain.join(" -> ")
                    );
                }
                seen.push(real);
                let file = location.describe(CONFIG_FILE);
                let mut cfg = read_config(&dir.join(CONFIG_FILE), &file)?;
                long_form_plugins(&mut cfg, &file, dir, false)?;
                next = match cfg.get("theme") {
                    Some(value) => choose_theme(value, dir, &file, site_dir)?,
                    None => None,
                };
                cfg
            }
        };
        themes.push(location);
        theme_cfgs.push(cfg);
    }

    // The theme at the end of the chain goes beneath the ones that build on it.
    let mut values = defaults;
    for cfg in theme_cfgs.into_iter().rev() {
        merge(&mut values, cfg);
    }
    let beneath = values.clone();
    merge(&mut values, site_cfg);
    settle(site_dir, values, beneath, themes)
}

impl Config {
    /// The config of a version of the site from a git ref, whose folder is
    /// at `site_dir`, as it was at the ref, and whose little.yml errors call
    /// `file`, like `v1.4:docs/little.yml`. Its little.yml goes over little's
    /// defaults and the site's themes, whichever theme it chose then, but
    /// the settings that are the whole site's, like `url`, are the site's.
    pub fn for_version(&self, site_dir: &Path, file: &str) -> Result<Config> {
        let mut own = read_config(&site_dir.join(CONFIG_FILE), file)?;
        long_form_plugins(&mut own, file, site_dir, true)?;
        let mut values = self.beneath.clone();
        merge(&mut values, own);
        for key in SITE_WIDE {
            match self.values.get(key) {
                Some(value) => values.insert(key.into(), value.clone()),
                None => values.shift_remove(key),
            };
        }
        let mut config = settle(site_dir, values, self.beneath.clone(), self.themes.clone())?;
        // The one setting of the site's that's a path, so it leads from the
        // site's folder.
        config.destination_dir = self.destination_dir.clone();
        Ok(config)
    }
}

/// Check the merged config, `values`, of the site at `site_dir`, which is in
/// `themes`, over `beneath`, and work out what it chooses.
fn settle(
    site_dir: &Path,
    values: Mapping,
    beneath: Mapping,
    themes: Vec<Location>,
) -> Result<Config> {
    let source_dir = folder(site_dir, &values, "source")?;
    let destination_dir = folder(site_dir, &values, "destination")?;
    let toc_depths = toc_depths(&values)?;
    let host = host(&values)?;
    let port = port(&values)?;
    let url = site_url(&values)?;
    let repo = repo(&values)?;
    let search = search(&values)?;
    let markdown = markdown(&values)?;
    let plugins = plugins(&values, site_dir)?;
    let versions = versions(&values)?;
    Ok(Config {
        values,
        themes,
        source_dir,
        destination_dir,
        toc_depths,
        host,
        port,
        url,
        repo,
        search,
        markdown,
        plugins,
        versions,
        beneath,
    })
}

/// The theme a `theme` setting chooses, if any. `file` names the file that
/// sets it, and `dir` is its folder: a path is relative to that, as a theme
/// can't know where the sites that use it are. A theme downloaded from a URL
/// is kept in the site at `site_dir`.
fn choose_theme(
    value: &Value,
    dir: &Path,
    file: &str,
    site_dir: &Path,
) -> Result<Option<Location>> {
    match value {
        Value::Bool(true) => Ok(Some(Location::BuiltIn)),
        Value::Bool(false) => Ok(None),
        Value::String(url) if url.starts_with("https://") => {
            let dir = download::theme(site_dir, url)
                .map_err(|e| anyhow!("{file} sets `theme` to `{url}`, but {e:#}"))?;
            Ok(Some(Location::Downloaded {
                url: url.clone(),
                dir,
            }))
        }
        Value::String(url) if url.starts_with("http://") => {
            bail!("{file} sets `theme` to `{url}`, but little only downloads themes over HTTPS")
        }
        Value::String(path) => {
            let folder = resolve(dir, path);
            if !folder.is_dir() {
                bail!(
                    "{file} sets `theme` to `{path}`, but there's no folder at {}",
                    folder.display()
                );
            }
            Ok(Some(Location::Folder(folder)))
        }
        _ => bail!(
            "`theme` in {file} must be `true`, `false`, the path to a theme folder, \
             or the URL of a theme's archive"
        ),
    }
}

/// The folder a setting names. It's relative to the site even when a theme
/// sets it: it's the site that's built.
fn folder(site_dir: &Path, values: &Mapping, key: &str) -> Result<PathBuf> {
    match values.get(key) {
        Some(Value::String(path)) => Ok(resolve(site_dir, path)),
        _ => bail!("`{key}` must be the path to a folder"),
    }
}

/// The levels of heading a table of contents lists. The min and max can be
/// set at different levels of the cascade, so a min over the max, which would
/// leave every table of contents empty, is an error.
fn toc_depths(values: &Mapping) -> Result<RangeInclusive<u8>> {
    let min = heading_level(values, "toc_min_depth")?;
    let max = heading_level(values, "toc_max_depth")?;
    if min > max {
        bail!("`toc_min_depth` ({min}) can't be more than `toc_max_depth` ({max})");
    }
    Ok(min..=max)
}

/// The level of heading a setting names, like 2 for `##`.
fn heading_level(values: &Mapping, key: &str) -> Result<u8> {
    match values.get(key).and_then(Value::as_u64) {
        Some(level @ 1..=6) => Ok(level as u8),
        _ => bail!("`{key}` must be a heading level from 1 to 6, like 3 for `###`"),
    }
}

/// The host `little serve` serves at: a host name, like `localhost`, or an IP
/// address, like `127.0.0.1` or `::1`.
fn host(values: &Mapping) -> Result<String> {
    match values.get("host") {
        Some(Value::String(host)) if !host.is_empty() => Ok(host.clone()),
        _ => bail!("`host` must be a host name or IP address, like 127.0.0.1"),
    }
}

/// The port `little serve` serves at. Port 0 would have the system pick one,
/// and the URL little prints would be wrong.
fn port(values: &Mapping) -> Result<u16> {
    match values.get("port").and_then(Value::as_u64) {
        Some(port @ 1..=65535) => Ok(port as u16),
        _ => bail!("`port` must be a number from 1 to 65535, like 8080"),
    }
}

/// Where the site is published, from `url`: its scheme and host, then the
/// path the site is under.
fn site_url(values: &Mapping) -> Result<SiteUrl> {
    let Some(value) = values.get("url") else {
        return Ok(SiteUrl::default());
    };
    let error = || {
        anyhow!(
            "`url` must be the address the site is published at, starting with https:// \
             or http://, like https://example.github.io/my-project"
        )
    };
    let url = value.as_str().ok_or_else(error)?;
    let rest = (url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(error)?;
    if rest.contains(['?', '#']) || rest.contains(char::is_whitespace) {
        return Err(error());
    }
    let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    if host.is_empty() {
        return Err(error());
    }
    Ok(SiteUrl {
        origin: url[..url.len() - path.len()].to_string(),
        path: path.trim_end_matches('/').to_string(),
    })
}

/// The repository the site's source is in, from `repo`, and the branch its
/// pages link to their files on, from `branch`. YAML reads a branch like
/// `2.0` as a number, so it must be quoted, or it would be linked as `2`.
fn repo(values: &Mapping) -> Result<Option<Repo>> {
    let branch = match values.get("branch") {
        Some(Value::String(branch)) if !branch.is_empty() => branch.clone(),
        _ => bail!(
            "`branch` must be the name of a branch, like main, in quotes if it looks like a \
             number, like \"2.0\""
        ),
    };
    let Some(value) = values.get("repo") else {
        return Ok(None);
    };
    let error = || {
        anyhow!(
            "`repo` must be the address of the site's repository, as a browser shows it, \
             like https://github.com/example/my-project"
        )
    };
    let url = value.as_str().ok_or_else(error)?.trim_end_matches('/');
    let rest = (url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(error)?;
    let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    // Not the address it's cloned from, which ends in `.git`.
    if host.is_empty()
        || path.is_empty()
        || url.ends_with(".git")
        || rest.contains(['?', '#'])
        || rest.contains(char::is_whitespace)
        || url::Url::parse(url).is_err()
    {
        return Err(error());
    }
    Ok(Some(Repo {
        url: url.to_string(),
        branch,
    }))
}

/// Whether a build indexes the site for search. YAML reads `yes` and `"true"`
/// as text, not `true`, so they're errors rather than a guess.
fn search(values: &Mapping) -> Result<bool> {
    match values.get("search") {
        Some(Value::Bool(on)) => Ok(*on),
        _ => bail!("`search` must be `true` or `false`"),
    }
}

/// The extensions to CommonMark each page is written with, each `true` or
/// `false`, how its math is written, how its diagrams are drawn, and the
/// classes its code is highlighted with, from the settings under `markdown`.
/// little's own config sets every one, so any other is a typo. Diagrams
/// drawn as SVG look as `mermaid` says.
fn markdown(values: &Mapping) -> Result<Markdown> {
    let Some(Value::Mapping(settings)) = values.get("markdown") else {
        bail!("`markdown` must be settings, like `mark: true`");
    };
    let defaults = builtin_defaults();
    let known = (defaults.get("markdown").and_then(Value::as_mapping))
        .expect("src/little.yml sets `markdown`");
    if let Some(key) = settings.keys().find(|key| !known.contains_key(key)) {
        let key = serde_yaml::to_string(key)?;
        let names: Vec<&str> = known.keys().filter_map(Value::as_str).collect();
        bail!(
            "`markdown` has no setting `{}`: its settings are {}",
            key.trim_end(),
            names.join(", ")
        );
    }
    let on = |key: &str| match settings.get(key) {
        Some(Value::Bool(on)) => Ok(*on),
        _ => Err(anyhow!("`markdown.{key}` must be `true` or `false`")),
    };
    let markdown = Markdown {
        tables: on("tables")?,
        strikethrough: on("strikethrough")?,
        autolinks: on("autolinks")?,
        task_lists: on("task_lists")?,
        footnotes: on("footnotes")?,
        alerts: on("alerts")?,
        emoji: on("emoji")?,
        raw_html: on("raw_html")?,
        // `true` isn't one, as a site's would undo its theme's `tex`.
        math: match settings.get("math") {
            Some(Value::String(form)) if form == "mathml" => Some(Math::MathML),
            Some(Value::String(form)) if form == "tex" => Some(Math::Tex),
            Some(Value::Bool(false)) => None,
            _ => bail!("`markdown.math` must be `mathml`, `tex`, or `false`"),
        },
        // Nor here, as a site's would undo its theme's `false`.
        mermaid: match settings.get("mermaid") {
            Some(Value::String(form)) if form == "svg" => Some(Mermaid::Svg(mermaid(values)?)),
            Some(Value::Bool(false)) => None,
            _ => bail!("`markdown.mermaid` must be `svg` or `false`"),
        },
        smart_punctuation: on("smart_punctuation")?,
        superscript: on("superscript")?,
        subscript: on("subscript")?,
        mark: on("mark")?,
        insert: on("insert")?,
        description_lists: on("description_lists")?,
        inline_footnotes: on("inline_footnotes")?,
        fenced_divs: on("fenced_divs")?,
        multiline_blockquotes: on("multiline_blockquotes")?,
        hard_breaks: on("hard_breaks")?,
        cjk_friendly_emphasis: on("cjk_friendly_emphasis")?,
        highlight: match settings.get("highlight") {
            Some(Value::String(classes)) if classes == "pygments" => Some(Highlight::Pygments),
            Some(Value::String(classes)) if classes == "github" => Some(Highlight::GitHub),
            Some(Value::Bool(false)) => None,
            _ => bail!("`markdown.highlight` must be `pygments`, `github`, or `false`"),
        },
    };
    // comrak reads `^[…]` only with footnotes, so it'd be left as text.
    if markdown.inline_footnotes && !markdown.footnotes {
        bail!("`markdown.inline_footnotes` needs `markdown.footnotes` to be `true` too");
    }
    // merman doesn't clean every diagram, so one can carry HTML into the page.
    if markdown.mermaid.is_some() && !markdown.raw_html {
        bail!(
            "`markdown.mermaid: svg` needs `markdown.raw_html` to be `true`, as a diagram can \
             carry HTML: set `markdown.mermaid: false` too"
        );
    }
    Ok(markdown)
}

/// Mermaid's config, which each diagram starts from, from the settings under
/// `mermaid`, as they're written, in Mermaid's own names. Mermaid ignores a
/// setting it doesn't have, as in a diagram's own config, so little checks
/// only for the mistakes that change every diagram without a word: a theme
/// Mermaid doesn't have, which it swaps for its default, and an empty
/// setting, as a colour like #6f42c1 is without quotes.
fn mermaid(values: &Mapping) -> Result<serde_json::Value> {
    let settings = match values.get("mermaid") {
        None => return Ok(serde_json::json!({})),
        Some(Value::Mapping(settings)) => settings,
        Some(_) => bail!("`mermaid` must be Mermaid's settings, like `theme: forest`"),
    };
    let themes = merman::supported_themes();
    if let Some(theme) = settings.get("theme")
        && !theme.as_str().is_some_and(|theme| themes.contains(&theme))
    {
        bail!(
            "`mermaid.theme` must be one of Mermaid's themes: {}",
            themes.join(", ")
        );
    }
    if let Some(path) = empty_setting(settings, "mermaid") {
        bail!(
            "`{path}` is empty: put a colour in quotes, like \"#6f42c1\", or YAML reads it as \
             a comment"
        );
    }
    serde_json::to_value(settings).context("reading `mermaid`")
}

/// The first setting in `settings`, or in the settings within them, that's
/// empty, named from `path`, like `mermaid.themeVariables.primaryColor`.
fn empty_setting(settings: &Mapping, path: &str) -> Option<String> {
    settings.iter().find_map(|(key, value)| {
        let key = serde_yaml::to_string(key).unwrap_or_default();
        let path = format!("{path}.{}", key.trim_end());
        match value {
            Value::Null => Some(path),
            Value::Mapping(settings) => empty_setting(settings, &path),
            _ => None,
        }
    })
}

/// Write each plugin the config in `file`, in `dir`, names in the long form,
/// as settings with `wasm`, its path from `dir`, so the levels merge setting
/// by setting, and each path leads from the `little.yml` that wrote it. Only
/// the site's config, `is_site`, can set `read`.
fn long_form_plugins(cfg: &mut Mapping, file: &str, dir: &Path, is_site: bool) -> Result<()> {
    let plugins = match cfg.get_mut("plugins") {
        None => return Ok(()),
        Some(Value::Mapping(plugins)) => plugins,
        // `plugins:` with nothing after it.
        Some(Value::Null) => {
            cfg.shift_remove("plugins");
            return Ok(());
        }
        Some(_) => bail!(
            "`plugins` in {file} must name each plugin and its `.wasm` file, like \
             `bob: ./plugins/bob.wasm`"
        ),
    };
    for (name, plugin) in plugins.iter_mut() {
        let Some(name) = name.as_str() else {
            bail!(
                "`plugins` in {file} must name each plugin and its `.wasm` file, like \
                 `bob: ./plugins/bob.wasm`"
            );
        };
        if let Value::String(wasm) = plugin {
            *plugin = Value::Mapping(Mapping::from_iter([("wasm".into(), wasm.as_str().into())]));
        }
        let settings = match plugin {
            Value::Mapping(settings) => settings,
            Value::Bool(false) => continue,
            _ => bail!(
                "plugin `{name}` in {file} must be its `.wasm` file, its settings, or `false`"
            ),
        };
        if !is_site && settings.contains_key("read") {
            bail!(
                "plugin `{name}` in {file} sets `read`, but only the site's {CONFIG_FILE} can \
                 choose what a plugin reads"
            );
        }
        match settings.get_mut("wasm") {
            None => {}
            Some(Value::String(url)) if url.starts_with("https://") => {}
            Some(Value::String(url)) if url.starts_with("http://") => bail!(
                "plugin `{name}` in {file} is at `{url}`, but little only downloads plugins \
                 over HTTPS"
            ),
            Some(Value::String(path)) => {
                *path = resolve(dir, path).to_string_lossy().into_owned();
            }
            Some(_) => bail!(
                "`wasm` of plugin `{name}` in {file} must be the path or URL of its `.wasm` file"
            ),
        }
    }
    Ok(())
}

/// The plugins the merged config names, each in the long form, but those
/// turned off with `false`.
fn plugins(values: &Mapping, site_dir: &Path) -> Result<Vec<Plugin>> {
    let Some(Value::Mapping(plugins)) = values.get("plugins") else {
        return Ok(Vec::new());
    };
    let mut on = Vec::new();
    for (name, settings) in plugins {
        let Value::Mapping(settings) = settings else {
            continue; // `false`
        };
        let name = name.as_str().expect("each plugin has a name").to_string();
        let wasm = match settings.get("wasm") {
            Some(Value::String(url)) if url.starts_with("https://") => Wasm::Url(url.clone()),
            Some(Value::String(path)) => Wasm::File(PathBuf::from(path)),
            _ => bail!("plugin `{name}` needs `wasm`, the path or URL of its `.wasm` file"),
        };
        let folders = match settings.get("read") {
            None => Vec::new(),
            Some(Value::String(folder)) => vec![folder.as_str()],
            Some(Value::Sequence(folders)) => (folders.iter())
                .map(Value::as_str)
                .collect::<Option<_>>()
                .ok_or_else(|| read_error(&name))?,
            Some(_) => return Err(read_error(&name)),
        };
        let read: Vec<Read> = (folders.into_iter())
            .map(|folder| Read {
                folder: resolve(site_dir, folder),
                path: match folder.trim_start_matches("./").trim_end_matches('/') {
                    "" => ".".to_string(),
                    path => path.to_string(),
                },
            })
            .collect();
        // The plugin is told the folders it can read as it sees them, and its
        // settings in the order they're written.
        let mut settings = settings.clone();
        settings.shift_remove("wasm");
        if let Some(folders) = settings.get_mut("read") {
            *folders = read.iter().map(|read| read.path.as_str()).collect();
        }
        let settings = serde_json::to_string(&settings).with_context(|| {
            format!("plugin `{name}`'s settings must each be named, like `style: google`")
        })?;
        on.push(Plugin {
            name,
            wasm,
            read,
            settings,
        });
    }
    Ok(on)
}

/// The error for a plugin's `read` that isn't a folder or a list of them.
fn read_error(name: &str) -> anyhow::Error {
    anyhow!("`read` of plugin `{name}` must be a folder, or a list of them, from the site's folder")
}

/// The versions of the site, from `versions`, in the order its menu lists
/// them. Each entry's other settings are for the theme.
fn versions(values: &Mapping) -> Result<Vec<Version>> {
    let entries = match values.get("versions") {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Sequence(entries)) => entries,
        Some(_) => return Err(versions_error()),
    };
    let mut versions: Vec<Version> = Vec::new();
    for entry in entries {
        let Value::Mapping(entry) = entry else {
            return Err(versions_error());
        };
        // YAML reads 1.10 as a number, 1.1.
        let title = match entry.get("title") {
            Some(Value::String(title)) if !title.trim().is_empty() => title,
            _ => bail!(
                "each version needs a `title`, in quotes if it looks like a number, like \"1.10\", \
                 which YAML would read as 1.1"
            ),
        };
        if versions.iter().any(|version| version.title == *title) {
            bail!("two versions are titled `{title}`");
        }
        let path = match entry.get("url") {
            Some(Value::String(url)) if !url.trim().is_empty() => version_path(url),
            None | Some(Value::Null | Value::String(_)) => bail!(
                "version `{title}` needs a `url`, where it's published from the root of the \
                 site, like `/1.4`, or `/` for the site's home"
            ),
            Some(_) => None,
        };
        let Some(path) = path else {
            bail!(
                "version `{title}`'s `url` must be a path from the root of the site, like `/1.4` \
                 or `/versions/1.4`, with letters, digits, `.`, `-`, and `_` between its `/`s, \
                 each part starting with a letter or a digit, as they name the folders the \
                 version's built into"
            );
        };
        for other in &versions {
            if other.path == path {
                bail!(
                    "versions `{}` and `{title}` are both at {}/",
                    other.title,
                    other.path
                );
            }
            // Only the version at `/` can have others in its folder, as it's
            // the one whose files little checks aren't where they go.
            if other.path.is_empty() || path.is_empty() {
                continue;
            }
            let (inner, outer) = if path.starts_with(&format!("{}/", other.path)) {
                ((title.as_str(), &path), (other.title.as_str(), &other.path))
            } else if other.path.starts_with(&format!("{path}/")) {
                ((other.title.as_str(), &other.path), (title.as_str(), &path))
            } else {
                continue;
            };
            bail!(
                "version `{}`, at {}/, is inside version `{}`, at {}/: only the version at `/` \
                 can have others inside it",
                inner.0,
                inner.1,
                outer.0,
                outer.1
            );
        }
        // A blank `ref:` is as good as none. One starting with `-` would be
        // read by git as an option.
        let git_ref = match entry.get("ref") {
            None | Some(Value::Null) => None,
            Some(Value::String(git_ref)) if !git_ref.is_empty() && !git_ref.starts_with('-') => {
                Some(git_ref.clone())
            }
            Some(_) => bail!(
                "version `{title}`'s `ref` must be the tag, branch, or commit it's built from, \
                 like v1.4, in quotes if it looks like a number"
            ),
        };
        if git_ref.is_none()
            && let Some(other) = versions.iter().find(|version| version.git_ref.is_none())
        {
            bail!(
                "versions `{}` and `{title}` both have no `ref`, but only one can be built from \
                 the site's files as they are: give the other the tag, branch, or commit it's \
                 built from",
                other.title
            );
        }
        versions.push(Version {
            title: title.clone(),
            path,
            git_ref,
        });
    }
    if !versions.is_empty() && !versions.iter().any(|version| version.path.is_empty()) {
        bail!(
            "no version is at `/`, the site's home: give the one readers should land on `url: /`"
        );
    }
    Ok(versions)
}

/// The path of a version at `url`, a path from the root of the site, like
/// `/1.4/` or `/versions/1.4`, without the `/` at its end, so empty for `/`,
/// as each part names a folder; `None` if a part can't.
fn version_path(url: &str) -> Option<String> {
    if url == "/" {
        return Some(String::new());
    }
    let path = url.strip_suffix('/').unwrap_or(url);
    let is_folder = |part: &str| {
        part.starts_with(|c: char| c.is_ascii_alphanumeric())
            && (part.chars()).all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    (path.strip_prefix('/')?.split('/').all(is_folder)).then(|| path.to_string())
}

/// The error for `versions` that isn't a list of versions.
fn versions_error() -> anyhow::Error {
    anyhow!(
        "`versions` must list the site's versions, each with its `title`, the `url` it's at, and \
         the `ref` it's built from, like `- {{ title: \"1.4\", url: /1.4, ref: v1.4 }}`"
    )
}

/// Resolve a path a config names against `dir`: an absolute path is just that
/// path. Tidied, so a site in `.` that sets `./docs` shows it as `./docs`.
fn resolve(dir: &Path, path: &str) -> PathBuf {
    dir.join(path).components().collect()
}

/// Read one level's config from `path`, which errors call `name`. A missing
/// file isn't an error: that level just has nothing to add.
fn read_config(path: &Path, name: &str) -> Result<Mapping> {
    match std::fs::read_to_string(path) {
        Ok(raw) => parse_config(&raw).with_context(|| format!("parsing {name}")),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Mapping::new()),
        Err(e) => Err(e).with_context(|| format!("reading {name}")),
    }
}

/// Parse a config file's settings. An empty or comment-only file has none.
/// Anything but settings is an error: merged, it would replace every level
/// beneath it.
fn parse_config(raw: &str) -> serde_yaml::Result<Mapping> {
    Ok(serde_yaml::from_str::<Option<Mapping>>(raw)?.unwrap_or_default())
}

/// Merge `over` into `base`, key by key. Where both set a key to settings,
/// those merge the same way; any other value replaces the one beneath it. The
/// build merges a page's frontmatter over `page`, and the config over
/// `site.nav`, the same way.
pub fn merge(base: &mut Mapping, over: Mapping) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(Value::Mapping(below)), Value::Mapping(above)) => merge(below, above),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn get<'a>(cfg: &'a Mapping, key: &str) -> Option<&'a str> {
        cfg.get(key).and_then(Value::as_str)
    }

    /// A fresh scratch folder holding `files`, each a (path, contents) pair.
    fn scratch(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("little-config-test-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for (file, contents) in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        root
    }

    /// Why the site's config doesn't load once its little.yml is `site_cfg`.
    fn error(site: &Path, site_cfg: &str) -> String {
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
        format!("{:#}", load(site).unwrap_err())
    }

    /// Put `files`, each a (path, contents) pair, where the theme at `url` is
    /// kept once it's downloaded, so a test needn't download it.
    fn downloaded(site: &Path, url: &str, files: &[(&str, &str)]) -> Location {
        let dir = download::folder(site, url);
        std::fs::create_dir_all(&dir).unwrap();
        for (file, contents) in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        Location::Downloaded {
            url: url.to_string(),
            dir,
        }
    }

    /// little's defaults, with the built-in theme's settings over them.
    fn with_builtin_theme() -> Mapping {
        let mut values = builtin_defaults();
        merge(&mut values, parse_config(theme::builtin::CONFIG).unwrap());
        values
    }

    #[test]
    fn a_site_gets_the_builtin_theme_unless_it_chooses_another() {
        let site = scratch(
            "choose",
            &[
                ("theme/little.yml", "font: mono\n"),
                ("path/to/my/theme/little.yml", "name: Mine\nfont: serif\n"),
            ],
        );
        let load_with = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap()
        };

        // Without a `theme`, or with `theme: true`, the built-in theme, even
        // for a site with a `theme/` folder.
        assert_eq!(load(&site).unwrap().themes, [Location::BuiltIn]);
        for site_cfg in ["name: My Docs\n", "theme: true\n"] {
            let cfg = load_with(site_cfg);
            assert_eq!(cfg.themes, [Location::BuiltIn], "with {site_cfg:?}");
            assert_eq!(get(&cfg.values, "font"), get(&with_builtin_theme(), "font"));
        }

        // With `theme: false`, no theme, so no theme's settings either.
        let cfg = load_with("theme: false\n");
        assert!(cfg.themes.is_empty());
        assert_eq!(get(&cfg.values, "font"), None);

        // A path chooses the theme in that folder.
        let cfg = load_with("theme: ./path/to/my/theme\nname: My Docs\n");
        let chosen = Location::Folder(site.join("path/to/my/theme"));
        assert_eq!(cfg.themes, [chosen]);
        assert_eq!(get(&cfg.values, "font"), Some("serif")); // the chosen theme's
        assert_eq!(get(&cfg.values, "name"), Some("My Docs")); // the site still wins
        assert_eq!(get(&cfg.values, "theme"), Some("./path/to/my/theme"));

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn theme_paths_are_relative_to_the_file_that_sets_them() {
        let root = scratch(
            "paths",
            &[
                ("site/themes/mine/_templates/default.html", ""),
                ("shared/theme/_templates/default.html", ""),
                ("shared/company/little.yml", "theme: ../theme\n"),
            ],
        );
        let site = root.join("site");
        // Unquoted, as people usually write them.
        let chosen = |theme: &str| {
            std::fs::write(site.join(CONFIG_FILE), format!("theme: {theme}\n")).unwrap();
            let themes = load(&site).unwrap().themes;
            themes
                .iter()
                .map(|theme| theme.folder().unwrap().canonicalize().unwrap())
                .collect::<Vec<_>>()
        };
        let real = |path: &str| root.join(path).canonicalize().unwrap();

        // A site's are relative to the site, not to wherever little runs.
        assert_eq!(chosen("themes/mine"), [real("site/themes/mine")]);
        assert_eq!(chosen("./themes/mine"), [real("site/themes/mine")]);
        assert_eq!(chosen("../shared/theme"), [real("shared/theme")]);

        // Absolute: the temp dir is /… on macOS and Linux, C:\… on Windows.
        let absolute = root.join("shared/theme").display().to_string();
        assert_eq!(chosen(&absolute), [real("shared/theme")]);

        // A theme's are relative to the theme: it can't know where sites are.
        let chain = [real("shared/company"), real("shared/theme")];
        assert_eq!(chosen("../shared/company"), chain);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_theme_can_build_on_another() {
        let site = scratch(
            "chain",
            &[
                ("little.yml", "theme: ./company\nname: My Docs\n"),
                (
                    "company/little.yml",
                    "theme: ../fancy\nfont: Inter\ncolors:\n  accent: navy\n",
                ),
                (
                    "fancy/little.yml",
                    "name: Fancy\nfont: serif\nlang: en\ncolors:\n  accent: pink\n  text: black\n",
                ),
            ],
        );
        let company = site.join("company");

        // little -> fancy -> company -> site, each over the one before, as
        // `little config` prints them.
        let cfg = load(&site).unwrap();
        let fancy = Location::Folder(company.join("../fancy"));
        assert_eq!(cfg.themes, [Location::Folder(company.clone()), fancy]);
        // Less little's Markdown settings, which `markdown_cascades` checks.
        let mut values = cfg.values.clone();
        values.shift_remove("markdown");
        assert_eq!(
            serde_yaml::to_string(&values).unwrap(),
            "source: ./docs\n\
             destination: ./dist\n\
             theme: ./company\n\
             toc_min_depth: 2\n\
             toc_max_depth: 6\n\
             host: 127.0.0.1\n\
             port: 8080\n\
             branch: main\n\
             search: false\n\
             name: My Docs\n\
             font: Inter\n\
             lang: en\n\
             colors:\n  accent: navy\n  text: black\n"
        );

        // A theme can build on little's built-in theme, too.
        std::fs::write(company.join(CONFIG_FILE), "theme: true\nfont: Inter\n").unwrap();
        let cfg = load(&site).unwrap();
        assert_eq!(cfg.themes, [Location::Folder(company), Location::BuiltIn]);
        assert_eq!(get(&cfg.values, "font"), Some("Inter"));
        assert_eq!(get(&cfg.values, "lang"), get(&with_builtin_theme(), "lang"));

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn a_theme_can_be_downloaded_from_a_url() {
        let url = "https://example.invalid/fancy-1.0.zip";
        let site_cfg = format!("theme: {url}\nname: My Docs\n");
        let site = scratch("url", &[(CONFIG_FILE, &site_cfg)]);
        let fancy_cfg = "name: Fancy\nfont: serif\nlang: en\n";
        let fancy = downloaded(&site, url, &[(CONFIG_FILE, fancy_cfg)]);

        // It cascades like any other theme.
        let cfg = load(&site).unwrap();
        assert_eq!(cfg.themes, std::slice::from_ref(&fancy));
        assert_eq!(get(&cfg.values, "font"), Some("serif")); // the theme's
        assert_eq!(get(&cfg.values, "name"), Some("My Docs")); // the site still wins
        assert_eq!(get(&cfg.values, "theme"), Some(url));

        // A company's theme can build on it, changing only what it needs to.
        let company_cfg = format!("theme: {url}\nfont: Inter\n");
        std::fs::create_dir_all(site.join("company")).unwrap();
        std::fs::write(site.join("company").join(CONFIG_FILE), company_cfg).unwrap();
        std::fs::write(site.join(CONFIG_FILE), "theme: ./company\n").unwrap();
        let company = Location::Folder(site.join("company"));
        let cfg = load(&site).unwrap();
        assert_eq!(cfg.themes, [company.clone(), fancy.clone()]);
        assert_eq!(get(&cfg.values, "font"), Some("Inter")); // the company's
        assert_eq!(get(&cfg.values, "name"), Some("Fancy")); // the downloaded theme's

        // And it can build on another theme: in its own folder, relative to
        // that, or downloaded too, or little's built-in theme.
        let fancy_dir = fancy.folder().unwrap().to_path_buf();
        let base = Location::Folder(fancy_dir.join("base"));
        std::fs::create_dir_all(fancy_dir.join("base")).unwrap();
        let plain_url = "https://example.invalid/plain-2.0.tar.gz";
        let plain = downloaded(&site, plain_url, &[]);
        for (theme, beneath) in [
            ("./base", base),
            (plain_url, plain),
            ("true", Location::BuiltIn),
        ] {
            let fancy_cfg = format!("theme: {theme}\n");
            std::fs::write(fancy_dir.join(CONFIG_FILE), fancy_cfg).unwrap();
            let themes = load(&site).unwrap().themes;
            assert_eq!(themes, [company.clone(), fancy.clone(), beneath]);
        }

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn themes_cannot_build_on_each_other_in_a_loop() {
        let site = scratch(
            "loop",
            &[
                ("a/little.yml", "theme: ../b\n"),
                ("b/little.yml", "theme: ../a\n"),
            ],
        );
        let (a, b) = (site.join("a"), site.join("a/../b"));
        let message = error(&site, "theme: ./a\n");
        let expected = format!(
            "themes build on each other in a loop: {} -> {} -> {}",
            a.display(),
            b.display(),
            b.join("../a").display()
        );
        assert_eq!(message, expected);

        // Even a theme that builds on itself.
        std::fs::write(a.join(CONFIG_FILE), "theme: .\n").unwrap();
        let message = error(&site, "theme: ./a\n");
        assert!(
            message.contains("themes build on each other in a loop"),
            "{message}"
        );

        // Downloaded themes are named by their URLs.
        let (a, b) = (
            "https://example.invalid/a.zip",
            "https://example.invalid/b.zip",
        );
        downloaded(&site, a, &[(CONFIG_FILE, &format!("theme: {b}\n"))]);
        downloaded(&site, b, &[(CONFIG_FILE, &format!("theme: {a}\n"))]);
        let message = error(&site, &format!("theme: {a}\n"));
        let expected = format!("themes build on each other in a loop: {a} -> {b} -> {a}");
        assert_eq!(message, expected);

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn bad_theme_choices_explain_themselves() {
        let site = scratch("bad-theme", &[("theme/_templates/default.html", "")]);
        let file = site.join(CONFIG_FILE).display().to_string();
        let must_be = |file: &str| {
            format!(
                "`theme` in {file} must be `true`, `false`, the path to a theme folder, \
                 or the URL of a theme's archive"
            )
        };

        let message = error(&site, "theme: ./missing\n");
        let expected = format!(
            "{file} sets `theme` to `./missing`, but there's no folder at {}",
            site.join("missing").display()
        );
        assert_eq!(message, expected);
        // `default` is a folder name like any other, and only unquoted `true`
        // and `false` are booleans.
        for theme in ["default", "\"true\"", "no"] {
            let message = error(&site, &format!("theme: {theme}\n"));
            assert!(message.contains("but there's no folder at"), "{message}");
        }

        // Nothing, or anything but `true`, `false`, or text, isn't a theme.
        for theme in ["", " [a, b]", " 1"] {
            let message = error(&site, &format!("theme:{theme}\n"));
            assert_eq!(message, must_be(&file));
        }

        // Themes are only downloaded over HTTPS.
        let url = "http://example.invalid/theme.zip";
        let message = error(&site, &format!("theme: {url}\n"));
        let expected =
            format!("{file} sets `theme` to `{url}`, but little only downloads themes over HTTPS");
        assert_eq!(message, expected);
        // A download that fails says why. Nothing listens on port 1, so this
        // one fails at once, without the network.
        let url = "https://127.0.0.1:1/theme.zip";
        let message = error(&site, &format!("theme: {url}\n"));
        let expected = format!("{file} sets `theme` to `{url}`, but little couldn't download it: ");
        assert!(message.starts_with(&expected), "{message}");

        // A theme's own `theme` is checked the same way, and named as its.
        let theme_file = site.join("theme").join(CONFIG_FILE);
        std::fs::write(&theme_file, "theme: [a, b]\n").unwrap();
        let message = error(&site, "theme: ./theme\n");
        assert_eq!(message, must_be(&theme_file.display().to_string()));

        // A downloaded theme's folder is named with a hash, so its files are
        // named by its URL.
        let url = "https://example.invalid/fancy-1.0.zip";
        let fancy_file = format!("{CONFIG_FILE} in the theme from {url}");
        let fancy = downloaded(&site, url, &[(CONFIG_FILE, "theme: [a, b]\n")]);
        let message = error(&site, &format!("theme: {url}\n"));
        assert_eq!(message, must_be(&fancy_file));
        let fancy_cfg = fancy.folder().unwrap().join(CONFIG_FILE);
        std::fs::write(fancy_cfg, "- font: serif\n").unwrap();
        let message = error(&site, &format!("theme: {url}\n"));
        assert!(
            message.starts_with(&format!("parsing {fancy_file}: ")),
            "{message}"
        );

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn folders_are_relative_to_the_site_wherever_theyre_set() {
        let root = scratch(
            "folders",
            &[("site/themes/mine/_templates/default.html", "")],
        );
        let site = root.join("site");
        let folders = |theme_cfg: &str, site_cfg: &str| {
            std::fs::write(site.join("themes/mine").join(CONFIG_FILE), theme_cfg).unwrap();
            let site_cfg = format!("theme: ./themes/mine\n{site_cfg}");
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            let cfg = load(&site).unwrap();
            (cfg.source_dir, cfg.destination_dir)
        };

        // little's own defaults.
        assert_eq!(folders("", ""), (site.join("docs"), site.join("dist")));

        // A theme can set them, but they're still relative to the site...
        let theme = "source: content\ndestination: ./public\n";
        let expected = (site.join("content"), site.join("public"));
        assert_eq!(folders(theme, ""), expected);

        // ...and the site has the last word, of course.
        let site_cfg = "source: ../shared/docs\n";
        let expected = (site.join("../shared/docs"), site.join("public"));
        assert_eq!(folders(theme, site_cfg), expected);

        // Absolute paths work too.
        let out = root.join("out");
        let site_cfg = format!("destination: {}\n", out.display());
        assert_eq!(folders("", &site_cfg), (site.join("docs"), out));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bad_folders_explain_themselves() {
        let site = scratch("bad-folders", &[]);
        for key in ["source", "destination"] {
            // A list, or no value at all, isn't a path.
            for value in [" [a, b]", ""] {
                let message = error(&site, &format!("{key}:{value}\n"));
                assert_eq!(message, format!("`{key}` must be the path to a folder"));
            }
        }

        // Whether the source is there is for the build to say: the config
        // doesn't read it.
        std::fs::write(site.join(CONFIG_FILE), "source: ./missing\n").unwrap();
        assert!(load(&site).is_ok());

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn toc_depths_cascade() {
        let site = scratch("toc-depths", &[("theme/_templates/default.html", "")]);
        let depths = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap().toc_depths
        };

        // little lists every heading but a page's `#` title, the built-in
        // theme only `##` and `###`, and a site has the last word.
        assert_eq!(depths("theme: ./theme\n"), 2..=6);
        assert_eq!(depths(""), 2..=3);
        assert_eq!(depths("toc_min_depth: 1\ntoc_max_depth: 6\n"), 1..=6);

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn bad_toc_depths_explain_themselves() {
        let site = scratch("bad-toc-depths", &[]);
        for key in ["toc_min_depth", "toc_max_depth"] {
            // There's no `#######`, a heading level is a number, and no value
            // at all isn't one.
            for value in [" 0", " 7", " -1", " \"3\"", ""] {
                let message = error(&site, &format!("{key}:{value}\n"));
                assert_eq!(
                    message,
                    format!("`{key}` must be a heading level from 1 to 6, like 3 for `###`")
                );
            }
        }

        // The built-in theme lists down to `###`.
        assert_eq!(
            error(&site, "toc_min_depth: 4\n"),
            "`toc_min_depth` (4) can't be more than `toc_max_depth` (3)"
        );

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn search_cascades() {
        let site = scratch("search", &[("theme/_templates/default.html", "")]);
        let search = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap().search
        };

        // little leaves search off, the built-in theme, with its search box,
        // turns it on, and a site has the last word.
        assert!(!search("theme: ./theme\n"));
        assert!(!search("theme: false\n"));
        assert!(search(""));
        assert!(!search("search: false\n"));
        assert!(search("theme: ./theme\nsearch: true\n"));

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn bad_searches_explain_themselves() {
        let site = scratch("bad-search", &[]);
        // YAML reads `yes` and a quoted `"true"` as text, not `true`.
        for value in [" yes", " \"true\"", " 1", ""] {
            let message = error(&site, &format!("search:{value}\n"));
            assert_eq!(message, "`search` must be `true` or `false`");
        }

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn markdown_cascades() {
        let site = scratch(
            "markdown",
            &[("theme/little.yml", "markdown:\n  math: tex\n")],
        );
        let markdown = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap().markdown
        };

        // little turns on GitHub's extensions, math as MathML among them, and
        // leaves the rest off.
        let builtin = markdown("");
        assert_eq!(builtin, Markdown::default());
        assert!(builtin.tables && builtin.alerts && builtin.emoji && builtin.raw_html);
        assert_eq!(builtin.math, Some(Math::MathML));
        assert_eq!(builtin.mermaid, Some(Mermaid::Svg(json!({}))));
        assert!(!builtin.mark);
        assert_eq!(builtin.highlight, Some(Highlight::Pygments));

        // A theme that renders math keeps its TeX, and a site turns other
        // extensions off, each merging over the settings beneath it.
        let site_cfg = "theme: ./theme\nmarkdown:\n  raw_html: false\n  mermaid: false\n";
        let expected = Markdown {
            math: Some(Math::Tex),
            raw_html: false,
            mermaid: None,
            ..Markdown::default()
        };
        assert_eq!(markdown(site_cfg), expected);

        // A site has the last word on math, as on anything.
        let site_cfg = "theme: ./theme\nmarkdown:\n  math: mathml\n";
        assert_eq!(markdown(site_cfg).math, Some(Math::MathML));
        let site_cfg = "theme: ./theme\nmarkdown:\n  math: false\n";
        assert_eq!(markdown(site_cfg).math, None);

        // A site leaves diagrams as code, for a theme that draws them.
        let site_cfg = "markdown:\n  mermaid: false\n";
        assert_eq!(markdown(site_cfg).mermaid, None);
        let site_cfg = "markdown:\n  mermaid: svg\n";
        assert_eq!(markdown(site_cfg).mermaid, Some(Mermaid::Svg(json!({}))));

        // A site chooses GitHub's classes, or none.
        let site_cfg = "markdown:\n  highlight: github\n";
        assert_eq!(markdown(site_cfg).highlight, Some(Highlight::GitHub));
        let site_cfg = "markdown:\n  highlight: false\n";
        assert_eq!(markdown(site_cfg).highlight, None);

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn bad_markdown_explains_itself() {
        let site = scratch("bad-markdown", &[]);
        // YAML reads `yes` and a quoted `"true"` as text, not `true`.
        for value in [" yes", " \"true\"", " 1", ""] {
            let message = error(&site, &format!("markdown:\n  mark:{value}\n"));
            assert_eq!(message, "`markdown.mark` must be `true` or `false`");
        }
        for site_cfg in ["markdown: true\n", "markdown:\n", "markdown: [math]\n"] {
            let message = error(&site, site_cfg);
            assert_eq!(message, "`markdown` must be settings, like `mark: true`");
        }

        // A typo is named, beside the settings there are.
        let message = error(&site, "markdown:\n  tabels: true\n");
        let expected = "`markdown` has no setting `tabels`: its settings are tables, \
                        strikethrough, autolinks, ";
        assert!(message.starts_with(expected), "{message}");
        assert!(
            message.ends_with(", cjk_friendly_emphasis, highlight"),
            "{message}"
        );

        // Math is written in a form little has, or not at all: not `true`,
        // which would undo a theme's `tex`.
        for value in ["true", "MathML", "katex", "\"false\"", ""] {
            let message = error(&site, &format!("markdown:\n  math: {value}\n"));
            assert_eq!(
                message,
                "`markdown.math` must be `mathml`, `tex`, or `false`"
            );
        }

        // Code is highlighted with a set of classes little has, or none.
        for value in ["true", "monokai", "Pygments", "\"false\"", ""] {
            let message = error(&site, &format!("markdown:\n  highlight: {value}\n"));
            assert_eq!(
                message,
                "`markdown.highlight` must be `pygments`, `github`, or `false`"
            );
        }

        // Diagrams are drawn as SVG or left as code: not `true`, which would
        // undo a theme's `false`.
        for value in ["true", "SVG", "png", "\"false\"", ""] {
            let message = error(&site, &format!("markdown:\n  mermaid: {value}\n"));
            assert_eq!(message, "`markdown.mermaid` must be `svg` or `false`");
        }

        // A diagram can carry HTML, so a site without raw HTML says so.
        for site_cfg in [
            "markdown:\n  raw_html: false\n",
            "markdown:\n  raw_html: false\n  mermaid: svg\n",
        ] {
            let message = error(&site, site_cfg);
            assert_eq!(
                message,
                "`markdown.mermaid: svg` needs `markdown.raw_html` to be `true`, as a diagram \
                 can carry HTML: set `markdown.mermaid: false` too"
            );
        }

        // comrak reads `^[…]` only with footnotes.
        let message = error(
            &site,
            "markdown:\n  footnotes: false\n  inline_footnotes: true\n",
        );
        assert_eq!(
            message,
            "`markdown.inline_footnotes` needs `markdown.footnotes` to be `true` too"
        );

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn mermaid_cascades() {
        let theme_cfg = "mermaid:\n  theme: base\n  themeVariables:\n    primaryColor: \"#6f42c1\"\n    \
                         lineColor: \"#333333\"\n";
        let site = scratch("mermaid", &[("theme/little.yml", theme_cfg)]);
        let mermaid = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            match load(&site).unwrap().markdown.mermaid {
                Some(Mermaid::Svg(config)) => config,
                None => panic!("diagrams should be drawn"),
            }
        };

        // little sets no look, so diagrams have Mermaid's default.
        assert_eq!(mermaid(""), json!({}));

        // A theme sets its look, and a site changes it, setting by setting.
        let theme = json!({
            "theme": "base",
            "themeVariables": { "primaryColor": "#6f42c1", "lineColor": "#333333" },
        });
        assert_eq!(mermaid("theme: ./theme\n"), theme);
        let site_cfg =
            "theme: ./theme\nmermaid:\n  themeVariables:\n    primaryColor: \"#0969da\"\n";
        let expected = json!({
            "theme": "base",
            "themeVariables": { "primaryColor": "#0969da", "lineColor": "#333333" },
        });
        assert_eq!(mermaid(site_cfg), expected);

        // Every setting is passed on as it's written, in Mermaid's names, for
        // Mermaid to read, or, like a typo, ignore.
        let site_cfg = "mermaid:\n  look: handDrawn\n  fontFamily: Georgia, serif\n  gantt:\n    \
                        useWidth: 1000\n  themeVariable:\n    primaryColor: \"#0969da\"\n";
        let expected = json!({
            "look": "handDrawn",
            "fontFamily": "Georgia, serif",
            "gantt": { "useWidth": 1000 },
            "themeVariable": { "primaryColor": "#0969da" },
        });
        assert_eq!(mermaid(site_cfg), expected);

        // Diagrams left as code, for a theme's script, aren't drawn, so their
        // look isn't read, but the theme still has it.
        let site_cfg = "theme: ./theme\nmarkdown:\n  mermaid: false\nmermaid:\n  theme: drak\n";
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
        let config = load(&site).unwrap();
        assert_eq!(config.markdown.mermaid, None);
        let theme = config.values.get("mermaid").and_then(|m| m.get("theme"));
        assert_eq!(theme.and_then(Value::as_str), Some("drak"));

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn bad_mermaid_explains_itself() {
        let site = scratch("bad-mermaid", &[]);
        for site_cfg in ["mermaid: forest\n", "mermaid:\n", "mermaid: [theme]\n"] {
            let message = error(&site, site_cfg);
            assert_eq!(
                message,
                "`mermaid` must be Mermaid's settings, like `theme: forest`"
            );
        }

        // Mermaid would swap a theme it doesn't have for its default.
        for value in ["drak", "Dark", "3", "true", "\"\"", ""] {
            let message = error(&site, &format!("mermaid:\n  theme: {value}\n"));
            assert_eq!(
                message,
                "`mermaid.theme` must be one of Mermaid's themes: default, base, dark, forest, \
                 neutral, neo, neo-dark, redux, redux-dark, redux-color, redux-dark-color"
            );
        }

        // YAML reads a colour after `#` as a comment, leaving its setting
        // empty, which is named, however deep it is.
        let site_cfg = "mermaid:\n  theme: base\n  themeVariables:\n    primaryColor: #6f42c1\n";
        let message = error(&site, site_cfg);
        assert_eq!(
            message,
            "`mermaid.themeVariables.primaryColor` is empty: put a colour in quotes, like \
             \"#6f42c1\", or YAML reads it as a comment"
        );
        let message = error(&site, "mermaid:\n  fontFamily:\n");
        assert!(
            message.starts_with("`mermaid.fontFamily` is empty: "),
            "{message}"
        );

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn host_and_port_cascade() {
        let site = scratch("host-port", &[("theme/little.yml", "port: 4000\n")]);
        let address = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            let cfg = load(&site).unwrap();
            (cfg.host, cfg.port)
        };

        // little serves at 127.0.0.1:8080, a theme can change that, and a site
        // has the last word.
        assert_eq!(address(""), ("127.0.0.1".into(), 8080));
        assert_eq!(address("theme: ./theme\n"), ("127.0.0.1".into(), 4000));
        let site_cfg = "theme: ./theme\nhost: localhost\nport: 3000\n";
        assert_eq!(address(site_cfg), ("localhost".into(), 3000));

        // An IPv6 address needs no quotes or brackets.
        assert_eq!(address("host: ::1\n"), ("::1".into(), 8080));

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn bad_hosts_and_ports_explain_themselves() {
        let site = scratch("bad-host-port", &[]);

        // A host is a name or an address, written as text.
        for value in [" 8080", " [a, b]", " \"\"", ""] {
            let message = error(&site, &format!("host:{value}\n"));
            assert_eq!(
                message,
                "`host` must be a host name or IP address, like 127.0.0.1"
            );
        }

        // Port 0 would have the system pick a port, there's none past 65535,
        // and a port is a number, not text, nor an address with it.
        for value in [" 0", " 65536", " -1", " \"8080\"", " 127.0.0.1:3000", ""] {
            let message = error(&site, &format!("port:{value}\n"));
            assert_eq!(
                message,
                "`port` must be a number from 1 to 65535, like 8080"
            );
        }

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn a_url_puts_the_site_under_its_path() {
        let site = scratch("site-url", &[]);
        let url = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap().url
        };
        let site_url = |origin: &str, path: &str| SiteUrl {
            origin: origin.into(),
            path: path.into(),
        };

        // Without a url, a site is at the root of whichever domain serves it.
        assert_eq!(url(""), SiteUrl::default());

        // A `/` at the end of the path is left off, so links don't get two.
        let project = site_url("https://withlittle.github.io", "/little");
        assert_eq!(url("url: https://withlittle.github.io/little\n"), project);
        assert_eq!(url("url: https://withlittle.github.io/little/\n"), project);
        let domain = site_url("https://docs.example.com", "");
        assert_eq!(url("url: https://docs.example.com\n"), domain);
        assert_eq!(url("url: https://docs.example.com/\n"), domain);
        let nested = site_url("http://localhost:4000", "/a/b");
        assert_eq!(url("url: http://localhost:4000/a/b\n"), nested);

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn bad_urls_explain_themselves() {
        let site = scratch("bad-site-url", &[]);

        // A url is the whole address, as text: not just its host or path, and
        // with nothing after the path.
        for value in [
            " withlittle.github.io/little",
            " /little",
            " ftp://example.com/docs",
            " https://",
            " https:///little",
            " https://example.com/docs?lang=en",
            " https://example.com/docs#top",
            " https://example.com/my docs",
            " 8080",
            " [https://example.com]",
            "",
        ] {
            let message = error(&site, &format!("url:{value}\n"));
            assert_eq!(
                message,
                "`url` must be the address the site is published at, starting with \
                 https:// or http://, like https://example.github.io/my-project",
                "url:{value}"
            );
        }

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn links_from_the_root_get_the_sites_path() {
        let url = SiteUrl {
            origin: "https://withlittle.github.io".into(),
            path: "/little".into(),
        };
        assert_eq!(url.link("/"), "/little/");
        assert_eq!(url.link("/guide/install"), "/little/guide/install");
        assert_eq!(url.link("/assets/style.css"), "/little/assets/style.css");

        // A link to another site, or from the current page, is left alone.
        for other in [
            "https://example.com/",
            "//cdn.example.com/lib.js",
            "mailto:me@example.com",
            "#usage",
            "install",
            "../",
            "",
        ] {
            assert_eq!(url.link(other), other);
        }

        // An absolute URL adds the scheme and host to a link with the path.
        let og = "https://withlittle.github.io/little/og.png";
        assert_eq!(url.absolute("/little/og.png"), og);
        assert_eq!(url.absolute(og), og);
        assert_eq!(url.absolute("og.png"), "og.png");

        // A site at the root of its domain keeps its links as they are.
        let domain = SiteUrl {
            origin: "https://docs.example.com".into(),
            path: String::new(),
        };
        assert_eq!(domain.link("/guide/"), "/guide/");
        assert_eq!(
            domain.absolute("/guide/"),
            "https://docs.example.com/guide/"
        );

        // And without a url, there's nothing to add.
        assert_eq!(SiteUrl::default().link("/guide/"), "/guide/");
        assert_eq!(SiteUrl::default().absolute("/guide/"), "/guide/");
    }

    #[test]
    fn a_repo_and_its_branch_cascade() {
        let site = scratch("repo", &[("theme/little.yml", "branch: trunk\n")]);
        let repo = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap().repo
        };
        let little = |branch: &str| {
            Some(Repo {
                url: "https://github.com/withlittle/little".into(),
                branch: branch.into(),
            })
        };

        // Without a repo, there's nothing to link to.
        assert_eq!(repo(""), None);
        assert_eq!(repo("theme: ./theme\n"), None);

        // little links to files on `main`, a theme can change that, and a
        // site has the last word. A `/` at the end of the repo is left off.
        let site_cfg = "repo: https://github.com/withlittle/little\n";
        assert_eq!(repo(site_cfg), little("main"));
        assert_eq!(repo(&site_cfg.replace('\n', "/\n")), little("main"));
        let site_cfg = format!("theme: ./theme\n{site_cfg}");
        assert_eq!(repo(&site_cfg), little("trunk"));
        assert_eq!(repo(&format!("{site_cfg}branch: \"2.0\"\n")), little("2.0"));

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn bad_repos_and_branches_explain_themselves() {
        let site = scratch("bad-repo", &[]);

        // A repo is the address a browser shows for it, as text: not the
        // address it's cloned from, or just its host, and with nothing after
        // its path.
        for value in [
            " withlittle/little",
            " git@github.com:withlittle/little.git",
            " ssh://git@github.com/withlittle/little",
            " https://github.com/withlittle/little.git",
            " https://github.com",
            " https://github.com/",
            " https://",
            " https:///withlittle/little",
            " https://github.com/withlittle/little?tab=readme",
            " https://github.com/withlittle/little#readme",
            " https://github.com/withlittle/my little",
            " https://github.com:x/withlittle/little",
            " 8080",
            " [https://github.com/withlittle/little]",
            "",
        ] {
            let message = error(&site, &format!("repo:{value}\n"));
            assert_eq!(
                message,
                "`repo` must be the address of the site's repository, as a browser shows it, \
                 like https://github.com/example/my-project",
                "repo:{value}"
            );
        }

        // YAML reads a branch like `2.0` as a number, which isn't its name.
        for value in [" 2.0", " 2024", " true", " \"\"", " [main]", ""] {
            let message = error(&site, &format!("branch:{value}\n"));
            assert_eq!(
                message,
                "`branch` must be the name of a branch, like main, in quotes if it looks like \
                 a number, like \"2.0\"",
                "branch:{value}"
            );
        }

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn a_repo_links_to_its_files_on_github_and_gitlab() {
        let repo = |url: &str, branch: &str| Repo {
            url: url.into(),
            branch: branch.into(),
        };
        let urls = |repo: &Repo, path: &str| {
            let urls = repo.file_urls(Path::new(path))?;
            Some([urls.source, urls.edit])
        };

        let github = repo("https://github.com/withlittle/little", "main");
        assert_eq!(
            urls(&github, "docs/guide/install.md").unwrap(),
            [
                "https://github.com/withlittle/little/blob/main/docs/guide/install.md",
                "https://github.com/withlittle/little/edit/main/docs/guide/install.md",
            ]
        );
        // A GitLab project can be in a group, and a branch in a folder.
        let gitlab = repo("https://gitlab.com/withlittle/docs/little", "release/2.0");
        assert_eq!(
            urls(&gitlab, "README.md").unwrap(),
            [
                "https://gitlab.com/withlittle/docs/little/-/blob/release/2.0/README.md",
                "https://gitlab.com/withlittle/docs/little/-/edit/release/2.0/README.md",
            ]
        );

        // Each part of the branch and path is encoded, so no name can break
        // the URL.
        let [source, _] = urls(&repo(github.url.as_str(), "fix#1"), "docs/my café.md").unwrap();
        assert_eq!(
            source,
            "https://github.com/withlittle/little/blob/fix%231/docs/my%20caf%C3%A9.md"
        );

        // A host is the same host in capitals, but little doesn't know any
        // other's addresses.
        let shouty = repo("https://GitHub.com/withlittle/little", "main");
        assert!(urls(&shouty, "README.md").is_some());
        for url in [
            "https://codeberg.org/withlittle/little",
            "https://github.example.com/withlittle/little",
            "https://notgithub.com/withlittle/little",
        ] {
            assert_eq!(urls(&repo(url, "main"), "README.md"), None, "{url}");
        }
    }

    #[test]
    fn config_files_hold_settings() {
        let site = scratch("settings", &[]);
        let parsing = format!("parsing {}: ", site.join(CONFIG_FILE).display());

        // Settings are keys and values: a list or a single value would replace
        // them all.
        for site_cfg in ["- source: ./docs\n", "My Docs\n"] {
            let message = error(&site, site_cfg);
            assert!(message.starts_with(&parsing), "{message}");
            assert!(message.contains("expected a YAML mapping"), "{message}");
        }

        // YAML that doesn't parse says where.
        let message = error(&site, "name: \"My Docs\n");
        assert!(message.starts_with(&parsing), "{message}");
        assert!(message.contains("at line"), "{message}");

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn config_files_are_optional_and_can_be_empty() {
        let site = scratch("optional", &[("theme/_templates/default.html", "")]);

        // Without a little.yml, or with one that's empty or only comments, a
        // site gets little's defaults and the built-in theme's.
        assert_eq!(load(&site).unwrap().values, with_builtin_theme());
        for site_cfg in ["", "# nothing to set yet\n"] {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            assert_eq!(load(&site).unwrap().values, with_builtin_theme());
        }

        // The same goes for a theme's little.yml.
        std::fs::write(site.join(CONFIG_FILE), "theme: ./theme\n").unwrap();
        let mut expected = builtin_defaults();
        expected.insert("theme".into(), "./theme".into());
        assert_eq!(load(&site).unwrap().values, expected);
        std::fs::write(site.join("theme").join(CONFIG_FILE), "# nothing\n").unwrap();
        assert_eq!(load(&site).unwrap().values, expected);

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn little_defaults_only_its_own_settings() {
        let cfg = builtin_defaults();
        assert_eq!(get(&cfg, "source"), Some("./docs"));
        assert_eq!(get(&cfg, "destination"), Some("./dist"));
        assert_eq!(cfg.get("theme"), Some(&Value::Bool(true)));
        assert_eq!(cfg.get("toc_min_depth").and_then(Value::as_u64), Some(2));
        assert_eq!(cfg.get("toc_max_depth").and_then(Value::as_u64), Some(6));
        assert_eq!(get(&cfg, "host"), Some("127.0.0.1"));
        assert_eq!(cfg.get("port").and_then(Value::as_u64), Some(8080));
        // Search is a theme's to turn on, with a box to search in.
        assert_eq!(cfg.get("search"), Some(&Value::Bool(false)));
        // A site's name, language, and look are the theme's to default.
        for key in ["name", "lang", "mermaid"] {
            assert_eq!(cfg.get(key), None, "little shouldn't default `{key}`");
        }
        // And only the site knows where it's published, and where its
        // repository is, though most link to their files on `main`.
        assert_eq!(cfg.get("url"), None);
        assert_eq!(cfg.get("repo"), None);
        assert_eq!(get(&cfg, "branch"), Some("main"));
    }

    #[test]
    fn the_builtin_theme_builds_on_nothing() {
        // The chain ends with the built-in theme, so a `theme` in its config
        // would be ignored.
        let cfg = parse_config(theme::builtin::CONFIG).unwrap();
        assert_eq!(cfg.get("theme"), None);
    }

    #[test]
    fn later_levels_override_earlier_ones() {
        let mut cfg = parse_config("name: Documentation\nfont: serif\n").unwrap();
        merge(&mut cfg, parse_config("name: My Docs\n").unwrap());

        assert_eq!(get(&cfg, "name"), Some("My Docs")); // last writer wins
        assert_eq!(get(&cfg, "font"), Some("serif")); // the earlier value survives
    }

    #[test]
    fn merging_recurses_into_settings_of_settings() {
        let mut cfg = parse_config("a:\n  x: 1\n  y: 2\nb: [1, 2]\n").unwrap();
        merge(
            &mut cfg,
            parse_config("a:\n  y: 20\n  z: 30\nb: [3]\n").unwrap(),
        );

        let a = cfg.get("a").unwrap();
        assert_eq!(a.get("x").and_then(Value::as_u64), Some(1));
        assert_eq!(a.get("y").and_then(Value::as_u64), Some(20));
        assert_eq!(a.get("z").and_then(Value::as_u64), Some(30));
        // Anything but settings, like a list, is replaced whole.
        let b: Value = serde_yaml::from_str("[3]").unwrap();
        assert_eq!(cfg.get("b"), Some(&b));
    }

    /// A plugin, as `load` gives it.
    fn plugin(name: &str, wasm: Wasm, read: &[(PathBuf, &str)], settings: &str) -> Plugin {
        Plugin {
            name: name.to_string(),
            wasm,
            read: (read.iter())
                .map(|(folder, path)| Read {
                    folder: folder.clone(),
                    path: path.to_string(),
                })
                .collect(),
            settings: settings.to_string(),
        }
    }

    #[test]
    fn plugins_cascade_plugin_by_plugin_and_setting_by_setting() {
        let theme_cfg = "plugins:\n  bob: ./plugins/bob.wasm\n  \
                         ann:\n    wasm: https://example.com/ann.wasm\n    style: plain\n    \
                         width: 80\n  old: ./old.wasm\n";
        let site = scratch(
            "plugins",
            &[
                ("theme/_templates/default.html", ""),
                ("theme/little.yml", theme_cfg),
            ],
        );
        // The site changes one of the theme's settings, turns one of its
        // plugins off, and adds its own, which reads two folders.
        let site_cfg = "theme: ./theme\nplugins:\n  ann:\n    style: fancy\n  old: false\n  \
                        cal:\n    wasm: tools/cal.wasm\n    read: [./src/, examples]\n    depth: 2\n";
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();

        // Each path leads from the little.yml that wrote it, and the plugin
        // is told the folders it reads as it sees them, in its settings.
        let url = Wasm::Url("https://example.com/ann.wasm".to_string());
        assert_eq!(
            load(&site).unwrap().plugins,
            [
                plugin(
                    "bob",
                    Wasm::File(site.join("theme/plugins/bob.wasm")),
                    &[],
                    "{}"
                ),
                plugin("ann", url, &[], r#"{"style":"fancy","width":80}"#),
                plugin(
                    "cal",
                    Wasm::File(site.join("tools/cal.wasm")),
                    &[
                        (site.join("src"), "src"),
                        (site.join("examples"), "examples")
                    ],
                    r#"{"read":["src","examples"],"depth":2}"#
                ),
            ]
        );

        // `read` can name one folder, like the site's own.
        let site_cfg = "plugins:\n  cal:\n    wasm: ./cal.wasm\n    read: .\n";
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
        assert_eq!(
            load(&site).unwrap().plugins,
            [plugin(
                "cal",
                Wasm::File(site.join("cal.wasm")),
                &[(site.clone(), ".")],
                r#"{"read":["."]}"#
            )]
        );

        // A site can name none.
        for site_cfg in ["", "plugins:\n", "plugins: {}\n"] {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            assert_eq!(load(&site).unwrap().plugins, [], "{site_cfg}");
        }
        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn plugins_must_be_named_with_their_wasm() {
        let site = scratch("bad-plugins", &[("theme/_templates/default.html", "")]);
        let file = site.join(CONFIG_FILE);
        let file = file.display();
        for (site_cfg, expected) in [
            (
                "plugins: [./bob.wasm]\n",
                format!(
                    "`plugins` in {file} must name each plugin and its `.wasm` file, like \
                     `bob: ./plugins/bob.wasm`"
                ),
            ),
            (
                "plugins:\n  bob: true\n",
                format!(
                    "plugin `bob` in {file} must be its `.wasm` file, its settings, or `false`"
                ),
            ),
            (
                "plugins:\n  bob:\n    style: plain\n",
                "plugin `bob` needs `wasm`, the path or URL of its `.wasm` file".to_string(),
            ),
            (
                "plugins:\n  bob: http://example.com/bob.wasm\n",
                format!(
                    "plugin `bob` in {file} is at `http://example.com/bob.wasm`, but little only \
                     downloads plugins over HTTPS"
                ),
            ),
            (
                "plugins:\n  bob:\n    wasm: [./bob.wasm]\n",
                format!(
                    "`wasm` of plugin `bob` in {file} must be the path or URL of its `.wasm` file"
                ),
            ),
            (
                "plugins:\n  bob:\n    wasm: ./bob.wasm\n    read: { src: true }\n",
                "`read` of plugin `bob` must be a folder, or a list of them, from the site's \
                 folder"
                    .to_string(),
            ),
            (
                "plugins:\n  bob:\n    wasm: ./bob.wasm\n    sizes: { ~: small }\n",
                "plugin `bob`'s settings must each be named, like `style: google`".to_string(),
            ),
        ] {
            let message = error(&site, site_cfg);
            assert!(message.starts_with(&expected), "{site_cfg}: {message}");
        }

        // Only the site chooses what a plugin reads, not a theme it uses.
        let theme_cfg = "plugins:\n  bob:\n    wasm: ./bob.wasm\n    read: /\n";
        std::fs::write(site.join("theme").join(CONFIG_FILE), theme_cfg).unwrap();
        let message = error(&site, "theme: ./theme\n");
        let theme_file = site.join("theme").join(CONFIG_FILE);
        let theme_file = theme_file.display();
        assert!(
            message.contains(&format!(
                "plugin `bob` in {theme_file} sets `read`, but only the site's little.yml can \
                 choose what a plugin reads"
            )),
            "{message}"
        );
        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn versions_have_titles_and_urls_and_are_built_from_refs() {
        let site = scratch("versions", &[]);
        let versions = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap().versions
        };
        let version = |title: &str, path: &str, git_ref: Option<&str>| Version {
            title: title.into(),
            path: path.into(),
            git_ref: git_ref.map(Into::into),
        };

        // A site without versions has just the one.
        for site_cfg in ["", "versions:\n", "versions: []\n"] {
            assert_eq!(versions(site_cfg), [], "{site_cfg}");
        }
        // In the menu's order, one at `/`, wherever it's listed, each from a
        // tag, a branch, or a commit, but for one, at most, from the site's
        // files as they are. A title is any text, and a `url` can end in `/`
        // or not. Each can have settings of its own, for the theme.
        let site_cfg = "versions:\n  - title: \"2.0 (Alpha)\"\n    url: /2.0-alpha\n    \
                        label: next\n  - title: 1.10 (Latest)\n    url: /\n    ref: v1.10\n  \
                        - title: 1.x\n    url: /versions/1.x/\n    ref: origin/1.x\n  \
                        - { title: Old docs, url: /old_docs, ref: \"1234567\" }\n";
        assert_eq!(
            versions(site_cfg),
            [
                version("2.0 (Alpha)", "/2.0-alpha", None),
                version("1.10 (Latest)", "", Some("v1.10")),
                version("1.x", "/versions/1.x", Some("origin/1.x")),
                version("Old docs", "/old_docs", Some("1234567")),
            ]
        );
        // A blank `ref` is none, and every version can be from a ref.
        let site_cfg = "versions:\n  - title: a\n    url: /\n    ref:\n  \
                        - { title: b, url: /b, ref: v1 }\n";
        assert_eq!(
            versions(site_cfg),
            [version("a", "", None), version("b", "/b", Some("v1"))]
        );
        let site_cfg = "versions:\n  - { title: a, url: /, ref: v2 }\n  \
                        - { title: b, url: /b, ref: v1 }\n";
        assert_eq!(
            versions(site_cfg),
            [version("a", "", Some("v2")), version("b", "/b", Some("v1"))]
        );
        // The version at `/` can have others in its folders, as can others
        // with the same start to their paths.
        let site_cfg = "versions:\n  - { title: a, url: / }\n  \
                        - { title: b, url: /versions/1, ref: v1 }\n  \
                        - { title: c, url: /versions/10, ref: v10 }\n";
        assert_eq!(
            versions(site_cfg),
            [
                version("a", "", None),
                version("b", "/versions/1", Some("v1")),
                version("c", "/versions/10", Some("v10")),
            ]
        );
        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn bad_versions_explain_themselves() {
        let site = scratch("bad-versions", &[]);
        let list = "`versions` must list the site's versions, each with its `title`, the `url` \
                    it's at, and the `ref` it's built from, like \
                    `- { title: \"1.4\", url: /1.4, ref: v1.4 }`";
        let title = "each version needs a `title`, in quotes if it looks like a number, like \
                     \"1.10\", which YAML would read as 1.1";
        let url = "version `a` needs a `url`, where it's published from the root of the site, \
                   like `/1.4`, or `/` for the site's home";
        let path = |title: &str| {
            format!(
                "version `{title}`'s `url` must be a path from the root of the site, like \
                 `/1.4` or `/versions/1.4`, with letters, digits, `.`, `-`, and `_` between \
                 its `/`s, each part starting with a letter or a digit, as they name the \
                 folders the version's built into"
            )
        };
        let git_ref = "version `a`'s `ref` must be the tag, branch, or commit it's built from, \
                       like v1.4, in quotes if it looks like a number";
        let home = "no version is at `/`, the site's home: give the one readers should land on \
                    `url: /`";
        for (site_cfg, expected) in [
            ("versions: \"2.0\"\n", list.to_string()),
            ("versions: { title: \"2.0\" }\n", list.to_string()),
            ("versions: [\"2.0\"]\n", list.to_string()),
            ("versions:\n  - { url: /, ref: v1.4 }\n", title.to_string()),
            (
                "versions:\n  - { name: \"1.4\", url: / }\n",
                title.to_string(),
            ),
            (
                "versions:\n  - { title: 1.10, url: / }\n",
                title.to_string(),
            ),
            (
                "versions:\n  - { title: \"\", url: / }\n",
                title.to_string(),
            ),
            (
                "versions:\n  - { title: \" \", url: / }\n",
                title.to_string(),
            ),
            ("versions:\n  - title: a\n", url.to_string()),
            ("versions:\n  - { title: a, url: }\n", url.to_string()),
            ("versions:\n  - { title: a, url: \"\" }\n", url.to_string()),
            ("versions:\n  - { title: a, url: 1.4 }\n", path("a")),
            ("versions:\n  - { title: a, url: \"1.4\" }\n", path("a")),
            ("versions:\n  - { title: a, url: // }\n", path("a")),
            ("versions:\n  - { title: a, url: /1.4// }\n", path("a")),
            ("versions:\n  - { title: a, url: /a//b }\n", path("a")),
            ("versions:\n  - { title: a, url: /.. }\n", path("a")),
            ("versions:\n  - { title: a, url: /a/../b }\n", path("a")),
            ("versions:\n  - { title: a, url: /.hidden }\n", path("a")),
            ("versions:\n  - { title: a, url: /one four }\n", path("a")),
            (
                "versions:\n  - { title: a, url: \"/1.4#top\" }\n",
                path("a"),
            ),
            (
                "versions:\n  - { title: a, url: \"https://example.com/1.4\" }\n",
                path("a"),
            ),
            ("versions:\n  - { title: a, url: [/] }\n", path("a")),
            (
                "versions:\n  - { title: \"1.4\", url: / }\n  \
                 - { title: \"1.4\", url: /1.4, ref: v1.4 }\n",
                "two versions are titled `1.4`".to_string(),
            ),
            (
                "versions:\n  - { title: a, url: /a }\n  - { title: b, url: /a/, ref: v1 }\n",
                "versions `a` and `b` are both at /a/".to_string(),
            ),
            (
                "versions:\n  - { title: a, url: / }\n  - { title: b, url: /, ref: v1 }\n",
                "versions `a` and `b` are both at /".to_string(),
            ),
            (
                "versions:\n  - { title: a, url: / }\n  - { title: b, url: /2.0, ref: v2 }\n  \
                 - { title: c, url: /2.0/beta, ref: v3 }\n",
                "version `c`, at /2.0/beta/, is inside version `b`, at /2.0/: only the version \
                 at `/` can have others inside it"
                    .to_string(),
            ),
            (
                "versions:\n  - { title: a, url: / }\n  - { title: c, url: /2.0/beta, ref: v3 }\n  \
                 - { title: b, url: /2.0, ref: v2 }\n",
                "version `c`, at /2.0/beta/, is inside version `b`, at /2.0/: only the version \
                 at `/` can have others inside it"
                    .to_string(),
            ),
            (
                "versions:\n  - { title: a, url: /a, ref: 1.4 }\n",
                git_ref.to_string(),
            ),
            (
                "versions:\n  - { title: a, url: /a, ref: \"\" }\n",
                git_ref.to_string(),
            ),
            (
                "versions:\n  - { title: a, url: /a, ref: --output=x }\n",
                git_ref.to_string(),
            ),
            (
                "versions:\n  - { title: a, url: / }\n  - { title: b, url: /b, ref: v1 }\n  \
                 - { title: c, url: /c }\n",
                "versions `a` and `c` both have no `ref`, but only one can be built from the \
                 site's files as they are: give the other the tag, branch, or commit it's built \
                 from"
                    .to_string(),
            ),
            ("versions:\n  - { title: a, url: /a }\n", home.to_string()),
            (
                "versions:\n  - { title: a, url: /a }\n  - { title: b, url: /b, ref: v1 }\n",
                home.to_string(),
            ),
        ] {
            assert_eq!(error(&site, site_cfg), expected, "{site_cfg}");
        }
        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn a_version_from_a_ref_has_its_own_config_in_the_sites_theme() {
        let theme_cfg = "name: Theme\nfont: serif\nplugins:\n  bob: ./bob.wasm\n";
        let old_cfg = "theme: ./elsewhere\nname: Old\nurl: https://old.example.com\n\
                       source: ./src\nversions: []\nnav: [{ title: Old, url: / }]\n\
                       plugins:\n  cal:\n    wasm: ./cal.wasm\n    read: ./lib\n";
        let site = scratch(
            "for-version",
            &[
                ("theme/little.yml", theme_cfg),
                ("theme/_templates/default.html", ""),
                ("old/little.yml", old_cfg),
            ],
        );
        let site_cfg = "theme: ./theme\nname: New\nurl: https://example.com/docs\n\
                        repo: https://github.com/example/docs\ndestination: ./public\n\
                        versions:\n  - { title: \"2.0\", url: / }\n  \
                        - { title: \"1.4\", url: /1.4, ref: v1.4 }\n";
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
        let config = load(&site).unwrap();
        let old = site.join("old");
        let file = "v1.4:old/little.yml";
        let version = config.for_version(&old, file).unwrap();

        // Its own config goes over the site's theme's, and its paths lead
        // from its own folder.
        assert_eq!(get(&version.values, "name"), Some("Old"));
        assert_eq!(get(&version.values, "font"), Some("serif"));
        let nav: Value = serde_yaml::from_str("[{ title: Old, url: / }]").unwrap();
        assert_eq!(version.values.get("nav"), Some(&nav));
        assert_eq!(version.source_dir, old.join("src"));
        assert_eq!(
            version.plugins,
            [
                plugin("bob", Wasm::File(site.join("theme/bob.wasm")), &[], "{}"),
                plugin(
                    "cal",
                    Wasm::File(old.join("cal.wasm")),
                    &[(old.join("lib"), "lib")],
                    r#"{"read":["lib"]}"#
                ),
            ]
        );
        // But what's the whole site's is the site's.
        assert_eq!(version.themes, config.themes);
        for key in SITE_WIDE {
            assert_eq!(version.values.get(key), config.values.get(key), "{key}");
        }
        assert_eq!(version.url, config.url);
        assert_eq!(version.repo, config.repo);
        assert_eq!(version.destination_dir, site.join("public"));
        assert_eq!(version.versions, config.versions);

        // Even what the site doesn't set: without a `url`, it has none.
        std::fs::write(
            site.join(CONFIG_FILE),
            site_cfg.replace("url: https", "description: https"),
        )
        .unwrap();
        let config = load(&site).unwrap();
        let version = config.for_version(&old, file).unwrap();
        assert_eq!(version.values.get("url"), None);
        assert_eq!(version.url, SiteUrl::default());

        // Without a little.yml, it has the theme's config.
        std::fs::remove_file(old.join(CONFIG_FILE)).unwrap();
        let version = config.for_version(&old, file).unwrap();
        assert_eq!(get(&version.values, "name"), Some("Theme"));
        assert_eq!(version.source_dir, old.join("docs"));

        // Its own little.yml is named in its errors as git names it.
        std::fs::write(old.join(CONFIG_FILE), "plugins: [./cal.wasm]\n").unwrap();
        let message = format!("{:#}", config.for_version(&old, file).unwrap_err());
        assert!(
            message.starts_with("`plugins` in v1.4:old/little.yml must name each plugin"),
            "{message}"
        );
        std::fs::write(old.join(CONFIG_FILE), "name: [Old\n").unwrap();
        let message = format!("{:#}", config.for_version(&old, file).unwrap_err());
        assert!(
            message.starts_with("parsing v1.4:old/little.yml: "),
            "{message}"
        );
        let _ = std::fs::remove_dir_all(&site);
    }
}
