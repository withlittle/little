//! The 3-level cascading config: little -> theme -> site, each a `little.yml`.
//! Each level deep-merges over the previous, so the *last* writer wins.
//! little's own file is compiled in; the theme's and the site's are optional.
//!
//! A `little.yml` holds two groups, and nothing else. `site` holds the
//! site's settings, which the theme gets as `site`. `page` holds settings
//! for every page, which each page's frontmatter goes over, as a level goes
//! over the one beneath it, so a page's cascade is little -> theme -> site
//! -> the page. The settings below are `site`'s.
//!
//! `theme` chooses the theme level, and holds how the site looks. Its
//! `enabled` is `true` for a theme, or `false` for none, and its `source` is
//! the path to the theme's folder, or the URL of its archive (see
//! `download`): without one, the theme is little's built-in one. A theme can
//! set them too, to build on another theme, whose settings go beneath its
//! own: a site whose theme builds on another cascades little -> that theme
//! -> its theme -> site. Its other settings are the look, like `mermaid`,
//! which holds Mermaid's own config, in Mermaid's own names, like `theme`
//! and `themeVariables`, which each diagram little draws starts from: little
//! sets none, so a theme can choose how diagrams look, and a site change it.
//!
//! `source` and `destination` name the folders a site is built from and
//! into. Any level can set them, but they're always relative to the site.
//!

//! `navigation` holds the site's nav, as `items`, if it sets one (see
//! `site`), and `prune`, whether each page's nav lists what's in a folder
//! only if the page is in it: little leaves it off, for a site of thousands
//! of pages to turn on.
//!
//! `serve` holds the `host` and `port` `little serve` serves the site at,
//! unless its `--host` and `--port` say otherwise.
//!
//! `url` is the address the site is published at, like
//! `https://example.github.io/my-project`. Its path, `/my-project`, goes
//! before each URL little gives the theme, and `little serve` serves the site
//! under it too.
//!
//! `repository` holds the `url` of the repository the site's source is in,
//! like `https://github.com/withlittle/little`, and the `branch` each page
//! links to its file on there.
//!
//! `search` holds `enabled`, whether each build indexes the site for a
//! search box (see `search`): little leaves it off, and a theme with a
//! search box turns it on. Its `exclude` lists the URLs of the pages it
//! leaves out, each with the pages under it, unless a page's own `search`
//! says otherwise.
//!
//! `markdown` holds a setting for each extension to CommonMark, like
//! `tables`, that turns it on or off: little turns on GitHub's, and a theme
//! or a site can change any of them. `math`, also GitHub's, chooses how math
//! is written, `mermaid` how diagrams are drawn, and `highlight` how code is.
//!
//! `plugins` names each plugin, with its settings: `wasm`, its `.wasm` file
//! (see `plugin`), by a path from the `little.yml` that names it, or an
//! `https://` URL; `read`, the folders it can read, which only the site can
//! set; `enabled`, which is `false` to turn a theme's plugin off; and the
//! plugin's own. They cascade plugin by plugin and setting by setting.
//!
//! `versions` lists the versions of the site, in the order its menu lists
//! them, each with the `title` readers see, the `url` it's published at, one
//! of them at `/`, and the git `ref` it's built from, but for one, at most,
//! that's built from the site's files as they are (see `site`). Every
//! version is built with the site's config, but one from a ref has its own
//! pages, from its `source` there, its own nav, and its own settings for
//! the site's plugins, which its entry can set (see `Config::for_version`).
//! Its entry can't set little's other settings, which are the site's, but any
//! other key in it is for the theme.
//!
//! `error` holds the `title` and `description` of the 404 page.
//!
//! `redirects` lists the site's redirects, each with `to`, the URL it leads
//! to, and `from`, the URLs that lead there, where a page used to be, which
//! the build writes a page at, to send its reader on (see `site`). One whose
//! `to` and `from` each end in `/*` is from every page under a folder that's
//! moved, to the same page under the folder it's now in.
//!
//! `page` has a group for each thing little works out for a page, which the
//! group's `enabled` turns off (see [`PageSettings`]): `outline`, its table
//! of contents, whose `depth` holds the levels of heading it lists, from
//! `min` to `max`, each from 1 for `#` to 6; `breadcrumb`, what's above it
//! in the nav; `previous` and `next`, the pages beside it there; and
//! `repository`, the links to its file in the site's repository. A page's
//! `search` holds `enabled`, whether search indexes it, whatever
//! `site.search`'s `exclude` says.

use anyhow::{Context, Result, anyhow, bail};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::{Deserialize, Deserializer, de::Error as _};
use serde_yaml::{Mapping, Value};
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};
use std::io::ErrorKind;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use crate::download;
use crate::theme::{self, Location};

/// The config file a theme and a site may each provide.
pub const CONFIG_FILE: &str = "little.yml";

/// A site's merged config, and what it chose.
#[derive(Clone, Debug)]
pub struct Config {
    /// The merged cascade of `site`, the site's settings: what the theme
    /// sees as `site`.
    pub site: Mapping,
    /// The merged cascade of `page`, every page's settings: what each page's
    /// frontmatter goes over.
    pub page: Mapping,
    /// The theme the site chose, then each theme it builds on in turn. Empty
    /// for a site whose `site.theme.enabled` is `false`.
    pub themes: Vec<Location>,
    /// The folder of Markdown files, and other files, the site is built from.
    pub source_dir: PathBuf,
    /// The folder the site is built into, which every build wipes.
    pub destination_dir: PathBuf,
    /// What `page` says of what little works out for every page, unless a
    /// page's own frontmatter says otherwise.
    pub page_settings: PageSettings,
    /// Whether each page's nav lists what's in a folder only if the page is
    /// in it, from `site.navigation.prune`.
    pub nav_prune: bool,
    /// The host `little serve` serves at, like `127.0.0.1`, from
    /// `site.serve.host`.
    pub host: String,
    /// The port `little serve` serves at, like 8080, from `site.serve.port`.
    pub port: u16,
    /// Where the site is published, from `site.url`.
    pub url: SiteUrl,
    /// The repository the site's source is in, from `site.repository`, if it
    /// sets its `url`.
    pub repo: Option<Repo>,
    /// What a build indexes for search, from `site.search`.
    pub search: Search,
    /// The extensions to CommonMark each page is written with, from
    /// `site.markdown`.
    pub markdown: Markdown,
    /// The plugins whose hooks the build calls, from `site.plugins`, in the
    /// order they're named.
    pub plugins: Vec<Plugin>,
    /// The versions of the site, from `site.versions`, in the order its menu
    /// lists them: none for a site that has just the one.
    pub versions: Vec<Version>,
    /// The site's redirects, from `site.redirects`, in the order they're
    /// listed: one for each URL an entry is `from`.
    pub redirects: Vec<Redirect>,
    /// little's defaults for `site`, with the settings of each theme over
    /// them: what a version of the site from a git ref puts the `source` and
    /// `navigation.items` of its own little.yml over.
    beneath: Mapping,
}

/// A version of a site, from `site.versions`.
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
    /// The folder its pages are in at its ref, from the site's folder there,
    /// like `./documentation`, if its `source` names one.
    pub source: Option<String>,
    /// Its nav, if its entry sets one, as `navigation.items`, with each URL
    /// from its home, like the site's.
    pub nav: Option<Value>,
    /// Its settings for the site's plugins, from its entry's `plugins`, like
    /// `api: { read: ./lib }`, or `api: { enabled: false }`: empty if it
    /// sets none.
    pub plugins: Mapping,
}

impl Version {
    /// The folders, one in another, of the destination it's built into,
    /// from its `path`: none for the version at `/`.
    pub fn folders(&self) -> impl Iterator<Item = &str> {
        self.path.split('/').filter(|folder| !folder.is_empty())
    }
}

/// One of a site's redirects, from `site.redirects`: a URL a page used to
/// be at, and where it leads now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redirect {
    /// The URL it's from, as it's written, from the root of each version,
    /// like `/guide/setup`, or, for every page under a folder, `/guide/*`.
    pub from: String,
    /// Where it leads, as it's written: a URL from the root of each
    /// version, like `/guide/install#usage`, another site's address, or, for
    /// one from every page under a folder, the folder they're now in, like
    /// `/manual/*`.
    pub to: String,
}

impl Redirect {
    /// The URL of the page or file it leads to, in its version, without any
    /// `?query` or `#fragment`, like `/guide/install`: `None` if it leads to
    /// another site.
    pub fn target(&self) -> Option<&str> {
        from_the_root(&self.to).then(|| self.to.split(['?', '#']).next().unwrap_or_default())
    }

    /// The folder it's from, and the one it leads to, each with the `/` at
    /// its end, like `/widgets/` and `/tools/`, if it's from every page
    /// under a folder, as `/widgets/*` is: `None` if it's from one URL.
    pub fn folders(&self) -> Option<(&str, &str)> {
        Some((every_page_under(&self.from)?, every_page_under(&self.to)?))
    }
}

/// The folder `url` is every page under, with the `/` at its end, like
/// `/widgets/` for `/widgets/*`: `None` if it's one URL, as it is with a `*`
/// at the end of its `?query` or `#fragment`.
fn every_page_under(url: &str) -> Option<&str> {
    let a_folder = |folder: &&str| {
        from_the_root(folder) && folder.ends_with('/') && !folder.contains(['*', '?', '#'])
    };
    url.strip_suffix('*').filter(a_folder)
}

/// A plugin a site uses, from `site.plugins`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plugin {
    /// Its name, like `bob`.
    pub name: String,
    /// Its `.wasm` file.
    pub wasm: Wasm,
    /// The folders it can read, from `read`.
    pub read: Vec<Read>,
    /// Its settings, all but `wasm` and `enabled`, as a JSON object, for its
    /// `setup`.
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SiteUrl {
    /// The scheme and host, like `https://example.github.io`; empty if the
    /// site doesn't set `url`.
    pub origin: String,
    /// The path the site is under, like `/my-project`, with no `/` at the
    /// end; empty for a site at the root of its domain.
    pub path: String,
    /// For a version of the site that isn't at its root, the theme's files,
    /// which only the version at the root has.
    pub shared: Option<Shared>,
}

/// The theme's files, which a site with versions writes once, for the
/// version at its root, for every version to share.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Shared {
    /// The path of the site's root, like `/my-project`, as `SiteUrl::path`.
    pub path: String,
    /// The URL of each file from the root of the site, like
    /// `/assets/style.css`.
    pub files: BTreeSet<String>,
}

impl Shared {
    /// Whether `url`, from the root of the site, is one of the files, after
    /// any `?query` or `#fragment`, with the characters encoded in it as
    /// they are in the file's name.
    fn has(&self, url: &str) -> bool {
        let path = url.split(['?', '#']).next().unwrap_or_default();
        self.files
            .contains(percent_decode_str(path).decode_utf8_lossy().as_ref())
    }
}

impl SiteUrl {
    /// `url` as the browser should follow it: a URL from the root of the
    /// site, like `/guide/`, gets the site's path, as `/my-project/guide/`,
    /// or, for one of the theme's files a version shares, the path of the
    /// site's root. Any other URL, like `https://…`, `//…`, `#…`, or
    /// `guide/`, is left as it is.
    pub fn link<'a>(&self, url: &'a str) -> Cow<'a, str> {
        if !from_the_root(url) {
            return Cow::Borrowed(url);
        }
        let path = match &self.shared {
            Some(shared) if shared.has(url) => &shared.path,
            _ => &self.path,
        };
        if path.is_empty() {
            return Cow::Borrowed(url);
        }
        Cow::Owned(format!("{path}{url}"))
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

/// The repository a site's source is in, from `site.repository`: its `url`,
/// and the `branch` its pages link to their files on.
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
        let (source, edit) = match url::Url::parse(&self.url).ok()?.host_str()? {
            "github.com" => ("blob", "edit"),
            "gitlab.com" => ("-/blob", "-/edit"),
            _ => return None,
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

    /// Its owner, if it has one, and its name, from its address: the last
    /// part of the address's path is its name, and what's before it its
    /// owner, like `withlittle` and `little`, or, for a project in a group
    /// on GitLab, `withlittle/docs` and `little`.
    pub fn owner_and_name(&self) -> (Option<String>, String) {
        let path = (url::Url::parse(&self.url).ok())
            .map(|url| url.path().trim_matches('/').to_string())
            .unwrap_or_default();
        match path.rsplit_once('/') {
            Some((owner, name)) => (Some(owner.to_string()), name.to_string()),
            None => (None, path),
        }
    }
}

/// What a build indexes for search, from `site.search`: if it's `enabled`,
/// every page but those `exclude` leaves out, unless a page's own `search`
/// says otherwise.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Search {
    /// Whether a build indexes the site at all.
    pub enabled: bool,
    /// The URLs of the pages it leaves out, each with the pages under it, as
    /// they're written, from the root of each version, like `/changelog/`.
    pub exclude: Vec<String>,
}

impl Search {
    /// Whether `exclude` leaves out the page at `url`, from the root of its
    /// version, like `/guide/install`.
    pub fn excludes(&self, url: &str) -> bool {
        self.exclude.iter().any(|entry| leaves_out(entry, url))
    }
}

/// Whether `entry`, a URL in `site.search.exclude`, leaves out the page at
/// `url`: the page at its URL, with or without the `/` at its end, as
/// `guide.md` and `guide/README.md` would be the same page, and every page
/// under it.
pub fn leaves_out(entry: &str, url: &str) -> bool {
    let (entry, url) = (entry.trim_end_matches('/'), url.trim_end_matches('/'));
    (url.strip_prefix(entry)).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// What a page's settings say of what little works out for it: whether it
/// has each thing, from the `enabled` of its group, and which headings its
/// table of contents lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageSettings {
    /// Whether it has a table of contents, from `outline.enabled`.
    pub outline: bool,
    /// The levels of heading its table of contents lists, from
    /// `outline.depth.min` to `outline.depth.max`.
    pub depths: RangeInclusive<u8>,
    /// Whether it has breadcrumbs, from `breadcrumb.enabled`.
    pub breadcrumb: bool,
    /// Whether it links to the page before it, from `previous.enabled`.
    pub previous: bool,
    /// Whether it links to the page after it, from `next.enabled`.
    pub next: bool,
    /// Whether it links to its file in the site's repository, from
    /// `repository.enabled`.
    pub repository: bool,
    /// Whether search indexes it, from `search.enabled`, if it's set: left
    /// unset, `site.search.exclude` says.
    pub search: Option<bool>,
}

impl PageSettings {
    /// Read `page`, a page's settings: every page's, from `page` in the
    /// config, or one page's, with its frontmatter over them. An error names
    /// each setting after `prefix`: `page.`, for the config's, or nothing,
    /// for a page's own, as its frontmatter writes it.
    pub fn read(page: &Mapping, prefix: &str) -> Result<PageSettings> {
        let outline = page_group(page, prefix, "outline")?;
        let depth = |key: &str| -> Result<u8> {
            let level = (outline.get("depth")).and_then(|depth| depth.get(key));
            match level.and_then(Value::as_u64) {
                Some(level @ 1..=6) => Ok(level as u8),
                _ => bail!(
                    "`{prefix}outline.depth.{key}` must be a heading level from 1 to 6, like 3 \
                     for `###`"
                ),
            }
        };
        // The min and max can be set at different levels of the cascade, so
        // a min over the max, which would leave the table of contents empty,
        // is an error.
        let (min, max) = (depth("min")?, depth("max")?);
        if min > max {
            bail!(
                "`{prefix}outline.depth.min` ({min}) can't be more than \
                 `{prefix}outline.depth.max` ({max})"
            );
        }
        let search = match page.get("search") {
            None => None,
            Some(Value::Mapping(search)) => match search.get("enabled") {
                None => None,
                Some(Value::Bool(enabled)) => Some(*enabled),
                Some(_) => bail!(
                    "`{prefix}search.enabled` must be `true`, to index the page for search, or \
                     `false`, to leave it out"
                ),
            },
            Some(_) => bail!("`{prefix}search` must be its settings, like `enabled: false`"),
        };
        Ok(PageSettings {
            outline: enabled(page, prefix, "outline")?,
            depths: min..=max,
            breadcrumb: enabled(page, prefix, "breadcrumb")?,
            previous: enabled(page, prefix, "previous")?,
            next: enabled(page, prefix, "next")?,
            repository: enabled(page, prefix, "repository")?,
            search,
        })
    }
}

/// The settings in a page's group `name`, like `outline`. little's own
/// config sets each group, so anything else there, like `false`, is a
/// mistake.
fn page_group<'a>(page: &'a Mapping, prefix: &str, name: &str) -> Result<&'a Mapping> {
    match page.get(name) {
        Some(Value::Mapping(settings)) => Ok(settings),
        _ => bail!("`{prefix}{name}` must be its settings, like `enabled: false`"),
    }
}

/// Whether a page has what its group `name` is for, from the group's
/// `enabled`. YAML reads `yes` and `"true"` as text, not `true`, so they're
/// errors rather than a guess.
fn enabled(page: &Mapping, prefix: &str, name: &str) -> Result<bool> {
    match page_group(page, prefix, name)?.get("enabled") {
        Some(Value::Bool(enabled)) => Ok(*enabled),
        _ => bail!("`{prefix}{name}.enabled` must be `true` or `false`"),
    }
}

/// Which extensions to CommonMark each page is written with, how its math is
/// written, how its diagrams are drawn, and which classes its code is
/// highlighted with, from the settings under `site.markdown`, each named
/// for what an author writes, as `src/little.yml` describes them; and how
/// its diagrams look, from `site.theme.mermaid`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, expecting = "settings, like `mark: true`")]
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
    #[serde(deserialize_with = "or_false")]
    pub math: Option<Math>,
    /// Mermaid's config, from `site.theme.mermaid`, as JSON, as Mermaid
    /// reads it, if a Mermaid block is drawn as an `<svg>` as little builds
    /// the page, rather than left as code.
    #[serde(deserialize_with = "svg_or_false")]
    pub mermaid: Option<serde_json::Value>,
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
    #[serde(deserialize_with = "or_false")]
    pub highlight: Option<Highlight>,
}

/// How math is written: as MathML, which browsers show as it is, or as its
/// TeX, marked with `data-math-style` for a theme's script to render.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Math {
    MathML,
    Tex,
}

/// A set of classes for highlighted code: Pygments', like `k` for a keyword,
/// or GitHub's, like `pl-k`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Highlight {
    Pygments,
    GitHub,
}

