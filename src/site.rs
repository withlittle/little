//! The build pipeline: the source (`docs/`) + the theme + config -> the
//! destination (`dist/`).
//!
//! A site with `versions` builds each in turn, each in the folder of the
//! destination at its `url`, one at the root, from the working tree or from
//! a git ref, checked out on its own. Each is built with the site's config,
//! but has its own pages, files, nav, and search; one from a ref has its
//! own `source`, too, and its own settings for the site's plugins. The
//! theme's files are written once, for the version at the root, and the
//! others link to them there.

use anyhow::{Context, Result, anyhow, bail};
use minijinja::{Value as MjValue, context};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::iter::Peekable;
use std::mem;
use std::path::{Component, Path, PathBuf};
use std::slice;
use time::OffsetDateTime;
use walkdir::WalkDir;

use crate::config::{Config, Repo, Shared, SiteUrl};
use crate::plugin::{Plugins, Warned};
use crate::theme::{PageType, StaticFile, Theme, ThemeFiles};
use crate::{config, git, math, mermaid, notebook, plugin, render, search};

/// Where a site's 404 page is written: the root of the destination, which is
/// where static hosts look for it.
pub const NOT_FOUND_FILE: &str = "404.html";

/// The file a build leaves in its destination, so the next build knows the
/// folder is little's to wipe.
const MARKER_FILE: &str = ".little-build";

/// A rendered content page.
struct Page {
    /// Path relative to the source folder (e.g. `guide/install.md`), or, for
    /// a file a plugin's `files` hook adds, its path there.
    rel: PathBuf,
    /// What it's built from, for an error or a warning: its file, like
    /// `./docs/guide/install.md`, or the plugin that adds it and where, like
    /// ``plugin `pydoc`'s reference/README.md``.
    origin: String,
    /// Whether a plugin adds it, so it has no file in the site's repository.
    added: bool,
    /// Pretty, extensionless URL (`/`, `/getting-started`, `/guide/install`).
    url: String,
    /// Output file on disk (`dist/guide/install/index.html`).
    out_path: PathBuf,
    title: String,
    /// The page's frontmatter, less any key left blank.
    frontmatter: Mapping,
    html: String,
    /// In-page table of contents: the page's headings from `toc_min_depth` to
    /// `toc_max_depth`, nested like `site.nav`.
    toc: Vec<NavItem>,
    page_type: PageType,
    /// The links and images in its Markdown, as they're written.
    links: Vec<render::WrittenLink>,
    /// The formulas in its Markdown that couldn't be rendered.
    unrendered_math: Vec<math::Unrendered>,
    /// The diagrams in its Markdown that couldn't be drawn.
    unrendered_diagrams: Vec<mermaid::Unrendered>,
    /// The code blocks in its Markdown that plugins couldn't render.
    unrendered_blocks: Vec<plugin::Unrendered>,
}

impl Page {
    /// Is this a folder's README (its home page)?
    fn is_index(&self) -> bool {
        matches!(self.page_type, PageType::Home | PageType::Section)
    }
}

/// What little works out for `page`, beneath the page's frontmatter (see
/// `page_value`). Anything a page doesn't have is left out rather than set to
/// null, which a template would print as "None".
#[derive(Serialize)]
struct PageData<'a> {
    #[serde(rename = "type")]
    page_type: &'static str,
    url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<&'a str>,
    /// The page's table of contents, shaped like `site.nav`.
    #[serde(skip_serializing_if = "Option::is_none")]
    toc: Option<&'a [NavItem]>,
    /// A home or section page's branch of the nav: what's in its folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<&'a [NavItem]>,
    /// What's above this page in the nav, from the top down.
    #[serde(skip_serializing_if = "Option::is_none")]
    breadcrumbs: Option<&'a [Breadcrumb<'a>]>,
    /// The page before this one in the nav, unless it's the first.
    #[serde(skip_serializing_if = "Option::is_none")]
    prev: Option<Link<'a>>,
    /// The page after this one in the nav, unless it's the last.
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<Link<'a>>,
    /// Where to view the page's file in the site's repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    source_url: Option<&'a str>,
    /// Where to edit the page's file in the site's repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    edit_url: Option<&'a str>,
    /// Each of the site's versions, with the `url` of this page in it, or,
    /// if it's not there, of its home page.
    #[serde(skip_serializing_if = "Option::is_none")]
    versions: Option<&'a [Mapping]>,
}

/// A link to another page: what a template sees as `page.prev` or
/// `page.next`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct Link<'a> {
    title: &'a str,
    url: &'a str,
}

/// A page above another in the nav, or a folder without a `README.md`, which
/// has no `url`: what a template sees in `page.breadcrumbs`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct Breadcrumb<'a> {
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
}

impl<'a> From<&'a NavItem> for Breadcrumb<'a> {
    fn from(item: &'a NavItem) -> Self {
        Breadcrumb {
            title: &item.title,
            url: item.url.as_deref(),
        }
    }
}

/// One node in the site-wide navigation tree (see `build_nav`), in a `nav`
/// that the config sets instead, or in a page's table of contents (see
/// `nest_headings`).
#[derive(Debug, Deserialize, Serialize)]
struct NavItem {
    title: String,
    /// A page's path, a heading's `#fragment`, or in a config's `nav`, any
    /// URL: so `url`, not `path`.
    /// `None` for a section folder that has no `README.md` (a plain label).
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    children: Vec<NavItem>,
}

/// A page the build writes.
struct Output {
    path: PathBuf,
    /// Its URL from the root of the site, like `/guide/`.
    url: String,
    html: String,
    /// Which page it is, for an error: "the page built from docs/README.md".
    name: String,
    /// What it's built from, for a warning: "docs/README.md".
    origin: String,
    /// The links and images in its Markdown, as they're written.
    links: Vec<render::WrittenLink>,
}

/// A site that's been built.
#[derive(Clone, Debug)]
pub struct Built {
    /// The folder it was built into.
    pub destination: PathBuf,
    /// Where it's published, which `little serve` serves it under.
    pub url: SiteUrl,
    /// What each file in the destination was built or copied from, by its
    /// path there, so a broken link names the file to fix: a page's
    /// Markdown, like `docs/guide.md`, or a static file's origin.
    pub origins: HashMap<PathBuf, String>,
    /// Each page's URL from the root of the site, by its path in the
    /// destination, like `/guide/install` for `guide/install/index.html`,
    /// so a broken link to it is shown as the theme links to it.
    pub urls: HashMap<PathBuf, String>,
    /// The links and images in each page's Markdown, by its path in the
    /// destination, so a broken one is shown as it's written.
    pub links: HashMap<PathBuf, Vec<render::WrittenLink>>,
    /// The repository the site's source is in, if it says, whose links
    /// aren't checked.
    pub repo: Option<Repo>,
    /// Each formula in its pages that couldn't be rendered, so is shown as
    /// its TeX, in the order of the pages, to warn of.
    pub unrendered_math: Vec<UnrenderedMath>,
    /// Each diagram in its pages that couldn't be drawn, so is shown as
    /// code, in the order of the pages, to warn of.
    pub unrendered_diagrams: Vec<UnrenderedDiagram>,
    /// Each code block in its pages that a plugin couldn't render, so is
    /// left as it was, in the order of the pages, to warn of.
    pub unrendered_blocks: Vec<UnrenderedBlock>,
    /// What each plugin warned of, and each of its hooks that failed, so
    /// the build went on without its change, in the order of the versions,
    /// then as they happened.
    pub plugin_warnings: Vec<PluginWarning>,
    /// The path in the destination of each file written for a version of
    /// the site from a git ref, whose links to other sites aren't checked,
    /// as a tag can't be fixed.
    pub from_refs: HashSet<PathBuf>,
}

/// Something a plugin warned of, or a hook of its that failed, and the git
/// ref of the version of the site it was building, if it's from one.
#[derive(Clone, Debug)]
pub struct PluginWarning {
    /// The ref, like `v1.4`.
    pub git_ref: Option<String>,
    pub warning: plugin::Warning,
}

impl fmt::Display for PluginWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A hook that failed names what it failed on, which names the ref.
        match (&self.git_ref, &self.warning.warned) {
            (Some(git_ref), Warned::Message(message)) => {
                let plugin = &self.warning.plugin;
                write!(f, "plugin `{plugin}` at {git_ref}: {message}")
            }
            _ => self.warning.fmt(f),
        }
    }
}

/// A formula in a page that couldn't be rendered, and the file it's in.
#[derive(Clone, Debug)]
pub struct UnrenderedMath {
    /// What it's in, like `docs/intro.md`.
    pub origin: String,
    pub formula: math::Unrendered,
}

impl fmt::Display for UnrenderedMath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let math::Unrendered { written, reason } = &self.formula;
        write!(f, "can't render {written} in {}: {reason}", self.origin)
    }
}

/// A diagram in a page that couldn't be drawn, and the file it's in.
#[derive(Clone, Debug)]
pub struct UnrenderedDiagram {
    /// What it's in, like `docs/intro.md`.
    pub origin: String,
    pub diagram: mermaid::Unrendered,
}

impl fmt::Display for UnrenderedDiagram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { origin, diagram } = self;
        match diagram.line {
            Some(line) => write!(f, "can't draw the diagram on line {line} of {origin}"),
            None => write!(f, "can't draw a diagram in {origin}"),
        }?;
        write!(f, ": {}", diagram.reason)
    }
}

/// A code block in a page that a plugin couldn't render, and the file it's
/// in.
#[derive(Clone, Debug)]
pub struct UnrenderedBlock {
    /// What it's in, like `docs/intro.md`.
    pub origin: String,
    pub block: plugin::Unrendered,
}

impl fmt::Display for UnrenderedBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { origin, block } = self;
        let plugin = &block.plugin;
        match block.line {
            Some(line) => write!(
                f,
                "plugin `{plugin}` can't render the block on line {line} of {origin}"
            ),
            None => write!(f, "plugin `{plugin}` can't render a block in {origin}"),
        }?;
        write!(f, ": {}", block.reason)
    }
}

/// Where a version of a site is built: the folder of the destination it's
/// written into, and the URL it's published at.
struct Place {
    /// Its title, like `1.4 (Latest)`, or none, for a site without
    /// `versions`.
    title: Option<String>,
    /// The path before its URLs, from the root of the site: empty for the
    /// version at `/`, or like `/1.4` for another.
    prefix: String,
    /// Where it's published: the site's `url`, with its prefix, and, for a
    /// version that isn't at the root, the theme's files, which it links to
    /// at the root.
    url: SiteUrl,
    /// The folder it's written into.
    destination: PathBuf,
}

impl Place {
    /// Where `version` of the site `config` builds goes: the folder at its
    /// `url`, or the root, for a site without `versions`. Only the root has
    /// the theme's files, at `theme_files`, their URLs from the root of the
    /// site, so another version links to them there.
    fn new(
        config: &Config,
        version: Option<&config::Version>,
        theme_files: &BTreeSet<String>,
    ) -> Place {
        let prefix = (version.map(|version| version.path.clone())).unwrap_or_default();
        // Folder by folder, as joining an empty path would end it in a `/`.
        let mut destination = config.destination_dir.clone();
        destination.extend(version.into_iter().flat_map(config::Version::folders));
        let shared = (!prefix.is_empty()).then(|| Shared {
            path: config.url.path.clone(),
            files: theme_files.clone(),
        });
        Place {
            url: SiteUrl {
                origin: config.url.origin.clone(),
                path: format!("{}{prefix}", config.url.path),
                shared,
            },
            destination,
            prefix,
            title: version.map(|version| version.title.clone()),
        }
    }

    /// Whether it's at the root of the site, as the version at `/` is, or a
    /// site without `versions`.
    fn is_root(&self) -> bool {
        self.prefix.is_empty()
    }
}

/// A version of a site, read: its pages rendered, and its other files read,
/// ready to put in its theme.
struct Source {
    place: Place,
    /// The git ref it's read from, like `v1.4`, if it's from one.
    git_ref: Option<String>,
    /// Its config, merged, which the theme gets as `site`.
    values: Mapping,
    /// Whether it's indexed for search.
    searchable: bool,
    /// Its theme, which writes URLs under the version's.
    theme: Theme,
    pages: Vec<Page>,
    /// The source's files that aren't pages, and those its plugins add.
    files: Vec<StaticFile>,
    /// Its plugins, which go on to hook into putting its pages in the theme.
    plugins: Plugins,
    /// Where its source folder is in the site's repository, if the site
    /// says which that is.
    in_repo: Option<InRepo>,
    /// The files it's read from, if it's from a git ref, which are kept
    /// until it's built, as its plugins can read them.
    checkout: Option<git::Checkout>,
}

/// Where a version's source folder is in the site's repository, so each of
/// its pages can link to its file there.
struct InRepo {
    /// The repository, with the branch its pages link to their files on, or,
    /// for a version from a git ref, the ref's commit, which a branch can
    /// move on from.
    repo: Repo,
    /// The folder, from the top of the repository, like `docs`.
    folder: PathBuf,
    /// Whether its pages' files can be edited there, as they can't at a
    /// commit.
    editable: bool,
}

/// A site's versions, as the theme gets them, and the pages each has, so
/// each page can link to itself in each version.
struct Versions {
    /// Where the site's published.
    url: SiteUrl,
    each: Vec<VersionPages>,
}

/// A version of a site, and the pages it has.
struct VersionPages {
    /// Its entry in `versions`, as the config writes it.
    entry: Mapping,
    /// The path before its URLs, from the root of the site, like `/1.4`, or
    /// empty, for the version at `/`.
    prefix: String,
    /// Each page's URL, from the root of the version, by the URL without
    /// the `/` at its end, so a folder's `README.md` is the same page as a
    /// file named for the folder, like `guide.md`.
    pages: HashMap<String, String>,
}

impl Versions {
    /// The versions in `entries`, the site's `versions`, published under
    /// `url`, and their pages, from `sources`, in the same order.
    fn new(url: &SiteUrl, entries: &[Value], sources: &[Source]) -> Versions {
        let each = (entries.iter().zip(sources))
            .map(|(entry, source)| VersionPages {
                entry: entry.as_mapping().cloned().unwrap_or_default(),
                prefix: source.place.prefix.clone(),
                pages: (source.pages.iter())
                    .map(|page| (page.url.trim_end_matches('/').to_string(), page.url.clone()))
                    .collect(),
            })
            .collect();
        Versions {
            url: url.clone(),
            each,
        }
    }

    /// Each version's entry, with the `url` of the page at `url`, from the
    /// root of a version, in that version, or, if it's not there, or `url`
    /// is `None`, of its home page, and `root`, whether it's the version at
    /// `/`, over what the entry sets.
    fn at(&self, url: Option<&str>) -> Vec<Mapping> {
        let url = url.map(|url| url.trim_end_matches('/'));
        (self.each.iter())
            .map(|version| {
                let page = (url.and_then(|url| version.pages.get(url))).map_or("/", String::as_str);
                let page = format!("{}{page}", version.prefix);
                let mut entry = version.entry.clone();
                entry.insert("url".into(), self.url.link(&page).as_ref().into());
                entry.insert("root".into(), version.prefix.is_empty().into());
                entry
            })
            .collect()
    }
}

/// A version of a site, ready to write.
struct Ready {
    /// The folder it's written into.
    destination: PathBuf,
    /// Whether it's from a git ref.
    from_ref: bool,
    /// How many pages it has.
    pages: usize,
    outputs: Vec<Output>,
    /// The source's other files, and, for the version at the root of the
    /// site, the theme's static files.
    static_files: Vec<StaticFile>,
    search_files: Vec<search::File>,
}

/// Build the site rooted at `site_dir`: each of its `versions`, or, if it
/// has none, its files as they are.
///
/// Pages, and the nav and links between them, are worked out at their URLs
/// from the root of their version, like `/guide/`. Only what the theme gets
/// has the path of the version's URL before them, like `/my-project/guide/`,
/// or `/my-project/1.4/guide/`.
pub fn build(site_dir: &Path) -> Result<Built> {
    let config = config::load(site_dir)?;
    // Once, so every page of a build has the same year, and every diagram
    // the same today, even at midnight.
    let now = time_of_build(env::var_os("SOURCE_DATE_EPOCH"))?;
    // The site's plugins start before anything's read, so their `config`
    // hooks change the config the build reads, and their `theme` hooks the
    // theme every version is in.
    let mut plugins = Plugins::start(&config.plugins, site_dir, now)?;
    let config = plugins.config(config, site_dir)?;
    let theme = ThemeFiles::read(&config.themes)?.hooked(&mut plugins)?;
    // A site without `versions` is one version, of its files as they are.
    let versions: Vec<Option<&config::Version>> = if config.versions.is_empty() {
        vec![None]
    } else {
        config.versions.iter().map(Some).collect()
    };
    let repository = (versions.iter().flatten())
        .any(|version| version.git_ref.is_some())
        .then(|| git::Repository::find(site_dir))
        .transpose()?;

    // Every version is in the site's theme, so its static files are read
    // once, and written once, for the version at the root of the site.
    let theme_files = theme.static_files()?;
    let theme_urls = (theme_files.iter())
        .map(|file| file_url(&file.path))
        .collect();

    // Every version is read, and every page rendered, before the destination
    // is touched, so a build that fails leaves the last one in place. The
    // site's plugins build the version of its files as they are; one from a
    // git ref starts its own, with its own settings.
    let mut site_plugins = Some(plugins);
    let mut sources = Vec::new();
    for &version in &versions {
        let place = Place::new(&config, version, &theme_urls);
        let git_ref = version.and_then(|version| version.git_ref.as_deref());
        let source = match (version, git_ref, &repository) {
            (Some(version), Some(git_ref), Some(repository)) => {
                check_out(&config, repository, version, git_ref).and_then(|(checkout, config)| {
                    let plugins = Plugins::start(&config.plugins, site_dir, now)?;
                    let checkout = Some(checkout);
                    read_source(site_dir, &config, checkout, plugins, &theme, place, now)
                })
            }
            _ => {
                let plugins = site_plugins.take().unwrap_or_default();
                read_source(site_dir, &config, None, plugins, &theme, place, now)
            }
        };
        sources.push(in_version(source, version)?);
    }
    let every = (!config.versions.is_empty()).then(|| {
        let entries = match config.values.get("versions") {
            Some(Value::Sequence(entries)) => entries.as_slice(),
            _ => &[],
        };
        Versions::new(&config.url, entries, &sources)
    });

    let mut built = Built {
        destination: config.destination_dir.clone(),
        url: config.url.clone(),
        origins: HashMap::new(),
        urls: HashMap::new(),
        links: HashMap::new(),
        repo: config.repo.clone(),
        unrendered_math: Vec::new(),
        unrendered_diagrams: Vec::new(),
        unrendered_blocks: Vec::new(),
        plugin_warnings: Vec::new(),
        from_refs: HashSet::new(),
    };
    // When every version is from a git ref, the site's plugins build none,
    // but may have warned as they started.
    if let Some(mut plugins) = site_plugins {
        built.plugin_warnings.extend(warned(&mut plugins, None));
    }
    let mut ready = Vec::new();
    for (source, &version) in sources.into_iter().zip(&versions) {
        let finished = finish(source, every.as_ref(), &theme_files, now, &mut built);
        ready.push(in_version(finished, version)?);
    }
    // The version at `/`, which config checks there's one of, has the
    // others in its folders.
    let root = (versions.iter())
        .position(|version| version.is_none_or(|version| version.path.is_empty()))
        .unwrap_or_default();
    check_versions(&ready[root], &config.versions, &config.destination_dir)?;

    let pages: usize = ready.iter().map(|version| version.pages).sum();
    let destination = &config.destination_dir;
    clear_destination(destination)?;
    for version in ready {
        write_version(version, destination, &mut built)?;
    }
    match config.versions.len() {
        0 => println!("built {pages} page(s) -> {}", destination.display()),
        count => println!(
            "built {pages} page(s), in {count} version(s) -> {}",
            destination.display()
        ),
    }
    Ok(built)
}

/// `result`, saying which version of the site it's from before any error,
/// if it's one from a git ref.
fn in_version<T>(result: Result<T>, version: Option<&config::Version>) -> Result<T> {
    match version {
        Some(config::Version {
            title,
            git_ref: Some(git_ref),
            ..
        }) => result.with_context(|| format!("building version `{title}`, from {git_ref}")),
        _ => result,
    }
}

/// What `plugins` have warned of since they were last asked, as warnings of
/// the version of the site from `git_ref`, if it's from one.
fn warned(plugins: &mut Plugins, git_ref: Option<&str>) -> Vec<PluginWarning> {
    (plugins.take_warnings().into_iter())
        .map(|warning| PluginWarning {
            git_ref: git_ref.map(str::to_string),
            warning,
        })
        .collect()
}