/// A setting that's one of `T`'s names, or `false`, for none: not `true`, as
/// a site's would undo its theme's choice.
fn or_false<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    match Value::deserialize(d)? {
        Value::Bool(false) => Ok(None),
        // As it's written, so `true` is a name `T` doesn't have, beside those it has.
        value => serde_yaml::to_string(&value)
            .and_then(|name| T::deserialize(Value::String(name.trim_end().into())))
            .map(Some)
            .map_err(D::Error::custom),
    }
}

/// `svg`, as config for `markdown` to fill in from `site.theme.mermaid`, or
/// `false`.
fn svg_or_false<'de, D: Deserializer<'de>>(d: D) -> Result<Option<serde_json::Value>, D::Error> {
    match Value::deserialize(d)? {
        Value::String(form) if form == "svg" => Ok(Some(serde_json::Value::Null)),
        Value::Bool(false) => Ok(None),
        _ => Err(D::Error::custom("expected `svg` or `false`")),
    }
}

impl Default for Markdown {
    /// little's own settings, from `src/little.yml`.
    fn default() -> Self {
        markdown(&builtin_defaults().site).expect("src/little.yml sets each Markdown extension")
    }
}

/// What one `little.yml` sets: a level of the cascade.
#[derive(Clone, Debug, Default)]
struct Level {
    /// Its `site`: the site's settings.
    site: Mapping,
    /// Its `page`: every page's settings, less any left blank.
    page: Mapping,
}

/// The two groups of `cfg`, the settings of the `little.yml` that errors
/// call `file`. A key beside them is an error, rather than a setting that
/// does nothing, and so is a group that isn't settings. A group left blank
/// has none.
fn level(cfg: Mapping, file: &str) -> Result<Level> {
    let mut level = Level::default();
    for (key, value) in cfg {
        let (group, holds) = match key.as_str() {
            Some("site") => (&mut level.site, "the site's settings, like `name: My Docs`"),
            Some("page") => (
                &mut level.page,
                "every page's settings, like `breadcrumb: { enabled: false }`",
            ),
            _ => bail!(
                "{file} sets `{}` beside `site` and `page`, but a {CONFIG_FILE} holds only those \
                 two: put it under `site`, with the site's settings, or under `page`, with every \
                 page's",
                written(&key)
            ),
        };
        match value {
            Value::Null => {}
            Value::Mapping(settings) => *group = settings,
            _ => bail!("`{}` in {file} must hold {holds}", written(&key)),
        }
    }
    // A key left blank is as good as not there, as in a page's frontmatter.
    level.page.retain(|_, value| !value.is_null());
    Ok(level)
}

/// little's built-in defaults: level 1 of the cascade. They live in
/// `src/little.yml`, compiled into the binary so little stays a single file
/// with nothing to install alongside it.
fn builtin_defaults() -> Level {
    let cfg = parse_config(include_str!("little.yml")).expect("src/little.yml holds settings");
    level(cfg, "src/little.yml").expect("src/little.yml holds `site` and `page`")
}

/// The built-in theme's settings, from `theme/little.yml`, compiled in too.
fn builtin_theme() -> Level {
    let cfg = parse_config(theme::builtin::CONFIG).expect("theme/little.yml holds settings");
    level(cfg, "theme/little.yml").expect("theme/little.yml holds `site` and `page`")
}

/// Follow the chain of themes from the one the site chooses, then merge the
/// cascade.
pub fn load(site_dir: &Path) -> Result<Config> {
    let defaults = builtin_defaults();
    let site_file = site_dir.join(CONFIG_FILE);
    let site_name = site_file.display().to_string();
    let mut own = read_level(&site_file, &site_name)?;
    plugin_files(&mut own.site, &site_name, site_dir, true)?;

    // The site's choice of theme, over little's: its built-in one.
    let chosen = theme_choice(&own.site, &site_name)?;
    let littles = theme_choice(&defaults.site, "src/little.yml")?;
    let enabled = chosen.enabled.or(littles.enabled).unwrap_or(true);
    let mut next = match (enabled, &chosen.source) {
        (false, _) => None,
        (true, Some(source)) => Some(locate(source, site_dir, &site_name, site_dir)?),
        (true, None) => Some(Location::BuiltIn),
    };
    let mut themes = Vec::new();
    let mut levels = Vec::new();
    // Each theme folder's real path, to catch themes that build on each other
    // in a loop however their paths are written.
    let mut seen = Vec::new();
    while let Some(location) = next {
        let level = match location.folder() {
            None => {
                next = None; // the built-in theme builds on nothing
                builtin_theme()
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
                let mut level = read_level(&dir.join(CONFIG_FILE), &file)?;
                plugin_files(&mut level.site, &file, dir, false)?;
                // A theme builds on another only if it says so: on the one
                // at its `source`, or, with `enabled: true` alone, on
                // little's built-in one.
                let builds_on = theme_choice(&level.site, &file)?;
                next = match (builds_on.enabled, builds_on.source) {
                    (Some(false), _) | (None, None) => None,
                    (_, Some(source)) => Some(locate(&source, dir, &file, site_dir)?),
                    (Some(true), None) => Some(Location::BuiltIn),
                };
                level
            }
        };
        themes.push(location);
        levels.push(level);
    }

    // The theme at the end of the chain goes beneath the ones that build on it.
    let Level { mut site, mut page } = defaults;
    for level in levels.into_iter().rev() {
        merge(&mut site, level.site);
        merge(&mut page, level.page);
    }
    let beneath = site.clone();
    merge(&mut site, own.site);
    merge(&mut page, own.page);
    // `site.theme` says which theme the site is in, as the site chose it,
    // not which theme that one builds on.
    if let Some(Value::Mapping(theme)) = site.get_mut("theme") {
        theme.insert("enabled".into(), enabled.into());
        match chosen.source {
            Some(source) => theme.insert("source".into(), source.into()),
            None => theme.shift_remove("source"),
        };
    }
    settle(site_dir, site, page, beneath, themes)
}

impl Config {
    /// Both groups, `site` and `page`, as a `little.yml` holds them: what
    /// `little config` prints, and what a plugin's `config` hook is given.
    pub fn groups(&self) -> Mapping {
        Mapping::from_iter([
            ("site".into(), Value::Mapping(self.site.clone())),
            ("page".into(), Value::Mapping(self.page.clone())),
        ])
    }

    /// The config of `version`, from a git ref, whose site's folder is at
    /// `site_dir`, as it was at the ref, and whose little.yml errors call
    /// `file`, like `v1.4:docs/little.yml`. It's the site's, with plugins
    /// that read their folders there, but for the version's own `source` and
    /// nav: as the `site` of its little.yml there sets them, as `source`
    /// and `navigation.items`, over little's defaults and the site's themes,
    /// as the site's own would. Nothing else in that little.yml counts, so
    /// it isn't checked. Without one, it's from before the site was built
    /// with little, so its pages are in the site's `source`. Its entry's
    /// `source` and `navigation.items` win over both, and its `plugins` go
    /// over the site's.
    pub fn for_version(&self, version: &Version, site_dir: &Path, file: &str) -> Result<Config> {
        let own = site_dir.join(CONFIG_FILE);
        let mut theirs = self.beneath.clone();
        let source = if own.is_file() {
            if let Some(Value::Mapping(site)) = read_config(&own, file)?.shift_remove("site") {
                merge(&mut theirs, site);
            }
            theirs.get("source")
        } else {
            self.site.get("source")
        };
        let source = match &version.source {
            Some(source) => Some(Value::from(source.as_str())),
            None => source.cloned(),
        };
        let nav = (version.nav.as_ref())
            .or_else(|| theirs.get("navigation")?.get("items"))
            .cloned();
        let mut site = self.site.clone();
        match source {
            Some(source) => site.insert("source".into(), source),
            None => site.shift_remove("source"),
        };
        // Its own nav, or none, for little to build one from its folders.
        if let Some(Value::Mapping(navigation)) = site.get_mut("navigation") {
            match nav {
                Some(nav) => navigation.insert("items".into(), nav),
                None => navigation.shift_remove("items"),
            };
        }
        // Each plugin its entry names is one the site uses, so its settings
        // go over the site's, as the site's go over its theme's.
        if !version.plugins.is_empty() {
            let plugins = Value::Mapping(version.plugins.clone());
            merge(&mut site, Mapping::from_iter([("plugins".into(), plugins)]));
        }
        Ok(Config {
            source_dir: folder(site_dir, &site, "source")?,
            plugins: plugins(&site, site_dir)?,
            site,
            ..self.clone()
        })
    }

    /// The config of the site at `site_dir`, with `groups`, its `site` and
    /// `page`, in place of its own, as a plugin's `config` hook gives them
    /// back. Errors, which follow ``plugin `name`'s `config` hook``, if they
    /// change one of [`FIXED`], or little can't use them.
    pub fn hooked(&self, site_dir: &Path, groups: Mapping) -> Result<Config> {
        let unusable = |e: anyhow::Error| anyhow!("gives back a config little can't use: {e:#}");
        let Level { mut site, page } =
            level(groups, "the config it gives back").map_err(unusable)?;
        for path in FIXED {
            let own = at(&self.site, path);
            if at(&site, path) != own {
                let fixed: Vec<String> = (FIXED.iter())
                    .map(|path| format!("`site.{}`", path.join(".")))
                    .collect();
                let (last, rest) = fixed.split_last().expect("some settings are fixed");
                bail!(
                    "changes `site.{}`, but plugins can't change {}, or {last}",
                    path.join("."),
                    rest.join(", ")
                );
            }
            // As it was, in its order, which a plugin's JSON may not keep.
            if let (&[key], Some(own)) = (path, own) {
                site.insert(key.into(), own.clone());
            }
        }
        settle(
            site_dir,
            site,
            page,
            self.beneath.clone(),
            self.themes.clone(),
        )
        .map_err(unusable)
    }
}

/// The settings a plugin's `config` hook can't change, each by its path in
/// `site`, which little reads before plugins start, and `little serve` reads
/// from the site's own config: the theme and plugins it uses, the folders it
/// reads and writes, and where it serves.
pub const FIXED: [&[&str]; 6] = [
    &["theme", "enabled"],
    &["theme", "source"],
    &["plugins"],
    &["source"],
    &["destination"],
    &["serve"],
];

/// The setting at `path` in `settings`, like `["theme", "source"]`, if it's
/// there.
fn at<'a>(settings: &'a Mapping, path: &[&str]) -> Option<&'a Value> {
    let (first, rest) = path.split_first()?;
    (rest.iter()).try_fold(settings.get(*first)?, |value, key| value.get(*key))
}

/// Check the merged settings of the site at `site_dir`, its `site` and its
/// `page`, which is in `themes`, over `beneath`, and work out what they
/// choose.
fn settle(
    site_dir: &Path,
    site: Mapping,
    page: Mapping,
    beneath: Mapping,
    themes: Vec<Location>,
) -> Result<Config> {
    let source_dir = folder(site_dir, &site, "source")?;
    let destination_dir = folder(site_dir, &site, "destination")?;
    let page_settings = PageSettings::read(&page, "page.")?;
    let nav_prune = nav_prune(&site)?;
    let host = host(&site)?;
    let port = port(&site)?;
    let url = site_url(&site)?;
    let repo = repo(&site)?;
    let search = search(&site)?;
    let markdown = markdown(&site)?;
    let plugins = plugins(&site, site_dir)?;
    let versions = versions(&site, &plugins)?;
    let redirects = redirects(&site)?;
    error_page(&site)?;
    Ok(Config {
        site,
        page,
        themes,
        source_dir,
        destination_dir,
        page_settings,
        nav_prune,
        host,
        port,
        url,
        repo,
        search,
        markdown,
        plugins,
        versions,
        redirects,
        beneath,
    })
}

/// The settings in `site`'s group `name`, like `serve`. little's own config
/// sets each group, so anything else there, like `true`, or nothing, is a
/// mistake, and the error ends with `like`, what to write instead.
fn group<'a>(site: &'a Mapping, name: &str, like: &str) -> Result<&'a Mapping> {
    match site.get(name) {
        Some(Value::Mapping(settings)) => Ok(settings),
        _ => bail!("`site.{name}` must be its settings, {like}"),
    }
}

/// What a `little.yml` says of the theme, in its `site.theme`: whether
/// there's one, and where it is, if it says either.
struct ThemeChoice {
    enabled: Option<bool>,
    source: Option<String>,
}

/// What the settings in `site`, from `file`, say of the theme, in
/// `site.theme`, beside how the site looks.
fn theme_choice(site: &Mapping, file: &str) -> Result<ThemeChoice> {
    let settings = match site.get("theme") {
        None => {
            return Ok(ThemeChoice {
                enabled: None,
                source: None,
            });
        }
        Some(Value::Mapping(settings)) => settings,
        Some(_) => bail!(
            "`site.theme` in {file} must be its settings, like `source: ./my-theme`, for the \
             path to a theme's folder, or the URL of its archive, or `enabled: false`, for no \
             theme"
        ),
    };
    let enabled = match settings.get("enabled") {
        None => None,
        Some(Value::Bool(enabled)) => Some(*enabled),
        Some(_) => bail!("`site.theme.enabled` in {file} must be `true`, or `false`, for no theme"),
    };
    let source = match settings.get("source") {
        None => None,
        Some(Value::String(source)) => Some(source.clone()),
        Some(_) => bail!(
            "`site.theme.source` in {file} must be the path to a theme's folder, or the URL of \
             a theme's archive"
        ),
    };
    Ok(ThemeChoice { enabled, source })
}

/// The theme at `source`, a `site.theme.source`: the path to its folder, or
/// the URL of its archive. `file` names the file that sets it, and `dir` is
/// its folder: a path is relative to that, as a theme can't know where the
/// sites that use it are. A theme downloaded from a URL is kept in the site
/// at `site_dir`.
fn locate(source: &str, dir: &Path, file: &str, site_dir: &Path) -> Result<Location> {
    if source.starts_with("https://") {
        let dir = download::theme(site_dir, source)
            .map_err(|e| anyhow!("{file} sets `site.theme.source` to `{source}`, but {e:#}"))?;
        return Ok(Location::Downloaded {
            url: source.to_string(),
            dir,
        });
    }
    if source.starts_with("http://") {
        bail!(
            "{file} sets `site.theme.source` to `{source}`, but little only downloads themes \
             over HTTPS"
        );
    }
    let folder = resolve(dir, source);
    if !folder.is_dir() {
        bail!(
            "{file} sets `site.theme.source` to `{source}`, but there's no folder at {}",
            folder.display()
        );
    }
    Ok(Location::Folder(folder))
}

/// The folder a setting names. It's relative to the site even when a theme
/// sets it: it's the site that's built.
fn folder(site_dir: &Path, site: &Mapping, key: &str) -> Result<PathBuf> {
    match site.get(key) {
        Some(Value::String(path)) => Ok(resolve(site_dir, path)),
        _ => bail!("`site.{key}` must be the path to a folder"),
    }
}

/// The settings in `site.navigation`: `prune`, and the site's nav, as
/// `items`, if it sets one.
fn navigation(site: &Mapping) -> Result<&Mapping> {
    let like = "like `prune: true`, or `items`, the site's nav, a list of links";
    group(site, "navigation", like)
}

/// Whether each page's nav lists what's in a folder only if the page is in
/// it, from `site.navigation.prune`. YAML reads `yes` and `"true"` as text,
/// not `true`, so they're errors rather than a guess, as for `site.search`.
fn nav_prune(site: &Mapping) -> Result<bool> {
    match navigation(site)?.get("prune") {
        Some(Value::Bool(prune)) => Ok(*prune),
        _ => bail!(
            "`site.navigation.prune` must be `true`, for each page's nav to list what's in a \
             folder only if the page is in it, or `false`"
        ),
    }
}

/// The settings in `site.serve`: the `host` and `port` `little serve`
/// serves at.
fn serve(site: &Mapping) -> Result<&Mapping> {
    group(site, "serve", "like `port: 8080`")
}

/// The host `little serve` serves at: a host name, like `localhost`, or an IP
/// address, like `127.0.0.1` or `::1`.
fn host(site: &Mapping) -> Result<String> {
    match serve(site)?.get("host") {
        Some(Value::String(host)) if !host.is_empty() => Ok(host.clone()),
        _ => bail!("`site.serve.host` must be a host name or IP address, like 127.0.0.1"),
    }
}

/// The port `little serve` serves at. Port 0 would have the system pick one,
/// and the URL little prints would be wrong.
fn port(site: &Mapping) -> Result<u16> {
    match serve(site)?.get("port").and_then(Value::as_u64) {
        Some(port @ 1..=65535) => Ok(port as u16),
        _ => bail!("`site.serve.port` must be a number from 1 to 65535, like 8080"),
    }
}

/// Where the site is published, from `site.url`: its scheme and host, then
/// the path the site is under.
fn site_url(site: &Mapping) -> Result<SiteUrl> {
    let Some(value) = site.get("url") else {
        return Ok(SiteUrl::default());
    };
    let error = || {
        anyhow!(
            "`site.url` must be the address the site is published at, starting with https:// \
             or http://, like https://example.github.io/my-project"
        )
    };
    let url = value.as_str().and_then(web_address).ok_or_else(error)?;
    Ok(SiteUrl {
        origin: url.origin().ascii_serialization(),
        path: url.path().trim_end_matches('/').to_string(),
        shared: None,
    })
}

/// `url`, if it's a whole address on the web: `https://` or `http://`, a
/// host, and a path, with no `?`, `#`, or spaces. `Url` alone would read
/// `https:///docs` as the host `docs`, and encode a space.
fn web_address(url: &str) -> Option<url::Url> {
    let rest = (url.strip_prefix("https://")).or_else(|| url.strip_prefix("http://"))?;
    let bad = |c: char| c == '?' || c == '#' || c.is_whitespace();
    (!rest.starts_with('/') && !rest.contains(bad)).then(|| url::Url::parse(url).ok())?
}