/// Check out `version` of the site, as it was at its ref, `git_ref`, in
/// `repository`, and its config: the site's, `config`, but with its own
/// pages, nav, and plugins' settings, from its entry in `versions`, or its
/// own little.yml there, if it has one. Only the files it reads are checked
/// out, apart from the site's own, and they're deleted once the checkout is
/// dropped.
fn check_out(
    config: &Config,
    repository: &git::Repository,
    version: &config::Version,
    git_ref: &str,
) -> Result<(git::Checkout, Config)> {
    let checkout = repository.checkout(git_ref)?;
    let file = checkout.site_dir().join(config::CONFIG_FILE);
    checkout.extract(&[&file])?;
    let config = config.for_version(version, checkout.site_dir(), &checkout.describe(&file))?;
    // Only the files a build reads: its source, and the folders its plugins
    // read. Each plugin's file is the site's.
    let mut paths = vec![config.source_dir.as_path()];
    for plugin in &config.plugins {
        paths.extend(plugin.read.iter().map(|read| read.folder.as_path()));
    }
    checkout.extract(&paths)?;
    // Each is named as git names it, not by where it's checked out.
    if !config.source_dir.is_dir() {
        bail!(
            "there's no source folder at {}: set the version's `source`, in `versions`, to the \
             folder its pages are in at {git_ref}",
            checkout.describe(&config.source_dir)
        );
    }
    let missing =
        |read: &&config::Read| checkout.in_repo(&read.folder).is_some() && !read.folder.is_dir();
    for plugin in &config.plugins {
        if let Some(read) = plugin.read.iter().find(missing) {
            bail!(
                "plugin `{name}` reads {}, but there's no folder there: set its `read`, in the \
                 version's `plugins`, in `versions`, to the folders it reads at {git_ref}, or \
                 turn it off there, with `{name}: false`",
                checkout.describe(&read.folder),
                name = plugin.name,
            );
        }
    }
    Ok((checkout, config))
}

/// A file a version of a site is built from, by its path in the source.
enum Found {
    /// One in the source folder, at its path.
    OnDisk(PathBuf),
    /// One a plugin's `files` hook adds, named, with its contents.
    Added { plugin: String, contents: Vec<u8> },
}

/// Read the version of the site at `site_dir` that `config` configures, to
/// build at `place`, in `theme`, with `plugins`: its files from `checkout`,
/// if it's from a git ref, or else as they are, and those its plugins add.
/// Its pages are rendered, but not yet put in its theme.
fn read_source(
    site_dir: &Path,
    config: &Config,
    checkout: Option<git::Checkout>,
    mut plugins: Plugins,
    theme: &ThemeFiles,
    mut place: Place,
    now: OffsetDateTime,
) -> Result<Source> {
    // A file from a git ref is named as git names it, like
    // `v1.4:docs/guide.md`.
    let describe = |path: &Path| match &checkout {
        Some(checkout) => checkout.describe(path),
        None => path.display().to_string(),
    };
    let source_dir = &config.source_dir;
    if !source_dir.is_dir() {
        bail!("there's no source folder at {}", describe(source_dir));
    }

    // Walk every file in the source folder but hidden ones, without walking
    // hidden folders, or the destination, which can be inside it, as it
    // would be at a git ref, had it been committed. The source folder itself
    // (depth 0) can be hidden.
    let destination = match &checkout {
        None => destination_in(source_dir, &config.destination_dir),
        Some(checkout) => (config.destination_dir.strip_prefix(site_dir).ok())
            .and_then(|rel| destination_in(source_dir, &checkout.site_dir().join(rel))),
    };
    let mut found = BTreeMap::new();
    let entries = WalkDir::new(source_dir).into_iter().filter_entry(|entry| {
        entry.depth() == 0
            || !(is_hidden(Path::new(entry.file_name()))
                || destination.as_deref() == Some(entry.path()))
    });
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        // A link to a file counts as the file, as it always has for pages.
        if path.is_file() {
            let rel = path.strip_prefix(source_dir)?;
            found.insert(slashed(rel), Found::OnDisk(path.to_path_buf()));
        }
    }
    // Plugins see the paths, and add files, or leave them out, before any
    // is read.
    plugins.files(&mut found, |plugin, contents| Found::Added {
        plugin: plugin.to_string(),
        contents,
    })?;

    // Each Markdown file and notebook is rendered into a page, and every
    // other file, like an image, is copied as it is.
    let mut documents = Vec::new();
    let mut files = Vec::new();
    for (path, found) in found {
        let (rel, origin, contents, added) = match found {
            Found::OnDisk(file) => {
                let origin = describe(&file);
                let contents = fs::read(&file).with_context(|| format!("reading {origin}"))?;
                let rel = file.strip_prefix(source_dir)?.to_path_buf();
                (rel, origin, contents, false)
            }
            Found::Added { plugin, contents } => {
                let origin = match &checkout {
                    Some(checkout) => format!("plugin `{plugin}`'s {path} at {}", checkout.git_ref),
                    None => format!("plugin `{plugin}`'s {path}"),
                };
                (PathBuf::from(path), origin, contents, true)
            }
        };
        let parse = match rel.extension().and_then(|s| s.to_str()) {
            Some("md") => render::parse_document,
            Some("ipynb") => notebook::parse_notebook,
            _ => {
                files.push(StaticFile {
                    path: rel,
                    contents,
                    origin,
                });
                continue;
            }
        };
        let text = String::from_utf8(contents).with_context(|| format!("reading {origin}"))?;
        documents.push((rel, origin, added, text, parse));
    }
    // The version's own file at the path of one of the theme's is the one
    // it links to, as the root's own file is.
    if let Some(shared) = &mut place.url.shared {
        for file in &files {
            shared.files.remove(&file_url(&file.path));
        }
    }

    let theme = Theme::load(theme, &place.url)?;
    let in_repo = match (&config.repo, &checkout) {
        (None, _) => None,
        (Some(repo), None) => folder_in_repo(source_dir, site_dir)?.map(|folder| InRepo {
            repo: repo.clone(),
            folder,
            editable: true,
        }),
        (Some(repo), Some(checkout)) => checkout.in_repo(source_dir).map(|folder| InRepo {
            repo: Repo {
                url: repo.url.clone(),
                branch: checkout.commit.clone(),
            },
            folder: PathBuf::from(folder),
            editable: false,
        }),
    };
    // A page's file is from the site's folder, like `docs/guide.md`,
    // wherever little is run from, even if a plugin adds it.
    let version_dir = checkout.as_ref().map_or(site_dir, git::Checkout::site_dir);
    let source_in_site = source_dir.strip_prefix(version_dir).unwrap_or(source_dir);

    let mut pages = Vec::new();
    for (rel, origin, added, text, parse) in documents {
        let file = slashed(&source_in_site.join(&rel));
        let text = plugins.source(&file, &origin, text);
        let mut rendered = parse(
            &text,
            &folder_url(&rel),
            &config.toc_depths,
            &place.url,
            &config.markdown,
            now,
            &mut plugins.on(&file),
        )
        .with_context(|| format!("rendering {origin}"))?;
        rendered.html = plugins.content(&file, &origin, mem::take(&mut rendered.html));
        pages.push(page_from(rel, origin, added, rendered, &place.destination));
    }

    // Deterministic output order (nav has its own ordering), and so a
    // deterministic error for files that would be the same page.
    pages.sort_by(|a, b| (&a.url, &a.rel).cmp(&(&b.url, &b.rel)));
    check_clashes(&pages)?;
    Ok(Source {
        place,
        git_ref: checkout.as_ref().map(|checkout| checkout.git_ref.clone()),
        values: config.values.clone(),
        searchable: config.search,
        theme,
        pages,
        files,
        plugins,
        in_repo,
        checkout,
    })
}

/// Put each page of a version of the site, `source`, in its theme, beside
/// its other files, and, for the version at the root of the site, the
/// theme's, `theme_files`, which every version links to there, and index it
/// for search. The version at the root has the 404 page, too. What's warned
/// of is noted in `built`.
fn finish(
    source: Source,
    versions: Option<&Versions>,
    theme_files: &[StaticFile],
    now: OffsetDateTime,
    built: &mut Built,
) -> Result<Ready> {
    let Source {
        place,
        git_ref,
        values,
        searchable,
        theme,
        pages,
        files,
        mut plugins,
        in_repo,
        checkout: _checkout,
    } = source;

    // What little works out is the bottom of the cascade: the nav it builds
    // from the folders, the year, and the version go beneath the config, so
    // a theme or site that sets `nav`, `year`, or `version` replaces it.
    let tree = build_tree(&pages);
    let mut site = Mapping::new();
    site.insert("nav".into(), serde_yaml::to_value(build_nav(&tree))?);
    site.insert("year".into(), now.year().into());
    if let Some(title) = &place.title {
        site.insert("version".into(), title.as_str().into());
    }
    config::merge(&mut site, values);
    // Each entry of `versions`, as it's written, with the URL of its home.
    if let Some(versions) = versions {
        let entries = versions.at(None).into_iter().map(Value::Mapping);
        site.insert("versions".into(), Value::Sequence(entries.collect()));
    }
    nav_in(&site)
        .context("`nav` in little.yml must be a list of links, each `{ title, url, children }`")?;
    if let Some(nav) = site.get_mut("nav") {
        link_items(nav, &place.url);
    }
    // Plugins get `site` as the theme does, and `page.breadcrumbs`,
    // `page.prev`, and `page.next` follow the nav they leave it with, so
    // they agree with the theme's sidebar.
    let site = plugins.site(site, |name, site| {
        nav_in(site).map(drop).with_context(|| {
            format!(
                "plugin `{name}`'s `site` hook gives back a `nav` that isn't a list of links, \
                 each `{{ title, url, children }}`"
            )
        })
    })?;
    let nav = nav_in(&site).context("`nav` must be a list of links")?;
    let site_value = MjValue::from_serialize(&site);
    let neighbours = neighbours_in(&nav);
    let trails = breadcrumbs_in(&nav, &place.url.link("/"));

    // The theme owns 100% of the markup; little only hands it structured
    // data, which plugins can change, then change what the theme makes of it.
    let mut render = |page_type: PageType, page: Mapping, url: &str, on: &str| {
        let page = plugins.page(on, page);
        // Search indexes the page's own content and title, as plugins leave
        // them, not the theme's markup around it.
        let text = |key: &str| page.get(key).and_then(Value::as_str).map(str::to_string);
        let searched = (text("title"), text("content"));
        let html = theme.render(
            page_type,
            context! { site => site_value.clone(), page => page },
        )?;
        let html = html.map(|html| plugins.html(url, on, html));
        anyhow::Ok(html.map(|html| (html, searched)))
    };
    // A page from a git ref links to its file at the ref's commit, which
    // can't be edited.
    let editable = in_repo.as_ref().is_some_and(|in_repo| in_repo.editable);
    let mut outputs = Vec::new();
    let mut searched = Vec::new();
    for page in &pages {
        let url = place.url.link(&page.url);
        let children = page.is_index().then(|| children_of(&tree, &page.rel));
        let (prev, next) = neighbours.get(&*url).copied().unwrap_or_default();
        // A page that isn't in the nav has nothing above it there.
        let breadcrumbs = (trails.get(&*url)).map(Vec::as_slice).unwrap_or_default();
        // A page a plugin adds has no file in the site's repository.
        let file_urls = (in_repo.as_ref())
            .filter(|_| !page.added)
            .and_then(|in_repo| in_repo.repo.file_urls(&in_repo.folder.join(&page.rel)));
        let in_versions = versions.map(|versions| versions.at(Some(&page.url)));
        let data = PageData {
            page_type: page.page_type.name(),
            url: &page.url,
            title: Some(&page.title),
            content: Some(&page.html),
            toc: Some(&page.toc),
            children: children.as_deref(),
            breadcrumbs: Some(breadcrumbs),
            prev,
            next,
            source_url: file_urls.as_ref().map(|urls| urls.source.as_str()),
            edit_url: (file_urls.as_ref())
                .filter(|_| editable)
                .map(|urls| urls.edit.as_str()),
            versions: in_versions.as_deref(),
        };
        let rendered = page_value(&data, &page.frontmatter, &place.url)
            .and_then(|value| render(page.page_type, value, &url, &page.origin))
            .with_context(|| format!("rendering {}", page.origin))?;
        if let Some((html, (title, content))) = rendered {
            // In the language the theme gives it, at its URL from the root
            // of its version, beside its index.
            if searchable {
                let title = title.unwrap_or_else(|| page.title.clone());
                let content = content.unwrap_or_else(|| page.html.clone());
                searched.push((&page.url, title, content, search::lang(&html)));
            }
            let origin = page.origin.clone();
            built
                .unrendered_math
                .extend(page.unrendered_math.iter().map(|formula| UnrenderedMath {
                    origin: origin.clone(),
                    formula: formula.clone(),
                }));
            built
                .unrendered_diagrams
                .extend(
                    page.unrendered_diagrams
                        .iter()
                        .map(|diagram| UnrenderedDiagram {
                            origin: origin.clone(),
                            diagram: diagram.clone(),
                        }),
                );
            built
                .unrendered_blocks
                .extend(page.unrendered_blocks.iter().map(|block| UnrenderedBlock {
                    origin: origin.clone(),
                    block: block.clone(),
                }));
            outputs.push(Output {
                path: page.out_path.clone(),
                url: format!("{}{}", place.prefix, page.url),
                html,
                name: format!("the page built from {origin}"),
                origin,
                links: page.links.clone(),
            });
        }
    }

    // A 404 page has no content, or file: all `page` has is its type and
    // URL, and the home of each version. Only the version at the root of
    // the site has one, at the root of the destination, where static hosts
    // look for it, and only if its theme has a template for it.
    if place.is_root() && theme.renders(PageType::NotFound) {
        let not_found_url = format!("/{NOT_FOUND_FILE}");
        let homes = versions.map(|versions| versions.at(None));
        let data = PageData {
            page_type: PageType::NotFound.name(),
            url: &not_found_url,
            title: None,
            content: None,
            toc: None,
            children: None,
            breadcrumbs: None,
            prev: None,
            next: None,
            source_url: None,
            edit_url: None,
            versions: homes.as_deref(),
        };
        let name = "the 404 page".to_string();
        let rendered = page_value(&data, &Mapping::new(), &place.url)
            .and_then(|value| {
                let url = place.url.link(&not_found_url);
                render(PageType::NotFound, value, &url, &name)
            })
            .context("rendering the 404 page")?;
        if let Some((html, _)) = rendered {
            outputs.push(Output {
                path: place.destination.join(NOT_FOUND_FILE),
                url: not_found_url.clone(),
                html,
                origin: name.clone(),
                name,
                links: Vec::new(),
            });
        }
    }

    // The source's other files go beside the pages built from it, and the
    // theme's static files (css/js/images) verbatim to the root of the site,
    // once, for every version. Where both have a file at the same path, the
    // source's wins, as a theme's own file wins over that of the theme it
    // builds on.
    let theme_files = if place.is_root() { theme_files } else { &[] };
    let mut static_files = BTreeMap::new();
    for file in files.into_iter().chain(theme_files.iter().cloned()) {
        static_files.entry(file.path.clone()).or_insert(file);
    }
    let mut static_files: Vec<_> = static_files.into_values().collect();

    // Every page the theme rendered is searchable, but the 404 page, which
    // isn't one to find.
    let search_files = if searchable {
        let pages: Vec<_> = (searched.iter())
            .map(|(url, title, content, lang)| search::Page {
                url,
                title,
                content,
                lang: lang.clone(),
            })
            .collect();
        search::index(&pages)?
    } else {
        Vec::new()
    };

    // Plugins add files beside every file the version writes, knowing their
    // paths in its folder, like a sitemap of its pages.
    let mut paths = BTreeSet::new();
    for output in &outputs {
        paths.insert(slashed(output.path.strip_prefix(&place.destination)?));
    }
    paths.extend(static_files.iter().map(|file| slashed(&file.path)));
    paths.extend(search_files.iter().map(|file| slashed(&file.path)));
    for file in plugins.destination(paths)? {
        let plugin = file.plugin.unwrap_or_default();
        let origin = match &git_ref {
            Some(git_ref) => format!("plugin `{plugin}`'s {} at {git_ref}", file.path),
            None => format!("plugin `{plugin}`'s {}", file.path),
        };
        static_files.push(StaticFile {
            path: PathBuf::from(file.path),
            contents: file.contents,
            origin,
        });
    }
    check_static_files(&static_files, &place.destination, &outputs)?;
    check_search_files(&search_files, &static_files, &place.destination, &outputs)?;

    built
        .plugin_warnings
        .extend(warned(&mut plugins, git_ref.as_deref()));
    Ok(Ready {
        destination: place.destination,
        from_ref: git_ref.is_some(),
        pages: pages.len(),
        outputs,
        static_files,
        search_files,
    })
}

/// Fail if a page or a file of the version of a site at its root, `root`, is
/// in the folder of the destination, `destination`, another of its
/// `versions` is built into, as `docs/1.4.md` would be for a version at
/// `/1.4`, or one of them is where little writes files for search.
fn check_versions(root: &Ready, versions: &[config::Version], destination: &Path) -> Result<()> {
    let folders: Vec<(PathBuf, &config::Version)> = (versions.iter())
        .filter(|version| !version.path.is_empty())
        .map(|version| (version.folders().collect(), version))
        .collect();
    let version_at = |path: &Path| {
        (folders.iter())
            .find(|(folder, _)| path.starts_with(folder))
            .map(|&(_, version)| version)
    };
    let clash = |name: &str, version: &config::Version| {
        anyhow!(
            "{name} clashes with version `{}`, built at {}/: rename or move one",
            version.title,
            version.path
        )
    };
    for page in &root.outputs {
        if let Some(version) = version_at(page.path.strip_prefix(destination)?) {
            return Err(clash(&page.name, version));
        }
    }
    for file in &root.static_files {
        if let Some(version) = version_at(&file.path) {
            return Err(clash(&file.origin, version));
        }
    }
    if let Some(version) = (root.search_files.iter()).find_map(|file| version_at(&file.path)) {
        bail!(
            "version `{}` is built at {}/, where little writes files for search: change its `url`",
            version.title,
            version.path
        );
    }
    Ok(())
}

/// Write `version` of a site into the destination, `root`, noting what each
/// file is built from, and, for a page, its URL and links, in `built`.
fn write_version(version: Ready, root: &Path, built: &mut Built) -> Result<()> {
    let Ready {
        destination,
        from_ref,
        outputs,
        static_files,
        search_files,
        ..
    } = version;
    let mut written = Vec::new();
    for output in outputs {
        write_file(&output.path, output.html.as_bytes())?;
        let path = output.path.strip_prefix(root)?.to_path_buf();
        built.origins.insert(path.clone(), output.origin);
        built.urls.insert(path.clone(), output.url);
        built.links.insert(path.clone(), output.links);
        written.push(path);
    }
    // Written from what was read, not with `fs::copy`: on macOS that clones
    // the file, and FSEvents then reports the *source* as changed, so `little
    // serve` would rebuild forever. A build must only ever read its sources.
    for file in static_files {
        let path = destination.join(&file.path);
        write_file(&path, &file.contents)?;
        let path = path.strip_prefix(root)?.to_path_buf();
        built.origins.insert(path.clone(), file.origin);
        written.push(path);
    }
    for file in search_files {
        write_file(&destination.join(&file.path), &file.contents)?;
    }
    if from_ref {
        built.from_refs.extend(written);
    }
    Ok(())
}

/// The page built from what `rendered` was rendered from, at `rel` in the
/// source, and what it's built from, `origin`, whether a plugin `added` it,
/// to be written to `destination`.
fn page_from(
    rel: PathBuf,
    origin: String,
    added: bool,
    rendered: render::Rendered,
    destination: &Path,
) -> Page {
    // A key left blank is as good as not there: a page never holds null,
    // and `title:` shouldn't wipe out the title little finds.
    let mut frontmatter = rendered.frontmatter;
    frontmatter.retain(|_, value| !value.is_null());

    let is_index = is_readme(&rel);

    // Title: frontmatter `title` -> first <h1> -> filename/folder-derived.
    let title = frontmatter
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(rendered.first_h1)
        .unwrap_or_else(|| fallback_title(&rel, is_index));

    let url = derive_url(&rel);
    let out_path = out_path_for_url(destination, &url);
    let page_type = page_type(&rel);

    Page {
        rel,
        origin,
        added,
        url,
        out_path,
        title,
        frontmatter,
        html: rendered.html,
        toc: nest_headings(&rendered.toc),
        page_type,
        links: rendered.links,
        unrendered_math: rendered.unrendered_math,
        unrendered_diagrams: rendered.unrendered_diagrams,
        unrendered_blocks: rendered.unrendered_blocks,
    }
}

/// Where the source folder is in the site's repository, like `docs`: from
/// the nearest folder above it with a `.git`, which in a worktree is a file,
/// or else from the site's folder, as in a copy of the repository without
/// one. A source outside the repository is nowhere in it.
fn folder_in_repo(source_dir: &Path, site_dir: &Path) -> Result<Option<PathBuf>> {
    let canonical = |dir: &Path| {
        dir.canonicalize()
            .with_context(|| format!("reading {}", dir.display()))
    };
    let source = canonical(source_dir)?;
    let root = match source.ancestors().find(|dir| dir.join(".git").exists()) {
        Some(root) => root.to_path_buf(),
        None => canonical(site_dir)?,
    };
    Ok(source.strip_prefix(&root).ok().map(Path::to_path_buf))
}