/// The repository the site's source is in, from `site.repository`: its
/// `url`, if it sets one, and the `branch` its pages link to their files
/// on. YAML reads a branch like `2.0` as a number, so it must be quoted, or
/// it would be linked as `2`.
fn repo(site: &Mapping) -> Result<Option<Repo>> {
    let like = "like `url: https://github.com/example/my-project`";
    let settings = group(site, "repository", like)?;
    let branch = match settings.get("branch") {
        Some(Value::String(branch)) if !branch.is_empty() => branch.clone(),
        _ => bail!(
            "`site.repository.branch` must be the name of a branch, like main, in quotes if it \
             looks like a number, like \"2.0\""
        ),
    };
    let Some(value) = settings.get("url") else {
        return Ok(None);
    };
    let error = || {
        anyhow!(
            "`site.repository.url` must be the address of the site's repository, as a browser \
             shows it, like https://github.com/example/my-project"
        )
    };
    let url = value.as_str().ok_or_else(error)?.trim_end_matches('/');
    // Not the address it's cloned from, which ends in `.git`.
    match web_address(url) {
        Some(parsed) if parsed.path() != "/" && !url.ends_with(".git") => Ok(Some(Repo {
            url: url.to_string(),
            branch,
        })),
        _ => Err(error()),
    }
}

/// What a build indexes for search, from `site.search`: whether it's
/// `enabled`, and the URLs it leaves out, in `exclude`. YAML reads `yes` and
/// `"true"` as text, not `true`, so they're errors rather than a guess, and
/// so is a setting it hasn't, like `excludes`, which would leave nothing
/// out.
fn search(site: &Mapping) -> Result<Search> {
    let like = "like `enabled: true`, or `exclude: [/changelog/]`";
    let settings = group(site, "search", like)?;
    let mut search = Search::default();
    for (key, value) in settings {
        match key.as_str() {
            Some("enabled") => {}
            Some("exclude") => search.exclude = left_out(value)?,
            _ => bail!(
                "`site.search` has no setting `{}`: its settings are `enabled` and `exclude`",
                written(key)
            ),
        }
    }
    search.enabled = match settings.get("enabled") {
        Some(Value::Bool(enabled)) => *enabled,
        _ => bail!(
            "`site.search.enabled` must be `true`, to index the site for a search box, or \
             `false`"
        ),
    };
    Ok(search)
}

/// The URLs `site.search.exclude` lists, as they're written, or none, if
/// it's left empty. Each must be a URL, not a file's path, as a page a
/// plugin adds has no file.
fn left_out(value: &Value) -> Result<Vec<String>> {
    let urls = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Sequence(urls) => urls,
        _ => {
            bail!("`site.search.exclude` must be a list of URLs, like `[/changelog/, /guide/old]`")
        }
    };
    (urls.iter())
        .map(|url| match url {
            Value::String(url) if url.starts_with('/') => Ok(url.clone()),
            _ => bail!(
                "`site.search.exclude` lists `{}`, but each must be a page's URL from the root \
                 of the site, as `site.navigation.items` writes it, like `/guide/` or \
                 `/guide/install`",
                written(url)
            ),
        })
        .collect()
}

/// `value` as YAML writes it, to name it in an error.
fn written(value: &Value) -> String {
    let yaml = serde_yaml::to_string(value).unwrap_or_default();
    yaml.trim_end().to_string()
}

/// The extensions to CommonMark each page is written with, each `true` or
/// `false`, how its math is written, how its diagrams are drawn, and the
/// classes its code is highlighted with, from the settings under
/// `site.markdown`. little's own config sets every one, so any other is a
/// typo. Diagrams drawn as SVG look as `site.theme.mermaid` says.
fn markdown(site: &Mapping) -> Result<Markdown> {
    let settings = site.get("markdown").cloned().unwrap_or_default();
    let mut markdown: Markdown =
        serde_path_to_error::deserialize(settings).context("reading `site.markdown`")?;
    if markdown.mermaid.is_some() {
        markdown.mermaid = Some(mermaid(site)?);
    }
    // comrak reads `^[…]` only with footnotes, so it'd be left as text.
    if markdown.inline_footnotes && !markdown.footnotes {
        bail!("`site.markdown.inline_footnotes` needs `site.markdown.footnotes` to be `true` too");
    }
    // merman doesn't clean every diagram, so one can carry HTML into the page.
    if markdown.mermaid.is_some() && !markdown.raw_html {
        bail!(
            "`site.markdown.mermaid: svg` needs `site.markdown.raw_html` to be `true`, as a \
             diagram can carry HTML: set `site.markdown.mermaid: false` too"
        );
    }
    Ok(markdown)
}

/// Mermaid's config, which each diagram starts from, from the settings under
/// `site.theme.mermaid`, as they're written, in Mermaid's own names. Mermaid
/// ignores a setting it doesn't have, as in a diagram's own config, so
/// little checks only for the mistakes that change every diagram without a
/// word: a theme Mermaid doesn't have, which it swaps for its default, and
/// an empty setting, as a colour like #6f42c1 is without quotes.
fn mermaid(site: &Mapping) -> Result<serde_json::Value> {
    let settings = match site.get("theme").and_then(|theme| theme.get("mermaid")) {
        None => return Ok(serde_json::json!({})),
        Some(Value::Mapping(settings)) => settings,
        Some(_) => {
            bail!("`site.theme.mermaid` must be Mermaid's settings, like `theme: forest`")
        }
    };
    let themes = merman::supported_themes();
    if let Some(theme) = settings.get("theme")
        && !theme.as_str().is_some_and(|theme| themes.contains(&theme))
    {
        bail!(
            "`site.theme.mermaid.theme` must be one of Mermaid's themes: {}",
            themes.join(", ")
        );
    }
    if let Some(path) = empty_setting(settings, "site.theme.mermaid") {
        bail!(
            "`{path}` is empty: put a colour in quotes, like \"#6f42c1\", or YAML reads it as \
             a comment"
        );
    }
    serde_json::to_value(settings).context("reading `site.theme.mermaid`")
}

/// The first setting in `settings`, or in the settings within them, that's
/// empty, named from `path`, like
/// `site.theme.mermaid.themeVariables.primaryColor`.
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