/// When a build is, in UTC, for a theme's copyright notice, and a diagram's
/// today: `SOURCE_DATE_EPOCH`, if it's set, so the build can be reproduced,
/// or now. A malformed `SOURCE_DATE_EPOCH` is an error, as its specification
/// asks.
fn time_of_build(source_date_epoch: Option<OsString>) -> Result<OffsetDateTime> {
    let Some(epoch) = source_date_epoch else {
        return Ok(OffsetDateTime::now_utc());
    };
    epoch
        .to_str()
        .and_then(|seconds| seconds.parse().ok())
        .and_then(|seconds| OffsetDateTime::from_unix_timestamp(seconds).ok())
        .with_context(|| {
            format!(
                "SOURCE_DATE_EPOCH must be a whole number of seconds since 1970, \
                 as `date +%s` prints it, not {epoch:?}"
            )
        })
}

/// What a template sees as `page`: what little works out for it, `data`,
/// with the page's frontmatter merged over it the way each level of the
/// config merges over the one beneath, so the author has the last word. It
/// changes only what the theme sees: a page is built at its own URL, with
/// the template for its own type, whatever its frontmatter says.
///
/// Each of the page's links gets the path of the site's `url`, whoever
/// wrote it, as do the links in its Markdown, unless it's to another site,
/// like its file in the site's repository. Those in `page.breadcrumbs`,
/// `page.prev`, and `page.next` have it already, from the nav the theme
/// gets.
fn page_value(data: &PageData, frontmatter: &Mapping, url: &SiteUrl) -> Result<Mapping> {
    let Value::Mapping(mut page) = serde_yaml::to_value(data)? else {
        unreachable!("a struct serializes to a mapping");
    };
    link(page.get_mut("url"), url);
    for key in ["children", "toc"] {
        if let Some(items) = page.get_mut(key) {
            link_items(items, url);
        }
    }
    let mut frontmatter = frontmatter.clone();
    for key in ["url", "source_url", "edit_url"] {
        link(frontmatter.get_mut(key), url);
    }
    for key in ["prev", "next"] {
        if let Some(Value::Mapping(item)) = frontmatter.get_mut(key) {
            link(item.get_mut("url"), url);
        }
    }
    for key in ["breadcrumbs", "children", "toc"] {
        if let Some(items) = frontmatter.get_mut(key) {
            link_items(items, url);
        }
    }
    config::merge(&mut page, frontmatter);
    Ok(page)
}

/// The nav in `site`, what the theme gets as `site.nav`, or why it isn't
/// one.
fn nav_in(site: &Mapping) -> Result<Vec<NavItem>, serde_yaml::Error> {
    let nav = site.get("nav").cloned();
    serde_yaml::from_value(nav.unwrap_or(Value::Sequence(Vec::new())))
}

/// Put the site's path before the `url` of each of `items`, a list shaped
/// like `site.nav`, and of each of their `children`.
fn link_items(items: &mut Value, url: &SiteUrl) {
    let Value::Sequence(items) = items else {
        return;
    };
    for item in items {
        if let Value::Mapping(item) = item {
            link(item.get_mut("url"), url);
            if let Some(children) = item.get_mut("children") {
                link_items(children, url);
            }
        }
    }
}

/// Put the site's path before `value`, if it's a URL from the root of the
/// site.
fn link(value: Option<&mut Value>, url: &SiteUrl) {
    if let Some(Value::String(value)) = value
        && let Cow::Owned(within) = url.link(value)
    {
        *value = within;
    }
}

/// Empty the destination for a new build, and mark it as little's. Only a
/// folder that's empty or that little built can be wiped, so a destination
/// that points at anything else (the site's folder, say) stops the build
/// instead of losing its files.
fn clear_destination(destination: &Path) -> Result<()> {
    if destination.join(MARKER_FILE).is_file() {
        fs::remove_dir_all(destination)
            .with_context(|| format!("clearing {}", destination.display()))?;
    } else {
        let has_files = match fs::read_dir(destination) {
            Ok(mut entries) => entries.next().is_some(),
            Err(e) if e.kind() == ErrorKind::NotFound => false,
            Err(e) => return Err(e).with_context(|| format!("reading {}", destination.display())),
        };
        if has_files {
            bail!(
                "destination {} isn't empty and wasn't built by little (it has no {MARKER_FILE} file): empty it, or choose another folder",
                destination.display()
            );
        }
    }
    fs::create_dir_all(destination)
        .with_context(|| format!("creating {}", destination.display()))?;
    let marker = destination.join(MARKER_FILE);
    fs::write(
        &marker,
        "Built by little, which wipes this folder on every build.\n",
    )
    .with_context(|| format!("writing {}", marker.display()))
}

/// Fail if two files in the source would be the same page, like `guide.md`
/// and `guide/README.md`, or `README.md` and `readme.md`: one would silently
/// overwrite the other. So would a page a plugin adds, as a file at its path.
/// Files whose URLs differ only in case, like `API.md` and `api/README.md`,
/// aren't caught, though they clash on a disk that ignores case, as macOS's
/// and Windows' do by default.
fn check_clashes(pages: &[Page]) -> Result<()> {
    let mut seen: HashMap<&Path, &Page> = HashMap::new();
    for page in pages {
        if let Some(other) = seen.insert(&page.out_path, page) {
            // `guide.md` is at `/guide` and `guide/README.md` at `/guide/`,
            // but both would be `guide/index.html`, served at `/guide/`.
            let url = if other.url.ends_with('/') {
                &other.url
            } else {
                &page.url
            };
            bail!(
                "{} and {} would both be the page at {url}: rename or move one",
                other.origin,
                page.origin,
            );
        }
    }
    Ok(())
}

/// Fail if a static file, a theme's or one from the source, would land on a
/// page in the destination: on the page itself, or on the folder it's in.
fn check_static_files(files: &[StaticFile], destination: &Path, outputs: &[Output]) -> Result<()> {
    for file in files {
        let target = destination.join(&file.path);
        if let Some(page) = outputs.iter().find(|page| page.path.starts_with(&target)) {
            bail!("{} clashes with {}", file.origin, page.name);
        }
    }
    Ok(())
}

/// Fail if a static file or a page would land on one of the files little
/// writes for search, or on a folder they're in, or one of them would land on
/// it, as a static file named `pagefind` would.
fn check_search_files(
    search_files: &[search::File],
    static_files: &[StaticFile],
    destination: &Path,
    outputs: &[Output],
) -> Result<()> {
    let taken: BTreeSet<&Path> = search_files
        .iter()
        .map(|file| file.path.as_path())
        .collect();
    let mut written: Vec<(&Path, &str)> = (static_files.iter())
        .map(|file| (file.path.as_path(), file.origin.as_str()))
        .collect();
    for page in outputs {
        written.push((page.path.strip_prefix(destination)?, &page.name));
    }
    let url = |path: &Path| path.to_string_lossy().replace('\\', "/");
    for (path, name) in written {
        if let Some(file) = path.ancestors().find(|path| taken.contains(path)) {
            bail!(
                "{name} clashes with /{}, which little writes for search: rename or move it",
                url(file)
            );
        }
        // Paths sort by their parts, so the files in a folder sort just after it.
        if (taken.range(path..).next()).is_some_and(|file| file.starts_with(path)) {
            bail!(
                "{name} clashes with /{}/, where little writes files for search: \
                 rename or move it",
                url(path)
            );
        }
    }
    Ok(())
}

/// Write a file, and the folders it's in.
fn write_file(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

/// Is the file or folder at `rel`, a path in the source folder, hidden, so
/// left out of the site? Its name starts with `.`, like Jupyter's
/// `.ipynb_checkpoints/` or an editor's swap file, or it's in a folder whose
/// name does. Only names inside the source folder count, so it can be hidden
/// itself, like `.github`.
pub fn is_hidden(rel: &Path) -> bool {
    rel.components().any(|component| {
        matches!(component, Component::Normal(name) if name.as_encoded_bytes().starts_with(b"."))
    })
}

/// Is this file a directory's home page? `README.md` is the documentation
/// convention (mirroring how GitHub renders READMEs), as is `README.ipynb`
/// for a notebook; little maps it to the web's `index.html`.
pub fn is_readme(rel: &Path) -> bool {
    rel.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("readme"))
        .unwrap_or(false)
}

/// What type of page the Markdown file or notebook at `rel` makes.
fn page_type(rel: &Path) -> PageType {
    if !is_readme(rel) {
        PageType::Page
    } else if folders(rel).is_empty() {
        PageType::Home
    } else {
        PageType::Section
    }
}

/// The folders `rel` is in, outermost first.
fn folders(rel: &Path) -> Vec<&str> {
    rel.parent()
        .into_iter()
        .flat_map(Path::components)
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect()
}

/// The URL of the folder the file at `rel` is in, which the links in it lead
/// from.
///
/// - `README.md`         -> `/`
/// - `guide/install.md`  -> `/guide/`
fn folder_url(rel: &Path) -> String {
    let mut url = String::from("/");
    for folder in folders(rel) {
        url.push_str(folder);
        url.push('/');
    }
    url
}

/// The URL of the file at `rel`, from the root of its version, like
/// `/assets/style.css` for `assets/style.css`.
fn file_url(rel: &Path) -> String {
    format!("/{}", slashed(rel))
}

/// `path`, with `/` between its parts, as plugins' hooks get it, like
/// `guide/install.md`.
fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Derive a page's pretty, extensionless URL: its folder's, then its name,
/// unless it's the folder's `README.md` or `README.ipynb`.
///
/// - `README.md`           -> `/`
/// - `getting-started.md`  -> `/getting-started`
/// - `guide/README.md`     -> `/guide/`
/// - `guide/install.md`    -> `/guide/install`
/// - `guide/analysis.ipynb` -> `/guide/analysis`
fn derive_url(rel: &Path) -> String {
    let folder = folder_url(rel);
    if is_readme(rel) {
        return folder;
    }
    let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("index");
    format!("{folder}{stem}")
}

/// Where a walk of the `source` folder finds the `destination`, if it's
/// inside it, like `docs/_build`, so a build doesn't copy the last one into
/// itself. Both are compared by their real paths, however they're written.
fn destination_in(source: &Path, destination: &Path) -> Option<PathBuf> {
    let real = destination.canonicalize().ok()?;
    let rel = real.strip_prefix(source.canonicalize().ok()?).ok()?;
    Some(source.join(rel))
}

/// Map a pretty URL to its directory-index output file in the destination.
fn out_path_for_url(destination: &Path, url: &str) -> PathBuf {
    let trimmed = url.trim_matches('/');
    if trimmed.is_empty() {
        destination.join("index.html")
    } else {
        destination.join(trimmed).join("index.html")
    }
}

/// Last-resort page title when there's no frontmatter `title` or `<h1>`.
/// Index pages borrow their folder name (root -> "Home").
fn fallback_title(rel: &Path, is_index: bool) -> String {
    if is_index {
        match rel
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
        {
            Some(dir) => humanize(dir),
            None => "Home".to_string(),
        }
    } else {
        let stem = rel
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");
        humanize(stem)
    }
}

/// "getting-started" -> "Getting Started".
fn humanize(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A leaf page collected while assembling the nav tree.
struct NavLeaf {
    title: String,
    url: String,
}

/// A folder in the docs tree: its own `README.md` (if any), leaf pages, and
/// subfolders.
#[derive(Default)]
struct DirNode {
    dirs: BTreeMap<String, DirNode>,
    pages: Vec<NavLeaf>,
    index: Option<NavLeaf>,
}

/// Arrange the pages in their folders in the source.
fn build_tree(pages: &[Page]) -> DirNode {
    let mut root = DirNode::default();
    for page in pages {
        insert_into_tree(&mut root, page);
    }
    root
}

/// Build the site-wide navigation tree from the source's folder structure.
fn build_nav(root: &DirNode) -> Vec<NavItem> {
    // Top-level: the site home (root README) first, then pages/sections.
    let mut items = Vec::new();
    if let Some(home) = &root.index {
        items.push(NavItem {
            title: home.title.clone(),
            url: Some(home.url.clone()),
            children: Vec::new(),
        });
    }
    items.extend(child_items(root));
    items
}

fn insert_into_tree(root: &mut DirNode, page: &Page) {
    let mut node = root;
    for folder in folders(&page.rel) {
        node = node.dirs.entry(folder.to_string()).or_default();
    }

    let leaf = NavLeaf {
        title: page.title.clone(),
        url: page.url.clone(),
    };
    if page.is_index() {
        node.index = Some(leaf);
    } else {
        node.pages.push(leaf);
    }
}

/// What's in the folder of the `README.md` at `rel`: its branch of the nav,
/// less the README itself.
fn children_of(root: &DirNode, rel: &Path) -> Vec<NavItem> {
    let mut node = root;
    for folder in folders(rel) {
        let Some(dir) = node.dirs.get(folder) else {
            return Vec::new();
        };
        node = dir;
    }
    child_items(node)
}

/// The child nav items of a folder (its leaf pages + subsections), sorted by
/// title, whatever its case, with the numbers in titles in order, so "2." comes
/// before "10.". A folder's own README is rendered by its parent as the
/// section link, so it's excluded here.
fn child_items(node: &DirNode) -> Vec<NavItem> {
    let mut items: Vec<NavItem> = Vec::new();
    for leaf in &node.pages {
        items.push(NavItem {
            title: leaf.title.clone(),
            url: Some(leaf.url.clone()),
            children: Vec::new(),
        });
    }
    for (name, sub) in &node.dirs {
        items.push(section_item(name, sub));
    }
    items.sort_by(|a, b| {
        alphanumeric_sort::compare_str(a.title.to_lowercase(), b.title.to_lowercase())
    });
    items
}

/// Turn a subfolder into a nav section. Its link/title come from the folder's
/// `README.md`; without one it's a plain label (`url: None`).
fn section_item(name: &str, node: &DirNode) -> NavItem {
    let (title, url) = match &node.index {
        Some(index) => (index.title.clone(), Some(index.url.clone())),
        None => (humanize(name), None),
    };
    NavItem {
        title,
        url,
        children: child_items(node),
    }
}

/// A page's table of contents, shaped like `site.nav`: each heading links to
/// its `#fragment`, and goes in the `children` of the nearest heading above it
/// with fewer `#`s, so a `####` goes under the `###` above it, or under the
/// `##` if there's no `###` between them. A heading with nothing above it,
/// like a `###` before the first `##`, goes at the top.
fn nest_headings(headings: &[render::TocItem]) -> Vec<NavItem> {
    nest_under(&mut headings.iter().peekable(), 0)
}

/// The headings that come next, up to the first that isn't deeper than
/// `depth`, each holding the deeper ones that follow it.
fn nest_under(
    headings: &mut Peekable<slice::Iter<'_, render::TocItem>>,
    depth: u8,
) -> Vec<NavItem> {
    let mut items = Vec::new();
    while let Some(heading) = headings.next_if(|heading| heading.depth > depth) {
        items.push(NavItem {
            title: heading.title.clone(),
            url: Some(heading.anchor.clone()),
            children: nest_under(headings, heading.depth),
        });
    }
    items
}

/// The pages before and after each page, by its URL: its `page.prev` and
/// `page.next`. They follow the nav from top to bottom, so they agree with the
/// sidebar: a section's own page, then what's in it, then on to the next item.
/// A folder without a `README.md` has no page, so it's passed over.
fn neighbours_in(nav: &[NavItem]) -> HashMap<&str, (Option<Link<'_>>, Option<Link<'_>>)> {
    let mut order = Vec::new();
    push_links(nav, &mut order);
    order
        .iter()
        .enumerate()
        .map(|(i, link)| {
            let prev = (i > 0).then(|| order[i - 1]);
            let next = order.get(i + 1).copied();
            (link.url, (prev, next))
        })
        .collect()
}

/// Add a link to each page in `items`, and in their children, to `links`, in
/// the order the nav lists them.
fn push_links<'a>(items: &'a [NavItem], links: &mut Vec<Link<'a>>) {
    for item in items {
        if let Some(url) = &item.url {
            links.push(Link {
                title: &item.title,
                url,
            });
        }
        push_links(&item.children, links);
    }
}

/// What's above each page in the nav, by its URL: its `page.breadcrumbs`. They
/// follow the nav from the top down, so they agree with the sidebar: each item
/// the page is in, as the nav titles it. The nav lists the home page, at
/// `home`, beside the items at its top, but it's above them, so their trails
/// start with it. A page listed twice gets the trail to the last place, where
/// its `page.prev` and `page.next` come from too.
fn breadcrumbs_in<'a>(nav: &'a [NavItem], home: &str) -> HashMap<&'a str, Vec<Breadcrumb<'a>>> {
    let is_home = |item: &NavItem| item.url.as_deref() == Some(home);
    let home = nav.iter().find(|item| is_home(item)).map(Breadcrumb::from);
    let mut trails = HashMap::new();
    for item in nav {
        let mut above: Vec<_> = home.filter(|_| !is_home(item)).into_iter().collect();
        push_trails(slice::from_ref(item), &mut above, &mut trails);
    }
    trails
}

/// Add the trail to each page in `items`, and in their children, to `trails`.
/// `above` holds what's above `items`.
fn push_trails<'a>(
    items: &'a [NavItem],
    above: &mut Vec<Breadcrumb<'a>>,
    trails: &mut HashMap<&'a str, Vec<Breadcrumb<'a>>>,
) {
    for item in items {
        if let Some(url) = &item.url {
            trails.insert(url, above.clone());
        }
        above.push(Breadcrumb::from(item));
        push_trails(&item.children, above, trails);
        above.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::PagePlugins;
    use std::io::Read;

    fn page(rel: &str, title: &str) -> Page {
        let origin = format!("docs/{rel}");
        let rel = PathBuf::from(rel);
        let url = derive_url(&rel);
        let out_path = out_path_for_url(Path::new("dist"), &url);
        let page_type = page_type(&rel);
        Page {
            rel,
            origin,
            added: false,
            url,
            out_path,
            title: title.to_string(),
            frontmatter: Mapping::new(),
            html: String::new(),
            toc: Vec::new(),
            page_type,
            links: Vec::new(),
            unrendered_math: Vec::new(),
            unrendered_diagrams: Vec::new(),
            unrendered_blocks: Vec::new(),
        }
    }

    /// `path`, written with `/`, with the system's separators, as little
    /// names files in what it warns of.
    fn native(path: &str) -> PathBuf {
        path.split('/').collect()
    }

    /// A fresh scratch site holding `files`, each a (path, contents) pair.
    fn scratch_site(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let site =
            std::env::temp_dir().join(format!("little-site-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&site);
        for (file, contents) in files {
            let path = site.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
        }
        site
    }

    /// What search found in each page of the site built into `dist`, as
    /// Pagefind keeps it: JSON of its URL, content, title, and headings.
    fn indexed(dist: &Path) -> Vec<String> {
        let fragments = fs::read_dir(dist.join("pagefind/fragment")).unwrap();
        let mut pages: Vec<String> = fragments
            .map(|fragment| {
                let file = fs::File::open(fragment.unwrap().path()).unwrap();
                let mut json = String::new();
                flate2::read::GzDecoder::new(file)
                    .read_to_string(&mut json)
                    .unwrap();
                json.trim_start_matches("pagefind_dcd").to_string()
            })
            .collect();
        pages.sort();
        pages
    }

    /// The titles (or "label") and URLs of `items`, nested like the nav.
    fn outline(items: &[NavItem]) -> Vec<String> {
        let mut lines = Vec::new();
        for item in items {
            lines.push(format!(
                "{} {}",
                item.title,
                item.url.as_deref().unwrap_or("(label)")
            ));
            lines.extend(
                outline(&item.children)
                    .iter()
                    .map(|line| format!("  {line}")),
            );
        }
        lines
    }

    /// The titles (or "(label)") and URLs of a page's breadcrumbs, top down.
    fn trail(breadcrumbs: &[Breadcrumb]) -> String {
        breadcrumbs
            .iter()
            .map(|crumb| format!("{} {}", crumb.title, crumb.url.unwrap_or("(label)")))
            .collect::<Vec<_>>()
            .join(" > ")
    }

    /// Every file in `dir`, by its path in it, sorted.
    fn files_in(dir: &Path) -> Vec<PathBuf> {
        let mut files: Vec<_> = WalkDir::new(dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.path().strip_prefix(dir).unwrap().to_path_buf())
            .collect();
        files.sort();
        files
    }

    #[test]
    fn detects_hidden_files() {
        for rel in [
            ".draft.md",
            ".ipynb_checkpoints/notes-checkpoint.md",
            "guide/.notes.md",
        ] {
            assert!(is_hidden(Path::new(rel)), "{rel}");
        }
        for rel in ["README.md", "guide/install.md", "v1.0/README.md"] {
            assert!(!is_hidden(Path::new(rel)), "{rel}");
        }
    }

    #[test]
    fn detects_readme_home_pages() {
        assert!(is_readme(Path::new("README.md")));
        assert!(is_readme(Path::new("guide/readme.md")));
        assert!(is_readme(Path::new("guide/README.ipynb")));
        assert!(!is_readme(Path::new("index.md")));
        assert!(!is_readme(Path::new("getting-started.md")));
    }

    #[test]
    fn classifies_pages_by_type() {
        for (rel, expected) in [
            ("README.md", PageType::Home),
            ("readme.md", PageType::Home),
            ("guide/README.md", PageType::Section),
            ("guide/advanced/README.md", PageType::Section),
            ("getting-started.md", PageType::Page),
            ("index.md", PageType::Page),
            ("guide/install.md", PageType::Page),
            ("README.ipynb", PageType::Home),
            ("guide/README.ipynb", PageType::Section),
            ("guide/analysis.ipynb", PageType::Page),
        ] {
            assert_eq!(page_type(Path::new(rel)), expected, "{rel}");
        }
    }

    #[test]
    fn derives_pretty_urls() {
        assert_eq!(derive_url(Path::new("README.md")), "/");
        assert_eq!(derive_url(Path::new("readme.md")), "/");
        assert_eq!(
            derive_url(Path::new("getting-started.md")),
            "/getting-started"
        );
        assert_eq!(derive_url(Path::new("guide/README.md")), "/guide/");
        assert_eq!(derive_url(Path::new("guide/install.md")), "/guide/install");
        assert_eq!(derive_url(Path::new("guide/README.ipynb")), "/guide/");
        assert_eq!(
            derive_url(Path::new("guide/analysis.ipynb")),
            "/guide/analysis"
        );
    }

    #[test]
    fn finds_the_url_of_a_files_folder() {
        for (rel, folder) in [
            ("README.md", "/"),
            ("logo.png", "/"),
            ("guide/README.md", "/guide/"),
            ("guide/install.md", "/guide/"),
            ("guide/advanced/tuning.md", "/guide/advanced/"),
        ] {
            assert_eq!(folder_url(Path::new(rel)), folder, "{rel}");
        }
    }

    #[test]
    fn maps_urls_to_directory_index_files() {
        let dist = Path::new("dist");
        assert_eq!(
            out_path_for_url(dist, "/"),
            PathBuf::from("dist/index.html")
        );
        assert_eq!(
            out_path_for_url(dist, "/getting-started"),
            PathBuf::from("dist/getting-started/index.html")
        );
        assert_eq!(
            out_path_for_url(dist, "/guide/"),
            PathBuf::from("dist/guide/index.html")
        );
        assert_eq!(
            out_path_for_url(dist, "/guide/install"),
            PathBuf::from("dist/guide/install/index.html")
        );
    }

    #[test]
    fn two_files_cannot_be_the_same_page() {
        let check = |a: &str, b: &str| {
            let pages = [page(a, "A"), page(b, "B")];
            check_clashes(&pages).map_err(|e| e.to_string())
        };
        assert_eq!(
            check("guide.md", "guide/README.md").unwrap_err(),
            "docs/guide.md and docs/guide/README.md would both be the page at /guide/: rename or move one"
        );
        assert_eq!(
            check("README.md", "readme.md").unwrap_err(),
            "docs/README.md and docs/readme.md would both be the page at /: rename or move one"
        );
        assert_eq!(
            check("analysis.ipynb", "analysis.md").unwrap_err(),
            "docs/analysis.ipynb and docs/analysis.md would both be the page at /analysis: \
             rename or move one"
        );

        // A page beside a folder is fine, and so, for now, are URLs that
        // differ only in case.
        assert!(check("guide.md", "guide/install.md").is_ok());
        assert!(check("API.md", "api/README.md").is_ok());
    }

    #[test]
    fn fallback_titles_use_folder_for_index() {
        assert_eq!(fallback_title(Path::new("README.md"), true), "Home");
        assert_eq!(fallback_title(Path::new("guide/README.md"), true), "Guide");
        assert_eq!(
            fallback_title(Path::new("getting-started.md"), false),
            "Getting Started"
        );
    }

    #[test]
    fn nav_nests_sections_under_folders() {
        let pages = vec![
            page("README.md", "Home"),
            page("getting-started.md", "Getting Started"),
            page("guide/README.md", "Guide"),
            page("guide/install.md", "Install"),
        ];
        let nav = build_nav(&build_tree(&pages));

        assert_eq!(nav.len(), 3);
        // Home first, then alphabetical.
        assert_eq!(nav[0].title, "Home");
        assert_eq!(nav[0].url.as_deref(), Some("/"));
        assert_eq!(nav[1].title, "Getting Started");
        assert_eq!(nav[1].url.as_deref(), Some("/getting-started"));

        let guide = &nav[2];
        assert_eq!(guide.title, "Guide");
        assert_eq!(guide.url.as_deref(), Some("/guide/"));
        assert_eq!(guide.children.len(), 1);
        assert_eq!(guide.children[0].title, "Install");
        assert_eq!(guide.children[0].url.as_deref(), Some("/guide/install"));
    }

    #[test]
    fn nav_puts_numbers_in_order() {
        let pages = vec![
            page("README.md", "Home"),
            page("adr/0010-research.md", "10. Create a research cluster"),
            page("adr/0002-hierarchy.md", "2. Use hierarchical elements"),
            page("adr/0001-record.md", "1. Record architecture decisions"),
            page("adr/0012-oidc.md", "8. OIDC with other platforms"),
            page("adr/0008-github.md", "8. GitHub OIDC"),
            page("adr/0014-registry.md", "ADR: Docker registry strategy"),
            page("adr/zebra.md", "about zebras"),
            page("steps/README.md", "Steps"),
            page("steps/ten.md", "Step 10"),
            page("steps/nine.md", "Step 9"),
            page("steps/one.md", "Step 1"),
        ];
        let nav = build_nav(&build_tree(&pages));

        // Numbers sort by their value, then by what follows them, and before
        // letters; letters sort as before, whatever their case.
        assert_eq!(
            outline(&nav),
            [
                "Home /",
                "Adr (label)",
                "  1. Record architecture decisions /adr/0001-record",
                "  2. Use hierarchical elements /adr/0002-hierarchy",
                "  8. GitHub OIDC /adr/0008-github",
                "  8. OIDC with other platforms /adr/0012-oidc",
                "  10. Create a research cluster /adr/0010-research",
                "  about zebras /adr/zebra",
                "  ADR: Docker registry strategy /adr/0014-registry",
                "Steps /steps/",
                "  Step 1 /steps/one",
                "  Step 9 /steps/nine",
                "  Step 10 /steps/ten",
            ]
        );
    }

    #[test]
    fn nav_section_without_readme_is_a_label() {
        let pages = vec![
            page("README.md", "Home"),
            page("api/reference.md", "Reference"),
        ];
        let nav = build_nav(&build_tree(&pages));

        let api = nav.iter().find(|i| i.title == "Api").expect("api section");
        assert_eq!(api.url, None); // no README -> plain label
        assert_eq!(api.children.len(), 1);
        assert_eq!(api.children[0].title, "Reference");
        assert_eq!(api.children[0].url.as_deref(), Some("/api/reference"));
    }

    #[test]
    fn home_and_section_pages_hold_their_folders() {
        let pages = vec![
            page("README.md", "Home"),
            page("getting-started.md", "Getting Started"),
            page("guide/README.md", "Guide"),
            page("guide/install.md", "Install"),
            page("guide/advanced/tuning.md", "Tuning"),
            page("api/reference.md", "Reference"),
        ];
        let tree = build_tree(&pages);

        // Home holds the whole site but itself; a folder without a README is
        // still there, as a label.
        assert_eq!(
            outline(&children_of(&tree, Path::new("README.md"))),
            [
                "Api (label)",
                "  Reference /api/reference",
                "Getting Started /getting-started",
                "Guide /guide/",
                "  Advanced (label)",
                "    Tuning /guide/advanced/tuning",
                "  Install /guide/install",
            ]
        );
        assert_eq!(
            outline(&children_of(&tree, Path::new("guide/README.md"))),
            [
                "Advanced (label)",
                "  Tuning /guide/advanced/tuning",
                "Install /guide/install",
            ]
        );
    }

    #[test]
    fn a_page_toc_nests_its_headings() {
        let md = "### Before\n\n### Also Before\n\n## One\n\n### One A\n\n#### One A i\n\n\
                  ### One B\n\n## Two\n\n#### Two i\n";
        let toc = render::parse_document(
            md,
            "/",
            &(2..=6),
            &SiteUrl::default(),
            &config::Markdown::default(),
            OffsetDateTime::UNIX_EPOCH,
            &mut PagePlugins::none(),
        )
        .unwrap()
        .toc;

        // A heading goes under the nearest one above it with fewer `#`s, even
        // two levels up; before the first `##`, it goes at the top, beside the
        // others.
        assert_eq!(
            outline(&nest_headings(&toc)),
            [
                "Before #before",
                "Also Before #also-before",
                "One #one",
                "  One A #one-a",
                "    One A i #one-a-i",
                "  One B #one-b",
                "Two #two",
                "  Two i #two-i",
            ]
        );
    }

    #[test]
    fn prev_and_next_follow_the_nav() {
        let pages = vec![
            page("README.md", "Home"),
            page("getting-started.md", "Getting Started"),
            page("guide/README.md", "Guide"),
            page("guide/install.md", "Install"),
            page("guide/advanced/tuning.md", "Tuning"),
            page("api/reference.md", "Reference"),
        ];
        let nav = build_nav(&build_tree(&pages));
        let neighbours = neighbours_in(&nav);

        // A section's own page comes before what's in it. The folders without
        // a README.md, api/ and guide/advanced/, have no page to link to.
        for (page, prev, next) in [
            ("/", None, Some("/api/reference")),
            ("/api/reference", Some("/"), Some("/getting-started")),
            ("/getting-started", Some("/api/reference"), Some("/guide/")),
            (
                "/guide/",
                Some("/getting-started"),
                Some("/guide/advanced/tuning"),
            ),
            (
                "/guide/advanced/tuning",
                Some("/guide/"),
                Some("/guide/install"),
            ),
            ("/guide/install", Some("/guide/advanced/tuning"), None),
        ] {
            let (before, after) = neighbours[page];
            assert_eq!(before.map(|link| link.url), prev, "{page}");
            assert_eq!(after.map(|link| link.url), next, "{page}");
        }
        assert_eq!(neighbours.len(), pages.len());
    }

    #[test]
    fn breadcrumbs_follow_the_nav() {
        let pages = vec![
            page("README.md", "Home"),
            page("getting-started.md", "Getting Started"),
            page("guide/README.md", "Guide"),
            page("guide/install.md", "Install"),
            page("guide/advanced/tuning.md", "Tuning"),
            page("api/reference.md", "Reference"),
        ];
        let nav = build_nav(&build_tree(&pages));
        let trails = breadcrumbs_in(&nav, "/");

        // The home page is above every other page. The folders without a
        // README.md, api/ and guide/advanced/, are labels on the way down.
        for (page, expected) in [
            ("/", ""),
            ("/getting-started", "Home /"),
            ("/guide/", "Home /"),
            ("/guide/install", "Home / > Guide /guide/"),
            (
                "/guide/advanced/tuning",
                "Home / > Guide /guide/ > Advanced (label)",
            ),
            ("/api/reference", "Home / > Api (label)"),
        ] {
            assert_eq!(trail(&trails[page]), expected, "{page}");
        }
        assert_eq!(trails.len(), pages.len());

        // Without a home page, the trails start at the top of the nav.
        let pages = vec![
            page("guide/README.md", "Guide"),
            page("guide/install.md", "Install"),
        ];
        let nav = build_nav(&build_tree(&pages));
        let trails = breadcrumbs_in(&nav, "/");
        assert_eq!(trail(&trails["/guide/"]), "");
        assert_eq!(trail(&trails["/guide/install"]), "Guide /guide/");
    }

    #[test]
    fn breadcrumbs_follow_a_nav_in_the_config() {
        // The home page can be anywhere at the top of a nav set by hand, and a
        // page can be listed twice.
        let nav: Vec<NavItem> = serde_yaml::from_str(
            "- { title: Tutorials, children: [\
                 { title: First Steps, url: /getting-started },\
                 { title: Setup, url: /guide/install }] }\n\
             - { title: Start, url: / }\n\
             - { title: Guide, url: /guide/, children: [{ title: Install, url: /guide/install }] }\n",
        )
        .unwrap();
        let trails = breadcrumbs_in(&nav, "/");

        // The trails use the nav's titles. A page listed twice gets the trail
        // to the last place, where its `prev` and `next` come from too.
        for (page, expected) in [
            ("/", ""),
            ("/getting-started", "Start / > Tutorials (label)"),
            ("/guide/", "Start /"),
            ("/guide/install", "Start / > Guide /guide/"),
        ] {
            assert_eq!(trail(&trails[page]), expected, "{page}");
        }
        let (prev, _) = neighbours_in(&nav)["/guide/install"];
        assert_eq!(prev.map(|link| link.url), Some("/guide/"));

        // Only a home page at the top of the nav is above everything else.
        let nav: Vec<NavItem> = serde_yaml::from_str(
            "- { title: Docs, children: [{ title: Home, url: / }, { title: Guide, url: /guide/ }] }\n",
        )
        .unwrap();
        let trails = breadcrumbs_in(&nav, "/");
        assert_eq!(trail(&trails["/"]), "Docs (label)");
        assert_eq!(trail(&trails["/guide/"]), "Docs (label)");
    }

    #[test]
    fn builds_with_a_local_theme() {
        let site = scratch_site(
            "theme",
            &[
                ("little.yml", "theme: ./path/to/my/theme\n"),
                ("docs/README.md", "# Home\n"),
                (
                    "path/to/my/theme/_templates/default.html",
                    "{{ site.greeting }} from my theme",
                ),
                ("path/to/my/theme/little.yml", "greeting: Hello\n"),
                ("path/to/my/theme/_static/style.css", "body {}\n"),
            ],
        );
        let png: &[u8] = &[0x89, b'P', b'N', b'G', 0x00, 0xff];
        let img = site.join("path/to/my/theme/_static/img");
        fs::create_dir_all(&img).unwrap();
        fs::write(img.join("logo.png"), png).unwrap();

        build(&site).unwrap();

        let home = fs::read_to_string(site.join("dist/index.html")).unwrap();
        assert_eq!(home, "Hello from my theme");
        // Static files go to the site's root, byte for byte.
        let css = fs::read(site.join("dist/style.css")).unwrap();
        assert_eq!(css, b"body {}\n");
        assert_eq!(fs::read(site.join("dist/img/logo.png")).unwrap(), png);
        // No 404.html template, no 404 page.
        assert!(!site.join("dist").join(NOT_FOUND_FILE).exists());
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_site_needs_only_its_docs() {
        // Without a little.yml or a theme, little's built-in theme builds it.
        let site = scratch_site("builtin", &[("docs/README.md", "# Home\n")]);

        build(&site).unwrap();

        let dist = site.join("dist");
        let home = fs::read_to_string(dist.join("index.html")).unwrap();
        assert!(
            home.contains("<title>Home · Documentation</title>"),
            "{home}"
        );
        assert!(dist.join(NOT_FOUND_FILE).is_file());
        let css = fs::read(dist.join("assets/style.css")).unwrap();
        assert_eq!(css, include_bytes!("../theme/_static/assets/style.css"));
        // And it's searchable, from the search box in its header.
        assert!(dist.join("pagefind/pagefind-component-ui.js").is_file());
        assert!(home.contains("<pagefind-modal-trigger>"), "{home}");
        // A site without versions has no menu of them.
        assert!(!home.contains("version-menu"), "{home}");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_site_can_turn_off_the_built_in_themes_search() {
        let site = scratch_site(
            "builtin-unsearched",
            &[
                ("little.yml", "search: false\n"),
                ("docs/README.md", "# Home\n"),
            ],
        );

        build(&site).unwrap();

        let dist = site.join("dist");
        let home = fs::read_to_string(dist.join("index.html")).unwrap();
        assert!(!home.contains("pagefind"), "{home}");
        assert!(!dist.join("pagefind").exists());
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn builds_with_a_theme_that_builds_on_another() {
        let site = scratch_site(
            "chain",
            &[
                ("little.yml", "theme: ./company\nname: My Docs\n"),
                ("docs/README.md", "# Home\n"),
                ("company/little.yml", "theme: ../base\ngreeting: Hi\n"),
                ("company/_templates/partials/brand.html", "company"),
                ("company/_static/logo.svg", "company logo"),
                ("base/little.yml", "greeting: Hello\nname: Base\n"),
                (
                    "base/_templates/default.html",
                    r#"{% include "partials/brand.html" %}: {{ site.greeting }} from {{ site.name }}"#,
                ),
                ("base/_templates/partials/brand.html", "base"),
                ("base/_static/logo.svg", "base logo"),
                ("base/_static/style.css", "body {}\n"),
            ],
        );

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        assert_eq!(read("index.html"), "company: Hi from My Docs");
        assert_eq!(read("logo.svg"), "company logo");
        assert_eq!(read("style.css"), "body {}\n");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn builds_with_a_theme_that_builds_on_a_downloaded_one() {
        let url = "https://example.invalid/fancy-1.0.zip";
        let company_cfg = format!("theme: {url}\ngreeting: Hi\n");
        let site = scratch_site(
            "downloaded",
            &[
                ("little.yml", "theme: ./company\n"),
                ("docs/README.md", "# Home\n"),
                ("company/little.yml", &company_cfg),
                ("company/_static/logo.svg", "company logo"),
            ],
        );
        // Where it's kept once it's downloaded, so the test needn't download it.
        let fancy = crate::download::folder(&site, url);
        for (file, contents) in [
            ("little.yml", "greeting: Hello\n"),
            (
                "_templates/default.html",
                "{{ site.greeting }} from {{ page.title }}",
            ),
            ("_static/logo.svg", "fancy logo"),
            ("_static/style.css", "body {}\n"),
        ] {
            let path = fancy.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        assert_eq!(read("index.html"), "Hi from Home");
        assert_eq!(read("logo.svg"), "company logo");
        assert_eq!(read("style.css"), "body {}\n");

        // Its folder is named with a hash, so its files are named by its URL.
        fs::write(fancy.join("_static/index.html"), "static").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "{} in the theme from {url} clashes with the page built from {}",
            native("_static/index.html").display(),
            site.join("docs").join("README.md").display()
        );
        assert_eq!(message, expected);
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_build_needs_a_source_folder_and_a_theme() {
        let site = scratch_site("needs", &[("little.yml", "name: My Docs\n")]);
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "there's no source folder at {}",
            site.join("docs").display()
        );
        assert_eq!(message, expected);

        fs::create_dir_all(site.join("docs")).unwrap();
        fs::write(site.join("little.yml"), "theme: false\n").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = "little can't build a site without a theme (`theme: false`) yet";
        assert_eq!(message, expected);
        assert!(!site.join("dist").exists());
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn builds_from_and_into_the_folders_the_config_chooses() {
        let site = scratch_site(
            "folders",
            &[
                (
                    "little.yml",
                    "theme: ./theme\nsource: ./content\ndestination: ./public\n",
                ),
                ("content/README.md", "# Home\n"),
                ("content/guide/install.md", "# Install\n"),
                ("docs/README.md", "# Not the source\n"),
                ("theme/_templates/default.html", "{{ page.title }}"),
            ],
        );

        let destination = build(&site).unwrap().destination;

        assert_eq!(destination, site.join("public"));
        let read = |path: &str| fs::read_to_string(destination.join(path)).unwrap();
        assert_eq!(read("index.html"), "Home");
        assert_eq!(read("guide/install/index.html"), "Install");
        assert!(!site.join("dist").exists());
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn hidden_files_and_folders_are_left_out() {
        // Like Jupyter's checkpoints, an editor's swap files, or drafts.
        let site = scratch_site(
            "hidden",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                ("docs/.draft.md", "# Draft\n"),
                (
                    "docs/.ipynb_checkpoints/notes-checkpoint.md",
                    "# Checkpoint\n",
                ),
                ("docs/.hidden/README.md", "# Hidden\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/guide/install.md", "# Install\n"),
                ("docs/guide/.notes.md", "# Notes\n"),
                ("docs/.DS_Store", ""),
                ("docs/.drafts/diagram.png", "png"),
                ("docs/guide/.install.md.swp", ""),
                (
                    "theme/_templates/default.html",
                    "{% macro list(items) %}{% for item in items %}\
                     [{{ item.title }}{% if item.children %} {{ list(item.children) }}{% endif %}]\
                     {% endfor %}{% endmacro %}{{ list(site.nav) }}",
                ),
                ("theme/_static/.nojekyll", ""),
            ],
        );

        build(&site).unwrap();

        // They aren't pages or copied, so they aren't in the nav either. A
        // theme's static files are copied whole, hidden ones too.
        let dist = site.join("dist");
        let built = [
            MARKER_FILE,
            ".nojekyll",
            "guide/index.html",
            "guide/install/index.html",
            "index.html",
        ];
        assert_eq!(files_in(&dist), built.map(PathBuf::from));
        let home = fs::read_to_string(dist.join("index.html")).unwrap();
        assert_eq!(home, "[Home][Guide [Install]]");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn only_hidden_names_inside_the_source_count() {
        // So the source can be a hidden folder, and the site can be in one.
        let root = scratch_site(
            "hidden-source",
            &[
                (".site/little.yml", "theme: ./theme\nsource: ./.github\n"),
                (".site/README.md", "# Site\n"),
                (".site/.github/README.md", "# Home\n"),
                (".site/.github/ISSUE_TEMPLATE/bug.md", "# Bug\n"),
                (".site/.github/.drafts/wip.md", "# WIP\n"),
                (".site/.little/themes/fancy/README.md", "# Fancy\n"),
                (".site/theme/_templates/default.html", "{{ page.title }}"),
            ],
        );
        let site = root.join(".site");
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();

        build(&site).unwrap();

        let built = [MARKER_FILE, "ISSUE_TEMPLATE/bug/index.html", "index.html"];
        assert_eq!(files_in(&dist), built.map(PathBuf::from));
        assert_eq!(read("index.html"), "Home");

        // With the site's folder as its source, what's in `.github/` and
        // `.little/` isn't a page. Everything else is published, its config
        // and theme too, but for the destination, which a build never copies
        // into itself.
        fs::write(site.join("little.yml"), "theme: ./theme\nsource: .\n").unwrap();
        build(&site).unwrap();

        let built = [
            MARKER_FILE,
            "index.html",
            "little.yml",
            "theme/_templates/default.html",
        ];
        assert_eq!(files_in(&dist), built.map(PathBuf::from));
        assert_eq!(read("index.html"), "Site");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_build_only_wipes_a_folder_it_built() {
        let files = [
            ("site/docs/README.md", "# Home\n"),
            ("site/theme/_templates/default.html", "{{ page.title }}"),
            ("site/theme/_static/style.css", "body {}\n"),
            ("other/notes.txt", "mine\n"),
        ];
        let root = scratch_site("wipe", &files);
        let site = root.join("site");
        let build_into = |destination: &str| {
            let site_cfg = format!("theme: ./theme\ndestination: {destination}\n");
            fs::write(site.join("little.yml"), site_cfg).unwrap();
            build(&site)
        };

        // A folder with files little didn't build stops the build, untouched.
        for destination in [".", "..", "./docs", "./theme", "../other"] {
            let message = format!("{:#}", build_into(destination).unwrap_err());
            assert!(message.contains("wasn't built by little"), "{message}");
        }
        for (file, contents) in files {
            assert_eq!(fs::read_to_string(root.join(file)).unwrap(), contents);
        }

        // A missing or empty folder is fine, and from then on it's little's:
        // the next build leaves nothing of the last.
        fs::create_dir_all(root.join("empty")).unwrap();
        for destination in ["./dist", "../empty", "./docs/_build"] {
            let dir = build_into(destination).unwrap().destination;
            fs::write(dir.join("stale.html"), "old").unwrap();
            build_into(destination).unwrap();
            assert!(!dir.join("stale.html").exists(), "{destination}");
            assert_eq!(fs::read_to_string(dir.join("index.html")).unwrap(), "Home");
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn every_other_file_in_the_source_is_copied() {
        let site = scratch_site(
            "copied",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                ("docs/logo.svg", "<svg/>"),
                ("docs/guide/install.md", "# Install\n"),
                ("docs/guide/install/demo.mp4", "mp4"),
                ("docs/assets/style.css", "site"),
                ("theme/_templates/default.html", "{{ page.title }}"),
                ("theme/_static/assets/style.css", "theme"),
                ("theme/_static/assets/theme.js", "theme"),
            ],
        );
        let png = [
            0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0x00, 0xff,
        ];
        let image = site.join("docs/guide/images/step 1.png");
        fs::create_dir_all(image.parent().unwrap()).unwrap();
        fs::write(&image, png).unwrap();

        build(&site).unwrap();

        // To the same path, beside the pages, even in a page's own folder.
        let dist = site.join("dist");
        let built = [
            MARKER_FILE,
            "assets/style.css",
            "assets/theme.js",
            "guide/images/step 1.png",
            "guide/install/demo.mp4",
            "guide/install/index.html",
            "index.html",
            "logo.svg",
        ];
        assert_eq!(files_in(&dist), built.map(PathBuf::from));
        assert_eq!(fs::read(dist.join("guide/images/step 1.png")).unwrap(), png);
        // Where the theme has a file at the same path, the site's wins.
        let style = fs::read_to_string(dist.join("assets/style.css")).unwrap();
        assert_eq!(style, "site");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn sass_in_the_theme_is_compiled_and_in_the_source_copied() {
        let site = scratch_site(
            "sass",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                ("docs/examples/button.scss", "$x: 1;"),
                ("docs/examples/_mixins.scss", "$y: 2;"),
                ("theme/_templates/default.html", "{{ page.title }}"),
                (
                    "theme/_static/assets/style.scss",
                    "@use 'colors';\na { color: colors.$link; }",
                ),
                ("theme/_sass/_colors.scss", "$link: teal;"),
            ],
        );
        build(&site).unwrap();

        let dist = site.join("dist");
        let built = [
            MARKER_FILE,
            "assets/style.css",
            "examples/_mixins.scss",
            "examples/button.scss",
            "index.html",
        ];
        assert_eq!(files_in(&dist), built.map(PathBuf::from));
        let css = fs::read_to_string(dist.join("assets/style.css")).unwrap();
        assert_eq!(css, "a {\n  color: teal;\n}\n");
        let example = fs::read_to_string(dist.join("examples/button.scss")).unwrap();
        assert_eq!(example, "$x: 1;");

        // A stylesheet that doesn't compile fails the build, which leaves the
        // last one.
        let colors = site.join(native("theme/_sass/_colors.scss"));
        fs::write(&colors, "$link: $missing;").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "compiling {}: {}:1:8: Undefined variable.",
            site.join(native("theme/_static/assets/style.scss"))
                .display(),
            colors.display()
        );
        assert_eq!(message, expected);
        let css = fs::read_to_string(dist.join("assets/style.css")).unwrap();
        assert_eq!(css, "a {\n  color: teal;\n}\n");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_destination_in_the_source_isnt_copied_into_itself() {
        let site = scratch_site(
            "inside",
            &[
                ("docs/README.md", "# Home\n"),
                ("docs/logo.svg", "<svg/>"),
                ("theme/_templates/default.html", "{{ page.title }}"),
            ],
        );
        // However its path is written. The second build finds the first's.
        for destination in ["./docs/_build", "./docs/../docs/_build"] {
            let site_cfg = format!("theme: ./theme\ndestination: {destination}\n");
            fs::write(site.join("little.yml"), site_cfg).unwrap();
            build(&site).unwrap();
            build(&site).unwrap();
            let built = [MARKER_FILE, "index.html", "logo.svg"];
            let files = files_in(&site.join("docs/_build"));
            assert_eq!(files, built.map(PathBuf::from), "{destination}");
        }
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn links_lead_from_the_folder_of_the_file_theyre_in() {
        // As on GitHub, whatever URL the page is served at, with or without a
        // `/` at its end, and under the site's path.
        let site = scratch_site(
            "relative",
            &[
                (
                    "little.yml",
                    "theme: ./theme\nurl: https://withlittle.github.io/little\n",
                ),
                ("docs/README.md", "![Logo](assets/logo.svg)\n"),
                (
                    "docs/guide/README.md",
                    "![Diagram](images/diagram.svg) [Install](install)\n",
                ),
                (
                    "docs/guide/install.md",
                    "![Diagram](images/diagram.svg) ![Logo](../assets/logo.svg)\n",
                ),
                ("docs/assets/logo.svg", "<svg/>"),
                ("docs/guide/images/diagram.svg", "<svg/>"),
                ("theme/_templates/default.html", "{{ page.content | safe }}"),
            ],
        );

        build(&site).unwrap();

        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();
        let logo = r#"<img src="/little/assets/logo.svg" alt="Logo" />"#;
        let diagram = r#"<img src="/little/guide/images/diagram.svg" alt="Diagram" />"#;
        let install = r#"<a href="/little/guide/install">Install</a>"#;
        assert_eq!(read("index.html"), format!("<p>{logo}</p>\n"));
        assert_eq!(
            read("guide/index.html"),
            format!("<p>{diagram} {install}</p>\n")
        );
        assert_eq!(
            read("guide/install/index.html"),
            format!("<p>{diagram} {logo}</p>\n")
        );
        // And each image is where it leads.
        for image in ["assets/logo.svg", "guide/images/diagram.svg"] {
            assert!(dist.join(image).is_file(), "{image}");
        }
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn notebooks_are_pages() {
        let notebook = |cells: serde_json::Value| {
            let notebook = serde_json::json!({
                "cells": cells,
                "metadata": { "language_info": { "name": "python" } },
                "nbformat": 4,
                "nbformat_minor": 5
            });
            notebook.to_string()
        };
        let markdown = |source: &str| serde_json::json!({ "cell_type": "markdown", "metadata": {}, "source": source });
        let analysis = notebook(serde_json::json!([
            markdown("# Analysis\n\n## Results\n\n[Home](../README.md)"),
            {
                "cell_type": "code",
                "execution_count": 1,
                "metadata": {},
                "outputs": [{ "output_type": "stream", "name": "stdout", "text": "42\n" }],
                "source": "print(6 * 7)"
            }
        ]));
        let data = notebook(serde_json::json!([markdown("What we have.")]));
        let site = scratch_site(
            "notebooks",
            &[
                ("little.yml", "theme: ./theme\n"),
                (
                    "docs/README.md",
                    "[Analysis](guide/analysis.ipynb#results)\n",
                ),
                ("docs/guide/analysis.ipynb", &analysis),
                ("docs/data/README.ipynb", &data),
                (
                    "theme/_templates/default.html",
                    "{{ page.title }}\n{{ page.content | safe }}",
                ),
            ],
        );
        build(&site).unwrap();

        // Each is built into its page, as a Markdown file is, and isn't
        // copied.
        let dist = site.join("dist");
        let built = [
            MARKER_FILE,
            "data/index.html",
            "guide/analysis/index.html",
            "index.html",
        ];
        assert_eq!(files_in(&dist), built.map(PathBuf::from));
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();
        let analysis = read("guide/analysis/index.html");
        assert!(analysis.starts_with("Analysis\n<h1"), "{analysis}");
        assert!(analysis.contains("<a href=\"/\">Home</a>"), "{analysis}");
        let output = "<pre class=\"highlight\"><code class=\"language-output\">42\n</code></pre>";
        assert!(analysis.contains(output), "{analysis}");
        assert_eq!(read("data/index.html"), "Data\n<p>What we have.</p>\n");
        // A link to a notebook leads to its page.
        let home = read("index.html");
        let link = "<a href=\"/guide/analysis#results\">Analysis</a>";
        assert!(home.contains(link), "{home}");

        // A notebook little can't read stops the build, naming it.
        let broken = site.join(native("docs/broken.ipynb"));
        fs::write(&broken, "{").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "rendering {}: reading the notebook: EOF while parsing an object at line 1 column 1",
            broken.display()
        );
        assert_eq!(message, expected);
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn each_formula_that_cant_be_rendered_is_noted() {
        let notebook = serde_json::json!({
            "cells": [{ "cell_type": "markdown", "metadata": {}, "source": "$x^2$ and $\\dv{f}{x}$" }],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 5
        });
        let site = scratch_site(
            "math",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "$$\\qty{1}{m}$$\n"),
                ("docs/analysis.ipynb", &notebook.to_string()),
                ("theme/_templates/default.html", "{{ page.content | safe }}"),
            ],
        );
        let built = build(&site).unwrap();

        // A notebook's math is MathML, as a Markdown file's is.
        let analysis = fs::read_to_string(site.join("dist/analysis/index.html")).unwrap();
        assert!(analysis.starts_with("<p><math><msup>"), "{analysis}");
        // Each formula that isn't is noted, with the file it's in, in the
        // order of the pages.
        let docs = site.join("docs");
        let warnings: Vec<_> = built
            .unrendered_math
            .iter()
            .map(|f| f.to_string())
            .collect();
        let expected = [
            format!(
                "can't render $$\\qty{{1}}{{m}}$$ in {}: Unknown command \"\\qty\".",
                docs.join("README.md").display()
            ),
            format!(
                "can't render $\\dv{{f}}{{x}}$ in {}: Unknown command \"\\dv\".",
                docs.join("analysis.ipynb").display()
            ),
        ];
        assert_eq!(warnings, expected);
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn each_diagram_that_cant_be_drawn_is_noted() {
        let notebook = serde_json::json!({
            "cells": [{ "cell_type": "markdown", "metadata": {}, "source": "```mermaid\nflowchat\n```" }],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 5
        });
        let site = scratch_site(
            "diagrams",
            &[
                ("little.yml", "theme: ./theme\n"),
                (
                    "docs/README.md",
                    "---\ntitle: Home\n---\n```mermaid\nflowchart LR\n  A --> B\n```\n\n\
                     ```mermaid\nflowchat LR\n```\n",
                ),
                ("docs/analysis.ipynb", &notebook.to_string()),
                ("theme/_templates/default.html", "{{ page.content | safe }}"),
            ],
        );
        let built = build(&site).unwrap();

        let home = fs::read_to_string(site.join("dist/index.html")).unwrap();
        assert!(home.starts_with("<svg id=\"mermaid-1\" "), "{home}");
        // Each that isn't is noted, with the file it's in, and the line its
        // block starts on in a Markdown file, in the order of the pages.
        let docs = site.join("docs");
        let warnings: Vec<_> = (built.unrendered_diagrams.iter())
            .map(|d| d.to_string())
            .collect();
        let expected = [
            format!(
                "can't draw the diagram on line 9 of {}: No diagram type detected",
                docs.join("README.md").display()
            ),
            format!(
                "can't draw a diagram in {}: No diagram type detected",
                docs.join("analysis.ipynb").display()
            ),
        ];
        assert_eq!(warnings, expected);
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn each_block_a_plugin_cant_render_is_noted() {
        let notebook = serde_json::json!({
            "cells": [{ "cell_type": "markdown", "metadata": {}, "source": "```test\nerror it is lost\n```" }],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 5
        });
        let site_cfg = format!(
            "theme: ./theme\nplugins:\n  test: {}\n",
            crate::plugin::tests::test_plugin().display()
        );
        let site = scratch_site(
            "plugins",
            &[
                ("little.yml", &site_cfg),
                (
                    "docs/README.md",
                    "---\ntitle: Home\n---\n```test\nblock\n```\n\n```test\nerror it is broken\n```\n",
                ),
                ("docs/analysis.ipynb", &notebook.to_string()),
                ("theme/_templates/default.html", "{{ page.content | safe }}"),
            ],
        );
        let built = build(&site).unwrap();

        // A plugin is told the file of each block's page from the site's
        // folder, wherever the site is.
        let home = fs::read_to_string(site.join("dist/index.html")).unwrap();
        assert!(
            home.starts_with("<p>test | test | docs/README.md</p>\n"),
            "{home}"
        );
        // Each block it can't render is noted, with the file it's in, and the
        // line it starts on in a Markdown file, in the order of the pages.
        let docs = site.join("docs");
        let warnings: Vec<_> = (built.unrendered_blocks.iter())
            .map(|b| b.to_string())
            .collect();
        let expected = [
            format!(
                "plugin `test` can't render the block on line 8 of {}: it is broken",
                docs.join("README.md").display()
            ),
            format!(
                "plugin `test` can't render a block in {}: it is lost",
                docs.join("analysis.ipynb").display()
            ),
        ];
        assert_eq!(warnings, expected);
        let _ = fs::remove_dir_all(&site);
    }

    /// A site's config that has the test plugin, as `api`, beside `config`,
    /// adding the files `add` lists to the source, and warning of `warn`.
    fn adding_files(config: &str, add: serde_json::Value, warn: Option<&str>) -> String {
        let mut plugin = serde_json::json!({
            "wasm": crate::plugin::tests::test_plugin(),
            "files": { "add": add },
        });
        if let Some(warn) = warn {
            plugin["warn"] = warn.into();
        }
        format!("{config}plugins:\n  api: {plugin}\n")
    }

    #[test]
    fn a_plugins_pages_are_built_as_files_at_their_paths_would_be() {
        let pages = serde_json::json!([
            {
                "path": "reference/README.md",
                "text": "# Reference\n\nSee [base](pkg/base.md#thing), and [the guide](../guide.md).\n",
            },
            {
                "path": "reference/pkg/base.md",
                "text": "---\ntitle: The base\n---\n## `Thing`\n\n```test\nblock\n```\n\n\
                         [Up](../README.md) [Gone](gone.md)\n",
            },
        ]);
        let site_cfg = adding_files(
            "theme: ./theme\nrepo: https://github.com/withlittle/little\n",
            pages,
            Some("can't read pkg/broken.py"),
        );
        let site = scratch_site(
            "added",
            &[
                ("little.yml", &site_cfg),
                ("docs/README.md", "# Home\n"),
                (
                    "docs/guide.md",
                    "# Guide\n\nSee [the API](reference/README.md).\n",
                ),
                (
                    "theme/_templates/default.html",
                    "{{ page.title }} | {{ 'source_url' in page }} | \
                     {{ page.breadcrumbs | map(attribute='title') | join(', ') }} | \
                     {{ page.prev.title if page.prev }} | {{ page.next.title if page.next }}\n\
                     {{ page.content | safe }}",
                ),
            ],
        );
        let built = build(&site).unwrap();
        let read = |file: &str| fs::read_to_string(site.join("dist").join(file)).unwrap();

        // Each is at its file's URL, in the nav, and its relative links lead
        // from its folder, but it has no file in the repository.
        let reference = read("reference/index.html");
        assert!(
            reference
                .starts_with("Reference | False | Home | Guide | The base\n<h1 id=\"reference\">"),
            "{reference}"
        );
        assert!(
            reference.contains(
                "<p>See <a href=\"/reference/pkg/base#thing\">base</a>, and \
                 <a href=\"/guide\">the guide</a>.</p>"
            ),
            "{reference}"
        );
        // Its frontmatter is read, and its blocks go to plugins, which are
        // told it's the file at its path.
        let base = read("reference/pkg/base/index.html");
        assert!(
            base.starts_with(
                "The base | False | Home, Reference, Pkg | Reference | \n<h2 id=\"thing\">"
            ),
            "{base}"
        );
        assert!(
            base.contains("<p>test | test | docs/reference/pkg/base.md</p>"),
            "{base}"
        );
        let guide = read("guide/index.html");
        assert!(
            guide.starts_with("Guide | True | Home | Home | Reference\n"),
            "{guide}"
        );

        // What it warns of is noted, and its links are checked, naming it.
        let warnings: Vec<_> = (built.plugin_warnings.iter())
            .map(|w| w.to_string())
            .collect();
        assert_eq!(warnings, ["plugin `api`: can't read pkg/broken.py"]);
        let broken: Vec<_> = (crate::links::check(&built, crate::links::Scope::Internal))
            .unwrap()
            .iter()
            .map(|b| b.to_string())
            .collect();
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert!(
            broken[0].starts_with("broken link gone.md in plugin `api`'s reference/pkg/base.md: "),
            "{broken:?}"
        );
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_plugins_page_cant_be_the_same_page_as_another() {
        let clash = |label: &str, files: &[(&str, &str)], pages: serde_json::Value| {
            let site_cfg = adding_files("theme: ./theme\n", pages, None);
            let mut files = files.to_vec();
            files.push(("little.yml", &site_cfg));
            files.push(("theme/_templates/default.html", "{{ page.content | safe }}"));
            let site = scratch_site(label, &files);
            let error = format!("{:#}", build(&site).err().unwrap());
            let _ = fs::remove_dir_all(&site);
            error.replace(&site.display().to_string(), ".")
        };
        let page = |path: &str| serde_json::json!({ "path": path, "text": "# API\n" });
        assert_eq!(
            clash(
                "clash-file",
                &[("docs/reference.md", "# Reference\n")],
                serde_json::json!([page("reference/README.md")]),
            ),
            format!(
                "{} and plugin `api`'s reference/README.md would both be the page at \
                 /reference/: rename or move one",
                Path::new(".").join(native("docs/reference.md")).display()
            )
        );
        assert_eq!(
            clash(
                "clash-plugin",
                &[("docs/README.md", "# Home\n")],
                serde_json::json!([page("api.md"), page("api/README.md")]),
            ),
            "plugin `api`'s api.md and plugin `api`'s api/README.md would both be the page at \
             /api/: rename or move one"
        );
    }

    #[test]
    fn plugins_hook_into_each_step_of_the_build() {
        let plugin = serde_json::json!({
            "wasm": crate::plugin::tests::test_plugin(),
            "config": { "merge": { "title": "Hooked" } },
            "files": {
                "remove": ["old.md"],
                "add": [{ "path": "added.md", "text": "# Added\n\nHello, world.\n" }],
            },
            "source": { "replace": ["world", "plugins"] },
            "content": { "append": "<p>{arg}</p>" },
            "site": { "merge": { "motto": "Hooked, too" } },
            "page": { "only": "/docs/", "merge": { "badge": "New", "title": "Welcome" } },
            "html": { "only": "/docs/added", "append": "<a href=\"/docs/gone\">Gone</a>" },
            "destination": { "list": "files.txt" },
        });
        let site_cfg =
            format!("theme: ./theme\nurl: https://example.com/docs\nplugins:\n  test: {plugin}\n");
        let site = scratch_site(
            "hooks",
            &[
                ("little.yml", &site_cfg),
                ("docs/README.md", "# Home\n\nHello, world.\n"),
                ("docs/old.md", "# Old\n"),
                ("theme/little.yml", "search: true\n"),
                (
                    "theme/_templates/default.html",
                    "<html lang=\"en\">{{ site.title }} | {{ site.motto }} | {{ page.badge }} | \
                     {{ page.title }}\n{{ page.content | safe }}</html>",
                ),
            ],
        );
        let built = build(&site).unwrap();
        let dist = site.join("dist");
        let read = |file: &str| fs::read_to_string(dist.join(file)).unwrap();

        // `config` titles the site, and `site` gives it a motto. `files`
        // leaves out a page and adds one, whose text `source` changes, as it
        // does every page's, and `content` adds to each page's HTML, naming
        // its file. `page` changes what the theme gets, for the page at its
        // URL under the site's path, and `html` changes what it makes.
        let home = read("index.html");
        assert!(
            home.starts_with("<html lang=\"en\">Hooked | Hooked, too | New | Welcome\n"),
            "{home}"
        );
        assert!(
            home.ends_with("<p>Hello, plugins.</p>\n<p>docs/README.md</p></html>"),
            "{home}"
        );
        let added = read("added/index.html");
        assert!(
            added.starts_with("<html lang=\"en\">Hooked | Hooked, too |  | Added\n"),
            "{added}"
        );
        assert!(
            added.ends_with(
                "<p>Hello, plugins.</p>\n<p>docs/added.md</p></html><a href=\"/docs/gone\">Gone</a>"
            ),
            "{added}"
        );
        assert!(!dist.join("old").exists());

        // `destination` adds a file, knowing the path of every file the
        // build writes, and it's noted as the plugin's.
        let files = read("files.txt");
        let paths: Vec<_> = files.lines().collect();
        assert_eq!(paths[..2], ["added/index.html", "index.html"], "{files}");
        assert!(paths.contains(&"pagefind/pagefind.js"), "{files}");
        assert!(
            paths[2..].iter().all(|path| path.starts_with("pagefind/")),
            "{files}"
        );
        assert_eq!(
            built.origins[Path::new("files.txt")],
            "plugin `test`'s files.txt"
        );

        // Search indexes each page as `page` leaves it...
        let pages = indexed(&dist);
        let home = (pages.iter())
            .find(|page| page.contains(r#""url":"/""#))
            .unwrap();
        assert!(home.contains(r#""title":"Welcome""#), "{home}");
        assert!(home.contains("Hello, plugins."), "{home}");
        assert!(home.contains("docs/README.md"), "{home}");
        // ...and links are checked as `html` leaves them.
        let broken: Vec<_> = (crate::links::check(&built, crate::links::Scope::Internal))
            .unwrap()
            .iter()
            .map(|b| b.to_string())
            .collect();
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert!(
            broken[0].starts_with("broken link /docs/gone in plugin `test`'s added.md: "),
            "{broken:?}"
        );
        assert!(
            built.plugin_warnings.is_empty(),
            "{:?}",
            built.plugin_warnings
        );
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_hook_that_fails_is_warned_of_or_stops_the_build() {
        let site = scratch_site(
            "hooks-fail",
            &[
                ("docs/README.md", "# Home\n\nHello, world.\n"),
                ("theme/_templates/default.html", "{{ page.content | safe }}"),
            ],
        );
        let with = |mut plugin: serde_json::Value| {
            plugin["wasm"] = serde_json::json!(crate::plugin::tests::test_plugin());
            let site_cfg = format!("theme: ./theme\nplugins:\n  test: {plugin}\n");
            fs::write(site.join("little.yml"), site_cfg).unwrap();
            build(&site)
        };

        // One that changes a page is warned of, naming what it's built from,
        // and the page is built as if it hadn't been called.
        let built = with(serde_json::json!({
            "source": { "error": "no source for {arg}", "replace": ["world", "plugins"] },
            "page": { "warn": "on {arg}" },
        }))
        .unwrap();
        let home = fs::read_to_string(site.join("dist/index.html")).unwrap();
        assert!(home.contains("Hello, world."), "{home}");
        let warnings: Vec<_> = (built.plugin_warnings.iter())
            .map(|w| w.to_string())
            .collect();
        let readme = site.join(native("docs/README.md"));
        assert_eq!(
            warnings,
            [
                format!(
                    "plugin `test`'s `source` hook failed on {}: no source for docs/README.md",
                    readme.display()
                ),
                "plugin `test`: on /".to_string(),
            ]
        );

        // One that changes the site, its files, or what's written, stops the
        // build.
        let error = |hooks| format!("{:#}", with(hooks).unwrap_err());
        assert_eq!(
            error(serde_json::json!({ "files": { "error": "boom" } })),
            "plugin `test`'s `files` hook failed: boom"
        );
        let message = error(serde_json::json!({ "site": { "merge": { "nav": "nowhere" } } }));
        assert!(
            message.starts_with(
                "plugin `test`'s `site` hook gives back a `nav` that isn't a list of links, each \
                 `{ title, url, children }`: "
            ),
            "{message}"
        );
        assert_eq!(
            error(serde_json::json!({
                "destination": { "add": [{ "path": "index.html", "text": "Mine" }] }
            })),
            "plugin `test`'s `destination` hook adds a file at `index.html`, but there's one there"
        );
        assert_eq!(
            error(serde_json::json!({
                "destination": { "add": [{ "path": "index.html/extra.txt", "text": "Mine" }] }
            })),
            "plugin `test`'s `destination` hook adds a file at `index.html/extra.txt`, but \
             there's a file at `index.html`, where its folder would be"
        );
        // Each stops the build before it's written, so the last build stays.
        let home = fs::read_to_string(site.join("dist/index.html")).unwrap();
        assert!(home.contains("Hello, world."), "{home}");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn each_type_of_page_gets_its_template_and_data() {
        // Each template says which it is, and what `page` holds.
        let site = scratch_site(
            "types",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                (
                    "docs/guide/README.md",
                    "---\ndescription: All about it\n---\n# Guide\n",
                ),
                ("docs/guide/install.md", "# Install\n"),
                ("docs/api/reference.md", "# Reference\n"),
                (
                    "theme/_templates/default.html",
                    "{{ page.type }} with default.html: {{ page | list | sort | join(' ') }}",
                ),
                (
                    "theme/_templates/section.html",
                    "{{ page.type }} with section.html: {{ page | list | sort | join(' ') }}",
                ),
                (
                    "theme/_templates/404.html",
                    "{{ page.type }} with 404.html: {{ page | list | sort | join(' ') }} = {{ page.url | safe }}",
                ),
            ],
        );

        build(&site).unwrap();

        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();
        // The first page has no `prev`, and the last no `next`.
        assert_eq!(
            read("index.html"),
            "home with default.html: breadcrumbs children content next title toc type url"
        );
        assert_eq!(
            read("guide/index.html"),
            "section with section.html: breadcrumbs children content description next prev title toc type url"
        );
        assert_eq!(
            read("guide/install/index.html"),
            "page with default.html: breadcrumbs content prev title toc type url"
        );
        // A folder without a README gets no page of its own.
        assert!(!dist.join("api/index.html").exists());
        assert_eq!(
            read("api/reference/index.html"),
            "page with default.html: breadcrumbs content next prev title toc type url"
        );
        // A 404 page has no content of its own.
        assert_eq!(
            read(NOT_FOUND_FILE),
            "404 with 404.html: type url = /404.html"
        );
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn each_page_links_to_the_pages_before_and_after_it() {
        let site = scratch_site(
            "pager",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/guide/install.md", "# Install\n"),
                (
                    "theme/_templates/default.html",
                    "{% if page.prev %}{{ page.prev.title }} {{ page.prev.url | safe }} {% endif %}\
                     < {{ page.title }} >\
                     {% if page.next %} {{ page.next.title }} {{ page.next.url | safe }}{% endif %}",
                ),
            ],
        );

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        assert_eq!(read("index.html"), "< Home > Guide /guide/");
        assert_eq!(
            read("guide/index.html"),
            "Home / < Guide > Install /guide/install"
        );
        assert_eq!(
            read("guide/install/index.html"),
            "Guide /guide/ < Install >"
        );
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn each_page_lists_the_pages_above_it() {
        let site = scratch_site(
            "breadcrumbs",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/guide/install.md", "# Install\n"),
                ("docs/api/reference.md", "# Reference\n"),
                (
                    "docs/api/errors.md",
                    "---\nbreadcrumbs: false\n---\n# Errors\n",
                ),
                (
                    "docs/api/limits.md",
                    "---\nbreadcrumbs:\n  - { title: Reference, url: /api/reference }\n---\n# Limits\n",
                ),
                (
                    "theme/_templates/default.html",
                    "{% if page.breadcrumbs %}{% for crumb in page.breadcrumbs %}\
                     {{ crumb.title }} {{ crumb.url | default('(label)') | safe }} > \
                     {% endfor %}{% endif %}{{ page.title }}",
                ),
            ],
        );

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        assert_eq!(read("index.html"), "Home");
        assert_eq!(read("guide/index.html"), "Home / > Guide");
        assert_eq!(
            read("guide/install/index.html"),
            "Home / > Guide /guide/ > Install"
        );
        assert_eq!(
            read("api/reference/index.html"),
            "Home / > Api (label) > Reference"
        );
        // Frontmatter has the last word: `breadcrumbs: false` turns them off,
        // and a list replaces them.
        assert_eq!(read("api/errors/index.html"), "Errors");
        assert_eq!(
            read("api/limits/index.html"),
            "Reference /api/reference > Limits"
        );

        // They follow a nav set by hand, so a page that isn't in it has none.
        let site_cfg = "theme: ./theme\nnav:\n\
                        - { title: Start, url: / }\n\
                        - { title: Install, url: /guide/install }\n";
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();
        assert_eq!(read("guide/install/index.html"), "Start / > Install");
        assert_eq!(read("guide/index.html"), "Guide");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn the_builtin_theme_shows_breadcrumbs() {
        let site = scratch_site(
            "builtin-breadcrumbs",
            &[
                ("docs/README.md", "# Home\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/guide/install.md", "# Install\n"),
                ("docs/api/reference.md", "# Reference\n"),
                (
                    "docs/api/errors.md",
                    "---\nbreadcrumbs: false\n---\n# Errors\n",
                ),
            ],
        );

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let nav = r#"<nav class="breadcrumbs""#;
        // The trail's items, which the theme writes one to a line.
        let trail = |path: &str| -> Vec<String> {
            read(path)
                .lines()
                .map(str::trim)
                .skip_while(|line| !line.starts_with(nav))
                .take_while(|line| *line != "</nav>")
                .filter(|line| line.starts_with("<li>"))
                .map(str::to_string)
                .collect()
        };
        // It ends with the page itself, and a folder without a README.md is
        // plain text, as in the sidebar.
        assert_eq!(
            trail("guide/install/index.html"),
            [
                r#"<li><a href="/">Home</a></li>"#,
                r#"<li><a href="/guide/">Guide</a></li>"#,
                r#"<li><span aria-current="page">Install</span></li>"#,
            ]
        );
        assert_eq!(
            trail("api/reference/index.html"),
            [
                r#"<li><a href="/">Home</a></li>"#,
                "<li>Api</li>",
                r#"<li><span aria-current="page">Reference</span></li>"#,
            ]
        );
        // The home page has nothing above it, a page can turn them off, and the
        // 404 page has none.
        for path in ["index.html", "api/errors/index.html", NOT_FOUND_FILE] {
            assert!(!read(path).contains(nav), "{path}");
        }
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn the_builtin_theme_folds_the_nav() {
        let site = scratch_site(
            "builtin-folds",
            &[
                ("docs/README.md", "# Home\n"),
                ("docs/api/reference.md", "# Reference\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/guide/install.md", "# Install\n"),
                ("docs/guide/advanced/README.md", "# Advanced\n"),
                ("docs/guide/advanced/tuning.md", "# Tuning\n"),
                ("docs/guide/advanced/deep/limits.md", "# Limits\n"),
                ("docs/guide/recipes/bake.md", "# Bake\n"),
                (
                    "docs/guide/recipes/marks.md",
                    "# Marking the aria-current=\"page\"\n",
                ),
            ],
        );

        build(&site).unwrap();

        // The sidebar's links, labels, and folds, which the theme writes one
        // to a line.
        let sidebar = |path: &str| -> Vec<String> {
            fs::read_to_string(site.join("dist").join(path))
                .unwrap()
                .lines()
                .map(str::trim)
                .skip_while(|line| *line != r#"<ul class="nav">"#)
                .take_while(|line| *line != "</nav>")
                .filter(|line| {
                    ["<a ", "<span", "<details", "<summary"]
                        .iter()
                        .any(|tag| line.starts_with(tag))
                })
                .map(str::to_string)
                .collect()
        };
        // A top-level folder is a group, which doesn't fold; a folder within
        // one folds, from a button beside its link, if it has a README.md, or
        // from its label. Only the folders the current page is in are open.
        assert_eq!(
            sidebar("guide/advanced/deep/limits/index.html"),
            [
                r#"<a href="/">Home</a>"#,
                r#"<span class="nav-label">Api</span>"#,
                r#"<a href="/api/reference">Reference</a>"#,
                r#"<a href="/guide/">Guide</a>"#,
                r#"<a href="/guide/advanced/">Advanced</a>"#,
                "<details open>",
                r#"<summary><span class="visually-hidden">Advanced</span></summary>"#,
                "<details open>",
                r#"<summary class="nav-label">Deep</summary>"#,
                r#"<a href="/guide/advanced/deep/limits" aria-current="page">Limits</a>"#,
                r#"<a href="/guide/advanced/tuning">Tuning</a>"#,
                r#"<a href="/guide/install">Install</a>"#,
                "<details>",
                r#"<summary class="nav-label">Recipes</summary>"#,
                r#"<a href="/guide/recipes/bake">Bake</a>"#,
                r#"<a href="/guide/recipes/marks">Marking the aria-current=&quot;page&quot;</a>"#,
            ]
        );
        // Whether each folder within a group is open, top down.
        let folds = |path: &str| -> Vec<String> {
            sidebar(path)
                .into_iter()
                .filter(|line| line.starts_with("<details"))
                .collect()
        };
        // A folder's own page opens it, but not the folders in it.
        assert_eq!(
            folds("guide/advanced/index.html"),
            ["<details open>", "<details>", "<details>"]
        );
        assert_eq!(
            folds("guide/recipes/bake/index.html"),
            ["<details>", "<details>", "<details open>"]
        );
        // Elsewhere, each is closed, even one with a title like the current
        // page's mark.
        for path in ["index.html", "guide/index.html", NOT_FOUND_FILE] {
            assert_eq!(
                folds(path),
                ["<details>", "<details>", "<details>"],
                "{path}"
            );
        }
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn each_page_links_to_its_file_in_the_repo() {
        let template = "{{ site.repo | safe }} \
                        | {% if page.source_url %}{{ page.source_url | safe }}{% endif %} \
                        | {% if page.edit_url %}{{ page.edit_url | safe }}{% endif %}";
        let repo = "https://github.com/withlittle/little";
        let site_cfg =
            format!("theme: ./theme\nurl: https://withlittle.github.io/little\nrepo: {repo}\n");
        let root = scratch_site(
            "repo",
            &[
                (".git/HEAD", "ref: refs/heads/main\n"),
                ("site/little.yml", &site_cfg),
                ("site/docs/README.md", "# Home\n"),
                ("site/docs/guide/install.md", "# Install\n"),
                (
                    "site/docs/guide/reference.md",
                    "---\nsource_url: /api/\nedit_url: false\n---\n# Reference\n",
                ),
                ("site/theme/_templates/default.html", template),
                ("site/theme/_templates/404.html", template),
            ],
        );
        let site = root.join("site");
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let links = |path: &str| format!("{repo} | {repo}/blob/{path} | {repo}/edit/{path}");

        // A page's file is found from the root of the repository: the
        // nearest folder above it with a `.git`.
        build(&site).unwrap();
        assert_eq!(
            read("guide/install/index.html"),
            links("main/site/docs/guide/install.md")
        );
        // A page's frontmatter has the last word, and a link from the root of
        // the site gets its path. The 404 page isn't built from a file.
        assert_eq!(
            read("guide/reference/index.html"),
            format!("{repo} | /little/api/ | ")
        );
        assert_eq!(read(NOT_FOUND_FILE), format!("{repo} |  | "));

        // In a worktree, `.git` is a file. A site can link to another branch.
        fs::remove_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git"), "gitdir: /elsewhere\n").unwrap();
        let branch_cfg = format!("{site_cfg}branch: release/2.0\n");
        fs::write(site.join("little.yml"), branch_cfg).unwrap();
        build(&site).unwrap();
        assert_eq!(read("index.html"), links("release/2.0/site/docs/README.md"));

        // Without a `.git`, as in a copy of the repository, the site's folder
        // is the root of it, unless the tests run inside another repository.
        fs::remove_file(root.join(".git")).unwrap();
        fs::write(site.join("little.yml"), &site_cfg).unwrap();
        if !root.ancestors().any(|dir| dir.join(".git").exists()) {
            build(&site).unwrap();
            assert_eq!(read("index.html"), links("main/docs/README.md"));
        }

        // little knows only GitHub's and GitLab's addresses for files, so a
        // repository anywhere else is all a theme can link to.
        let codeberg = "https://codeberg.org/withlittle/little";
        fs::write(site.join("little.yml"), site_cfg.replace(repo, codeberg)).unwrap();
        build(&site).unwrap();
        assert_eq!(read("index.html"), format!("{codeberg} |  | "));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_builtin_theme_links_to_the_repo_and_each_pages_file() {
        let repo = "https://github.com/withlittle/little";
        let site = scratch_site(
            "builtin-repo",
            &[
                (".git/HEAD", "ref: refs/heads/main\n"),
                ("little.yml", &format!("repo: {repo}\n")),
                ("docs/README.md", "# Home\n"),
                ("docs/guide.md", "---\nedit_url: false\n---\n# Guide\n"),
            ],
        );

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        // The repo link is an icon, so it's named by its label.
        let repo_link = format!(r#"repo-link" href="{repo}" aria-label="Repository""#);
        let home = read("index.html");
        for link in [
            repo_link.clone(),
            format!(r#"<a href="{repo}/edit/main/docs/README.md">"#),
            "Edit this page".to_string(),
            format!(r#"<a href="{repo}/blob/main/docs/README.md">"#),
            "View source of this page".to_string(),
        ] {
            assert!(home.contains(&link), "{link} in {home}");
        }
        // A page can turn either link off, and the 404 page has neither.
        let guide = read("guide/index.html");
        assert!(!guide.contains("Edit this page"), "{guide}");
        assert!(guide.contains("View source of this page"), "{guide}");
        let not_found = read(NOT_FOUND_FILE);
        assert!(not_found.contains(&repo_link), "{not_found}");
        assert!(
            !not_found.contains("View source of this page"),
            "{not_found}"
        );

        // Without a repo, there's nothing to link to.
        fs::write(site.join("little.yml"), "").unwrap();
        build(&site).unwrap();
        let home = read("index.html");
        assert!(!home.contains("repo-link"), "{home}");
        assert!(!home.contains("View source of this page"), "{home}");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_page_toc_is_shaped_like_the_site_nav() {
        // So one macro can list both.
        let site = scratch_site(
            "page-toc",
            &[
                ("little.yml", "theme: ./theme\n"),
                (
                    "docs/README.md",
                    "# Home\n\n## Setup\n\n### Requirements\n\n## Usage\n",
                ),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/guide/install.md", "# Install\n"),
                (
                    "theme/_templates/default.html",
                    "{% macro list(items) %}{% for item in items %}\
                     [{{ item.title }} {{ item.url | safe }}{% if item.children %} {{ list(item.children) }}{% endif %}]\
                     {% endfor %}{% endmacro %}\
                     {{ list(site.nav) }} / {{ list(page.toc) }}",
                ),
            ],
        );

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let nav = "[Home /][Guide /guide/ [Install /guide/install]]";
        assert_eq!(
            read("index.html"),
            format!("{nav} / [Setup #setup [Requirements #requirements]][Usage #usage]")
        );
        // A page without headings has an empty `page.toc`.
        assert_eq!(read("guide/install/index.html"), format!("{nav} / "));
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_page_can_turn_off_its_toc() {
        // `toc: false` replaces the table of contents, and the built-in theme
        // lists one only `{% if page.toc %}`.
        let site = scratch_site(
            "no-toc",
            &[
                ("docs/README.md", "# Home\n\n## Setup\n"),
                (
                    "docs/guide.md",
                    "---\ntoc: false\n---\n# Guide\n\n## Setup\n",
                ),
            ],
        );

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let toc = r#"<nav class="toc""#;
        assert!(read("index.html").contains(toc));
        assert!(!read("guide/index.html").contains(toc));
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_page_toc_lists_the_headings_its_config_chooses() {
        // little lists every heading but the title, the built-in theme's
        // `little.yml` only `##` and `###`, and a site can choose otherwise.
        let page = "# Home\n\n## Setup\n\n### Install\n\n#### From Source\n";
        for (label, site_cfg, listed) in [
            ("toc-theme", "", &["#setup", "#install"][..]),
            ("toc-max-2", "toc_max_depth: 2\n", &["#setup"]),
            (
                "toc-max-4",
                "toc_max_depth: 4\n",
                &["#setup", "#install", "#from-source"],
            ),
            (
                "toc-min-1",
                "toc_min_depth: 1\n",
                &["#home", "#setup", "#install"],
            ),
            (
                "toc-min-3",
                "toc_min_depth: 3\ntoc_max_depth: 4\n",
                &["#install", "#from-source"],
            ),
        ] {
            let site = scratch_site(label, &[("little.yml", site_cfg), ("docs/README.md", page)]);

            build(&site).unwrap();

            // The headings in the content link to themselves, so look only
            // at the table of contents.
            let html = fs::read_to_string(site.join("dist/index.html")).unwrap();
            let (_, toc) = html.split_once(r#"<nav class="toc""#).unwrap();
            let (toc, _) = toc.split_once("</nav>").unwrap();
            let links: Vec<&str> = ["#home", "#setup", "#install", "#from-source"]
                .into_iter()
                .filter(|url| toc.contains(&format!(r#"href="{url}""#)))
                .collect();
            assert_eq!(links, listed, "{label}");
            let _ = fs::remove_dir_all(&site);
        }
    }

    #[test]
    fn every_frontmatter_key_reaches_the_page() {
        let site = scratch_site(
            "frontmatter",
            &[
                ("little.yml", "theme: ./theme\n"),
                (
                    "docs/README.md",
                    "---\nauthor: Ada\ntags: [setup, cli]\nhero:\n  image: hero.png\ntitle:\ndraft:\n---\n# Home\n",
                ),
                (
                    "theme/_templates/default.html",
                    r#"{{ page.author }}, {{ page.tags | join("+") }}, {{ page.hero.image }}, {{ page.title }}, {{ page.draft | default("not a draft") }}"#,
                ),
            ],
        );

        build(&site).unwrap();

        // A key left blank is ignored, so `title:` leaves the title little
        // finds.
        let home = fs::read_to_string(site.join("dist/index.html")).unwrap();
        assert_eq!(home, "Ada, setup+cli, hero.png, Home, not a draft");

        // Frontmatter that isn't settings stops the build, naming the page.
        let readme = site.join("docs").join("README.md");
        fs::write(&readme, "---\n- author: Ada\n---\n# Home\n").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!("rendering {}: parsing frontmatter: ", readme.display());
        assert!(message.starts_with(&expected), "{message}");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn frontmatter_has_the_last_word_on_the_page() {
        let site = scratch_site(
            "last-word",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                (
                    "docs/guide.md",
                    "---\ntype: tutorial\nurl: /elsewhere\nprev:\n  title: Start here\nnext: false\n---\n# Guide\n",
                ),
                ("docs/reference.md", "# Reference\n"),
                (
                    "theme/_templates/default.html",
                    "{{ page.type }} {{ page.url | safe }}: \
                     {% if page.prev %}{{ page.prev.title }} {{ page.prev.url | safe }} {% endif %}\
                     < {{ page.title }} >\
                     {% if page.next %} {{ page.next.title }} {{ page.next.url | safe }}{% endif %}",
                ),
                ("theme/_templates/tutorial.html", "tutorial.html"),
            ],
        );

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        // Settings merge key by key, so `prev` keeps its URL; anything else,
        // like `next: false`, replaces what little works out.
        assert_eq!(
            read("guide/index.html"),
            "tutorial /elsewhere: Start here / < Guide >"
        );
        // Only the theme sees it: the page is built at its own URL, with the
        // template for its own type, and the pages beside it link to it there.
        assert!(!site.join("dist/elsewhere").exists());
        assert_eq!(read("index.html"), "home /: < Home > Guide /guide");
        assert_eq!(
            read("reference/index.html"),
            "page /reference: Guide /guide < Reference >"
        );
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_nav_in_the_config_replaces_littles() {
        let site_cfg = "theme: ./theme\nnav:\n\
                        - { title: Start, url: /guide/install }\n\
                        - { title: Home, url: /, icon: house }\n\
                        - { title: GitHub, url: \"https://github.com/withlittle/little\" }\n";
        let site = scratch_site(
            "own-nav",
            &[
                ("little.yml", site_cfg),
                ("docs/README.md", "# Home\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/guide/install.md", "# Install\n"),
                (
                    "theme/_templates/default.html",
                    "{% for item in site.nav %}{{ item.title }}{% if item.icon %} ({{ item.icon }}){% endif %}; {% endfor %}\
                     {% if page.prev %}{{ page.prev.url | safe }} {% endif %}\
                     < {{ page.title }} >\
                     {% if page.next %} {{ page.next.url | safe }}{% endif %}",
                ),
            ],
        );

        build(&site).unwrap();

        // The theme gets the nav as the config sets it, and `prev` and `next`
        // follow it. A page that isn't in it has neither.
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let nav = "Start; Home (house); GitHub; ";
        assert_eq!(
            read("guide/install/index.html"),
            format!("{nav}< Install > /")
        );
        assert_eq!(
            read("index.html"),
            format!("{nav}/guide/install < Home > https://github.com/withlittle/little")
        );
        assert_eq!(read("guide/index.html"), format!("{nav}< Guide >"));

        // A nav that isn't a list of links stops the build.
        for nav in ["nav: { style: tabs }\n", "nav:\n  - url: /\n"] {
            let site_cfg = format!("theme: ./theme\n{nav}");
            fs::write(site.join("little.yml"), site_cfg).unwrap();
            let message = format!("{:#}", build(&site).unwrap_err());
            let expected = "`nav` in little.yml must be a list of links";
            assert!(message.starts_with(expected), "{message}");
        }
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn every_page_gets_the_year_of_the_build() {
        let template = "&copy; {{ site.year }} {{ site.name }}";
        let site = scratch_site(
            "year",
            &[
                ("little.yml", "theme: ./theme\nname: Little\n"),
                ("docs/README.md", "# Home\n"),
                ("docs/guide.md", "# Guide\n"),
                ("theme/_templates/default.html", template),
                ("theme/_templates/404.html", template),
            ],
        );
        let year = time_of_build(env::var_os("SOURCE_DATE_EPOCH"))
            .unwrap()
            .year();

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        for path in ["index.html", "guide/index.html", "404.html"] {
            assert_eq!(read(path), format!("&copy; {year} Little"), "{path}");
        }

        // It's beneath the config, like the nav, so a `year` there replaces it.
        let site_cfg = "theme: ./theme\nname: Little\nyear: 2019–2026\n";
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();
        assert_eq!(read("index.html"), "&copy; 2019–2026 Little");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn source_date_epoch_chooses_the_time_of_the_build() {
        let time = |epoch: &str| time_of_build(Some(epoch.into()));
        // The last second of 2025, and the first of 2026, in UTC.
        assert_eq!(time("1767225599").unwrap().year(), 2025);
        let new_year = OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap();
        assert_eq!(time("1767225600").unwrap(), new_year);

        // Anything but a whole number of seconds stops the build, as does one
        // too far from 1970 to be a date.
        for epoch in [
            "",
            "soon",
            "1767225600.5",
            "99999999999999999999",
            "999999999999",
        ] {
            let message = format!("{:#}", time(epoch).unwrap_err());
            let expected = format!(
                "SOURCE_DATE_EPOCH must be a whole number of seconds since 1970, \
                 as `date +%s` prints it, not {epoch:?}"
            );
            assert_eq!(message, expected);
        }
    }

    #[test]
    fn a_url_puts_every_link_under_the_sites_path() {
        let site_cfg = "theme: ./theme\nurl: https://withlittle.github.io/little\n";
        let site = scratch_site(
            "site-url",
            &[
                ("little.yml", site_cfg),
                (
                    "docs/README.md",
                    "# Home\n\nRead the [guide](/guide/), or see ![the logo](/logo.png).\n",
                ),
                ("docs/guide/README.md", "# Guide\n\n## Steps\n"),
                (
                    "docs/guide/install.md",
                    "---\nprev: { url: /start/ }\nlogo: /logo.png\n---\n# Install\n",
                ),
                ("theme/_templates/home.html", "{{ page.content | safe }}"),
                (
                    "theme/_templates/default.html",
                    "{% for item in site.nav recursive %}{{ item.url | safe }} \
                     {% if item.children %}{{ loop(item.children) }}{% endif %}{% endfor %}\
                     | {{ page.url | safe }} \
                     | {% if page.prev %}{{ page.prev.url | safe }}{% endif %} \
                     {% if page.next %}{{ page.next.url | safe }}{% endif %} \
                     | {% for crumb in page.breadcrumbs %}{{ crumb.url | safe }} {% endfor %}\
                     | {% for item in page.children %}{{ item.url | safe }} {% endfor %}\
                     | {% for item in page.toc %}{{ item.url | safe }} {% endfor %}\
                     | {{ page.logo | safe }}",
                ),
                (
                    "theme/_templates/404.html",
                    "{{ page.url | safe }} {{ '/' | url }} {{ site.url | safe }}",
                ),
            ],
        );

        build(&site).unwrap();

        // Pages are built where they'd be without a url: it's the host that
        // puts them under the path.
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        assert!(!site.join("dist/little").exists());

        // Every URL little works out gets the path, but a heading's
        // `#fragment`.
        let nav = "/little/ /little/guide/ /little/guide/install ";
        assert_eq!(
            read("guide/index.html"),
            format!(
                "{nav}| /little/guide/ | /little/ /little/guide/install | /little/ \
                 | /little/guide/install | #steps | "
            )
        );
        // So does one the author writes where little would, like `prev`, but
        // not a setting of the theme's own, like `logo`.
        assert_eq!(
            read("guide/install/index.html"),
            format!(
                "{nav}| /little/guide/install | /little/start/  | /little/ /little/guide/ \
                 | | | /logo.png"
            )
        );
        // As do links and images in Markdown.
        let home = read("index.html");
        assert!(
            home.contains(r#"<a href="/little/guide/">guide</a>"#),
            "{home}"
        );
        assert!(home.contains(r#"<img src="/little/logo.png""#), "{home}");
        // And the 404 page's own URL. `site.url` is as the config sets it.
        assert_eq!(
            read(NOT_FOUND_FILE),
            "/little/404.html /little/ https://withlittle.github.io/little"
        );

        // A nav in the config gets it too, but for links to other sites.
        let site_cfg = format!(
            "{site_cfg}nav:\n\
             - {{ title: Start, url: /guide/install }}\n\
             - {{ title: GitHub, url: \"https://github.com/withlittle/little\" }}\n"
        );
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();
        let github = "https://github.com/withlittle/little";
        assert_eq!(
            read("guide/install/index.html"),
            format!(
                "/little/guide/install {github} | /little/guide/install \
                 | /little/start/ {github} | | | | /logo.png"
            )
        );

        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_failed_build_leaves_the_last_one() {
        let site = scratch_site(
            "failed",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                ("theme/_templates/default.html", "{{ page.title }}"),
            ],
        );
        build(&site).unwrap();
        let home = site.join("dist/index.html");
        assert_eq!(fs::read_to_string(&home).unwrap(), "Home");

        // A template that fails on a page...
        let template = site.join("theme/_templates/default.html");
        fs::write(&template, "{{ page.missing.title }}").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        assert!(message.contains("README.md"), "{message}");
        assert!(message.contains("default.html"), "{message}");
        assert_eq!(fs::read_to_string(&home).unwrap(), "Home");

        // ...or a theme without a default.html.
        fs::remove_file(&template).unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        assert!(
            message.contains("has no _templates/default.html"),
            "{message}"
        );
        assert_eq!(fs::read_to_string(&home).unwrap(), "Home");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn static_files_cannot_take_the_place_of_pages() {
        let site = scratch_site(
            "clash",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                ("docs/guide/install.md", "# Install\n"),
                ("theme/_templates/default.html", "{{ page.title }}"),
                ("theme/_templates/404.html", "Not found"),
                ("theme/_static/guide/style.css", "body {}\n"),
            ],
        );
        // Beside the pages is fine...
        build(&site).unwrap();
        assert!(site.join("dist/guide/style.css").is_file());

        // ...but not on one, or where one's folder goes, whether it's the
        // theme's or one in the source.
        let built_from = |file: &str| {
            let source = site.join("docs").join(native(file));
            format!("the page built from {}", source.display())
        };
        for folder in [site.join("theme").join("_static"), site.join("docs")] {
            for (file, page) in [
                ("index.html", built_from("README.md")),
                ("guide/install", built_from("guide/install.md")),
                ("guide/install/index.html", built_from("guide/install.md")),
                ("404.html", "the 404 page".to_string()),
            ] {
                let path = folder.join(native(file));
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, "static").unwrap();
                let message = format!("{:#}", build(&site).unwrap_err());
                let expected = format!("{} clashes with {page}", path.display());
                assert_eq!(message, expected);
                fs::remove_file(&path).unwrap();
            }
        }
        let home = fs::read_to_string(site.join("dist/index.html")).unwrap();
        assert_eq!(home, "Home");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn pages_cannot_take_each_others_place() {
        let site = scratch_site(
            "same-page",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("theme/_templates/default.html", "{{ page.title }}"),
            ],
        );
        build(&site).unwrap();

        // `guide.md` would be built into `guide/index.html` too, so the build
        // stops, and leaves the last one in place.
        fs::write(site.join("docs/guide.md"), "# Also the guide\n").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let docs = site.join("docs");
        let expected = format!(
            "{} and {} would both be the page at /guide/: rename or move one",
            docs.join("guide.md").display(),
            docs.join("guide").join("README.md").display(),
        );
        assert_eq!(message, expected);
        let guide = fs::read_to_string(site.join("dist/guide/index.html")).unwrap();
        assert_eq!(guide, "Guide");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn each_page_is_indexed_for_search() {
        let site = scratch_site(
            "search",
            &[
                (
                    "little.yml",
                    "theme: ./theme\nurl: https://example.com/docs\n",
                ),
                ("docs/README.md", "# Home\n\nWelcome aboard.\n"),
                (
                    "docs/guide/install.md",
                    "---\ntitle: Installing\n---\n# Install\n\n## From source\n\nRun cargo.\n",
                ),
                ("theme/little.yml", "search: true\n"),
                (
                    "theme/_templates/default.html",
                    "<html lang=\"fr\"><header>Theme header</header>{{ page.content }}</html>",
                ),
                ("theme/_templates/404.html", "<html lang=\"fr\">Lost</html>"),
            ],
        );
        build(&site).unwrap();

        // Each page but the 404 page, in the language the theme gives it...
        let dist = site.join("dist");
        let entry = fs::read_to_string(dist.join("pagefind/pagefind-entry.json")).unwrap();
        assert!(entry.contains(r#""wasm":"fr","page_count":2"#), "{entry}");
        assert!(dist.join("pagefind/pagefind.js").is_file());

        // ...at its URL from the root of the site, without the site's path,
        // which Pagefind puts back from where it finds the index, under its
        // title, with its headings, and with only its own content, not the
        // theme's.
        let pages = indexed(&dist);
        let expected = [
            [
                r#""url":"/""#,
                r#""title":"Home""#,
                r#""content":"Home. Welcome aboard.""#,
            ],
            [
                r#""url":"/guide/install""#,
                r#""title":"Installing""#,
                r#""id":"from-source","text":"From source""#,
            ],
        ];
        assert_eq!(pages.len(), expected.len());
        for (page, expected) in pages.iter().zip(expected) {
            for part in expected {
                assert!(page.contains(part), "{part} in {page}");
            }
            assert!(!page.contains("Theme header"), "{page}");
        }

        // A site can go without.
        let site_cfg = "theme: ./theme\nsearch: false\n";
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();
        assert!(dist.join("index.html").is_file());
        assert!(!dist.join("pagefind").exists());
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn nothing_can_take_the_place_of_search_files() {
        let site = scratch_site(
            "search-clash",
            &[
                ("little.yml", "theme: ./theme\n"),
                ("docs/README.md", "# Home\n"),
                ("theme/little.yml", "search: true\n"),
                ("theme/_templates/default.html", "{{ page.title }}"),
            ],
        );
        // A page at /pagefind, or a file beside search's files, is fine...
        fs::write(site.join("docs/pagefind.md"), "# Pagefind\n").unwrap();
        fs::create_dir_all(site.join("docs/pagefind")).unwrap();
        fs::write(site.join("docs/pagefind/notes.txt"), "notes").unwrap();
        build(&site).unwrap();
        let dist = site.join("dist");
        assert_eq!(
            fs::read_to_string(dist.join("pagefind/index.html")).unwrap(),
            "Pagefind"
        );
        assert!(dist.join("pagefind/notes.txt").is_file());
        assert!(dist.join("pagefind/pagefind.js").is_file());

        // ...but not one on one of them, whether it's the theme's or one in
        // the source, or a page...
        let docs = site.join("docs");
        let theme_file = site.join(native("theme/_static/pagefind/pagefind.js"));
        let source_file = docs.join(native("pagefind/pagefind.js"));
        let page = docs.join(native("pagefind/pagefind.js.md"));
        for (file, name) in [
            (&theme_file, theme_file.display().to_string()),
            (&source_file, source_file.display().to_string()),
            (&page, format!("the page built from {}", page.display())),
        ] {
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, "# Mine\n").unwrap();
            let message = format!("{:#}", build(&site).unwrap_err());
            let expected = format!(
                "{name} clashes with /pagefind/pagefind.js, which little writes for search: \
                 rename or move it"
            );
            assert_eq!(message, expected);
            fs::remove_file(file).unwrap();
        }

        // ...or one where their folder goes.
        fs::remove_file(docs.join("pagefind.md")).unwrap();
        fs::remove_dir_all(docs.join("pagefind")).unwrap();
        fs::write(docs.join("pagefind"), "a file, not a folder").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "{} clashes with /pagefind/, where little writes files for search: rename or move it",
            docs.join("pagefind").display()
        );
        assert_eq!(message, expected);

        // The last build is left in place.
        assert!(dist.join("pagefind/pagefind.js").is_file());
        let _ = fs::remove_dir_all(&site);
    }

    /// A site in a git repository whose files were `old`, at the tag `v1.4`,
    /// and are now `new`, uncommitted. Its destination is ignored.
    fn versioned_site(label: &str, old: &[(&str, &str)], new: &[(&str, &str)]) -> PathBuf {
        let site = scratch_site(label, old);
        fs::write(site.join(".gitignore"), "dist/\n").unwrap();
        git::tests::init(&site);
        git::tests::commit_all(&site);
        git::tests::git_in(&site, &["tag", "v1.4"]);
        for (file, _) in old {
            fs::remove_file(site.join(file)).unwrap();
        }
        for (file, contents) in new {
            let path = site.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
        }
        site
    }

    #[test]
    fn each_version_is_built_from_its_git_ref() {
        use git::tests::{commit_all, git_in};
        let template = "{{ site.name }} {{ site.version }} \
                        | {% for v in site.versions %}{{ v.title }}{{ '*' if v.root }} {{ v.url | safe }} {% endfor %}\
                        | {% for v in page.versions %}{{ v.title }}{{ '*' if v.root }} {{ v.url | safe }} {% endfor %}\
                        | {{ page.source_url | safe if page.source_url }} \
                        | {{ page.edit_url | safe if page.edit_url }}\n{{ page.content | safe }}";
        let site_cfg = |versions: &str| {
            format!(
                "theme: ./theme\nname: New\nurl: https://example.com/proj\n\
                 repo: https://github.com/o/r\nversions:\n{versions}"
            )
        };
        let site = versioned_site(
            "versions",
            &[
                // Of its own settings, only its `source` and `nav` count.
                ("little.yml", "name: Old\nurl: https://example.com/old\n"),
                ("docs/README.md", "# Home\n\nSee [the old page](old.md).\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/old.md", "# Old\n"),
            ],
            &[
                (
                    "little.yml",
                    &site_cfg(
                        "  - { title: \"2.0\", url: / }\n  \
                         - { title: \"1.4\", url: /1.4, ref: v1.4 }\n",
                    ),
                ),
                ("docs/README.md", "# Home\n"),
                ("docs/guide.md", "# Guide\n"),
                ("docs/new.md", "# New\n"),
                ("theme/_templates/default.html", template),
                (
                    "theme/_templates/404.html",
                    "{% for v in page.versions %}{{ v.url | safe }} {% endfor %}",
                ),
                ("theme/_static/style.css", "body {}\n"),
            ],
        );
        let built = build(&site).unwrap();
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();
        let head = |path: &str| read(path).lines().next().unwrap().to_string();
        let file =
            |commit: &str, path: &str| format!("https://github.com/o/r/blob/{commit}/docs/{path}");
        let edit = |path: &str| format!("https://github.com/o/r/edit/main/docs/{path}");
        let v1_4 = git_in(&site, &["rev-parse", "v1.4^{commit}"]);

        // The version at `/` is at the root, here from the site's files as
        // they are, and each page links to itself in each version, or, if
        // it's not there, to its home.
        let all = "2.0* /proj/ 1.4 /proj/1.4/ ";
        assert_eq!(
            head("guide/index.html"),
            format!(
                "New 2.0 | {all}| 2.0* /proj/guide 1.4 /proj/1.4/guide/ | {} | {}",
                file("main", "guide.md"),
                edit("guide.md")
            )
        );
        assert_eq!(
            head("new/index.html"),
            format!(
                "New 2.0 | {all}| 2.0* /proj/new 1.4 /proj/1.4/ | {} | {}",
                file("main", "new.md"),
                edit("new.md")
            )
        );
        // Another is in the folder at its `url`, as it was at its ref, but
        // with the site's settings. Its pages link to their files at the
        // ref's commit, which can't be edited.
        assert_eq!(
            head("1.4/guide/index.html"),
            format!(
                "New 1.4 | {all}| 2.0* /proj/guide 1.4 /proj/1.4/guide/ | {} | ",
                file(&v1_4, "guide/README.md")
            )
        );
        assert_eq!(
            head("1.4/old/index.html"),
            format!(
                "New 1.4 | {all}| 2.0* /proj/ 1.4 /proj/1.4/old | {} | ",
                file(&v1_4, "old.md")
            )
        );
        // Its links stay in it.
        let old_home = read("1.4/index.html");
        assert!(
            old_home.contains(r#"<a href="/proj/1.4/old">the old page</a>"#),
            "{old_home}"
        );
        // Only the version at `/` has the theme's files, which the others
        // share, and a 404 page, which links to each version's home.
        assert!(dist.join("style.css").is_file());
        assert!(!dist.join("1.4/style.css").exists());
        assert_eq!(read(NOT_FOUND_FILE), "/proj/ /proj/1.4/ ");
        assert!(!dist.join("1.4").join(NOT_FOUND_FILE).exists());

        // A file from a ref is named as git names it, and a page's URL is
        // from the root of the site.
        let old = Path::new("1.4/old/index.html");
        assert_eq!(built.origins[old], "v1.4:docs/old.md");
        assert_eq!(built.urls[old], "/1.4/old");
        let from_refs = [
            "1.4/index.html",
            "1.4/guide/index.html",
            "1.4/old/index.html",
        ];
        let from_refs: HashSet<_> = from_refs.into_iter().map(PathBuf::from).collect();
        assert_eq!(built.from_refs, from_refs);

        // The version at `/` can be from a ref, too, and listed anywhere, and
        // the site's files, as they are, in a folder, like any other, as deep
        // as its `url` says.
        commit_all(&site);
        git_in(&site, &["tag", "v2.0"]);
        let v2_0 = git_in(&site, &["rev-parse", "v2.0^{commit}"]);
        let versions = "  - { title: dev, url: /dev/ }\n  \
                        - { title: \"2.0\", url: /, ref: v2.0 }\n  \
                        - { title: \"1.4\", url: /versions/1.4, ref: v1.4 }\n";
        fs::write(site.join("little.yml"), site_cfg(versions)).unwrap();
        fs::write(site.join("docs/new.md"), "# Newer\n").unwrap();
        let built = build(&site).unwrap();
        let all = "dev /proj/dev/ 2.0* /proj/ 1.4 /proj/versions/1.4/ ";
        let new = "dev /proj/dev/new 2.0* /proj/new 1.4 /proj/versions/1.4/ ";
        assert_eq!(
            head("new/index.html"),
            format!("New 2.0 | {all}| {new}| {} | ", file(&v2_0, "new.md"))
        );
        assert!(read("new/index.html").contains("<h1 id=\"new\">New<"));
        assert_eq!(
            head("dev/new/index.html"),
            format!(
                "New dev | {all}| {new}| {} | {}",
                file("main", "new.md"),
                edit("new.md")
            )
        );
        assert!(read("dev/new/index.html").contains("<h1 id=\"newer\">Newer<"));
        assert_eq!(
            head("versions/1.4/old/index.html"),
            format!(
                "New 1.4 | {all}| dev /proj/dev/ 2.0* /proj/ 1.4 /proj/versions/1.4/old | {} | ",
                file(&v1_4, "old.md")
            )
        );
        assert!(
            read("versions/1.4/index.html")
                .contains(r#"<a href="/proj/versions/1.4/old">the old page</a>"#)
        );
        assert_eq!(
            built.urls[Path::new("versions/1.4/old/index.html")],
            "/versions/1.4/old"
        );
        assert_eq!(
            read(NOT_FOUND_FILE),
            "/proj/dev/ /proj/ /proj/versions/1.4/ "
        );
        assert!(!dist.join("dev").join(NOT_FOUND_FILE).exists());
        assert!(!dist.join("1.4").exists());
        assert!(built.from_refs.contains(Path::new("new/index.html")));
        assert!(!built.from_refs.contains(Path::new("dev/new/index.html")));
        assert!((built.from_refs).contains(Path::new("versions/1.4/old/index.html")));
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_version_from_a_ref_has_its_own_pages_and_nav_in_the_sites_config() {
        use git::tests::{commit_all, git_in};
        let site_cfg = |version: &str| {
            format!(
                "theme: ./theme\nname: New\nsource: ./content\nurl: https://example.com/proj\n\
                 versions:\n  - {{ title: \"2.0\", url: / }}\n  \
                 - {{ title: \"1.4\", url: /1.4, {version} }}\n"
            )
        };
        // At the ref, the site wasn't built with little, so has no
        // little.yml.
        let site = versioned_site(
            "version-pages",
            &[
                (
                    "content/README.md",
                    "# Home\n\nSee [its styles](/style.css).\n",
                ),
                ("content/guide.md", "# Guide\n"),
                ("documentation/README.md", "# Documentation\n"),
                ("documentation/style.css", "old {}\n"),
            ],
            &[
                ("little.yml", &site_cfg("ref: v1.4")),
                ("content/README.md", "# Home\n"),
                (
                    "theme/_templates/default.html",
                    "{{ site.name }} {{ '/style.css' | url }} \
                     | {% for item in site.nav %}{{ item.title }} {{ item.url | safe }}; {% endfor %}\n\
                     {{ page.content | safe }}",
                ),
                ("theme/_static/style.css", "body {}\n"),
            ],
        );
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();
        let head = |path: &str| read(path).lines().next().unwrap().to_string();

        // So its pages are in the site's `source`, and its nav is built
        // from their folders. Only the version at `/` has the theme's files,
        // which every version's pages, and Markdown, link to there.
        build(&site).unwrap();
        assert_eq!(head("index.html"), "New /proj/style.css | Home /proj/; ");
        assert_eq!(
            head("1.4/index.html"),
            "New /proj/style.css | Home /proj/1.4/; Guide /proj/1.4/guide; "
        );
        let old_home = read("1.4/index.html");
        assert!(
            old_home.contains(r#"<a href="/proj/style.css">its styles</a>"#),
            "{old_home}"
        );
        assert_eq!(read("style.css"), "body {}\n");
        assert!(!dist.join("1.4/style.css").exists());

        // Its `source` can name another folder, and a file of its own at the
        // path of one of the theme's is its own, as the site's is.
        let moved = site_cfg("ref: v1.4, source: ./documentation");
        fs::write(site.join("little.yml"), moved).unwrap();
        build(&site).unwrap();
        assert_eq!(
            head("1.4/index.html"),
            "New /proj/1.4/style.css | Documentation /proj/1.4/; "
        );
        assert_eq!(read("1.4/style.css"), "old {}\n");
        assert_eq!(read("style.css"), "body {}\n");

        // Of a little.yml at the ref, its `source` and `nav` count, but no
        // other setting.
        let own = "name: Old\nsource: ./documentation\nnav:\n  - { title: Docs, url: / }\n";
        fs::write(site.join("little.yml"), own).unwrap();
        fs::create_dir_all(site.join("documentation")).unwrap();
        fs::write(site.join("documentation/README.md"), "# Documentation\n").unwrap();
        commit_all(&site);
        git_in(&site, &["tag", "v1.5"]);
        fs::write(site.join("little.yml"), site_cfg("ref: v1.5")).unwrap();
        build(&site).unwrap();
        assert_eq!(
            head("1.4/index.html"),
            "New /proj/style.css | Docs /proj/1.4/; "
        );
        // But its entry's `nav` wins, with each URL from its home.
        let own = "ref: v1.5, nav: [{ title: Start, url: / }]";
        fs::write(site.join("little.yml"), site_cfg(own)).unwrap();
        build(&site).unwrap();
        assert_eq!(
            head("1.4/index.html"),
            "New /proj/style.css | Start /proj/1.4/; "
        );
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_site_whose_versions_have_no_ref_needs_no_repository() {
        let site = scratch_site(
            "versions-no-ref",
            &[
                // little says which version is at `/`, whatever an entry
                // says.
                (
                    "little.yml",
                    "theme: ./theme\nversions:\n  - { title: Latest, url: /, root: false }\n",
                ),
                ("docs/README.md", "# Home\n"),
                (
                    "theme/_templates/default.html",
                    "{{ site.version }} | {{ site.versions | map(attribute='url') | join(' ') | safe }} \
                     | {{ page.versions | map(attribute='url') | join(' ') | safe }} \
                     | {{ page.versions | map(attribute='root') | join(' ') }}",
                ),
            ],
        );
        build(&site).unwrap();
        let home = fs::read_to_string(site.join("dist/index.html")).unwrap();
        // MiniJinja writes a boolean as Jinja2 does.
        assert_eq!(home, "Latest | / | / | True");
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_version_cant_be_where_a_page_or_search_is() {
        let site_cfg = |url: &str| {
            format!(
                "theme: ./theme\nversions:\n  - {{ title: \"2.0\", url: / }}\n  \
                 - {{ title: \"1.4\", url: {url}, ref: v1.4 }}\n"
            )
        };
        let site = versioned_site(
            "version-clash",
            &[("docs/README.md", "# Old\n")],
            &[
                ("little.yml", &site_cfg("/1.4")),
                ("docs/README.md", "# Home\n"),
                ("theme/_templates/default.html", "{{ page.title }}"),
            ],
        );
        // A page of the version at `/`, or a file, where another version
        // goes, stops the build...
        let docs = site.join("docs");
        let clashes = |url: &str, files: &[(&str, &str)]| {
            fs::write(site.join("little.yml"), site_cfg(url)).unwrap();
            for (file, name) in files {
                let path = docs.join(native(file));
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, "# Mine\n").unwrap();
                let message = format!("{:#}", build(&site).unwrap_err());
                let expected = format!(
                    "{name}{} clashes with version `1.4`, built at {url}/: rename or move one",
                    path.display()
                );
                assert_eq!(message, expected);
                fs::remove_file(&path).unwrap();
            }
        };
        clashes(
            "/1.4",
            &[
                ("1.4.md", "the page built from "),
                ("1.4/README.md", "the page built from "),
                ("1.4/notes.txt", ""),
            ],
        );
        clashes(
            "/versions/1.4",
            &[
                ("versions/1.4.md", "the page built from "),
                ("versions/1.4/notes.txt", ""),
            ],
        );
        // ...but one in the folders above it, or deeper down, of the same
        // name, doesn't.
        fs::write(docs.join("versions/README.md"), "# Versions\n").unwrap();
        fs::write(docs.join("versions/1.md"), "# 1\n").unwrap();
        fs::create_dir_all(docs.join("notes")).unwrap();
        fs::write(docs.join("notes/1.4.md"), "# Notes\n").unwrap();
        build(&site).unwrap();
        let dist = site.join("dist");
        for page in ["versions", "versions/1", "versions/1.4", "notes/1.4"] {
            assert!(dist.join(page).join("index.html").is_file(), "{page}");
        }

        // Nor can a version be where search's files are.
        fs::write(site.join("theme/little.yml"), "search: true\n").unwrap();
        fs::write(site.join("little.yml"), site_cfg("/pagefind")).unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        assert_eq!(
            message,
            "version `1.4` is built at /pagefind/, where little writes files for search: change \
             its `url`"
        );
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_version_from_a_ref_is_named_in_what_goes_wrong() {
        use git::tests::{commit_all, git_in};
        let site_cfg = |version: &str| {
            format!(
                "theme: ./theme\nversions:\n  - {{ title: \"2.0\", url: / }}\n  \
                 - {{ title: 1.4 (Old), url: /1.4, {version} }}\n"
            )
        };
        let site = versioned_site(
            "version-errors",
            &[("docs/README.md", "# Old\n\n$\\qty{1}{m}$\n")],
            &[
                ("little.yml", &site_cfg("ref: v1.4")),
                ("docs/README.md", "# Home\n"),
                ("theme/_templates/default.html", "{{ page.content | safe }}"),
            ],
        );
        // What's warned of in its files names each as git does.
        let built = build(&site).unwrap();
        let warnings: Vec<_> = (built.unrendered_math.iter())
            .map(|f| f.to_string())
            .collect();
        assert_eq!(
            warnings,
            ["can't render $\\qty{1}{m}$ in v1.4:docs/README.md: Unknown command \"\\qty\"."]
        );

        // A ref that isn't there stops the build, naming the version.
        fs::write(site.join("little.yml"), site_cfg("ref: v1.5")).unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        // The site's folder, as git names it.
        let top = dunce::canonicalize(&site).unwrap();
        assert_eq!(
            message,
            format!(
                "building version `1.4 (Old)`, from v1.5: there's no tag, branch, or commit \
                 `v1.5` in {}",
                top.display()
            )
        );
        // As does a version that can't be built, as it was.
        fs::write(site.join("little.yml"), "source: content\n").unwrap();
        commit_all(&site);
        git_in(&site, &["tag", "v1.3"]);
        fs::write(site.join("little.yml"), site_cfg("ref: v1.3")).unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        assert_eq!(
            message,
            "building version `1.4 (Old)`, from v1.3: there's no source folder at v1.3:content: \
             set the version's `source`, in `versions`, to the folder its pages are in at v1.3"
        );
        // ...which its `source` can name.
        let fixed = site_cfg("ref: v1.3, source: ./docs");
        fs::write(site.join("little.yml"), fixed).unwrap();
        build(&site).unwrap();
        assert!(site.join("dist/1.4/index.html").is_file());
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_versions_plugins_are_the_sites_reading_its_files_at_its_ref() {
        use git::tests::{commit_all, git_in};
        let site_cfg = |version: &str| {
            format!(
                "theme: ./theme\nplugins:\n  api:\n    wasm: api.wasm\n    read: pkg\n    \
                 files: {{ add: [{{ path: reference/hello.md, read: pkg/hello.md }}] }}\n    \
                 warn: \"can't read pkg/broken.py\"\n\
                 versions:\n  - {{ title: \"2.0\", url: / }}\n  \
                 - {{ title: \"1.4\", url: /1.4, {version} }}\n"
            )
        };
        // At the ref, the site had no plugins, and the plugin's file, which
        // is built, is never committed.
        let site = versioned_site(
            "version-plugins",
            &[
                ("docs/README.md", "# Home\n"),
                ("pkg/hello.md", "# Hello\n"),
            ],
            &[
                ("little.yml", &site_cfg("ref: v1.4")),
                ("docs/README.md", "# Home\n"),
                ("pkg/hello.md", "# Hi\n"),
                ("theme/_templates/default.html", "{{ page.content | safe }}"),
            ],
        );
        let wasm = fs::read(crate::plugin::tests::test_plugin()).unwrap();
        fs::write(site.join("api.wasm"), wasm).unwrap();

        let built = build(&site).unwrap();
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let hello = read("1.4/reference/hello/index.html");
        assert!(hello.starts_with("<h1 id=\"hello\">Hello<"), "{hello}");
        let hi = read("reference/hello/index.html");
        assert!(hi.starts_with("<h1 id=\"hi\">Hi<"), "{hi}");
        // What it warns of, and the pages it adds, say which version they're
        // in.
        let warnings: Vec<_> = (built.plugin_warnings.iter())
            .map(|w| w.to_string())
            .collect();
        assert_eq!(
            warnings,
            [
                "plugin `api`: can't read pkg/broken.py",
                "plugin `api` at v1.4: can't read pkg/broken.py"
            ]
        );
        assert_eq!(
            built.origins[Path::new("1.4/reference/hello/index.html")],
            "plugin `api`'s reference/hello.md at v1.4"
        );

        // A folder it reads that wasn't there at the ref stops the build...
        fs::rename(site.join("pkg"), site.join("lib")).unwrap();
        commit_all(&site);
        git_in(&site, &["tag", "v1.3"]);
        fs::rename(site.join("lib"), site.join("pkg")).unwrap();
        fs::write(site.join("little.yml"), site_cfg("ref: v1.3")).unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        assert_eq!(
            message,
            "building version `1.4`, from v1.3: plugin `api` reads v1.3:pkg, but there's no \
             folder there: set its `read`, in the version's `plugins`, in `versions`, to the \
             folders it reads at v1.3, or turn it off there, with `api: false`"
        );
        // ...unless the version's `plugins` say where it reads there, over the
        // site's settings...
        let moved = "ref: v1.3, plugins: { api: { read: lib, \
                     files: { add: [{ path: reference/hello.md, read: lib/hello.md }] } } }";
        fs::write(site.join("little.yml"), site_cfg(moved)).unwrap();
        let built = build(&site).unwrap();
        let hi = read("1.4/reference/hello/index.html");
        assert!(hi.starts_with("<h1 id=\"hi\">Hi<"), "{hi}");
        assert_eq!(built.plugin_warnings.len(), 2);
        // ...or turn it off, for that version alone.
        let off = "ref: v1.3, plugins: { api: false }";
        fs::write(site.join("little.yml"), site_cfg(off)).unwrap();
        let built = build(&site).unwrap();
        assert!(!site.join("dist/1.4/reference").exists());
        assert!(site.join("dist/1.4/index.html").is_file());
        assert!(site.join("dist/reference/hello/index.html").is_file());
        let warnings: Vec<_> = (built.plugin_warnings.iter())
            .map(|w| w.to_string())
            .collect();
        assert_eq!(warnings, ["plugin `api`: can't read pkg/broken.py"]);
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn the_builtin_theme_has_a_menu_of_versions() {
        let pages = [
            ("docs/README.md", "# Home\n"),
            ("docs/guide.md", "# Guide\n"),
        ];
        let mut new = vec![(
            "little.yml",
            "versions:\n  - title: \"2.0 (Alpha)\"\n    url: /2.0-alpha\n  \
             - title: \"1.4 (Latest)\"\n    url: /\n    ref: v1.4\n",
        )];
        new.extend(pages);
        let site = versioned_site("versions-builtin", &pages, &new);

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let latest = read("guide/index.html");
        assert!(
            latest.contains(
                "<summary><span class=\"visually-hidden\">Version </span>1.4 (Latest)</summary>"
            ),
            "{latest}"
        );
        // In the order they're listed.
        let alpha_item = latest.find("<a href=\"/2.0-alpha/guide\">2.0 (Alpha)</a>");
        let latest_item = latest.find("<a href=\"/guide\" aria-current=\"true\">1.4 (Latest)</a>");
        assert!(alpha_item.is_some() && alpha_item < latest_item, "{latest}");
        assert!(!latest.contains("version-note"), "{latest}");
        // Each version but the one at `/` notes which it is, and links to the
        // one at `/`.
        let alpha = read("2.0-alpha/guide/index.html");
        assert!(
            alpha.contains("<a href=\"/2.0-alpha/guide\" aria-current=\"true\">2.0 (Alpha)</a>"),
            "{alpha}"
        );
        assert!(
            alpha.contains(
                "These docs are for version <strong>2.0 (Alpha)</strong>. \
                 <a href=\"/guide\">Go to version 1.4 (Latest)</a>"
            ),
            "{alpha}"
        );
        // The 404 page is the version at `/`'s, and links to each's home.
        let not_found = read(NOT_FOUND_FILE);
        assert!(
            not_found.contains("<a href=\"/\" aria-current=\"true\">1.4 (Latest)</a>"),
            "{not_found}"
        );
        assert!(
            not_found.contains("<a href=\"/2.0-alpha/\">2.0 (Alpha)</a>"),
            "{not_found}"
        );
        assert!(!not_found.contains("version-note"), "{not_found}");
        assert!(!site.join("dist/2.0-alpha").join(NOT_FOUND_FILE).exists());
        // Its files are written once, at the root, but each version is
        // searched on its own.
        assert!(alpha.contains("<link rel=\"stylesheet\" href=\"/assets/style.css\">"));
        assert!(alpha.contains("<script src=\"/2.0-alpha/pagefind/pagefind-component-ui.js\""));
        assert!(site.join("dist/assets/style.css").is_file());
        assert!(!site.join("dist/2.0-alpha/assets").exists());
        assert!(site.join("dist/pagefind/pagefind.js").is_file());
        assert!(site.join("dist/2.0-alpha/pagefind/pagefind.js").is_file());

        // A site with one version has no menu.
        fs::write(
            site.join("little.yml"),
            "versions:\n  - { title: Latest, url: / }\n",
        )
        .unwrap();
        build(&site).unwrap();
        let only = read("guide/index.html");
        assert!(!only.contains("version-menu"), "{only}");
        assert!(!only.contains("version-note"), "{only}");
        let _ = fs::remove_dir_all(&site);
    }
}