/// Check each plugin the settings in `site`, from `file`, in `dir`, name,
/// each with its own settings, and lead its `wasm`, if it's a path, from
/// `dir`, so each path leads from the `little.yml` that wrote it, once the
/// levels merge, setting by setting. Only the site's config, `is_site`, can
/// set `read`.
fn plugin_files(site: &mut Mapping, file: &str, dir: &Path, is_site: bool) -> Result<()> {
    let must_name = || {
        anyhow!(
            "`site.plugins` in {file} must name each plugin, with its settings, like \
             `bob: {{ wasm: ./plugins/bob.wasm }}`"
        )
    };
    let plugins = match site.get_mut("plugins") {
        None => return Ok(()),
        Some(Value::Mapping(plugins)) => plugins,
        // `plugins:` with nothing after it.
        Some(Value::Null) => {
            site.shift_remove("plugins");
            return Ok(());
        }
        Some(_) => return Err(must_name()),
    };
    for (name, plugin) in plugins.iter_mut() {
        let name = name.as_str().ok_or_else(must_name)?;
        let Value::Mapping(settings) = plugin else {
            bail!(
                "plugin `{name}` in {file} must be its settings, like \
                 `wasm: ./plugins/{name}.wasm`, for its `.wasm` file, or `enabled: false`, to \
                 turn it off"
            );
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

/// The plugins the merged settings in `site` name, but those turned off,
/// with `enabled: false`.
fn plugins(site: &Mapping, site_dir: &Path) -> Result<Vec<Plugin>> {
    let Some(Value::Mapping(plugins)) = site.get("plugins") else {
        return Ok(Vec::new());
    };
    let mut on = Vec::new();
    for (name, settings) in plugins {
        let name = name.as_str().expect("each plugin has a name").to_string();
        let Value::Mapping(settings) = settings else {
            bail!("plugin `{name}` must be its settings, like `wasm: ./plugins/{name}.wasm`");
        };
        match settings.get("enabled") {
            None | Some(Value::Bool(true)) => {}
            Some(Value::Bool(false)) => continue,
            Some(_) => {
                bail!("`enabled` of plugin `{name}` must be `true`, or `false`, to turn it off")
            }
        }
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
        // settings in the order they're written, less those that are
        // little's to read.
        let mut settings = settings.clone();
        settings.shift_remove("wasm");
        settings.shift_remove("enabled");
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

/// little's settings that are the site's, for every version, so an entry in
/// `site.versions` can't set them, though one from a ref can set its own
/// `source`, `navigation.items`, and `plugins`. `navigation` isn't among
/// them, as only its `prune` is the site's.
const SITES_SETTINGS: [&str; 11] = [
    "destination",
    "theme",
    "repository",
    "search",
    "markdown",
    "versions",
    "redirects",
    "built",
    "modified",
    "serve",
    "error",
];

/// The versions of the site, from `site.versions`, in the order its menu
/// lists them, whose `plugins` can name only the site's, `sites_plugins`.
/// Each entry's other keys, but little's settings, are for the theme.
fn versions(site: &Mapping, sites_plugins: &[Plugin]) -> Result<Vec<Version>> {
    let entries = match site.get("versions") {
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
        // A blank `source:`, `navigation:`, or `plugins:` is as good as none.
        let source = match entry.get("source") {
            None | Some(Value::Null) => None,
            Some(Value::String(source)) => Some(source.clone()),
            Some(_) => bail!(
                "version `{title}`'s `source` must be the path to the folder its pages are in \
                 at its `ref`, like `./docs`"
            ),
        };
        // Its nav, in `navigation.items`, as the site's is. One that isn't a
        // nav stops the build of the version, as the site's does.
        let nav = match entry.get("navigation") {
            None | Some(Value::Null) => None,
            Some(Value::Mapping(navigation)) => {
                if let Some(key) = (navigation.keys()).find(|key| key.as_str() != Some("items")) {
                    return Err(sites_setting(
                        title,
                        &format!("navigation.{}", written(key)),
                    ));
                }
                let items = navigation.get("items");
                items.filter(|items| !items.is_null()).cloned()
            }
            Some(_) => bail!(
                "version `{title}`'s `navigation` must hold its nav, as `items`, like \
                 `navigation: {{ items: [{{ title: Home, url: / }}] }}`"
            ),
        };
        let plugins = version_plugins(title, entry.get("plugins"), sites_plugins)?;
        // The one without a `ref` is built from the site's files as they
        // are, with the site's settings, so `little serve` shows them.
        let own = [
            ("source", source.is_some()),
            ("navigation.items", nav.is_some()),
            ("plugins", !plugins.is_empty()),
        ];
        if git_ref.is_none()
            && let Some((key, _)) = own.iter().find(|(_, set)| *set)
        {
            bail!(
                "version `{title}` sets `{key}` but has no `ref`, so it's built from the site's \
                 files as they are: set the site's `{key}` instead"
            );
        }
        if let Some(key) = SITES_SETTINGS.iter().find(|key| entry.contains_key(**key)) {
            return Err(sites_setting(title, key));
        }
        versions.push(Version {
            title: title.clone(),
            path,
            git_ref,
            source,
            nav,
            plugins,
        });
    }
    if !versions.is_empty() && !versions.iter().any(|version| version.path.is_empty()) {
        bail!(
            "no version is at `/`, the site's home: give the one readers should land on `url: /`"
        );
    }
    Ok(versions)
}

/// The error for version `title`'s entry setting `key`, one of little's
/// settings that are the site's.
fn sites_setting(title: &str, key: &str) -> anyhow::Error {
    anyhow!(
        "version `{title}` sets `{key}`, but every version is built with the site's `{key}`: a \
         version from a `ref` can set only its own `source`, `navigation.items`, and `plugins`"
    )
}

/// The settings version `title` gives the site's plugins, `sites_plugins`,
/// in its entry's `plugins`, if it has any: each the plugin's settings, which
/// go over the site's, like `enabled: false`, which turns it off. A version
/// can't give one a `.wasm` file, as every version is built with the site's,
/// the ones `little serve` watches.
fn version_plugins(
    title: &str,
    plugins: Option<&Value>,
    sites_plugins: &[Plugin],
) -> Result<Mapping> {
    let not_settings = || {
        anyhow!(
            "version `{title}`'s `plugins` must name the site's plugins it changes, each with \
             its settings, like `api: {{ enabled: false }}`, to turn it off"
        )
    };
    let plugins = match plugins {
        None | Some(Value::Null) => return Ok(Mapping::new()),
        Some(Value::Mapping(plugins)) => plugins,
        Some(_) => return Err(not_settings()),
    };
    for (name, plugin) in plugins {
        let name = name.as_str().ok_or_else(not_settings)?;
        if !sites_plugins.iter().any(|plugin| plugin.name == name) {
            bail!(
                "version `{title}` sets plugin `{name}`, but the site doesn't use it: a version \
                 can change only the settings of the site's plugins"
            );
        }
        match plugin {
            Value::Mapping(settings) if !settings.contains_key("wasm") => {}
            Value::Mapping(_) => bail!(
                "version `{title}` gives plugin `{name}` a `.wasm` file, but every version is \
                 built with the site's: a version can change only a plugin's settings, or turn \
                 it off, with `enabled: false`"
            ),
            _ => bail!(
                "plugin `{name}` in version `{title}` must be its settings, like \
                 `enabled: false`, to turn it off"
            ),
        }
    }
    Ok(plugins.clone())
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

/// The error for `site.versions` that isn't a list of versions.
fn versions_error() -> anyhow::Error {
    anyhow!(
        "`site.versions` must list the site's versions, each with its `title`, the `url` it's \
         at, and the `ref` it's built from, like `- {{ title: \"1.4\", url: /1.4, ref: v1.4 }}`"
    )
}

/// The site's redirects, from `site.redirects`: one for each URL in each
/// entry's `from`, in the order they're listed, or none, if it's left empty.
/// Each is from a URL little can write a page at, or from every page under
/// a folder, to every page under another, and none leads to a URL that's
/// redirected too, as that would send a reader on twice.
fn redirects(site: &Mapping) -> Result<Vec<Redirect>> {
    let entries = match site.get("redirects") {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Sequence(entries)) => entries,
        Some(_) => return Err(redirects_error()),
    };
    let mut redirects = Vec::new();
    for entry in entries {
        let Value::Mapping(entry) = entry else {
            return Err(redirects_error());
        };
        let other = |key: &&Value| !matches!(key.as_str(), Some("to" | "from"));
        if let Some(key) = entry.keys().find(other) {
            bail!(
                "a redirect in `site.redirects` sets `{}`, but each has only `to`, the URL it \
                 leads to, and `from`, the URLs that lead there",
                written(key)
            );
        }
        let to = match entry.get("to") {
            Some(Value::String(to)) if leads_somewhere(to) => to,
            None | Some(Value::Null) => bail!(
                "each redirect in `site.redirects` needs a `to`, the URL it leads to, like \
                 `to: /guide/install`"
            ),
            Some(to) => bail!(
                "`site.redirects` has a redirect to `{}`, but each must lead to a URL from the \
                 root of the site, as `site.navigation.items` writes it, like `/guide/install`, \
                 or to another site's address, starting with https:// or http://",
                written(to)
            ),
        };
        // Always a list, so there's one way to write it.
        let from = match entry.get("from") {
            Some(Value::Sequence(from)) if !from.is_empty() => from,
            _ => bail!(
                "the redirect to {to} in `site.redirects` needs a `from`, a list of the URLs \
                 that lead there, like `from: [/guide/setup]`"
            ),
        };
        // Every page under a folder leads to the same page under another,
        // in the site, whose pages little can list. A `*` in a `?query` or
        // a `#fragment` names no file, so it's left as it's written.
        let folder = every_page_under(to);
        let path = to.split(['?', '#']).next().unwrap_or_default();
        if folder.is_none() && from_the_root(to) && path.contains('*') {
            bail!(
                "`site.redirects` has a redirect to `{to}`, but a `*` is only for every page \
                 under a folder, as the end of a URL, like `/tools/*`"
            );
        }
        for url in from {
            let Some(url) = url
                .as_str()
                .filter(|url| names_a_page(every_page_under(url).unwrap_or(url)))
            else {
                bail!(
                    "the redirect to {to} in `site.redirects` is from `{}`, but each must be \
                     from a URL from the root of the site, where a page was, like `/guide/setup` \
                     or `/setup.html`, with no `?` or `#`, or from every page under a folder, \
                     like `/guide/*`",
                    written(url)
                )
            };
            match (every_page_under(url), folder) {
                (Some(_), None) => bail!(
                    "`site.redirects` sends {url}, every page under a folder, to {to}, but \
                     each leads to the same page under another of the site's folders, like \
                     `to: /tools/*`"
                ),
                (None, Some(_)) => bail!(
                    "`site.redirects` sends {url} to {to}, every page under a folder, so it \
                     must be from every page under one too, like `{}/*`",
                    url.trim_end_matches('/')
                ),
                _ => redirects.push(Redirect {
                    from: url.to_string(),
                    to: to.clone(),
                }),
            }
        }
    }
    // Each by the page it's from: `/guide` and `/guide/` are the same page.
    fn page(url: &str) -> &str {
        url.trim_end_matches('/')
    }
    let itself = |redirect: &&Redirect| {
        let target = redirect.target();
        target.is_some_and(|target| page(target) == page(&redirect.from))
    };
    if let Some(redirect) = redirects.iter().find(itself) {
        bail!("`site.redirects` sends {} to itself", redirect.from);
    }
    let sent_on: HashMap<&str, &Redirect> = (redirects.iter())
        .map(|redirect| (page(&redirect.from), redirect))
        .collect();
    for redirect in &redirects {
        let next = (redirect.target()).and_then(|target| sent_on.get(page(target)));
        let Some(next) = next else {
            continue;
        };
        let Redirect { from, to } = redirect;
        bail!(
            "`site.redirects` sends {from} to {to}, but sends {} on to {}: send {from} to {} \
             too, so a reader is sent on once",
            next.from,
            next.to,
            next.to
        );
    }
    Ok(redirects)
}

/// Whether a redirect can lead to `url`: a URL from the root of the site,
/// like `/guide/install#usage`, or another site's address.
fn leads_somewhere(url: &str) -> bool {
    let on_the_web =
        |address: url::Url| matches!(address.scheme(), "https" | "http") && address.has_host();
    from_the_root(url) || url::Url::parse(url).is_ok_and(on_the_web)
}

/// Whether a redirect can be from `url`: a URL from the root of the site
/// with a page's path, which little can write a page at, in the site's
/// folder, on any system. A page has no `?query` or `#fragment`, and none of
/// its path's parts is empty, `.`, or `..`, or has a `\`, a `:`, or a `*`,
/// which mean something else on Windows.
fn names_a_page(url: &str) -> bool {
    let Some(path) = url.strip_prefix('/') else {
        return false;
    };
    let path = path.strip_suffix('/').unwrap_or(path);
    let folders = |path: &str| !path.split('/').any(|part| matches!(part, "" | "." | ".."));
    !url.contains(['?', '#', '\\', ':', '*']) && (path.is_empty() || folders(path))
}

/// The error for `site.redirects` that isn't a list of redirects.
fn redirects_error() -> anyhow::Error {
    anyhow!(
        "`site.redirects` must list the site's redirects, each with the URL it leads `to`, and \
         the URLs it's `from`, like `- {{ to: /guide/install, from: [/guide/setup] }}`"
    )
}

/// Check `site.error`, the `title` and `description` of the 404 page, which
/// the page gets as its own: each is text, if it's there.
fn error_page(site: &Mapping) -> Result<()> {
    let settings = group(site, "error", "like `title: Page not found`")?;
    for key in ["title", "description"] {
        if !matches!(settings.get(key), None | Some(Value::String(_))) {
            bail!(
                "`site.error.{key}` must be text, in quotes if YAML would read it as anything \
                 else, like \"404\""
            );
        }
    }
    Ok(())
}

/// Resolve a path a config names against `dir`: an absolute path is just that
/// path. Tidied, so a site in `.` that sets `./docs` shows it as `./docs`.
fn resolve(dir: &Path, path: &str) -> PathBuf {
    dir.join(path).components().collect()
}

/// Read one level of the cascade from the `little.yml` at `path`, which
/// errors call `name`. A missing file isn't an error: that level just has
/// nothing to add.
fn read_level(path: &Path, name: &str) -> Result<Level> {
    level(read_config(path, name)?, name)
}

/// Read the settings in the `little.yml` at `path`, which errors call
/// `name`: none, if it's not there.
fn read_config(path: &Path, name: &str) -> Result<Mapping> {
    match std::fs::read_to_string(path) {
        Ok(raw) => parse_config(&raw).with_context(|| format!("parsing {name}")),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Mapping::new()),
        Err(e) => Err(e).with_context(|| format!("reading {name}")),
    }
}

/// Parse settings, from a config file or a page's frontmatter, whose keys
/// become `page`'s. Empty or comment-only, there are none. Anything but
/// settings, like a list or plain text, is an error, rather than dropped:
/// merged, it would replace every level beneath it.
pub(crate) fn parse_config(raw: &str) -> serde_yaml::Result<Mapping> {
    Ok(serde_yaml::from_str::<Option<Mapping>>(raw)?.unwrap_or_default())
}

/// Merge `over` into `base`, key by key. Where both set a key to settings,
/// those merge the same way; any other value replaces the one beneath it. The
/// build merges a page's frontmatter over every page's settings, and those
/// over what it works out for `page`, the same way, and the config over what
/// it works out for `site`.
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
    use crate::testing::{self, IN_THEME, scratch, write};
    use serde_json::json;

    /// The setting at `path` in `cfg`, like `theme.fonts.body`, if it's
    /// text.
    fn get<'a>(cfg: &'a Mapping, path: &str) -> Option<&'a str> {
        value(cfg, path).and_then(Value::as_str)
    }

    /// The setting at `path` in `cfg`, like `serve.port`, if it's there.
    fn value<'a>(cfg: &'a Mapping, path: &str) -> Option<&'a Value> {
        at(cfg, &path.split('.').collect::<Vec<_>>())
    }

    /// Why the site's config doesn't load once its little.yml is `site_cfg`.
    fn error(site: &Path, site_cfg: &str) -> String {
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
        format!("{:#}", load(site).unwrap_err())
    }

    /// A little.yml whose `site` holds `settings`, which are written from
    /// their first column, for a table of them.
    fn under_site(settings: &str) -> String {
        let lines = settings.lines().map(|line| format!("  {line}\n"));
        format!("site:\n{}", lines.collect::<String>())
    }

    /// Put `files`, each a (path, contents) pair, where the theme at `url` is
    /// kept once it's downloaded, so a test needn't download it.
    fn downloaded(site: &Path, url: &str, files: &[(&str, &str)]) -> Location {
        let dir = download::folder(site, url);
        std::fs::create_dir_all(&dir).unwrap();
        write(&dir, files);
        Location::Downloaded {
            url: url.to_string(),
            dir,
        }
    }

    /// little's defaults, with the built-in theme's settings over them.
    fn with_builtin_theme() -> Level {
        let Level { mut site, mut page } = builtin_defaults();
        let theme = builtin_theme();
        merge(&mut site, theme.site);
        merge(&mut page, theme.page);
        Level { site, page }
    }

    #[test]
    fn a_little_yml_holds_only_site_and_page() {
        let (_tmp, site) = scratch(&[("theme/_templates/default.html", "")]);
        let file = site.join(CONFIG_FILE).display().to_string();
        let beside = |key: &str, file: &str| {
            format!(
                "{file} sets `{key}` beside `site` and `page`, but a little.yml holds only \
                 those two: put it under `site`, with the site's settings, or under `page`, \
                 with every page's"
            )
        };

        // A setting beside them would do nothing, so it stops the build.
        for (site_cfg, key) in [
            ("name: My Docs\n", "name"),
            ("site: { name: My Docs }\nauthor: Jo\n", "author"),
            ("pages: { author: Jo }\n", "pages"),
            ("1: one\n", "1"),
        ] {
            assert_eq!(error(&site, site_cfg), beside(key, &file), "{site_cfg}");
        }

        // Each holds settings, or nothing.
        assert_eq!(
            error(&site, "site: My Docs\n"),
            format!("`site` in {file} must hold the site's settings, like `name: My Docs`")
        );
        assert_eq!(
            error(&site, "page: [outline]\n"),
            format!(
                "`page` in {file} must hold every page's settings, like \
                 `breadcrumb: {{ enabled: false }}`"
            )
        );
        let builtin = with_builtin_theme();
        for site_cfg in [
            "site:\n",
            "page:\n",
            "site:\npage:\n",
            "site: {}\npage: {}\n",
        ] {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            let cfg = load(&site).unwrap();
            assert_eq!(cfg.site, builtin.site, "{site_cfg}");
            assert_eq!(cfg.page, builtin.page, "{site_cfg}");
        }

        // A theme's little.yml is checked the same way, and named as its.
        let theme_file = site.join("theme").join(CONFIG_FILE);
        std::fs::write(&theme_file, "name: Fancy\n").unwrap();
        let message = error(&site, IN_THEME);
        assert_eq!(message, beside("name", &theme_file.display().to_string()));
    }

    #[test]
    fn every_pages_settings_cascade() {
        let theme_cfg = "page:\n  author: Theme\n  layout: wide\n  \
                         social:\n    image: /theme.png\n    card: summary\n";
        let (_tmp, site) = scratch(&[
            ("theme/_templates/default.html", ""),
            ("theme/little.yml", theme_cfg),
        ]);
        let page = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            let cfg = load(&site).unwrap();
            // Less little's own, which `little_defaults_only_its_own_settings`
            // checks.
            let mut page = cfg.page.clone();
            page.retain(|key, _| !builtin_defaults().page.contains_key(key));
            serde_yaml::to_string(&page).unwrap()
        };

        // A theme sets what every page has, and a site changes it, key by
        // key, as it changes its theme's `site`.
        assert_eq!(page(""), "{}\n");
        assert_eq!(
            page(IN_THEME),
            "author: Theme\nlayout: wide\nsocial:\n  image: /theme.png\n  card: summary\n"
        );
        let site_cfg =
            format!("{IN_THEME}page:\n  author: Site\n  social:\n    image: /site.png\n");
        assert_eq!(
            page(&site_cfg),
            "author: Site\nlayout: wide\nsocial:\n  image: /site.png\n  card: summary\n"
        );

        // A key left blank is ignored, as it is in a page's frontmatter.
        let site_cfg = format!("{IN_THEME}page:\n  author:\n  layout: narrow\n");
        assert_eq!(
            page(&site_cfg),
            "author: Theme\nlayout: narrow\nsocial:\n  image: /theme.png\n  card: summary\n"
        );
    }

    #[test]
    fn a_site_gets_the_builtin_theme_unless_it_chooses_another() {
        let (_tmp, site) = scratch(&[
            (
                "theme/little.yml",
                "site: { theme: { fonts: { body: mono } } }\n",
            ),
            (
                "path/to/my/theme/little.yml",
                "site:\n  name: Mine\n  theme:\n    fonts:\n      body: serif\n",
            ),
        ]);
        let load_with = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap()
        };
        let font = "theme.fonts.body";

        // Without a `theme`, or with one that's `enabled`, and has no
        // `source`, the built-in theme, even for a site with a `theme/`
        // folder.
        assert_eq!(load(&site).unwrap().themes, [Location::BuiltIn]);
        for site_cfg in [
            "site: { name: My Docs }\n",
            "site: { theme: { enabled: true } }\n",
        ] {
            let cfg = load_with(site_cfg);
            assert_eq!(cfg.themes, [Location::BuiltIn], "with {site_cfg:?}");
            assert_eq!(get(&cfg.site, font), get(&with_builtin_theme().site, font));
        }

        // With `enabled: false`, no theme, so no theme's settings either,
        // whatever its `source`.
        for theme in ["{ enabled: false }", "{ enabled: false, source: ./theme }"] {
            let cfg = load_with(&format!("site: {{ theme: {theme} }}\n"));
            assert!(cfg.themes.is_empty(), "{theme}");
            assert_eq!(get(&cfg.site, font), None);
            assert_eq!(value(&cfg.site, "theme.enabled"), Some(&false.into()));
        }

        // A `source` chooses the theme in that folder.
        let cfg = load_with("site:\n  theme:\n    source: ./path/to/my/theme\n  name: My Docs\n");
        let chosen = Location::Folder(site.join("path/to/my/theme"));
        assert_eq!(cfg.themes, [chosen]);
        assert_eq!(get(&cfg.site, font), Some("serif")); // the chosen theme's
        assert_eq!(get(&cfg.site, "name"), Some("My Docs")); // the site still wins
        assert_eq!(get(&cfg.site, "theme.source"), Some("./path/to/my/theme"));
        assert_eq!(value(&cfg.site, "theme.enabled"), Some(&true.into()));
    }

    #[test]
    fn theme_paths_are_relative_to_the_file_that_sets_them() {
        let (_tmp, root) = scratch(&[
            ("site/themes/mine/_templates/default.html", ""),
            ("shared/theme/_templates/default.html", ""),
            (
                "shared/company/little.yml",
                "site: { theme: { source: ../theme } }\n",
            ),
        ]);
        let site = root.join("site");
        // Unquoted, as people usually write them.
        let chosen = |theme: &str| {
            let site_cfg = format!("site:\n  theme:\n    source: {theme}\n");
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
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
    }

    #[test]
    fn a_theme_can_build_on_another() {
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                "site:\n  theme:\n    source: ./company\n  name: My Docs\n",
            ),
            (
                "company/little.yml",
                "site:\n  theme:\n    source: ../fancy\n    fonts:\n      body: Inter\n  \
                 footer:\n    note: Company\npage:\n  author: Company\n",
            ),
            (
                "fancy/little.yml",
                "site:\n  name: Fancy\n  language: en\n  theme:\n    fonts:\n      body: serif\n  \
                 footer:\n    note: Fancy\n    links: true\npage:\n  author: Fancy\n  \
                 layout: wide\n",
            ),
        ]);
        let company = site.join("company");

        // little -> fancy -> company -> site, each over the one before, as
        // `little config` prints them. `theme.source` is the site's choice,
        // not its theme's.
        let cfg = load(&site).unwrap();
        let fancy = Location::Folder(company.join("../fancy"));
        assert_eq!(cfg.themes, [Location::Folder(company.clone()), fancy]);
        // Less little's Markdown settings, which `markdown_cascades` checks.
        let mut groups = cfg.groups();
        let Some(Value::Mapping(settings)) = groups.get_mut("site") else {
            panic!("`site` should hold settings");
        };
        settings.shift_remove("markdown");
        assert_eq!(
            serde_yaml::to_string(&groups).unwrap(),
            "site:\n  \
               source: ./docs\n  \
               destination: ./dist\n  \
               theme:\n    enabled: true\n    fonts:\n      body: Inter\n    source: ./company\n  \
               navigation:\n    prune: false\n  \
               serve:\n    host: 127.0.0.1\n    port: 8080\n  \
               repository:\n    branch: main\n  \
               search:\n    enabled: false\n    exclude: []\n  \
               error:\n    title: Page not found\n    \
               description: The page you are looking for could not be found.\n  \
               name: My Docs\n  \
               language: en\n  \
               footer:\n    note: Company\n    links: true\n\
             page:\n  \
               outline:\n    enabled: true\n    depth:\n      min: 2\n      max: 6\n  \
               breadcrumb:\n    enabled: true\n  \
               previous:\n    enabled: true\n  \
               next:\n    enabled: true\n  \
               repository:\n    enabled: true\n  \
               author: Company\n  \
               layout: wide\n"
        );

        // A theme can build on little's built-in theme, too.
        let company_cfg = "site: { theme: { enabled: true, fonts: { body: Inter } } }\n";
        std::fs::write(company.join(CONFIG_FILE), company_cfg).unwrap();
        let cfg = load(&site).unwrap();
        assert_eq!(cfg.themes, [Location::Folder(company), Location::BuiltIn]);
        assert_eq!(get(&cfg.site, "theme.fonts.body"), Some("Inter"));
        assert_eq!(
            get(&cfg.site, "language"),
            get(&with_builtin_theme().site, "language")
        );
        assert_eq!(get(&cfg.site, "theme.source"), Some("./company"));
    }

    #[test]
    fn a_theme_can_be_downloaded_from_a_url() {
        let url = "https://example.invalid/fancy-1.0.zip";
        let site_cfg = format!("site:\n  theme:\n    source: {url}\n  name: My Docs\n");
        let (_tmp, site) = scratch(&[(CONFIG_FILE, &site_cfg)]);
        let fancy_cfg =
            "site:\n  name: Fancy\n  language: en\n  theme:\n    fonts:\n      body: serif\n";
        let fancy = downloaded(&site, url, &[(CONFIG_FILE, fancy_cfg)]);
        let font = "theme.fonts.body";

        // It cascades like any other theme.
        let cfg = load(&site).unwrap();
        assert_eq!(cfg.themes, std::slice::from_ref(&fancy));
        assert_eq!(get(&cfg.site, font), Some("serif")); // the theme's
        assert_eq!(get(&cfg.site, "name"), Some("My Docs")); // the site still wins
        assert_eq!(get(&cfg.site, "theme.source"), Some(url));

        // A company's theme can build on it, changing only what it needs to.
        let company_cfg =
            format!("site:\n  theme:\n    source: {url}\n    fonts:\n      body: Inter\n");
        std::fs::create_dir_all(site.join("company")).unwrap();
        std::fs::write(site.join("company").join(CONFIG_FILE), company_cfg).unwrap();
        let in_company = "site:\n  theme:\n    source: ./company\n";
        std::fs::write(site.join(CONFIG_FILE), in_company).unwrap();
        let company = Location::Folder(site.join("company"));
        let cfg = load(&site).unwrap();
        assert_eq!(cfg.themes, [company.clone(), fancy.clone()]);
        assert_eq!(get(&cfg.site, font), Some("Inter")); // the company's
        assert_eq!(get(&cfg.site, "name"), Some("Fancy")); // the downloaded theme's
        assert_eq!(get(&cfg.site, "theme.source"), Some("./company"));

        // And it can build on another theme: in its own folder, relative to
        // that, or downloaded too, or little's built-in theme.
        let fancy_dir = fancy.folder().unwrap().to_path_buf();
        let base = Location::Folder(fancy_dir.join("base"));
        std::fs::create_dir_all(fancy_dir.join("base")).unwrap();
        let plain_url = "https://example.invalid/plain-2.0.tar.gz";
        let plain = downloaded(&site, plain_url, &[]);
        let from_plain = format!("source: {plain_url}");
        for (theme, beneath) in [
            ("source: ./base", base),
            (from_plain.as_str(), plain),
            ("enabled: true", Location::BuiltIn),
        ] {
            let fancy_cfg = format!("site:\n  theme:\n    {theme}\n");
            std::fs::write(fancy_dir.join(CONFIG_FILE), fancy_cfg).unwrap();
            let themes = load(&site).unwrap().themes;
            assert_eq!(themes, [company.clone(), fancy.clone(), beneath]);
        }
        // With neither, or with `enabled: false`, it builds on none.
        for theme in ["fonts: { body: serif }", "enabled: false"] {
            let fancy_cfg = format!("site:\n  theme:\n    {theme}\n");
            std::fs::write(fancy_dir.join(CONFIG_FILE), fancy_cfg).unwrap();
            let themes = load(&site).unwrap().themes;
            assert_eq!(themes, [company.clone(), fancy.clone()], "{theme}");
        }
    }

    #[test]
    fn themes_cannot_build_on_each_other_in_a_loop() {
        let theme = |source: &str| format!("site:\n  theme:\n    source: {source}\n");
        let (_tmp, site) = scratch(&[
            ("a/little.yml", &theme("../b")),
            ("b/little.yml", &theme("../a")),
        ]);
        let (a, b) = (site.join("a"), site.join("a").join("..").join("b"));
        let message = error(&site, &theme("./a"));
        let expected = format!(
            "themes build on each other in a loop: {} -> {} -> {}",
            a.display(),
            b.display(),
            b.join("..").join("a").display()
        );
        assert_eq!(message, expected);

        // Even a theme that builds on itself.
        std::fs::write(a.join(CONFIG_FILE), theme(".")).unwrap();
        let message = error(&site, &theme("./a"));
        assert!(
            message.contains("themes build on each other in a loop"),
            "{message}"
        );

        // Downloaded themes are named by their URLs.
        let (a, b) = (
            "https://example.invalid/a.zip",
            "https://example.invalid/b.zip",
        );
        downloaded(&site, a, &[(CONFIG_FILE, &theme(b))]);
        downloaded(&site, b, &[(CONFIG_FILE, &theme(a))]);
        let message = error(&site, &theme(a));
        let expected = format!("themes build on each other in a loop: {a} -> {b} -> {a}");
        assert_eq!(message, expected);
    }

    #[test]
    fn bad_theme_choices_explain_themselves() {
        let (_tmp, site) = scratch(&[("theme/_templates/default.html", "")]);
        let file = site.join(CONFIG_FILE).display().to_string();
        let source = |source: &str| format!("site:\n  theme:\n    source: {source}\n");
        let must_be = |file: &str| {
            format!(
                "`site.theme` in {file} must be its settings, like `source: ./my-theme`, for \
                 the path to a theme's folder, or the URL of its archive, or `enabled: false`, \
                 for no theme"
            )
        };

        let message = error(&site, &source("./missing"));
        let expected = format!(
            "{file} sets `site.theme.source` to `./missing`, but there's no folder at {}",
            site.join("missing").display()
        );
        assert_eq!(message, expected);
        // `default` is a folder name like any other, and only unquoted `true`
        // and `false` are booleans.
        for theme in ["default", "\"true\"", "no"] {
            let message = error(&site, &source(theme));
            assert!(message.contains("but there's no folder at"), "{message}");
        }

        // A value where its settings go, like a path, `true`, or `false`,
        // isn't a theme, nor is nothing.
        for theme in ["", " ./theme", " true", " false", " [a, b]", " 1"] {
            let message = error(&site, &format!("site:\n  theme:{theme}\n"));
            assert_eq!(message, must_be(&file), "theme:{theme}");
        }
        // `enabled` is `true` or `false`, and `source` is text.
        for enabled in ["", " yes", " \"true\"", " 1", " ./theme"] {
            let message = error(&site, &format!("site:\n  theme:\n    enabled:{enabled}\n"));
            let expected =
                format!("`site.theme.enabled` in {file} must be `true`, or `false`, for no theme");
            assert_eq!(message, expected, "enabled:{enabled}");
        }
        for theme in ["", " true", " [a, b]", " 1"] {
            let message = error(&site, &format!("site:\n  theme:\n    source:{theme}\n"));
            let expected = format!(
                "`site.theme.source` in {file} must be the path to a theme's folder, or the \
                 URL of a theme's archive"
            );
            assert_eq!(message, expected, "source:{theme}");
        }

        // Themes are only downloaded over HTTPS.
        let url = "http://example.invalid/theme.zip";
        let message = error(&site, &source(url));
        let expected = format!(
            "{file} sets `site.theme.source` to `{url}`, but little only downloads themes over \
             HTTPS"
        );
        assert_eq!(message, expected);
        // A download that fails says why. Nothing listens on port 1, so this
        // one fails at once, without the network.
        let url = "https://127.0.0.1:1/theme.zip";
        let message = error(&site, &source(url));
        let expected = format!(
            "{file} sets `site.theme.source` to `{url}`, but little couldn't download it: "
        );
        assert!(message.starts_with(&expected), "{message}");

        // A theme's own `theme` is checked the same way, and named as its.
        let theme_file = site.join("theme").join(CONFIG_FILE);
        std::fs::write(&theme_file, "site: { theme: [a, b] }\n").unwrap();
        let message = error(&site, &source("./theme"));
        assert_eq!(message, must_be(&theme_file.display().to_string()));

        // A downloaded theme's folder is named with a hash, so its files are
        // named by its URL.
        let url = "https://example.invalid/fancy-1.0.zip";
        let fancy_file = format!("{CONFIG_FILE} in the theme from {url}");
        let fancy_cfg = "site: { theme: [a, b] }\n";
        let fancy = downloaded(&site, url, &[(CONFIG_FILE, fancy_cfg)]);
        let message = error(&site, &source(url));
        assert_eq!(message, must_be(&fancy_file));
        let fancy_cfg = fancy.folder().unwrap().join(CONFIG_FILE);
        std::fs::write(fancy_cfg, "- font: serif\n").unwrap();
        let message = error(&site, &source(url));
        assert!(
            message.starts_with(&format!("parsing {fancy_file}: ")),
            "{message}"
        );
    }

    #[test]
    fn folders_are_relative_to_the_site_wherever_theyre_set() {
        let (_tmp, root) = scratch(&[("site/themes/mine/_templates/default.html", "")]);
        let site = root.join("site");
        // `site_cfg` goes under the site's `site`.
        let folders = |theme_cfg: &str, site_cfg: &str| {
            std::fs::write(site.join("themes/mine").join(CONFIG_FILE), theme_cfg).unwrap();
            let site_cfg = format!("site:\n  theme:\n    source: ./themes/mine\n{site_cfg}");
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            let cfg = load(&site).unwrap();
            (cfg.source_dir, cfg.destination_dir)
        };

        // little's own defaults.
        assert_eq!(folders("", ""), (site.join("docs"), site.join("dist")));

        // A theme can set them, but they're still relative to the site...
        let theme = "site:\n  source: content\n  destination: ./public\n";
        let expected = (site.join("content"), site.join("public"));
        assert_eq!(folders(theme, ""), expected);

        // ...and the site has the last word, of course.
        let site_cfg = "  source: ../shared/docs\n";
        let expected = (site.join("../shared/docs"), site.join("public"));
        assert_eq!(folders(theme, site_cfg), expected);

        // Absolute paths work too.
        let out = root.join("out");
        let site_cfg = format!("  destination: {}\n", out.display());
        assert_eq!(folders("", &site_cfg), (site.join("docs"), out));
    }

    #[test]
    fn bad_folders_explain_themselves() {
        let (_tmp, site) = scratch(&[]);
        for key in ["source", "destination"] {
            // A list, or no value at all, isn't a path.
            for value in [" [a, b]", ""] {
                let message = error(&site, &format!("site:\n  {key}:{value}\n"));
                assert_eq!(
                    message,
                    format!("`site.{key}` must be the path to a folder")
                );
            }
        }

        // Whether the source is there is for the build to say: the config
        // doesn't read it.
        std::fs::write(site.join(CONFIG_FILE), "site: { source: ./missing }\n").unwrap();
        assert!(load(&site).is_ok());
    }

    /// little's own settings for every page, but for the levels of heading
    /// its table of contents lists, `depths`.
    fn every_page(depths: RangeInclusive<u8>) -> PageSettings {
        PageSettings {
            outline: true,
            depths,
            breadcrumb: true,
            previous: true,
            next: true,
            repository: true,
            search: None,
        }
    }

    #[test]
    fn each_pages_groups_cascade() {
        let theme_cfg = "page:\n  breadcrumb:\n    enabled: false\n  search:\n    enabled: false\n";
        let (_tmp, site) = scratch(&[
            ("theme/_templates/default.html", ""),
            ("theme/little.yml", theme_cfg),
        ]);
        let settings = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap().page_settings
        };

        // little gives every page each thing it works out, and lists every
        // heading but a page's `#` title in its table of contents, and the
        // built-in theme only `##` and `###`.
        assert_eq!(settings(""), every_page(2..=3));
        // A theme turns one off, or leaves every page out of search, and a
        // site has the last word, setting by setting.
        let themed = PageSettings {
            breadcrumb: false,
            search: Some(false),
            ..every_page(2..=6)
        };
        assert_eq!(settings(IN_THEME), themed);
        let site_cfg = format!(
            "{IN_THEME}page:\n  breadcrumb:\n    enabled: true\n  next:\n    enabled: false\n  \
             outline:\n    depth:\n      min: 1\n"
        );
        let sited = PageSettings {
            next: false,
            search: Some(false),
            ..every_page(1..=6)
        };
        assert_eq!(settings(&site_cfg), sited);
        // A group left blank is ignored, as any key under `page` is.
        let site_cfg = format!("{IN_THEME}page:\n  breadcrumb:\n  search:\n");
        assert_eq!(settings(&site_cfg), themed);
    }

    #[test]
    fn bad_page_groups_explain_themselves() {
        let (_tmp, site) = scratch(&[]);
        for group in ["outline", "breadcrumb", "previous", "next", "repository"] {
            // A value where its settings go, like `false`, isn't them.
            for value in [" false", " true", " [items]"] {
                let message = error(&site, &format!("page:\n  {group}:{value}\n"));
                assert_eq!(
                    message,
                    format!("`page.{group}` must be its settings, like `enabled: false`")
                );
            }
            // YAML reads `yes` and a quoted `"true"` as text, not `true`.
            for value in [" yes", " \"true\"", " 1", ""] {
                let site_cfg = format!("page:\n  {group}:\n    enabled:{value}\n");
                assert_eq!(
                    error(&site, &site_cfg),
                    format!("`page.{group}.enabled` must be `true` or `false`")
                );
            }
        }

        // Whether search indexes every page is written the same way.
        for value in [" false", " true"] {
            let message = error(&site, &format!("page:\n  search:{value}\n"));
            assert_eq!(
                message,
                "`page.search` must be its settings, like `enabled: false`"
            );
        }
        for value in [" yes", " 1", ""] {
            let site_cfg = format!("page:\n  search:\n    enabled:{value}\n");
            assert_eq!(
                error(&site, &site_cfg),
                "`page.search.enabled` must be `true`, to index the page for search, or \
                 `false`, to leave it out"
            );
        }

        for key in ["min", "max"] {
            // There's no `#######`, a heading level is a number, and no value
            // at all isn't one.
            for value in [" 0", " 7", " -1", " \"3\"", ""] {
                let site_cfg = format!("page:\n  outline:\n    depth:\n      {key}:{value}\n");
                assert_eq!(
                    error(&site, &site_cfg),
                    format!(
                        "`page.outline.depth.{key}` must be a heading level from 1 to 6, like 3 \
                         for `###`"
                    )
                );
            }
        }
        // The built-in theme lists down to `###`.
        assert_eq!(
            error(&site, "page: { outline: { depth: { min: 4 } } }\n"),
            "`page.outline.depth.min` (4) can't be more than `page.outline.depth.max` (3)"
        );
    }

    #[test]
    fn nav_prune_cascades() {
        let (_tmp, site) = scratch(&[
            ("theme/_templates/default.html", ""),
            ("big/_templates/default.html", ""),
            ("big/little.yml", "site: { navigation: { prune: true } }\n"),
        ]);
        let prune = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap().nav_prune
        };
        let in_big = "site:\n  theme:\n    source: ./big\n";

        // little gives each page the whole nav, as does the built-in theme.
        // A theme can prune it, and a site has the last word.
        assert!(!prune(IN_THEME));
        assert!(!prune(""));
        assert!(prune("site: { navigation: { prune: true } }\n"));
        assert!(prune(in_big));
        assert!(!prune(&format!(
            "{in_big}  navigation:\n    prune: false\n"
        )));
        // A site that lists its own nav, beside it, leaves it as it is.
        let site_cfg = format!("{in_big}  navigation:\n    items: [{{ title: Home, url: / }}]\n");
        assert!(prune(&site_cfg));
    }

    #[test]
    fn a_bad_nav_prune_explains_itself() {
        let (_tmp, site) = scratch(&[]);
        // YAML reads `yes` and a quoted `"true"` as text, not `true`.
        for value in [" yes", " \"true\"", " 1", "", " {}"] {
            let message = error(&site, &format!("site:\n  navigation:\n    prune:{value}\n"));
            assert_eq!(
                message,
                "`site.navigation.prune` must be `true`, for each page's nav to list what's in \
                 a folder only if the page is in it, or `false`"
            );
        }

        // A list of links where its settings go isn't them, nor is nothing.
        for value in [" [{ title: Home, url: / }]", " true", ""] {
            let message = error(&site, &format!("site:\n  navigation:{value}\n"));
            assert_eq!(
                message,
                "`site.navigation` must be its settings, like `prune: true`, or `items`, the \
                 site's nav, a list of links"
            );
        }
    }

    #[test]
    fn search_cascades() {
        let (_tmp, site) = scratch(&[
            ("theme/_templates/default.html", ""),
            ("drafty/_templates/default.html", ""),
            (
                "drafty/little.yml",
                "site:\n  search:\n    enabled: true\n    exclude: [/drafts/]\n",
            ),
        ]);
        let search = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap().search
        };
        let is = |enabled: bool, exclude: &[&str]| Search {
            enabled,
            exclude: exclude.iter().map(|url| url.to_string()).collect(),
        };
        let in_drafty = "site:\n  theme:\n    source: ./drafty\n";

        // little leaves search off, the built-in theme, with its search box,
        // turns it on, and a site has the last word.
        assert_eq!(search(IN_THEME), is(false, &[]));
        assert_eq!(
            search("site: { theme: { enabled: false } }\n"),
            is(false, &[])
        );
        assert_eq!(search(""), is(true, &[]));
        assert_eq!(
            search("site: { search: { enabled: false } }\n"),
            is(false, &[])
        );
        let site_cfg = format!("{IN_THEME}  search:\n    enabled: true\n");
        assert_eq!(search(&site_cfg), is(true, &[]));

        // `exclude` lists the URLs it leaves out, as they're written, or
        // none, if it's left empty, but doesn't turn search on.
        let exclude = "  search:\n    exclude: [/changelog/, /guide/old]\n";
        let left_out = ["/changelog/", "/guide/old"];
        assert_eq!(search(&format!("site:\n{exclude}")), is(true, &left_out));
        assert_eq!(
            search(&format!("{IN_THEME}{exclude}")),
            is(false, &left_out)
        );
        assert_eq!(search("site: { search: { exclude: [] } }\n"), is(true, &[]));
        assert_eq!(search("site:\n  search:\n    exclude:\n"), is(true, &[]));

        // A site's `exclude` replaces its theme's, as any list does, and its
        // `enabled` leaves its theme's `exclude` as it is.
        assert_eq!(search(in_drafty), is(true, &["/drafts/"]));
        let site_cfg = format!("{in_drafty}  search:\n    exclude: [/old/]\n");
        assert_eq!(search(&site_cfg), is(true, &["/old/"]));
        let site_cfg = format!("{in_drafty}  search:\n    enabled: false\n");
        assert_eq!(search(&site_cfg), is(false, &["/drafts/"]));
    }

    #[test]
    fn bad_searches_explain_themselves() {
        let (_tmp, site) = scratch(&[]);
        // A value where its settings go, like `true`, isn't them, nor is
        // nothing.
        for value in [" true", " false", " yes", " [/changelog/]", ""] {
            let message = error(&site, &format!("site:\n  search:{value}\n"));
            assert_eq!(
                message,
                "`site.search` must be its settings, like `enabled: true`, or \
                 `exclude: [/changelog/]`"
            );
        }
        // YAML reads `yes` and a quoted `"true"` as text, not `true`.
        for value in [" yes", " \"true\"", " 1", "", " {}"] {
            let message = error(&site, &format!("site:\n  search:\n    enabled:{value}\n"));
            assert_eq!(
                message,
                "`site.search.enabled` must be `true`, to index the site for a search box, or \
                 `false`"
            );
        }

        // Its other setting is `exclude`...
        assert_eq!(
            error(&site, "site:\n  search:\n    excludes: [/changelog/]\n"),
            "`site.search` has no setting `excludes`: its settings are `enabled` and `exclude`"
        );
        // ...a list...
        for value in [" /changelog/", " { url: /changelog/ }"] {
            let message = error(&site, &format!("site:\n  search:\n    exclude:{value}\n"));
            assert_eq!(
                message,
                "`site.search.exclude` must be a list of URLs, like `[/changelog/, /guide/old]`"
            );
        }
        // ...of pages' URLs, not their files' paths.
        for entry in ["other", "other/notes.md", "1.4"] {
            let site_cfg = format!("site:\n  search:\n    exclude:\n      - {entry}\n");
            assert_eq!(
                error(&site, &site_cfg),
                format!(
                    "`site.search.exclude` lists `{entry}`, but each must be a page's URL from \
                     the root of the site, as `site.navigation.items` writes it, like `/guide/` \
                     or `/guide/install`"
                )
            );
        }
    }

    #[test]
    fn an_excluded_url_leaves_out_its_page_and_those_under_it() {
        let search = Search {
            enabled: true,
            exclude: vec!["/guide/".into(), "/changelog".into()],
        };
        // The page at its URL, with or without the `/` at its end, and each
        // page under it...
        for url in [
            "/guide/",
            "/guide",
            "/guide/install",
            "/guide/a/b/",
            "/changelog",
        ] {
            assert!(search.excludes(url), "{url}");
        }
        assert!(search.excludes("/changelog/"));
        assert!(search.excludes("/changelog/2.0"));
        // ...but not one that only starts with the same letters, or above it.
        for url in ["/", "/guides", "/guide-old/", "/changelogs/", "/about"] {
            assert!(!search.excludes(url), "{url}");
        }

        // `/` leaves out every page, and no `exclude`, none.
        assert!(leaves_out("/", "/"));
        assert!(leaves_out("/", "/guide/install"));
        assert!(!Search::default().excludes("/"));
    }

    #[test]
    fn the_404_page_has_a_title_and_a_description() {
        let (_tmp, site) = scratch(&[("theme/little.yml", "site: { error: { title: Lost } }\n")]);
        let error_page = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            let cfg = load(&site).unwrap();
            let text = |key: &str| get(&cfg.site, key).unwrap_or_default().to_string();
            (text("error.title"), text("error.description"))
        };
        let page = |title: &str, description: &str| (title.to_string(), description.to_string());

        // little sets both, a theme can change either, and a site has the
        // last word.
        let littles = "The page you are looking for could not be found.";
        assert_eq!(error_page(""), page("Page not found", littles));
        assert_eq!(error_page(IN_THEME), page("Lost", littles));
        let site_cfg = format!("{IN_THEME}  error:\n    description: Try the search box.\n");
        assert_eq!(error_page(&site_cfg), page("Lost", "Try the search box."));

        // Each is text.
        for key in ["title", "description"] {
            for value in [" 404", " [Lost]", ""] {
                let message = error(&site, &format!("site:\n  error:\n    {key}:{value}\n"));
                assert_eq!(
                    message,
                    format!(
                        "`site.error.{key}` must be text, in quotes if YAML would read it as \
                         anything else, like \"404\""
                    )
                );
            }
        }
        for value in [" Lost", ""] {
            let message = error(&site, &format!("site:\n  error:{value}\n"));
            assert_eq!(
                message,
                "`site.error` must be its settings, like `title: Page not found`"
            );
        }
    }

    #[test]
    fn redirects_list_where_each_old_url_leads() {
        let (_tmp, site) = scratch(&[
            ("theme/_templates/default.html", ""),
            (
                "theme/little.yml",
                "site:\n  redirects:\n    - { to: /theme, from: [/themes] }\n",
            ),
        ]);
        let redirects = |site_cfg: &str| -> Vec<String> {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            let redirects = load(&site).unwrap().redirects;
            let sends = |redirect: &Redirect| format!("{} to {}", redirect.from, redirect.to);
            redirects.iter().map(sends).collect()
        };
        let none: [&str; 0] = [];

        // little has none, and a list left empty is none.
        assert_eq!(redirects(""), none);
        assert_eq!(redirects("site:\n  redirects:\n"), none);
        assert_eq!(redirects("site: { redirects: [] }\n"), none);

        // One for each URL an entry is from, in the order they're listed, as
        // they're written: to a page, with any `?query` or `#fragment`, or
        // to another site.
        let site_cfg = "site:\n  redirects:\n    - to: /guide/install#usage\n      \
                        from:\n        - /guide/setup\n        - /setup.html\n    \
                        - from: [/news/, /, /Old Guide/a.b]\n      \
                        to: https://example.com/blog?tag=new#top\n";
        assert_eq!(
            redirects(site_cfg),
            [
                "/guide/setup to /guide/install#usage",
                "/setup.html to /guide/install#usage",
                "/news/ to https://example.com/blog?tag=new#top",
                "/ to https://example.com/blog?tag=new#top",
                "/Old Guide/a.b to https://example.com/blog?tag=new#top",
            ]
        );
        // One from every page under a folder leads to every page under
        // another, even from, or to, every page of the site.
        let site_cfg = "site:\n  redirects:\n    - { to: /tools/*, from: [/widgets/*, /Old Guide/*] }\n    \
                        - { to: /manual/*, from: [/*] }\n    \
                        - { to: \"https://example.com/*\", from: [/all] }\n    \
                        - { to: \"/tools/?tag=/*\", from: [/tagged] }\n";
        assert_eq!(
            redirects(site_cfg),
            [
                "/widgets/* to /tools/*",
                "/Old Guide/* to /tools/*",
                "/* to /manual/*",
                "/all to https://example.com/*",
                "/tagged to /tools/?tag=/*",
            ]
        );
        let folders = |from: &str, to: &str| {
            let (from, to) = (from.to_string(), to.to_string());
            let redirect = Redirect { from, to };
            redirect
                .folders()
                .map(|(from, to)| format!("{from} to {to}"))
        };
        assert_eq!(
            folders("/widgets/*", "/tools/*").unwrap(),
            "/widgets/ to /tools/"
        );
        assert_eq!(folders("/*", "/manual/*").unwrap(), "/ to /manual/");
        assert_eq!(folders("/all", "https://example.com/*"), None);
        assert_eq!(folders("/widgets", "/tools"), None);
        assert_eq!(folders("/widgets/*", "/tools/?tag=/*"), None);

        // A site's list replaces its theme's, as any list does.
        assert_eq!(redirects(IN_THEME), ["/themes to /theme"]);
        let site_cfg = format!("{IN_THEME}  redirects:\n    - {{ to: /new, from: [/old] }}\n");
        assert_eq!(redirects(&site_cfg), ["/old to /new"]);

        // One to a page or file in the site leads to its URL, without any
        // `?query` or `#fragment`.
        let target = |to: &str| {
            let from = "/old".to_string();
            let to = to.to_string();
            Redirect { from, to }.target().map(str::to_string)
        };
        for (to, page) in [
            ("/", "/"),
            ("/guide/install", "/guide/install"),
            ("/guide/#usage", "/guide/"),
            ("/files/guide.pdf?download#page=2", "/files/guide.pdf"),
        ] {
            assert_eq!(target(to).as_deref(), Some(page), "{to}");
        }
        assert_eq!(target("https://example.com/guide/"), None);
        assert_eq!(target("http://example.com"), None);
    }

    #[test]
    fn bad_redirects_explain_themselves() {
        let (_tmp, site) = scratch(&[]);
        let redirects =
            |redirects: &str| error(&site, &format!("site:\n  redirects:{redirects}\n"));

        // A list of redirects, each its settings...
        for value in [
            " /old",
            " true",
            " { /old: /new }",
            " { to: /new, from: [/old] }",
            "\n    - /old",
            "\n    - [/old, /new]",
        ] {
            assert_eq!(
                redirects(value),
                "`site.redirects` must list the site's redirects, each with the URL it leads \
                 `to`, and the URLs it's `from`, like \
                 `- { to: /guide/install, from: [/guide/setup] }`",
                "{value}"
            );
        }
        // ...which are `to` and `from`.
        assert_eq!(
            redirects(" [{ to: /new, from: [/old], status: 301 }]"),
            "a redirect in `site.redirects` sets `status`, but each has only `to`, the URL it \
             leads to, and `from`, the URLs that lead there"
        );

        // `to` is a URL from the root of the site, or another site's address.
        for entry in ["{ from: [/old] }", "{ to: , from: [/old] }"] {
            assert_eq!(
                redirects(&format!(" [{entry}]")),
                "each redirect in `site.redirects` needs a `to`, the URL it leads to, like \
                 `to: /guide/install`",
                "{entry}"
            );
        }
        for (to, written) in [
            ("guide/install", "guide/install"),
            ("install.md", "install.md"),
            ("//example.com/guide", "//example.com/guide"),
            ("ftp://example.com/guide", "ftp://example.com/guide"),
            ("mailto:docs@example.com", "mailto:docs@example.com"),
            ("\"\"", "''"),
            ("1.4", "1.4"),
            ("[/new]", "- /new"),
        ] {
            assert_eq!(
                redirects(&format!(" [{{ to: {to}, from: [/old] }}]")),
                format!(
                    "`site.redirects` has a redirect to `{written}`, but each must lead to a \
                     URL from the root of the site, as `site.navigation.items` writes it, like \
                     `/guide/install`, or to another site's address, starting with https:// or \
                     http://"
                ),
                "{to}"
            );
        }

        // `from` is always a list, so there's one way to write it...
        for from in [
            "",
            ", from:",
            ", from: /old",
            ", from: []",
            ", from: { url: /old }",
        ] {
            assert_eq!(
                redirects(&format!(" [{{ to: /new#here{from} }}]")),
                "the redirect to /new#here in `site.redirects` needs a `from`, a list of the \
                 URLs that lead there, like `from: [/guide/setup]`",
                "{from}"
            );
        }
        // ...of URLs from the root of the site, each of which little can
        // write a page at, in the site's folder.
        for (from, written) in [
            ("old", "old"),
            ("old.md", "old.md"),
            ("https://example.com/old", "https://example.com/old"),
            ("//old", "//old"),
            ("/old?page=2", "/old?page=2"),
            ("\"/old#usage\"", "/old#usage"),
            ("/a//b", "/a//b"),
            ("/a/./b", "/a/./b"),
            ("/../../etc/passwd", "/../../etc/passwd"),
            ("/a/..", "/a/.."),
            ("'/a\\b'", "/a\\b"),
            ("/c:/old", "/c:/old"),
            ("/a*", "/a*"),
            ("/a/*/b", "/a/*/b"),
            ("/a/**", "/a/**"),
            ("\"*\"", "'*'"),
            ("1.4", "1.4"),
            ("[/old]", "- /old"),
            ("", "null"),
        ] {
            assert_eq!(
                redirects(&format!("\n    - to: /new\n      from:\n        - {from}")),
                format!(
                    "the redirect to /new in `site.redirects` is from `{written}`, but each \
                     must be from a URL from the root of the site, where a page was, like \
                     `/guide/setup` or `/setup.html`, with no `?` or `#`, or from every page \
                     under a folder, like `/guide/*`"
                ),
                "{from}"
            );
        }

        // A `*` is only for every page under a folder, at the end of a URL
        // in the site...
        for to in ["/a*", "/a/*/b", "/a/**", "/*/*", "\"/a/*#usage\""] {
            let to = to.trim_matches('"');
            assert_eq!(
                redirects(&format!(" [{{ to: \"{to}\", from: [/old/*] }}]")),
                format!(
                    "`site.redirects` has a redirect to `{to}`, but a `*` is only for every \
                     page under a folder, as the end of a URL, like `/tools/*`"
                ),
                "{to}"
            );
        }
        // ...which leads to every page under another, in the site.
        for to in ["/new", "/new/", "https://example.com/new/*"] {
            assert_eq!(
                redirects(&format!(" [{{ to: \"{to}\", from: [/a, /old/*] }}]")),
                format!(
                    "`site.redirects` sends /old/*, every page under a folder, to {to}, but \
                     each leads to the same page under another of the site's folders, like \
                     `to: /tools/*`"
                ),
                "{to}"
            );
        }
        for (from, like) in [("/old", "/old/*"), ("/old/", "/old/*"), ("/", "/*")] {
            assert_eq!(
                redirects(&format!(" [{{ to: /new/*, from: [/a/*, {from}] }}]")),
                format!(
                    "`site.redirects` sends {from} to /new/*, every page under a folder, so \
                     it must be from every page under one too, like `{like}`"
                ),
                "{from}"
            );
        }

        // One to the URL it's from leads nowhere, whether or not either
        // ends in a `/`...
        for (to, from) in [("/a", "/a"), ("/a/#usage", "/a"), ("/", "/")] {
            assert_eq!(
                redirects(&format!(" [{{ to: {to}, from: [/b, {from}] }}]")),
                format!("`site.redirects` sends {from} to itself"),
                "{to}"
            );
        }
        assert_eq!(
            redirects(" [{ to: /a/*, from: [/b/*, /a/*] }]"),
            "`site.redirects` sends /a/* to itself"
        );
        // ...and one to a URL that's redirected would send a reader on twice.
        let chained = "\n    - { to: /b/#usage, from: [/a] }\n    - { to: https://example.com/c, from: [/b] }";
        assert_eq!(
            redirects(chained),
            "`site.redirects` sends /a to /b/#usage, but sends /b on to https://example.com/c: \
             send /a to https://example.com/c too, so a reader is sent on once"
        );
        let chained = "\n    - { to: /b/*, from: [/a/*] }\n    - { to: /c/*, from: [/b/*] }";
        assert_eq!(
            redirects(chained),
            "`site.redirects` sends /a/* to /b/*, but sends /b/* on to /c/*: send /a/* to /c/* \
             too, so a reader is sent on once"
        );
    }

    #[test]
    fn markdown_cascades() {
        let (_tmp, site) = scratch(&[("theme/little.yml", "site: { markdown: { math: tex } }\n")]);
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
        assert_eq!(builtin.mermaid, Some(json!({})));
        assert!(!builtin.mark);
        assert_eq!(builtin.highlight, Some(Highlight::Pygments));

        // A theme that renders math keeps its TeX, and a site turns other
        // extensions off, each merging over the settings beneath it.
        let site_cfg = format!("{IN_THEME}  markdown:\n    raw_html: false\n    mermaid: false\n");
        let expected = Markdown {
            math: Some(Math::Tex),
            raw_html: false,
            mermaid: None,
            ..Markdown::default()
        };
        assert_eq!(markdown(&site_cfg), expected);

        // A site has the last word on math, as on anything.
        let site_cfg = format!("{IN_THEME}  markdown:\n    math: mathml\n");
        assert_eq!(markdown(&site_cfg).math, Some(Math::MathML));
        let site_cfg = format!("{IN_THEME}  markdown:\n    math: false\n");
        assert_eq!(markdown(&site_cfg).math, None);

        // A site leaves diagrams as code, for a theme that draws them.
        let site_cfg = "site: { markdown: { mermaid: false } }\n";
        assert_eq!(markdown(site_cfg).mermaid, None);
        let site_cfg = "site: { markdown: { mermaid: svg } }\n";
        assert_eq!(markdown(site_cfg).mermaid, Some(json!({})));

        // A site chooses GitHub's classes, or none.
        let site_cfg = "site: { markdown: { highlight: github } }\n";
        assert_eq!(markdown(site_cfg).highlight, Some(Highlight::GitHub));
        let site_cfg = "site: { markdown: { highlight: false } }\n";
        assert_eq!(markdown(site_cfg).highlight, None);
    }

    #[test]
    fn bad_markdown_explains_itself() {
        let (_tmp, site) = scratch(&[]);
        let setting = |setting: &str| format!("site:\n  markdown:\n    {setting}\n");
        // YAML reads `yes` and a quoted `"true"` as text, not `true`.
        for value in [" yes", " \"true\"", " 1", ""] {
            let message = error(&site, &setting(&format!("mark:{value}")));
            assert!(
                message.starts_with("reading `site.markdown`: mark: "),
                "{message}"
            );
            assert!(message.ends_with("expected a boolean"), "{message}");
        }
        for value in [" true", "", " [math]"] {
            let message = error(&site, &format!("site:\n  markdown:{value}\n"));
            assert!(
                message.starts_with("reading `site.markdown`: "),
                "{message}"
            );
        }

        // A typo is named, beside the settings there are.
        let message = error(&site, &setting("tabels: true"));
        let expected = "reading `site.markdown`: tabels: unknown field `tabels`, expected one \
                        of `tables`, `strikethrough`, `autolinks`, ";
        assert!(message.starts_with(expected), "{message}");
        assert!(
            message.ends_with(", `cjk_friendly_emphasis`, `highlight`"),
            "{message}"
        );

        // Math is written in a form little has, or not at all: not `true`,
        // which would undo a theme's `tex`.
        for value in ["true", "MathML", "katex", "\"false\"", ""] {
            let message = error(&site, &setting(&format!("math: {value}")));
            assert!(
                message.starts_with("reading `site.markdown`: math: "),
                "{message}"
            );
            assert!(message.ends_with("expected `mathml` or `tex`"), "{message}");
        }

        // Code is highlighted with a set of classes little has, or none.
        for value in ["true", "monokai", "Pygments", "\"false\"", ""] {
            let message = error(&site, &setting(&format!("highlight: {value}")));
            assert!(
                message.starts_with("reading `site.markdown`: highlight: "),
                "{message}"
            );
            assert!(
                message.ends_with("expected `pygments` or `github`"),
                "{message}"
            );
        }

        // Diagrams are drawn as SVG or left as code: not `true`, which would
        // undo a theme's `false`.
        for value in ["true", "SVG", "png", "\"false\"", ""] {
            let message = error(&site, &setting(&format!("mermaid: {value}")));
            assert_eq!(
                message,
                "reading `site.markdown`: mermaid: expected `svg` or `false`"
            );
        }

        // A diagram can carry HTML, so a site without raw HTML says so.
        for settings in ["raw_html: false", "raw_html: false\n    mermaid: svg"] {
            let message = error(&site, &setting(settings));
            assert_eq!(
                message,
                "`site.markdown.mermaid: svg` needs `site.markdown.raw_html` to be `true`, as a \
                 diagram can carry HTML: set `site.markdown.mermaid: false` too"
            );
        }

        // comrak reads `^[…]` only with footnotes.
        let message = error(
            &site,
            &setting("footnotes: false\n    inline_footnotes: true"),
        );
        assert_eq!(
            message,
            "`site.markdown.inline_footnotes` needs `site.markdown.footnotes` to be `true` too"
        );
    }

    #[test]
    fn mermaid_cascades() {
        let theme_cfg = "site:\n  theme:\n    mermaid:\n      theme: base\n      \
                         themeVariables:\n        primaryColor: \"#6f42c1\"\n        \
                         lineColor: \"#333333\"\n";
        let (_tmp, site) = scratch(&[("theme/little.yml", theme_cfg)]);
        let mermaid = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            (load(&site).unwrap().markdown.mermaid).expect("diagrams should be drawn")
        };

        // little sets no look, so diagrams have Mermaid's default.
        assert_eq!(mermaid(""), json!({}));

        // A theme sets its look, and a site changes it, setting by setting,
        // beside the theme it chooses.
        let theme = json!({
            "theme": "base",
            "themeVariables": { "primaryColor": "#6f42c1", "lineColor": "#333333" },
        });
        assert_eq!(mermaid(IN_THEME), theme);
        let site_cfg = format!(
            "{IN_THEME}    mermaid:\n      themeVariables:\n        primaryColor: \"#0969da\"\n"
        );
        let expected = json!({
            "theme": "base",
            "themeVariables": { "primaryColor": "#0969da", "lineColor": "#333333" },
        });
        assert_eq!(mermaid(&site_cfg), expected);

        // Every setting is passed on as it's written, in Mermaid's names, for
        // Mermaid to read, or, like a typo, ignore.
        let site_cfg = "site:\n  theme:\n    mermaid:\n      look: handDrawn\n      \
                        fontFamily: Georgia, serif\n      gantt:\n        useWidth: 1000\n      \
                        themeVariable:\n        primaryColor: \"#0969da\"\n";
        let expected = json!({
            "look": "handDrawn",
            "fontFamily": "Georgia, serif",
            "gantt": { "useWidth": 1000 },
            "themeVariable": { "primaryColor": "#0969da" },
        });
        assert_eq!(mermaid(site_cfg), expected);

        // Diagrams left as code, for a theme's script, aren't drawn, so their
        // look isn't read, but the theme still has it.
        let site_cfg =
            format!("{IN_THEME}    mermaid:\n      theme: drak\n  markdown:\n    mermaid: false\n");
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
        let config = load(&site).unwrap();
        assert_eq!(config.markdown.mermaid, None);
        assert_eq!(get(&config.site, "theme.mermaid.theme"), Some("drak"));
    }

    #[test]
    fn bad_mermaid_explains_itself() {
        let (_tmp, site) = scratch(&[]);
        let mermaid = |settings: &str| format!("site:\n  theme:\n    mermaid:{settings}\n");
        for value in [" forest", "", " [theme]"] {
            let message = error(&site, &mermaid(value));
            assert_eq!(
                message,
                "`site.theme.mermaid` must be Mermaid's settings, like `theme: forest`"
            );
        }

        // Mermaid would swap a theme it doesn't have for its default.
        for value in ["drak", "Dark", "3", "true", "\"\"", ""] {
            let message = error(&site, &mermaid(&format!("\n      theme: {value}")));
            assert_eq!(
                message,
                "`site.theme.mermaid.theme` must be one of Mermaid's themes: default, base, \
                 dark, forest, neutral, neo, neo-dark, redux, redux-dark, redux-color, \
                 redux-dark-color"
            );
        }

        // YAML reads a colour after `#` as a comment, leaving its setting
        // empty, which is named, however deep it is.
        let settings = "\n      theme: base\n      themeVariables:\n        primaryColor: #6f42c1";
        let message = error(&site, &mermaid(settings));
        assert_eq!(
            message,
            "`site.theme.mermaid.themeVariables.primaryColor` is empty: put a colour in quotes, \
             like \"#6f42c1\", or YAML reads it as a comment"
        );
        let message = error(&site, &mermaid("\n      fontFamily:"));
        assert!(
            message.starts_with("`site.theme.mermaid.fontFamily` is empty: "),
            "{message}"
        );
    }

    #[test]
    fn host_and_port_cascade() {
        let (_tmp, site) = scratch(&[("theme/little.yml", "site: { serve: { port: 4000 } }\n")]);
        let address = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            let cfg = load(&site).unwrap();
            (cfg.host, cfg.port)
        };

        // little serves at 127.0.0.1:8080, a theme can change that, and a site
        // has the last word.
        assert_eq!(address(""), ("127.0.0.1".into(), 8080));
        assert_eq!(address(IN_THEME), ("127.0.0.1".into(), 4000));
        let site_cfg = format!("{IN_THEME}  serve:\n    host: localhost\n    port: 3000\n");
        assert_eq!(address(&site_cfg), ("localhost".into(), 3000));

        // An IPv6 address needs no quotes or brackets.
        let site_cfg = "site:\n  serve:\n    host: ::1\n";
        assert_eq!(address(site_cfg), ("::1".into(), 8080));
    }

    #[test]
    fn bad_hosts_and_ports_explain_themselves() {
        let (_tmp, site) = scratch(&[]);

        // A host is a name or an address, written as text.
        for value in [" 8080", " [a, b]", " \"\"", ""] {
            let message = error(&site, &format!("site:\n  serve:\n    host:{value}\n"));
            assert_eq!(
                message,
                "`site.serve.host` must be a host name or IP address, like 127.0.0.1"
            );
        }

        // Port 0 would have the system pick a port, there's none past 65535,
        // and a port is a number, not text, nor an address with it.
        for value in [" 0", " 65536", " -1", " \"8080\"", " 127.0.0.1:3000", ""] {
            let message = error(&site, &format!("site:\n  serve:\n    port:{value}\n"));
            assert_eq!(
                message,
                "`site.serve.port` must be a number from 1 to 65535, like 8080"
            );
        }

        // An address where its settings go isn't them, nor is nothing.
        for value in [" 8080", " 127.0.0.1:8080", ""] {
            let message = error(&site, &format!("site:\n  serve:{value}\n"));
            assert_eq!(
                message,
                "`site.serve` must be its settings, like `port: 8080`"
            );
        }
    }

    #[test]
    fn a_url_puts_the_site_under_its_path() {
        let (_tmp, site) = scratch(&[]);
        let url = |url: &str| {
            std::fs::write(site.join(CONFIG_FILE), format!("site:\n  url: {url}\n")).unwrap();
            load(&site).unwrap().url
        };
        let site_url = |origin: &str, path: &str| SiteUrl {
            origin: origin.into(),
            path: path.into(),
            shared: None,
        };

        // Without a url, a site is at the root of whichever domain serves it.
        assert_eq!(load(&site).unwrap().url, SiteUrl::default());

        // A `/` at the end of the path is left off, so links don't get two.
        let project = site_url("https://withlittle.github.io", "/little");
        assert_eq!(url("https://withlittle.github.io/little"), project);
        assert_eq!(url("https://withlittle.github.io/little/"), project);
        let domain = site_url("https://docs.example.com", "");
        assert_eq!(url("https://docs.example.com"), domain);
        assert_eq!(url("https://docs.example.com/"), domain);
        let nested = site_url("http://localhost:4000", "/a/b");
        assert_eq!(url("http://localhost:4000/a/b"), nested);
    }

    #[test]
    fn bad_urls_explain_themselves() {
        let (_tmp, site) = scratch(&[]);

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
            let message = error(&site, &format!("site:\n  url:{value}\n"));
            assert_eq!(
                message,
                "`site.url` must be the address the site is published at, starting with \
                 https:// or http://, like https://example.github.io/my-project",
                "url:{value}"
            );
        }
    }

    #[test]
    fn links_from_the_root_get_the_sites_path() {
        let url = testing::site_url();
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
            shared: None,
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
    fn a_version_links_to_the_theme_files_at_the_root() {
        let shared = |path: &str| Shared {
            path: path.into(),
            files: ["/assets/style.css", "/assets/my font.woff2"]
                .map(String::from)
                .into(),
        };
        let version = SiteUrl {
            path: "/little/1.4".into(),
            shared: Some(shared("/little")),
            ..testing::site_url()
        };
        // The theme's files are the root's, whatever follows their path, and
        // however it's encoded.
        for (link, expected) in [
            ("/assets/style.css", "/little/assets/style.css"),
            ("/assets/style.css?v=2", "/little/assets/style.css?v=2"),
            ("/assets/style.css#top", "/little/assets/style.css#top"),
            ("/assets/my%20font.woff2", "/little/assets/my%20font.woff2"),
            ("/assets/my font.woff2", "/little/assets/my font.woff2"),
            // The version's own pages and files are its own.
            ("/", "/little/1.4/"),
            ("/guide/", "/little/1.4/guide/"),
            ("/assets/", "/little/1.4/assets/"),
            ("/assets/style.css/", "/little/1.4/assets/style.css/"),
            ("/assets/STYLE.css", "/little/1.4/assets/STYLE.css"),
            (
                "/pagefind/pagefind-ui.js",
                "/little/1.4/pagefind/pagefind-ui.js",
            ),
            // And links elsewhere are left alone.
            (
                "https://example.com/assets/style.css",
                "https://example.com/assets/style.css",
            ),
            ("assets/style.css", "assets/style.css"),
        ] {
            assert_eq!(version.link(link), expected, "{link}");
        }
        assert_eq!(
            version.absolute("/little/assets/style.css"),
            "https://withlittle.github.io/little/assets/style.css"
        );

        // On a site at the root of its domain, they're at its root.
        let version = SiteUrl {
            origin: String::new(),
            path: "/1.4".into(),
            shared: Some(shared("")),
        };
        assert_eq!(version.link("/assets/style.css"), "/assets/style.css");
        assert_eq!(version.link("/guide/"), "/1.4/guide/");
    }

    #[test]
    fn a_repo_and_its_branch_cascade() {
        let theme_cfg = "site: { repository: { branch: trunk } }\n";
        let (_tmp, site) = scratch(&[("theme/little.yml", theme_cfg)]);
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

        // Without a repository's `url`, there's nothing to link to.
        assert_eq!(repo(""), None);
        assert_eq!(repo(IN_THEME), None);

        // little links to files on `main`, a theme can change that, and a
        // site has the last word. A `/` at the end of the `url` is left off.
        let repository = "  repository:\n    url: https://github.com/withlittle/little\n";
        assert_eq!(repo(&format!("site:\n{repository}")), little("main"));
        let slashed = repository.replace("little\n", "little/\n");
        assert_eq!(repo(&format!("site:\n{slashed}")), little("main"));
        let site_cfg = format!("{IN_THEME}{repository}");
        assert_eq!(repo(&site_cfg), little("trunk"));
        let site_cfg = format!("{site_cfg}    branch: \"2.0\"\n");
        assert_eq!(repo(&site_cfg), little("2.0"));
    }

    #[test]
    fn bad_repos_and_branches_explain_themselves() {
        let (_tmp, site) = scratch(&[]);

        // A repository's `url` is the address a browser shows for it, as
        // text: not the address it's cloned from, or just its host, and with
        // nothing after its path.
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
            let message = error(&site, &format!("site:\n  repository:\n    url:{value}\n"));
            assert_eq!(
                message,
                "`site.repository.url` must be the address of the site's repository, as a \
                 browser shows it, like https://github.com/example/my-project",
                "url:{value}"
            );
        }

        // YAML reads a branch like `2.0` as a number, which isn't its name.
        for value in [" 2.0", " 2024", " true", " \"\"", " [main]", ""] {
            let message = error(
                &site,
                &format!("site:\n  repository:\n    branch:{value}\n"),
            );
            assert_eq!(
                message,
                "`site.repository.branch` must be the name of a branch, like main, in quotes if \
                 it looks like a number, like \"2.0\"",
                "branch:{value}"
            );
        }

        // An address where its settings go isn't them, nor is nothing.
        for value in [" https://github.com/withlittle/little", ""] {
            let message = error(&site, &format!("site:\n  repository:{value}\n"));
            assert_eq!(
                message,
                "`site.repository` must be its settings, like \
                 `url: https://github.com/example/my-project`"
            );
        }
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
    fn a_repo_has_an_owner_and_a_name() {
        let named = |url: &str| {
            let repo = Repo {
                url: url.into(),
                branch: "main".into(),
            };
            let (owner, name) = repo.owner_and_name();
            (owner.unwrap_or_default(), name)
        };
        let is = |owner: &str, name: &str| (owner.to_string(), name.to_string());

        assert_eq!(
            named("https://github.com/withlittle/little"),
            is("withlittle", "little")
        );
        // A GitLab project can be in a group, in a group.
        assert_eq!(
            named("https://gitlab.com/withlittle/docs/little"),
            is("withlittle/docs", "little")
        );
        // One at the top of its host has no owner.
        let repo = Repo {
            url: "https://git.example.com/little".into(),
            branch: "main".into(),
        };
        assert_eq!(repo.owner_and_name(), (None, "little".to_string()));
    }

    #[test]
    fn config_files_hold_settings() {
        let (_tmp, site) = scratch(&[]);
        let parsing = format!("parsing {}: ", site.join(CONFIG_FILE).display());

        // Settings are keys and values: a list or a single value would replace
        // them all.
        for site_cfg in ["- site: { source: ./docs }\n", "My Docs\n"] {
            let message = error(&site, site_cfg);
            assert!(message.starts_with(&parsing), "{message}");
            assert!(message.contains("expected a YAML mapping"), "{message}");
        }

        // YAML that doesn't parse says where.
        let message = error(&site, "site:\n  name: \"My Docs\n");
        assert!(message.starts_with(&parsing), "{message}");
        assert!(message.contains("at line"), "{message}");
    }

    #[test]
    fn config_files_are_optional_and_can_be_empty() {
        let (_tmp, site) = scratch(&[("theme/_templates/default.html", "")]);
        let loaded = || {
            let cfg = load(&site).unwrap();
            (cfg.site, cfg.page)
        };

        // Without a little.yml, or with one that's empty or only comments, a
        // site gets little's defaults and the built-in theme's.
        let builtin = with_builtin_theme();
        let builtin = (builtin.site, builtin.page);
        assert_eq!(loaded(), builtin);
        for site_cfg in ["", "# nothing to set yet\n"] {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            assert_eq!(loaded(), builtin);
        }

        // The same goes for a theme's little.yml.
        std::fs::write(site.join(CONFIG_FILE), IN_THEME).unwrap();
        let Level {
            site: mut expected,
            page,
        } = builtin_defaults();
        let Some(Value::Mapping(theme)) = expected.get_mut("theme") else {
            panic!("little should default `theme`");
        };
        theme.insert("source".into(), "./theme".into());
        let expected = (expected, page);
        assert_eq!(loaded(), expected);
        std::fs::write(site.join("theme").join(CONFIG_FILE), "# nothing\n").unwrap();
        assert_eq!(loaded(), expected);
    }

    #[test]
    fn little_defaults_only_its_own_settings() {
        let Level { site, page } = builtin_defaults();
        assert_eq!(get(&site, "source"), Some("./docs"));
        assert_eq!(get(&site, "destination"), Some("./dist"));
        let number = |path: &str| value(&site, path).and_then(Value::as_u64);
        assert_eq!(value(&site, "theme.enabled"), Some(&true.into()));
        assert_eq!(get(&site, "serve.host"), Some("127.0.0.1"));
        assert_eq!(number("serve.port"), Some(8080));
        // Search is a theme's to turn on, with a box to search in.
        assert_eq!(value(&site, "search.enabled"), Some(&false.into()));
        // Each page gets the whole nav, unless a big site prunes it, and
        // little lists a site's pages unless the site does.
        assert_eq!(value(&site, "navigation.prune"), Some(&false.into()));
        assert_eq!(value(&site, "navigation.items"), None);
        // A site's name, language, and look are the theme's to default.
        for key in [
            "name",
            "language",
            "theme.source",
            "theme.mermaid",
            "theme.fonts",
        ] {
            assert_eq!(value(&site, key), None, "little shouldn't default `{key}`");
        }
        // And only the site knows where it's published, and where its
        // repository is, though most link to their files on `main`.
        assert_eq!(value(&site, "url"), None);
        assert_eq!(value(&site, "repository.url"), None);
        assert_eq!(get(&site, "repository.branch"), Some("main"));
        // The 404 page says what it is.
        assert_eq!(get(&site, "error.title"), Some("Page not found"));
        assert!(get(&site, "error.description").is_some());
        // Every page has each thing little works out for it, with every
        // heading but its `#` title in its table of contents, and is in
        // search unless the site leaves it out. Any other key is a theme's
        // to set, and a site's.
        let settings = PageSettings::read(&page, "page.").unwrap();
        assert_eq!(settings, every_page(2..=6));
        let groups = ["outline", "breadcrumb", "previous", "next", "repository"];
        assert_eq!(Vec::from_iter(page.keys()), groups);
    }

    #[test]
    fn the_builtin_theme_builds_on_nothing() {
        // The chain ends with the built-in theme, so a `theme.enabled` or a
        // `theme.source` in its config would be ignored.
        let theme = builtin_theme();
        for key in ["theme.enabled", "theme.source"] {
            assert_eq!(value(&theme.site, key), None, "{key}");
        }
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
        let theme_cfg = under_site(
            "plugins:\n  bob:\n    wasm: ./plugins/bob.wasm\n  \
             ann:\n    wasm: https://example.com/ann.wasm\n    style: plain\n    width: 80\n  \
             old:\n    wasm: ./old.wasm\n  \
             new:\n    wasm: ./new.wasm\n    enabled: false\n",
        );
        let (_tmp, site) = scratch(&[
            ("theme/_templates/default.html", ""),
            ("theme/little.yml", &theme_cfg),
        ]);
        // The site changes one of the theme's settings, turns one of its
        // plugins off, and another on, and adds its own, which reads two
        // folders.
        let site_cfg = format!(
            "{IN_THEME}  plugins:\n    ann:\n      style: fancy\n    \
             old:\n      enabled: false\n    new:\n      enabled: true\n    \
             cal:\n      wasm: tools/cal.wasm\n      read: [./src/, examples]\n      depth: 2\n"
        );
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();

        // Each path leads from the little.yml that wrote it, and the plugin
        // is told the folders it reads as it sees them, in its settings,
        // which are its own, but for `read`.
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
                plugin("new", Wasm::File(site.join("theme/new.wasm")), &[], "{}"),
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
        let site_cfg = "site:\n  plugins:\n    cal:\n      wasm: ./cal.wasm\n      read: .\n";
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
        for site_cfg in ["", "site:\n  plugins:\n", "site: { plugins: {} }\n"] {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            assert_eq!(load(&site).unwrap().plugins, [], "{site_cfg}");
        }
    }

    #[test]
    fn plugins_must_be_named_with_their_wasm() {
        let (_tmp, site) = scratch(&[("theme/_templates/default.html", "")]);
        let file = site.join(CONFIG_FILE);
        let file = file.display();
        // A file alone, `true`, `false`, and nothing aren't its settings.
        let not_settings = format!(
            "plugin `bob` in {file} must be its settings, like `wasm: ./plugins/bob.wasm`, for \
             its `.wasm` file, or `enabled: false`, to turn it off"
        );
        for (plugins, expected) in [
            (
                "plugins: [./bob.wasm]\n",
                format!(
                    "`site.plugins` in {file} must name each plugin, with its settings, like \
                     `bob: {{ wasm: ./plugins/bob.wasm }}`"
                ),
            ),
            ("plugins:\n  bob: ./bob.wasm\n", not_settings.clone()),
            ("plugins:\n  bob: false\n", not_settings.clone()),
            ("plugins:\n  bob: true\n", not_settings.clone()),
            ("plugins:\n  bob:\n", not_settings.clone()),
            (
                "plugins:\n  bob:\n    style: plain\n",
                "plugin `bob` needs `wasm`, the path or URL of its `.wasm` file".to_string(),
            ),
            (
                "plugins:\n  bob:\n    wasm: http://example.com/bob.wasm\n",
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
            // YAML reads `no` as text, not `false`.
            (
                "plugins:\n  bob:\n    wasm: ./bob.wasm\n    enabled: no\n",
                "`enabled` of plugin `bob` must be `true`, or `false`, to turn it off".to_string(),
            ),
        ] {
            let message = error(&site, &under_site(plugins));
            assert!(message.starts_with(&expected), "{plugins}: {message}");
        }

        // Only the site chooses what a plugin reads, not a theme it uses.
        let theme_cfg = under_site("plugins:\n  bob:\n    wasm: ./bob.wasm\n    read: /\n");
        std::fs::write(site.join("theme").join(CONFIG_FILE), theme_cfg).unwrap();
        let message = error(&site, IN_THEME);
        let theme_file = site.join("theme").join(CONFIG_FILE);
        let theme_file = theme_file.display();
        assert!(
            message.contains(&format!(
                "plugin `bob` in {theme_file} sets `read`, but only the site's little.yml can \
                 choose what a plugin reads"
            )),
            "{message}"
        );
    }

    #[test]
    fn versions_have_titles_and_urls_and_are_built_from_refs() {
        let (_tmp, site) = scratch(&[]);
        // Each is the site's settings, written from their first column.
        let versions = |settings: &str| {
            std::fs::write(site.join(CONFIG_FILE), under_site(settings)).unwrap();
            load(&site).unwrap().versions
        };
        let version = |title: &str, path: &str, git_ref: Option<&str>| Version {
            title: title.into(),
            path: path.into(),
            git_ref: git_ref.map(Into::into),
            source: None,
            nav: None,
            plugins: Mapping::new(),
        };

        // A site without versions has just the one.
        for settings in ["", "versions:\n", "versions: []\n"] {
            assert_eq!(versions(settings), [], "{settings}");
        }
        // In the menu's order, one at `/`, wherever it's listed, each from a
        // tag, a branch, or a commit, but for one, at most, from the site's
        // files as they are. A title is any text, and a `url` can end in `/`
        // or not. Each can have settings of its own, for the theme.
        let settings = "versions:\n  - title: \"2.0 (Alpha)\"\n    url: /2.0-alpha\n    \
                        label: next\n  - title: 1.10 (Latest)\n    url: /\n    ref: v1.10\n  \
                        - title: 1.x\n    url: /versions/1.x/\n    ref: origin/1.x\n  \
                        - { title: Old docs, url: /old_docs, ref: \"1234567\" }\n";
        assert_eq!(
            versions(settings),
            [
                version("2.0 (Alpha)", "/2.0-alpha", None),
                version("1.10 (Latest)", "", Some("v1.10")),
                version("1.x", "/versions/1.x", Some("origin/1.x")),
                version("Old docs", "/old_docs", Some("1234567")),
            ]
        );
        // A blank `ref` is none, and every version can be from a ref.
        let settings = "versions:\n  - title: a\n    url: /\n    ref:\n  \
                        - { title: b, url: /b, ref: v1 }\n";
        assert_eq!(
            versions(settings),
            [version("a", "", None), version("b", "/b", Some("v1"))]
        );
        let settings = "versions:\n  - { title: a, url: /, ref: v2 }\n  \
                        - { title: b, url: /b, ref: v1 }\n";
        assert_eq!(
            versions(settings),
            [version("a", "", Some("v2")), version("b", "/b", Some("v1"))]
        );
        // The version at `/` can have others in its folders, as can others
        // with the same start to their paths.
        let settings = "versions:\n  - { title: a, url: / }\n  \
                        - { title: b, url: /versions/1, ref: v1 }\n  \
                        - { title: c, url: /versions/10, ref: v10 }\n";
        assert_eq!(
            versions(settings),
            [
                version("a", "", None),
                version("b", "/versions/1", Some("v1")),
                version("c", "/versions/10", Some("v10")),
            ]
        );
        // A version from a ref can name the folder its pages are in there,
        // and a blank `source` is none.
        let settings = "versions:\n  - { title: a, url: / }\n  \
                        - { title: b, url: /b, ref: v1, source: ./documentation }\n  \
                        - { title: c, url: /c, ref: v0, source: }\n";
        let mut b = version("b", "/b", Some("v1"));
        b.source = Some("./documentation".into());
        assert_eq!(
            versions(settings),
            [version("a", "", None), b, version("c", "/c", Some("v0"))]
        );
        // And its own nav, as `navigation.items`, and settings for the
        // site's plugins, each over the site's; a blank `navigation`, or
        // `items`, or `plugins` is none. Any other key, like `name`, is for
        // the theme.
        let settings = "plugins:\n  api: { wasm: ./api.wasm }\n  bob: { wasm: ./bob.wasm }\n\
                        versions:\n  - { title: a, url: /, plugins: {}, navigation: {} }\n  \
                        - title: b\n    url: /b\n    ref: v1\n    name: Old\n    \
                        navigation:\n      items: [{ title: Home, url: / }]\n    \
                        plugins:\n      api: { read: ./lib, style: plain }\n      \
                        bob: { enabled: false }\n  \
                        - title: c\n    url: /c\n    ref: v0\n    navigation:\n    plugins:\n  \
                        - { title: d, url: /d, ref: v0, navigation: { items: } }\n";
        let mut b = version("b", "/b", Some("v1"));
        b.nav = Some(serde_yaml::from_str("[{ title: Home, url: / }]").unwrap());
        b.plugins =
            serde_yaml::from_str("{ api: { read: ./lib, style: plain }, bob: { enabled: false } }")
                .unwrap();
        assert_eq!(
            versions(settings),
            [
                version("a", "", None),
                b,
                version("c", "/c", Some("v0")),
                version("d", "/d", Some("v0")),
            ]
        );
    }

    #[test]
    fn bad_versions_explain_themselves() {
        let (_tmp, site) = scratch(&[]);
        let list = "`site.versions` must list the site's versions, each with its `title`, the \
                    `url` it's at, and the `ref` it's built from, like \
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
        // Each is the site's settings, written from their first column.
        for (settings, expected) in [
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
            (
                "versions:\n  - { title: a, url: /, ref: v1, source: [./docs] }\n",
                "version `a`'s `source` must be the path to the folder its pages are in at its \
                 `ref`, like `./docs`"
                    .to_string(),
            ),
            (
                "versions:\n  - { title: a, url: /, source: ./documentation }\n",
                "version `a` sets `source` but has no `ref`, so it's built from the site's files \
                 as they are: set the site's `source` instead"
                    .to_string(),
            ),
            (
                "versions:\n  \
                 - { title: a, url: /, navigation: { items: [{ title: Home, url: / }] } }\n",
                "version `a` sets `navigation.items` but has no `ref`, so it's built from the \
                 site's files as they are: set the site's `navigation.items` instead"
                    .to_string(),
            ),
            (
                "plugins:\n  api: { wasm: ./api.wasm }\n\
                 versions:\n  - { title: a, url: /, plugins: { api: { enabled: false } } }\n",
                "version `a` sets `plugins` but has no `ref`, so it's built from the site's files \
                 as they are: set the site's `plugins` instead"
                    .to_string(),
            ),
            // Its nav is its `navigation.items`, not its `navigation`.
            (
                "versions:\n  - { title: a, url: /, ref: v1, navigation: [{ title: Home }] }\n",
                "version `a`'s `navigation` must hold its nav, as `items`, like \
                 `navigation: { items: [{ title: Home, url: / }] }`"
                    .to_string(),
            ),
        ] {
            let site_cfg = under_site(settings);
            assert_eq!(error(&site, &site_cfg), expected, "{settings}");
        }

        // A version's `plugins` can change only the settings of those the
        // site uses.
        let site_cfg = |plugins: &str| {
            under_site(&format!(
                "plugins:\n  api: {{ wasm: ./api.wasm }}\n  \
                 bob: {{ wasm: ./bob.wasm, enabled: false }}\n\
                 versions:\n  - {{ title: a, url: /, ref: v2, plugins: {plugins} }}\n"
            ))
        };
        let not_settings = "version `a`'s `plugins` must name the site's plugins it changes, \
                            each with its settings, like `api: { enabled: false }`, to turn it \
                            off";
        let not_the_sites = |name: &str| {
            format!(
                "version `a` sets plugin `{name}`, but the site doesn't use it: a version can \
                 change only the settings of the site's plugins"
            )
        };
        let wasm = "version `a` gives plugin `api` a `.wasm` file, but every version is built \
                    with the site's: a version can change only a plugin's settings, or turn it \
                    off, with `enabled: false`";
        let neither = "plugin `api` in version `a` must be its settings, like `enabled: false`, \
                       to turn it off";
        for (plugins, expected) in [
            ("[api]", not_settings.to_string()),
            ("api", not_settings.to_string()),
            ("{ 1: { enabled: false } }", not_settings.to_string()),
            ("{ apy: { enabled: false } }", not_the_sites("apy")),
            ("{ bob: { style: plain } }", not_the_sites("bob")),
            ("{ api: { wasm: ./old/api.wasm } }", wasm.to_string()),
            ("{ api: ./old/api.wasm }", neither.to_string()),
            ("{ api: false }", neither.to_string()),
            ("{ api: true }", neither.to_string()),
            ("{ api: [./lib] }", neither.to_string()),
            ("{ api: }", neither.to_string()),
        ] {
            assert_eq!(error(&site, &site_cfg(plugins)), expected, "{plugins}");
        }

        // Nor can it set little's other settings, which are the site's.
        let sites = |key: &str| {
            format!(
                "version `a` sets `{key}`, but every version is built with the site's `{key}`: \
                 a version from a `ref` can set only its own `source`, `navigation.items`, and \
                 `plugins`"
            )
        };
        for (key, value) in [
            ("destination", "./old"),
            ("theme", "{ source: ./old-theme }"),
            ("repository", "{ branch: old }"),
            ("search", "{ enabled: false }"),
            ("markdown", "{ math: false }"),
            ("versions", "[]"),
            ("redirects", "[]"),
            ("built", "2020-01-01"),
            ("modified", "2020-01-01"),
            ("serve", "{ port: 3000 }"),
            ("error", "{ title: Gone }"),
        ] {
            let settings =
                format!("versions:\n  - {{ title: a, url: /, ref: v2, {key}: {value} }}\n");
            assert_eq!(error(&site, &under_site(&settings)), sites(key), "{key}");
        }
        // Of `navigation`, only its `items` are its own.
        let settings = "versions:\n  \
                        - { title: a, url: /, ref: v2, navigation: { items: [], prune: true } }\n";
        let message = error(&site, &under_site(settings));
        assert_eq!(message, sites("navigation.prune"));
    }

    #[test]
    fn a_version_from_a_ref_has_the_sites_config_but_its_own_pages_and_nav() {
        let theme_cfg = under_site(
            "name: Theme\ntheme:\n  fonts:\n    body: serif\nplugins:\n  bob:\n    wasm: ./bob.wasm\n",
        );
        // Settings that would stop the build, were they the site's, aren't
        // read, nor is anything beside its `site`.
        let old_cfg = "site:\n  theme: ./elsewhere\n  name: Old\n  url: https://old.example.com\n  \
                       source: ./src\n  versions: []\n  \
                       navigation:\n    items: [{ title: Old, url: / }]\n    prune: 1\n  \
                       search: true\n  plugins: [./cal.wasm]\n\
                       author: Old\npage:\n  outline: false\n";
        let (_tmp, site) = scratch(&[
            ("theme/little.yml", &theme_cfg),
            ("theme/_templates/default.html", ""),
            ("old/little.yml", old_cfg),
        ]);
        let site_cfg = format!(
            "{IN_THEME}  name: New\n  url: https://example.com/docs\n  \
             repository:\n    url: https://github.com/example/docs\n  destination: ./public\n  \
             source: ./content\n  navigation:\n    items: [{{ title: New, url: / }}]\n  \
             plugins:\n    ann:\n      wasm: ./ann.wasm\n      read: ./lib\n  \
             versions:\n    - {{ title: \"2.0\", url: / }}\n    \
             - {{ title: \"1.4\", url: /1.4, ref: v1.4 }}\n\
             page:\n  author: New\n"
        );
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
        let config = load(&site).unwrap();
        let old = site.join("old");
        let file = "v1.4:old/little.yml";
        let mut v14 = config.versions[1].clone();
        let version = config.for_version(&v14, &old, file).unwrap();

        // Its little.yml chooses its pages and its nav, as the site's would.
        assert_eq!(version.source_dir, old.join("src"));
        assert_eq!(get(&version.site, "source"), Some("./src"));
        let nav: Value = serde_yaml::from_str("[{ title: Old, url: / }]").unwrap();
        assert_eq!(value(&version.site, "navigation.items"), Some(&nav));
        // But the rest is the site's, with plugins that read their folders
        // at the ref.
        for key in [
            "theme",
            "name",
            "url",
            "repository",
            "versions",
            "plugins",
            "search",
            "navigation.prune",
        ] {
            assert_eq!(value(&version.site, key), value(&config.site, key), "{key}");
        }
        assert_eq!(version.page, config.page);
        assert_eq!(version.themes, config.themes);
        assert_eq!(version.url, config.url);
        assert_eq!(version.repo, config.repo);
        assert_eq!(version.page_settings, config.page_settings);
        assert_eq!(version.destination_dir, site.join("public"));
        assert_eq!(version.versions, config.versions);
        assert_eq!(
            version.plugins,
            [
                plugin("bob", Wasm::File(site.join("theme/bob.wasm")), &[], "{}"),
                plugin(
                    "ann",
                    Wasm::File(site.join("ann.wasm")),
                    &[(old.join("lib"), "lib")],
                    r#"{"read":["lib"]}"#
                ),
            ]
        );

        // A little.yml that sets neither has little's `source`, and a nav
        // worked out from its folders, not the site's.
        std::fs::write(old.join(CONFIG_FILE), "site: { name: Old }\n").unwrap();
        let version = config.for_version(&v14, &old, file).unwrap();
        assert_eq!(version.source_dir, old.join("docs"));
        assert_eq!(get(&version.site, "source"), Some("./docs"));
        assert_eq!(value(&version.site, "navigation.items"), None);
        assert_eq!(get(&version.site, "name"), Some("New"));

        // Without one, it's from before the site was built with little, so
        // its pages are in the site's `source`.
        std::fs::remove_file(old.join(CONFIG_FILE)).unwrap();
        let version = config.for_version(&v14, &old, file).unwrap();
        assert_eq!(version.source_dir, old.join("content"));
        assert_eq!(get(&version.site, "source"), Some("./content"));
        assert_eq!(value(&version.site, "navigation.items"), None);

        // Its entry's `source` wins, with or without one.
        v14.source = Some("./documentation".into());
        let version = config.for_version(&v14, &old, file).unwrap();
        assert_eq!(version.source_dir, old.join("documentation"));
        assert_eq!(get(&version.site, "source"), Some("./documentation"));
        std::fs::write(old.join(CONFIG_FILE), old_cfg).unwrap();
        let version = config.for_version(&v14, &old, file).unwrap();
        assert_eq!(version.source_dir, old.join("documentation"));
        assert_eq!(value(&version.site, "navigation.items"), Some(&nav));

        // As does its nav, and its `plugins` go over the site's, setting by
        // setting, reading their folders at the ref, or turn one off.
        let own_nav: Value = serde_yaml::from_str("[{ title: Mine, url: / }]").unwrap();
        v14.nav = Some(own_nav.clone());
        v14.plugins =
            serde_yaml::from_str("{ ann: { read: [./src, lib] }, bob: { enabled: false } }")
                .unwrap();
        let ann = plugin(
            "ann",
            Wasm::File(site.join("ann.wasm")),
            &[(old.join("src"), "src"), (old.join("lib"), "lib")],
            r#"{"read":["src","lib"]}"#,
        );
        let version = config.for_version(&v14, &old, file).unwrap();
        assert_eq!(value(&version.site, "navigation.items"), Some(&own_nav));
        assert_eq!(version.plugins, std::slice::from_ref(&ann));
        // Each path as little writes it: on Windows, with `\` throughout.
        let plugins: Value = serde_yaml::from_str(&format!(
            "{{ bob: {{ wasm: '{}', enabled: false }}, \
             ann: {{ wasm: '{}', read: [./src, lib] }} }}",
            site.join("theme").join("bob.wasm").display(),
            site.join("ann.wasm").display()
        ))
        .unwrap();
        assert_eq!(version.site.get("plugins"), Some(&plugins));
        std::fs::remove_file(old.join(CONFIG_FILE)).unwrap();
        let version = config.for_version(&v14, &old, file).unwrap();
        assert_eq!(value(&version.site, "navigation.items"), Some(&own_nav));
        assert_eq!(version.plugins, [ann]);

        // Its little.yml is named in its errors as git names it.
        std::fs::write(old.join(CONFIG_FILE), "site:\n  name: [Old\n").unwrap();
        let message = format!("{:#}", config.for_version(&v14, &old, file).unwrap_err());
        assert!(
            message.starts_with("parsing v1.4:old/little.yml: "),
            "{message}"
        );
    }

    #[test]
    fn a_plugin_changes_the_config_but_not_what_little_reads_before_it_starts() {
        let site_cfg = "site:\n  title: Docs\n  serve:\n    port: 9000\npage:\n  author: Ada\n";
        let (_tmp, site) = scratch(&[(CONFIG_FILE, site_cfg)]);
        let config = load(&site).unwrap();
        // The config a plugin's hook is given, with `change` made to its
        // `site` and its `page`, given back.
        let hook = |change: &dyn Fn(&mut Mapping, &mut Mapping)| {
            let (mut settings, mut page) = (config.site.clone(), config.page.clone());
            change(&mut settings, &mut page);
            let groups = Mapping::from_iter([
                ("site".into(), Value::Mapping(settings)),
                ("page".into(), Value::Mapping(page)),
            ]);
            config.hooked(&site, groups).map_err(|e| format!("{e:#}"))
        };
        let settings = |yaml: &str| serde_yaml::from_str::<Value>(yaml).unwrap();

        // It's given both groups, as `little config` prints them.
        let groups = config.groups();
        assert_eq!(Vec::from_iter(groups.keys()), ["site", "page"]);

        let hooked = hook(&|site, page| {
            site.insert("title".into(), "Hooked".into());
            page.insert("author".into(), "Grace".into());
            let outline = settings("{ enabled: false, depth: { min: 2, max: 4 } }");
            page.insert("outline".into(), outline);
        })
        .unwrap();
        assert_eq!(get(&hooked.site, "title"), Some("Hooked"));
        assert_eq!(hooked.port, 9000);
        assert_eq!(get(&hooked.page, "author"), Some("Grace"));
        let outlined = PageSettings {
            outline: false,
            ..every_page(2..=4)
        };
        assert_eq!(hooked.page_settings, outlined);

        let fixed = "but plugins can't change `site.theme.enabled`, `site.theme.source`, \
                     `site.plugins`, `site.source`, `site.destination`, or `site.serve`";
        let changed = hook(&|site, _| {
            site.insert("serve".into(), settings("{ host: 127.0.0.1, port: 9001 }"));
        });
        assert_eq!(
            changed.unwrap_err(),
            format!("changes `site.serve`, {fixed}")
        );
        let changed = hook(&|site, _| {
            site.shift_remove("theme");
        });
        assert_eq!(
            changed.unwrap_err(),
            format!("changes `site.theme.enabled`, {fixed}")
        );
        let changed = hook(&|site, _| {
            site.insert(
                "theme".into(),
                settings("{ enabled: true, source: ./mine }"),
            );
        });
        assert_eq!(
            changed.unwrap_err(),
            format!("changes `site.theme.source`, {fixed}")
        );
        let changed = hook(&|site, _| {
            site.insert("plugins".into(), settings("{ bob: { wasm: ./bob.wasm } }"));
        });
        assert_eq!(
            changed.unwrap_err(),
            format!("changes `site.plugins`, {fixed}")
        );
        // How the site looks is a plugin's to change, beside the theme it's
        // in.
        let hooked = hook(&|site, _| {
            let theme = settings("{ enabled: true, fonts: { body: serif } }");
            site.insert("theme".into(), theme);
        })
        .unwrap();
        assert_eq!(get(&hooked.site, "theme.fonts.body"), Some("serif"));

        let changed = hook(&|_, page| {
            page.insert("outline".into(), settings("{ enabled: true }"));
        });
        let message = changed.unwrap_err();
        assert!(
            message.starts_with("gives back a config little can't use: "),
            "{message}"
        );
        // A setting beside the groups is no more use from a plugin than in
        // a little.yml.
        let mut groups = config.groups();
        groups.insert("title".into(), "Hooked".into());
        let message = format!("{:#}", config.hooked(&site, groups).unwrap_err());
        assert_eq!(
            message,
            "gives back a config little can't use: the config it gives back sets `title` beside \
             `site` and `page`, but a little.yml holds only those two: put it under `site`, with \
             the site's settings, or under `page`, with every page's"
        );
    }
}
