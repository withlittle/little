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
//!
//! Each page is built into a file named for it, like `guide/install.html`,
//! at `/guide/install`, where a host that serves a `.html` file at its path
//! without the extension finds it, with no redirect. A folder's own page is
//! the folder's `index.html`, at `/guide/`. A page can't be beside a folder
//! of its name, as a host can't tell `/guide` from `/guide/`.
//!
//! The file a page is read from, its source, is copied to its own path
//! too, as it's written, like any other file in the source, so beside the
//! page, at `/guide/install.md`, which the theme gets as `page.source.url`.
//! A page a plugin adds has no file, so has none, unless its own settings
//! name one.
//!
//! Each of the site's `redirects` is a page too, at the URL it's from, in a
//! folder there, so a host finds it with or without the URL's `/`, which
//! the theme writes, to send its reader on to where the redirect leads. A
//! site with `versions` writes one in each version that has a page or file
//! where it leads, and none where it's from, as an old version may still
//! have a page where it was. One from every page under a folder, like
//! `/widgets/*`, is a page for each of the version's pages under the folder
//! it leads to, where the page would have been.
//!
//! [`build`] builds a site from nothing, and [`rebuild`] after the build
//! before it, for `little serve`: it reads the site as [`build`] does, but
//! renders and writes only what differs.
//!
//! A theme gets when the site was built, as `site.built`, and, from git,
//! when the site last changed, and when each page's file was added, and
//! last changed (see `git`). A rebuild is as of when the build before it
//! was, so the time alone renders no page again.

use anyhow::{Context, Result, bail};
use minijinja::value::Serde;
use minijinja::{Value as MjValue, context};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::ErrorKind;
use std::iter::Peekable;
use std::mem;
use std::path::{Component, Path, PathBuf};
use std::slice;
use std::sync::{Arc, Mutex};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};
use walkdir::WalkDir;

use crate::config::{Config, PageSettings, Repo, Shared, SiteUrl};
use crate::plugin::{Plugins, Warned};
use crate::theme::{PageType, StaticFile, Theme, ThemeFiles};
use crate::{config, git, parts, render, search};

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
    /// When its file was added to the site's repository, and when it last
    /// changed there, if git says.
    dates: Option<git::Dates>,
    /// Pretty, extensionless URL (`/`, `/getting-started`, `/guide/install`).
    url: String,
    /// Output file on disk (`dist/guide/install.html`).
    out_path: PathBuf,
    title: String,
    /// The page's settings: every page's, from `page` in the config, with
    /// its frontmatter over them, less any key left blank.
    settings: Mapping,
    /// What its settings say of what little works out for it.
    wants: PageSettings,
    html: String,
    /// Its table of contents: its headings at the levels its
    /// `outline.depth` lists, nested like `site.navigation.items`. It has
    /// none if its `outline` is off.
    outline: Vec<NavItem>,
    page_type: PageType,
    /// The links and images in its Markdown, as they're written.
    links: Vec<render::WrittenLink>,
    /// Each formula in its Markdown that couldn't be rendered, then each
    /// diagram that couldn't be drawn, and each code block plugins couldn't
    /// render.
    warnings: Vec<Warning>,
}

/// What little works out for `page`, beneath the page's settings (see
/// `page_value`). Anything a page doesn't have is left out rather than set to
/// null, which a template would print as "None". Each group but `previous`
/// and `next` is always there, so a template can read `page.outline.items`
/// on any page.
#[derive(Serialize)]
struct PageData<'a> {
    #[serde(rename = "type")]
    page_type: &'static str,
    url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<&'a str>,
    /// When the page's file was added to the site's repository, like
    /// `2026-11-19T02:37:42Z`.
    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<&'a str>,
    /// When it last changed there.
    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<&'a str>,
    /// A home or section page's branch of the nav: what's in its folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<&'a [NavItem]>,
    /// Each of the site's versions, with the `url` of this page in it, or,
    /// if it's not there, of its home page.
    #[serde(skip_serializing_if = "Option::is_none")]
    versions: Option<&'a [Mapping]>,
    /// Whether search indexes the page.
    search: Enabled,
    /// The page's table of contents, shaped like `site.navigation.items`.
    outline: Items<&'a [NavItem]>,
    /// What's above this page in the nav, from the top down.
    breadcrumb: Items<&'a [Breadcrumb<'a>]>,
    /// The page before this one in the nav, unless it's the first.
    #[serde(skip_serializing_if = "Option::is_none")]
    previous: Option<Link<'a>>,
    /// The page after this one in the nav, unless it's the last.
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<Link<'a>>,
    /// Where to view and to edit the page's file in the site's repository.
    repository: FileLinks<'a>,
    /// Where the page's file is copied to, in the site.
    source: FileUrl<'a>,
    /// Where a redirect leads.
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect: Option<Target<'a>>,
}

impl<'a> PageData<'a> {
    /// What little works out for a page of `page_type` at `url` that has no
    /// file, or content, like the 404 page: its type and URL, the home of
    /// each of the site's `versions`, and each group, for its template, with
    /// nothing little works out in it. Search never indexes it.
    fn bare(page_type: PageType, url: &'a str, versions: Option<&'a [Mapping]>) -> Self {
        PageData {
            page_type: page_type.name(),
            url,
            title: None,
            content: None,
            created: None,
            modified: None,
            children: None,
            versions,
            search: Enabled { enabled: false },
            outline: Items { items: &[] },
            breadcrumb: Items { items: &[] },
            previous: None,
            next: None,
            repository: FileLinks {
                view: FileUrl { url: None },
                edit: FileUrl { url: None },
            },
            source: FileUrl { url: None },
            redirect: None,
        }
    }
}

/// Whether a page has something: what a template sees as `page.search`.
#[derive(Serialize)]
struct Enabled {
    enabled: bool,
}

/// A list a page has: what a template sees as `page.outline` or
/// `page.breadcrumb`, beside the group's settings.
#[derive(Serialize)]
struct Items<T> {
    items: T,
}

/// A page's links to its file in the site's repository: what a template
/// sees as `page.repository`. Each is always there, for its settings, so a
/// template can read `page.repository.edit.url` on any page.
#[derive(Serialize)]
struct FileLinks<'a> {
    /// Where to view the file there, like
    /// `https://github.com/withlittle/little/blob/main/docs/guide/install.md`.
    view: FileUrl<'a>,
    /// Where to edit it there.
    edit: FileUrl<'a>,
}

/// Where a page's file is, if little knows: what a template sees as
/// `page.source`, for where it's copied to in the site, with the path of
/// the site's `url`, like `/my-project/guide/install.md`, and as each of
/// `page.repository`'s links, each beside its settings.
#[derive(Serialize)]
struct FileUrl<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
}

/// Where a redirect leads: what a template sees as `page.redirect`.
#[derive(Serialize)]
struct Target<'a> {
    /// A URL from the root of the version, or another site's address.
    url: &'a str,
    /// The title of the page there, if it's one of the version's.
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
}

/// A link to another page: what a template sees as `page.previous` or
/// `page.next`, beside the group's settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct Link<'a> {
    title: &'a str,
    url: &'a str,
}

/// A page above another in the nav, or a folder without a `README.md`, which
/// has no `url`: what a template sees in `page.breadcrumb.items`.
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

/// One node in the site-wide navigation tree (see `build_nav`), in a nav
/// that the config sets instead, as `site.navigation.items`, or in a page's
/// table of contents (see `nest_headings`).
#[derive(Clone, Debug, Deserialize, Serialize)]
struct NavItem {
    title: String,
    /// A page's path, a heading's `#fragment`, or in a config's nav, any
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
    /// As the theme renders it, or `None` if it's what the last build wrote
    /// there, so a rebuild leaves it.
    html: Option<String>,
    /// What it's rendered from.
    wrote: Wrote,
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
    /// destination, like `/guide/install` for `guide/install.html`, so a
    /// broken link to it is shown as the theme links to it.
    pub urls: HashMap<PathBuf, String>,
    /// The links and images in each page's Markdown, by its path in the
    /// destination, so a broken one is shown as it's written.
    pub links: HashMap<PathBuf, Vec<render::WrittenLink>>,
    /// The repository the site's source is in, if it says, whose links
    /// aren't checked.
    pub repo: Option<Repo>,
    /// What it warns of, as it happened: each URL in `site.search.exclude`
    /// that matches no page, each path in `site.source.exclude` that leaves
    /// out no file, each formula in its pages that couldn't be
    /// rendered, so is shown as its TeX, each diagram that couldn't be drawn,
    /// so is shown as code, each code block a plugin couldn't render, so is
    /// left as it was, what each plugin warned of, and each of its hooks
    /// that failed, so the build went on without its change, and each of
    /// `site.redirects` that no version of the site has, or all of them, if
    /// the theme has no template to write them with.
    pub warnings: Vec<Warning>,
    /// The path in the destination of each file written for a version of
    /// the site from a git ref, whose links to other sites aren't checked,
    /// as a tag can't be fixed.
    pub from_refs: HashSet<PathBuf>,
    /// What each file it wrote was written from, by its path in the
    /// destination, so a [`rebuild`] writes only those that differ.
    pub files: HashMap<PathBuf, Wrote>,
    /// The path in the destination of each file it wrote, and didn't leave
    /// as the build before it wrote it, as a [`rebuild`] can.
    pub written: HashSet<PathBuf>,
    /// What search read of each version's pages, by the folder the version
    /// is written into, which a [`rebuild`] takes, to index only the pages
    /// that differ.
    pub indexes: Arc<Mutex<HashMap<PathBuf, search::Index>>>,
    /// Each Markdown page it parsed, by a hash of all it was parsed from,
    /// which a [`rebuild`] parses again only if that differs.
    pub parsed: Arc<HashMap<u64, render::Rendered>>,
    /// When it started, or `SOURCE_DATE_EPOCH`: the theme's `site.built`,
    /// which a [`rebuild`] keeps, so the time alone renders no page again.
    pub time: OffsetDateTime,
    /// The commit the site's files, as they are, were built from, if its
    /// repository has one, which a [`rebuild`] asks git the date of only if
    /// it's another.
    head: Option<git::Commit>,
    /// The dates git has for the files of each version, by its commit, and
    /// its source folder, from the top of the repository, which a
    /// [`rebuild`] reads again only for another commit.
    histories: Arc<HashMap<(String, String), Arc<FileDates>>>,
}

/// When each file in a folder of the site's repository was added, and when
/// it last changed, as git has it: each by its path from the top of the
/// repository, like `docs/guide.md`.
type FileDates = HashMap<String, git::Dates>;

/// When a site was built, and when it last changed, as its theme gets them.
#[derive(Clone, Copy)]
struct Times {
    /// When the build started, or, for a [`rebuild`], the first one did.
    built: OffsetDateTime,
    /// When the commit the site is built from was made, if git says.
    modified: Option<OffsetDateTime>,
}

/// What a build wrote a file from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wrote {
    /// A hash of it: for a page, of all the theme renders it from, and the
    /// theme's templates, and for any other file, of its contents.
    from: u64,
    /// The language a page's theme gives it, like `en`, which search indexes
    /// it in.
    lang: Option<String>,
}

impl Wrote {
    /// What a file with `contents` is written from.
    fn contents(contents: &[u8]) -> Wrote {
        let mut hasher = DefaultHasher::new();
        contents.hash(&mut hasher);
        Wrote {
            from: hasher.finish(),
            lang: None,
        }
    }
}

/// Something a build warns of.
#[derive(Clone, Debug)]
pub struct Warning {
    /// What it is, as `little build --strict` counts it, like
    /// `formula(s) that can't be rendered`.
    pub kind: &'static str,
    /// What it says, naming where it is, like `can't draw the diagram on
    /// line 3 of docs/intro.md: No diagram type detected`.
    pub message: String,
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
    /// Its `site` settings, merged, which the theme gets as `site`.
    site: Mapping,
    /// Every page's settings, merged, which each page's frontmatter goes
    /// over.
    page: Mapping,
    /// What they say of what little works out for a page.
    page_settings: PageSettings,
    /// The repository the site's source is in, if it says.
    repo: Option<Repo>,
    /// What it indexes for search.
    search: config::Search,
    /// Each path its `source.exclude` lists, as it's written, and whether
    /// it left out a file or a folder.
    excluded: Vec<(String, bool)>,
    /// Whether each page's nav lists what's in a folder only if the page is
    /// in it.
    nav_prune: bool,
    /// The site's redirects, each of which it has if it can.
    redirects: Vec<config::Redirect>,
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
    /// Which of the site's redirects it has, each by its place in
    /// `site.redirects`.
    redirected: Vec<usize>,
    outputs: Vec<Output>,
    /// The source's other files, and, for the version at the root of the
    /// site, the theme's static files.
    static_files: Vec<StaticFile>,
    search_files: Vec<search::File>,
}

impl Ready {
    /// The path of each file it writes, from `root`, the destination.
    fn paths<'a>(&'a self, root: &'a Path) -> impl Iterator<Item = PathBuf> + 'a {
        let pages = self.outputs.iter().map(|output| output.path.clone());
        let files = (self.static_files.iter().map(|file| &file.path))
            .chain(self.search_files.iter().map(|file| &file.path))
            .map(|path| self.destination.join(path));
        pages
            .chain(files)
            .filter_map(move |path| Some(path.strip_prefix(root).ok()?.to_path_buf()))
    }
}

/// Build the site rooted at `site_dir`: each of its `versions`, or, if it
/// has none, its files as they are.
///
/// Pages, and the nav and links between them, are worked out at their URLs
/// from the root of their version, like `/guide/`. Only what the theme gets
/// has the path of the version's URL before them, like `/my-project/guide/`,
/// or `/my-project/1.4/guide/`.
pub fn build(site_dir: &Path) -> Result<Built> {
    build_after(site_dir, None, None)
}

/// Build the site rooted at `site_dir`, as [`build`] does, but as of `time`,
/// not now: when `little serve` started, which it keeps as the time of each
/// build it makes, whole ones too.
pub fn build_as_of(site_dir: &Path, time: OffsetDateTime) -> Result<Built> {
    build_after(site_dir, None, Some(time))
}

/// Build the site rooted at `site_dir` again, after `last`, the build before
/// it, as `little serve` does on each save. The destination ends up as
/// [`build`] would leave it, but only what differs is written: a page is
/// rendered again only if what its theme renders it from has changed, or the
/// theme's templates have, and a file that's no longer built is deleted.
///
/// Every file is still read, but a Markdown page is parsed again only if
/// its text, or the settings it's parsed with, have changed. If `last`
/// went to another destination, or its files aren't there now, every page
/// is rendered and written, as [`build`] does.
///
/// It's built as of when `last` was, so `site.built` is the same, and a
/// page isn't rendered again for the time alone.
pub fn rebuild(site_dir: &Path, last: &Built) -> Result<Built> {
    build_after(site_dir, Some(last), Some(last.time))
}

/// The Markdown pages a build parses, by a hash of all each is parsed from:
/// its text, the URL of its folder, and the site's settings.
#[derive(Default)]
struct Parses {
    /// Those the build before it parsed.
    before: Arc<HashMap<u64, render::Rendered>>,
    /// Those this build has, parsed or taken from `before`.
    kept: HashMap<u64, render::Rendered>,
}

/// The dates git has for the files of each version of a site: when each was
/// added to the site's repository, and when it last changed there.
struct Histories<'a> {
    /// The repository the site is in, if it has its whole history: without
    /// one, or in a shallow clone, whose oldest commit seems to add every
    /// file, no file has dates.
    repository: Option<&'a git::Repository>,
    /// The commit the site's files, as they are, are built from.
    head: Option<&'a str>,
    /// Those the build before it read, by each version's commit, and its
    /// source folder, from the top of the repository.
    before: Arc<HashMap<(String, String), Arc<FileDates>>>,
    /// Those this build has, read or taken from `before`.
    kept: HashMap<(String, String), Arc<FileDates>>,
}

impl Histories<'_> {
    /// The dates of the files in `folder`, from the top of the repository,
    /// in the history of `commit`: none, without either, or if git can't
    /// say, as a date it can't give is left out.
    fn of(&mut self, commit: Option<&str>, folder: Option<&str>) -> Arc<FileDates> {
        let (Some(repository), Some(commit), Some(folder)) = (self.repository, commit, folder)
        else {
            return Arc::default();
        };
        let key = (commit.to_string(), folder.to_string());
        let dates = match self.kept.get(&key).or_else(|| self.before.get(&key)) {
            Some(dates) => Arc::clone(dates),
            None => match repository.dates(commit, folder) {
                Ok(dates) => Arc::new(dates),
                Err(_) => return Arc::default(),
            },
        };
        self.kept.insert(key, Arc::clone(&dates));
        dates
    }
}

/// [`build`], or, after `last`, [`rebuild`]: as of `time`, or, without one,
/// now.
fn build_after(
    site_dir: &Path,
    last: Option<&Built>,
    time: Option<OffsetDateTime>,
) -> Result<Built> {
    let mut parses = Parses {
        before: last
            .map(|last| Arc::clone(&last.parsed))
            .unwrap_or_default(),
        kept: HashMap::new(),
    };
    let config = config::load(site_dir)?;
    // Once, so every page of a build has the same time, and every diagram
    // the same today, even at midnight.
    let now = match time {
        Some(time) => time,
        None => time_of_build(env::var_os("SOURCE_DATE_EPOCH"))?,
    };
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
    // A site needn't be in a repository, unless one of its versions is from
    // a git ref: without one, git has no dates for it.
    let from_refs = (versions.iter().flatten()).any(|version| version.git_ref.is_some());
    let repository = match git::Repository::find(site_dir) {
        Ok(repository) => Some(repository),
        Err(e) if from_refs => return Err(e),
        Err(_) => None,
    };
    // The commit the site is built from, and when it was made, which git is
    // asked only of a commit the last build wasn't built from.
    let head = repository.as_ref().and_then(|repository| {
        let id = repository.head()?;
        let kept = (last.and_then(|last| last.head.as_ref())).filter(|head| head.id == id);
        match kept {
            Some(head) => Some(head.clone()),
            None => Some(git::Commit {
                id: id.to_string(),
                date: repository.date(id).ok()?,
            }),
        }
    });
    let times = Times {
        built: now,
        modified: head.as_ref().map(|head| head.date),
    };
    let mut histories = Histories {
        repository: (repository.as_ref()).filter(|repository| !repository.is_shallow()),
        head: head.as_ref().map(|head| head.id.as_str()),
        before: (last.map(|last| Arc::clone(&last.histories))).unwrap_or_default(),
        kept: HashMap::new(),
    };

    // Every version is in the site's theme, so its static files are read
    // once, and written once, for the version at the root of the site.
    let theme_files = theme.static_files()?;
    let theme_urls = (theme_files.iter())
        .map(|file| file_url(&file.path))
        .collect();
    // Only a destination the last build left its marker in still holds what
    // it wrote there.
    let destination = &config.destination_dir;
    let last = last
        .filter(|last| last.destination == *destination && destination.join(MARKER_FILE).is_file());
    let mut templates = DefaultHasher::new();
    theme.hash_templates(&mut templates);

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
                    let theme = (&theme, &mut parses, &mut histories);
                    read_source(site_dir, &config, checkout, plugins, theme, place, now)
                })
            }
            _ => {
                let plugins = site_plugins.take().unwrap_or_default();
                let theme = (&theme, &mut parses, &mut histories);
                read_source(site_dir, &config, None, plugins, theme, place, now)
            }
        };
        sources.push(in_version(source, version)?);
    }
    let every = (!config.versions.is_empty()).then(|| {
        let entries = match config.site.get("versions") {
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
        warnings: Vec::new(),
        from_refs: HashSet::new(),
        files: HashMap::new(),
        written: HashSet::new(),
        indexes: Arc::default(),
        parsed: Arc::new(parses.kept),
        time: now,
        histories: Arc::new(histories.kept),
        head,
    };
    // When every version is from a git ref, the site's plugins build none,
    // but may have warned as they started.
    if let Some(mut plugins) = site_plugins {
        built.warnings.extend(warned(&mut plugins, None));
    }
    // A URL in `site.search.exclude` that leaves out no page, in any version,
    // is likely a typo, or a page that's moved, as a broken link would be.
    let urls: Vec<&str> = (sources.iter())
        .flat_map(|source| &source.pages)
        .map(|page| page.url.as_str())
        .collect();
    let excluded = config.search.exclude.iter();
    for entry in excluded.filter(|_| config.search.enabled) {
        if !urls.iter().any(|url| config::leaves_out(entry, url)) {
            built.warnings.push(Warning {
                kind: "URL(s) in `site.search.exclude` that match no page",
                message: format!(
                    "`site.search.exclude` lists {entry}, but there's no page at that URL, or \
                     under it"
                ),
            });
        }
    }
    // So is a path in a version's `source.exclude` that leaves nothing out,
    // in any version that lists it: the file it meant is published.
    let mut excluded: Vec<(&str, bool)> = Vec::new();
    for (listed, left_out) in sources.iter().flat_map(|source| &source.excluded) {
        match excluded.iter_mut().find(|(path, _)| path == listed) {
            Some((_, any)) => *any |= *left_out,
            None => excluded.push((listed, *left_out)),
        }
    }
    for (listed, _) in excluded.iter().filter(|(_, left_out)| !left_out) {
        built.warnings.push(Warning {
            kind: "path(s) in `site.source.exclude` that leave out no file",
            message: format!(
                "`site.source.exclude` lists {listed}, but the source has no file at that path, \
                 or under it, to leave out"
            ),
        });
    }
    // Every version is in the one theme, which writes redirects or doesn't.
    let redirects = &config.redirects;
    let writes_redirects =
        (sources.first()).is_some_and(|source| source.theme.renders(PageType::Redirect));
    let mut redirected = vec![false; redirects.len()];
    let mut ready = Vec::new();
    for (source, &version) in sources.into_iter().zip(&versions) {
        let theme = (theme_files.as_slice(), &templates);
        let finished = finish(source, every.as_ref(), theme, times, last, &mut built);
        let finished = in_version(finished, version)?;
        for &index in &finished.redirected {
            redirected[index] = true;
        }
        ready.push(finished);
    }
    if !writes_redirects && !redirects.is_empty() {
        built.warnings.push(Warning {
            kind: "theme(s) with no template for the site's redirects",
            message: format!(
                "`site.redirects` lists {} redirect(s), but the theme has no \
                 _templates/redirect.html to write them with, so none is written",
                redirects.len()
            ),
        });
    }
    // A redirect that no version has is likely a typo, or a page that's
    // moved again, as a URL in `site.search.exclude` that matches no page
    // is. On a site without versions, it has stopped the build already.
    let unwritten = (redirects.iter().zip(&redirected))
        .filter(|(_, written)| writes_redirects && every.is_some() && !**written);
    for (redirect, _) in unwritten {
        let config::Redirect { from, to } = redirect;
        let why = match (redirect.folders(), redirect.target()) {
            (Some((was, now)), _) => format!(
                "no version of the site has a page under {now} without a page or file where it \
                 would be under {was}"
            ),
            (None, Some(target)) => format!(
                "no version of the site has a page or file at {target} without one at {from}"
            ),
            (None, None) => format!("every version of the site has a page or file at {from}"),
        };
        built.warnings.push(Warning {
            kind: "redirect(s) in `site.redirects` that no version of the site has",
            message: format!("`site.redirects` sends {from} to {to}, but {why}"),
        });
    }

    let pages: usize = ready.iter().map(|version| version.pages).sum();
    match last {
        None => clear_destination(destination)?,
        // Before anything's written, as on a disk that ignores case, a file
        // renamed `guide.md` from `Guide.md` is written where it was.
        Some(last) => {
            let written: HashSet<PathBuf> = (ready.iter())
                .flat_map(|version| version.paths(destination))
                .collect();
            for path in last.files.keys().filter(|path| !written.contains(*path)) {
                remove_file(destination, path)?;
            }
        }
    }
    for version in ready {
        write_version(version, destination, last, &mut built)?;
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
fn warned(plugins: &mut Plugins, git_ref: Option<&str>) -> Vec<Warning> {
    (plugins.take_warnings().into_iter())
        .map(|warning| Warning {
            kind: "warning(s) from plugins",
            // A hook that failed names what it failed on, which names the ref.
            message: match (git_ref, &warning.warned) {
                (Some(git_ref), Warned::Message(message)) => {
                    format!("plugin `{}` at {git_ref}: {message}", warning.plugin)
                }
                _ => warning.to_string(),
            },
        })
        .collect()
}

/// Check out `version` of the site, as it was at its ref, `git_ref`, in
/// `repository`, and its config: the site's, `config`, but with its own
/// pages, nav, and plugins' settings, from its entry in `site.versions`, or
/// its own little.yml there, if it has one. Only the files it reads are
/// checked out, apart from the site's own, and they're deleted once the
/// checkout is dropped.
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
            "there's no source folder at {}: set the version's `source`, in `site.versions`, to \
             the folder its pages are in at {git_ref}",
            checkout.describe(&config.source_dir)
        );
    }
    let missing =
        |read: &&config::Read| checkout.in_repo(&read.folder).is_some() && !read.folder.is_dir();
    for plugin in &config.plugins {
        if let Some(read) = plugin.read.iter().find(missing) {
            bail!(
                "plugin `{name}` reads {}, but there's no folder there: set its `read`, in the \
                 version's `plugins`, in `site.versions`, to the folders it reads at {git_ref}, \
                 or turn it off there, with `enabled: false`",
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

/// How a page's file is read.
#[derive(Clone, Copy)]
enum Format {
    /// As Markdown, by little.
    Markdown,
    /// By the plugin whose `formats` hook reads its extension.
    Plugin,
}

/// Read the version of the site at `site_dir` that `config` configures, to
/// build at `place`, in `theme`, with `plugins`: its files from `checkout`,
/// if it's from a git ref, or else as they are, and those its plugins add.
/// Its pages are rendered, but not yet put in its theme. Each has the dates
/// of its file, from `histories`, and a Markdown page's parse is kept in
/// `parses`.
fn read_source(
    site_dir: &Path,
    config: &Config,
    checkout: Option<git::Checkout>,
    mut plugins: Plugins,
    (theme, parses, histories): (&ThemeFiles, &mut Parses, &mut Histories),
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

    // Walk every file in the source folder but hidden ones, and those its
    // `exclude` leaves out, without walking a folder that's either, or the
    // destination, which can be inside it, as it would be at a git ref, had
    // it been committed. The source folder itself (depth 0) can be hidden.
    let destination = match &checkout {
        None => destination_in(source_dir, &config.destination_dir),
        Some(checkout) => (config.destination_dir.strip_prefix(site_dir).ok())
            .and_then(|rel| destination_in(source_dir, &checkout.site_dir().join(rel))),
    };
    let mut excluded: Vec<(String, bool)> = (config.source_exclude.iter())
        .map(|listed| (listed.clone(), false))
        .collect();
    let mut found = BTreeMap::new();
    let entries = WalkDir::new(source_dir).into_iter().filter_entry(|entry| {
        if entry.depth() == 0 {
            return true;
        }
        let hidden = is_hidden(Path::new(entry.file_name()));
        if hidden || destination.as_deref() == Some(entry.path()) {
            return false;
        }
        // Each path that leaves it out has left something out, not only the
        // first of them.
        let Ok(rel) = entry.path().strip_prefix(source_dir) else {
            return true;
        };
        let path = from_source_root(rel);
        let mut kept = true;
        for (listed, left_out) in &mut excluded {
            if config::leaves_out(listed, &path) {
                *left_out = true;
                kept = false;
            }
        }
        kept
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

    // Each Markdown file, and each file a plugin reads, like a notebook, is
    // rendered into a page, and every other file, like an image, is copied
    // as it is.
    let mut documents = Vec::new();
    let mut files = Vec::new();
    // The plugin that adds each file a plugin adds, by the file's path.
    let mut added_by: HashMap<String, String> = HashMap::new();
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
                added_by.insert(path.clone(), plugin);
                (PathBuf::from(path), origin, contents, true)
            }
        };
        let format = match rel.extension().and_then(|s| s.to_str()) {
            Some("md") => Format::Markdown,
            Some(extension) if plugins.reads(extension) => Format::Plugin,
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
        documents.push((rel, origin, added, text, format));
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

    // When each page's file was added to the site's repository, and when it
    // last changed there: in the history of the version's commit, which for
    // the site's files as they are is `HEAD`, whatever isn't committed.
    let (commit, tracked) = match &checkout {
        Some(checkout) => (Some(&*checkout.commit), checkout.in_repo(source_dir)),
        None => {
            let repository = histories.repository;
            let tracked = repository.and_then(|repo| repo.in_repo(site_dir, source_dir));
            (histories.head, tracked)
        }
    };
    let dates = histories.of(commit, tracked.as_deref());

    // The settings as the site writes them, for the plugins that read pages.
    let markdown = (config.site.get("markdown").and_then(Value::as_mapping))
        .cloned()
        .unwrap_or_default();
    // What every Markdown page of the version is parsed from, but its own
    // text and folder. A plugin that renders code blocks can make anything
    // of one, so its site's pages are parsed each time.
    let mut settings = DefaultHasher::new();
    (&place.url, &markdown, plugins.extensions()).hash(&mut settings);
    let kept = !plugins.renders_blocks();
    let mut pages = Vec::new();
    for (rel, origin, added, text, format) in documents {
        let file = slashed(&source_in_site.join(&rel));
        // A page's file is copied as it's written, so it's kept if a
        // plugin's `source` hook can make something else of it.
        let written = plugins.changes_source().then(|| text.clone());
        let text = plugins.source(&file, &origin, text);
        let folder = folder_url(&rel);
        let rendered = match format {
            Format::Markdown => {
                // A diagram is drawn as of now, so its page is parsed each
                // time: any page that names Mermaid, to be sure.
                let key = (kept && !text.contains("mermaid")).then(|| {
                    let mut key = settings.clone();
                    (&text, &folder).hash(&mut key);
                    key.finish()
                });
                let before = key.and_then(|key| parses.before.get(&key));
                let rendered = match before {
                    Some(rendered) => Ok(rendered.clone()),
                    None => render::parse_document(
                        &text,
                        &folder,
                        &place.url,
                        &config.markdown,
                        now,
                        &mut plugins.on(&file),
                    ),
                };
                if let (Some(key), Ok(rendered)) = (key, &rendered) {
                    parses.kept.insert(key, rendered.clone());
                }
                rendered
            }
            Format::Plugin => plugins.read(&file, &text, &markdown).and_then(|parts| {
                parts::parse_parts(
                    parts,
                    &folder,
                    &place.url,
                    &config.markdown,
                    now,
                    &mut plugins.on(&file),
                )
            }),
        };
        let mut rendered = rendered.with_context(|| format!("rendering {origin}"))?;
        rendered.html = plugins.content(&file, &origin, mem::take(&mut rendered.html));
        // A file a plugin adds isn't in the repository, whatever is there
        // at its path.
        let dated = tracked.as_deref().filter(|_| !added).and_then(|tracked| {
            let file = slashed(&rel);
            match tracked {
                "" => dates.get(&file),
                tracked => dates.get(&format!("{tracked}/{file}")),
            }
        });
        let every_page = &config.page;
        let destination = &place.destination;
        let mut page = page_from(rel, origin, added, rendered, every_page, destination)?;
        page.dates = dated.copied();
        // The file a page is read from is its source, which is copied to
        // the same path in the site, as any other file in the source is. A
        // page a plugin adds has no file: what the plugin gives is made
        // from something else.
        if page.wants.source && !page.added {
            files.push(StaticFile {
                path: page.rel.clone(),
                contents: written.unwrap_or(text).into_bytes(),
                origin: page.origin.clone(),
            });
        }
        pages.push(page);
    }

    // A page a plugin adds names the one file it's made from, if there's
    // one, as its `source.url`. Where the plugin adds that file too, it's
    // the page's source, as the file little reads a page from is that
    // page's: left out, unless a page that names it has its `source` on.
    let mut named: HashMap<String, bool> = HashMap::new();
    for page in pages.iter().filter(|page| page.added) {
        let url = (page.settings.get("source"))
            .and_then(|source| source.get("url"))
            .and_then(Value::as_str);
        let plugin = added_by.get(&slashed(&page.rel));
        let file = (url.into_iter().flat_map(files_at))
            .find(|file| plugin.is_some() && added_by.get(file) == plugin);
        if let Some(file) = file {
            *named.entry(file).or_default() |= page.wants.source;
        }
    }
    files.retain(|file| named.get(&slashed(&file.path)) != Some(&false));

    // Deterministic output order (nav has its own ordering), and so a
    // deterministic error for files that would be the same page.
    pages.sort_by(|a, b| (&a.url, &a.rel).cmp(&(&b.url, &b.rel)));
    check_clashes(&pages)?;
    Ok(Source {
        place,
        git_ref: checkout.as_ref().map(|checkout| checkout.git_ref.clone()),
        site: config.site.clone(),
        page: config.page.clone(),
        page_settings: config.page_settings.clone(),
        repo: config.repo.clone(),
        search: config.search.clone(),
        excluded,
        nav_prune: config.nav_prune,
        redirects: config.redirects.clone(),
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
/// for search. The version at the root has the 404 page, too, and each has
/// the site's redirects that it can. What's warned of is noted in `built`.
///
/// After `last`, the build before it, a page is left as that wrote it, not
/// rendered, if all the theme would render it from hashes as it did then,
/// with `templates`, a hash of the theme's.
fn finish(
    source: Source,
    versions: Option<&Versions>,
    (theme_files, templates): (&[StaticFile], &DefaultHasher),
    times: Times,
    last: Option<&Built>,
    built: &mut Built,
) -> Result<Ready> {
    let Source {
        place,
        git_ref,
        site: values,
        page: every_page,
        page_settings: every_page_wants,
        repo,
        search,
        excluded: _,
        nav_prune,
        redirects,
        theme,
        pages,
        files,
        mut plugins,
        in_repo,
        checkout: _checkout,
    } = source;

    // What little works out is the bottom of the cascade: the nav it builds
    // from the folders, when the site was built and last changed, the
    // version, and the owner and name of the repository go beneath the
    // config, so a theme or site that sets `navigation.items`, `built`,
    // `modified`, `version`, or `repository.owner` replaces it.
    let tree = build_tree(&pages);
    let mut site = Mapping::new();
    let items = serde_yaml::to_value(build_nav(&tree))?;
    let navigation = Mapping::from_iter([("items".into(), items)]);
    site.insert("navigation".into(), Value::Mapping(navigation));
    site.insert("built".into(), timestamp(times.built)?.into());
    if let Some(modified) = times.modified {
        site.insert("modified".into(), timestamp(modified)?.into());
    }
    if let Some(title) = &place.title {
        site.insert("version".into(), title.as_str().into());
    }
    if let Some(repo) = &repo {
        let (owner, name) = repo.owner_and_name();
        let mut repository = Mapping::new();
        if let Some(owner) = owner {
            repository.insert("owner".into(), owner.into());
        }
        repository.insert("name".into(), name.into());
        site.insert("repository".into(), Value::Mapping(repository));
    }
    config::merge(&mut site, values);
    // Each entry of `versions`, as it's written, with the URL of its home.
    if let Some(versions) = versions {
        let entries = versions.at(None).into_iter().map(Value::Mapping);
        site.insert("versions".into(), Value::Sequence(entries.collect()));
    }
    nav_in(&site).context(
        "`site.navigation.items` in little.yml must be a list of links, each \
         `{ title, url, children }`",
    )?;
    if let Some(nav) = nav_mut(&mut site) {
        link_items(nav, &place.url);
    }
    // Plugins get `site` as the theme does, and `page.breadcrumb`,
    // `page.previous`, and `page.next` follow the nav they leave it with, so
    // they agree with the theme's sidebar.
    let site = plugins.site(site, |name, site| {
        nav_in(site).map(drop).with_context(|| {
            format!(
                "plugin `{name}`'s `site` hook gives back a `navigation.items` that isn't a \
                 list of links, each `{{ title, url, children }}`"
            )
        })
    })?;
    let nav = nav_in(&site).context("`site.navigation.items` must be a list of links")?;
    let site_value = MjValue::from(Serde(&site));
    // What every page of the version is rendered from: the theme's
    // templates, the URL their filters link under, and `site`, but for a
    // pruned nav, which is each page's own.
    let mut shared = templates.clone();
    place.url.hash(&mut shared);
    let neighbours = neighbours_in(&nav);
    let trails = breadcrumbs_in(&nav, &place.url.link("/"));
    // With `site.navigation.prune`, each page gets a nav of its own, pruned
    // from the whole one, which its breadcrumbs, `previous`, and `next`
    // still follow.
    let pruned = nav_prune
        .then(|| {
            let urls = (pages.iter()).map(|page| place.url.link(&page.url).into_owned());
            PrunedNav::new(&site, urls)
        })
        .flatten();
    match &pruned {
        Some(pruned) => pruned.site.hash(&mut shared),
        None => site.hash(&mut shared),
    }
    // A plugin's `html` hook can make anything of a page, from more than
    // what the theme gets, so its pages are rendered each time.
    let rendered = last.filter(|_| !plugins.changes_html());
    let root = built.destination.clone();

    // The theme owns 100% of the markup; little only hands it structured
    // data, which plugins can change, then change what the theme makes of it.
    let mut render = |page_type: PageType, page: Mapping, url: &str, on: &str, path: &Path| {
        let page = plugins.page(on, page);
        // Search indexes the page's own content and title, as plugins leave
        // them, not the theme's markup around it; its `search.enabled`, as
        // they leave it, says whether it's indexed at all.
        let text = |key: &str| page.get(key).and_then(Value::as_str).map(str::to_string);
        let listed = match page.get("search").and_then(|search| search.get("enabled")) {
            Some(Value::Bool(listed)) => *listed,
            _ => bail!(
                "`search.enabled` must be `true`, to index the page for search, or `false`, to \
                 leave it out"
            ),
        };
        let searched = (text("title"), text("content"), listed);
        let nav = pruned.as_ref().map(|pruned| pruned.at(url));
        let mut from = shared.clone();
        (page_type, url, &nav, &page).hash(&mut from);
        let from = from.finish();
        let written = (rendered.zip(path.strip_prefix(&root).ok()))
            .and_then(|(last, in_root)| last.files.get(in_root))
            .filter(|wrote| wrote.from == from && path.is_file());
        if let Some(wrote) = written {
            return Ok(Some((None, wrote.clone(), searched)));
        }
        let site = match (&pruned, nav) {
            (Some(pruned), Some(nav)) => pruned.site(nav),
            _ => site_value.clone(),
        };
        let html = theme.render(page_type, context! { site => site, page => Serde(page) })?;
        let html = html.map(|html| plugins.html(url, on, html));
        anyhow::Ok(html.map(|html| {
            let lang = search::lang(&html);
            (Some(html), Wrote { from, lang }, searched)
        }))
    };
    // A page from a git ref links to its file at the ref's commit, which
    // can't be edited.
    let editable = in_repo.as_ref().is_some_and(|in_repo| in_repo.editable);
    let mut outputs = Vec::new();
    let mut searched = Vec::new();
    for page in &pages {
        let url = place.url.link(&page.url);
        let children = is_readme(&page.rel).then(|| children_of(&tree, &page.rel));
        // little acts on each group's `enabled`, so a theme shows only what
        // it's given, and a page turns something off whatever its theme.
        let wants = &page.wants;
        let (previous, next) = neighbours.get(&*url).copied().unwrap_or_default();
        // A page that isn't in the nav has nothing above it there.
        let breadcrumbs = (trails.get(&*url))
            .filter(|_| wants.breadcrumb)
            .map(Vec::as_slice)
            .unwrap_or_default();
        // A page a plugin adds has no file in the site's repository.
        let file_urls = (in_repo.as_ref())
            .filter(|_| !page.added)
            .and_then(|in_repo| in_repo.repo.file_urls(&in_repo.folder.join(&page.rel)));
        // Its file is copied to its own path in the version's folder (see
        // `read_source`), so its URL has the version's path, even where one
        // of the theme's files, which versions share, has that path too.
        let copied = (wants.source && !page.added)
            .then(|| format!("{}{}", place.url.path, file_url(&page.rel)));
        let in_versions = versions.map(|versions| versions.at(Some(&page.url)));
        let created = (page.dates.map(|dates| timestamp(dates.created))).transpose()?;
        let modified = (page.dates.map(|dates| timestamp(dates.modified))).transpose()?;
        let data = PageData {
            page_type: page.page_type.name(),
            url: &page.url,
            title: Some(&page.title),
            content: Some(&page.html),
            created: created.as_deref(),
            modified: modified.as_deref(),
            children: children.as_deref(),
            versions: in_versions.as_deref(),
            // Its own `search.enabled` says, else `site.search.exclude`.
            search: Enabled {
                enabled: (wants.search).unwrap_or_else(|| !search.excludes(&page.url)),
            },
            outline: Items {
                items: &page.outline,
            },
            breadcrumb: Items { items: breadcrumbs },
            previous: previous.filter(|_| wants.previous),
            next: next.filter(|_| wants.next),
            repository: FileLinks {
                view: FileUrl {
                    url: (file_urls.as_ref())
                        .filter(|_| wants.view)
                        .map(|urls| urls.view.as_str()),
                },
                edit: FileUrl {
                    url: (file_urls.as_ref())
                        .filter(|_| wants.edit && editable)
                        .map(|urls| urls.edit.as_str()),
                },
            },
            source: FileUrl {
                url: copied.as_deref(),
            },
            redirect: None,
        };
        let rendered = page_value(&data, &page.settings, wants, &place.url)
            .and_then(|value| render(page.page_type, value, &url, &page.origin, &page.out_path))
            .with_context(|| format!("rendering {}", page.origin))?;
        if let Some((html, wrote, (title, content, listed))) = rendered {
            // In the language the theme gives it, at its URL from the root
            // of its version, beside its index, unless its `search.enabled`
            // leaves it out.
            if search.enabled && listed {
                let title = title.unwrap_or_else(|| page.title.clone());
                let content = content.unwrap_or_else(|| page.html.clone());
                searched.push((&page.url, title, content, wrote.lang.clone()));
            }
            let origin = page.origin.clone();
            built.warnings.extend_from_slice(&page.warnings);
            outputs.push(Output {
                path: page.out_path.clone(),
                url: format!("{}{}", place.prefix, page.url),
                html,
                wrote,
                name: format!("the page built from {origin}"),
                origin,
                links: page.links.clone(),
            });
        }
    }

    // A 404 page, like a redirect, has no content, or file: all `page` has
    // is its type and URL, the home of each version, and what every page
    // has, from `page` in the config. Each of its groups is there, for its
    // template, with nothing little works out in it. Search never indexes
    // either, whatever every page's settings say.
    let homes = versions.map(|versions| versions.at(None));
    let mut bare = every_page.clone();
    bare.shift_remove("search");

    // A 404 page has its title and description too, from `site.error`. Only
    // the version at the root of the site has one, at the root of the
    // destination, where static hosts look for it, and only if its theme has
    // a template for it.
    if place.is_root() && theme.renders(PageType::NotFound) {
        let not_found_url = format!("/{NOT_FOUND_FILE}");
        let data = PageData::bare(PageType::NotFound, &not_found_url, homes.as_deref());
        let name = "the 404 page".to_string();
        let path = place.destination.join(NOT_FOUND_FILE);
        let mut settings = bare.clone();
        for key in ["title", "description"] {
            let text = site.get("error").and_then(|error| error.get(key));
            if let Some(text) = text.and_then(Value::as_str) {
                settings.insert(key.into(), text.into());
            }
        }
        let rendered = page_value(&data, &settings, &every_page_wants, &place.url)
            .and_then(|value| {
                let url = place.url.link(&not_found_url);
                render(PageType::NotFound, value, &url, &name, &path)
            })
            .context("rendering the 404 page")?;
        if let Some((html, wrote, _)) = rendered {
            outputs.push(Output {
                path,
                url: not_found_url.clone(),
                html,
                wrote,
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
    // isn't one to find, and those left out.
    let search_files = if search.enabled {
        let pages: Vec<_> = (searched.iter())
            .map(|(url, title, content, lang)| search::Page {
                url,
                title,
                content,
                lang: lang.clone(),
            })
            .collect();
        // A build that fails leaves the next no index, so that starts again.
        let kept = last.and_then(|last| last.indexes.lock().unwrap().remove(&place.destination));
        let (files, index) = search::index(&pages, kept)?;
        let mut indexes = built.indexes.lock().unwrap();
        indexes.insert(place.destination.clone(), index);
        files
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

    // Each of the site's redirects is a page too, at the URL it's from, in
    // a folder there, which a host serves with or without the URL's `/`. The
    // theme writes it, to send its reader on, if it has a template for one.
    // It leads to one of the version's pages or files, or to another site,
    // from a URL where the version has neither. On a site with versions,
    // each has the redirects it can, as an old one may still have a page
    // where it was, and none where it's gone. On any other site, one it
    // can't have is a mistake.
    //
    // One from every page under a folder is a redirect to each of the
    // version's pages under the folder it leads to, from where the page
    // would be under the other. It gives way to a page or file that's
    // there, on any site, as some of a folder's pages may have stayed, and
    // to a redirect from that URL alone, as a page may have been renamed
    // as its folder was.
    let mut redirected = Vec::new();
    if theme.renders(PageType::Redirect) {
        let titles: HashMap<&str, &str> = (pages.iter())
            .map(|page| (page.url.trim_end_matches('/'), page.title.as_str()))
            .collect();
        let alone: HashSet<String> = (redirects.iter())
            .filter(|redirect| redirect.folders().is_none())
            .map(|redirect| redirect_file(&redirect.from))
            .collect();
        let mut files = Vec::new();
        for (index, redirect) in redirects.iter().enumerate() {
            let folders = redirect.folders();
            let each: Vec<(Cow<str>, &str)> = match folders {
                None => vec![(Cow::from(&redirect.from), redirect.to.as_str())],
                Some((was, now)) => (pages.iter())
                    .filter_map(|page| {
                        let rest = page.url.strip_prefix(now)?;
                        Some((Cow::from(format!("{was}{rest}")), page.url.as_str()))
                    })
                    .collect(),
            };
            if let (Some((_, now)), true, None) = (folders, each.is_empty(), versions) {
                let config::Redirect { from, to } = redirect;
                bail!("`site.redirects` sends {from} to {to}, but there's no page under {now}");
            }
            for (from, to) in each {
                let from = from.as_ref();
                let file = redirect_file(from);
                // What's there already is a file where the redirect would
                // be, or a page at its URL, which is in a file of its own.
                let taken =
                    paths.contains(&file) || paths.contains(&page_file(from.trim_end_matches('/')));
                if folders.is_some() && (taken || alone.contains(&file)) {
                    continue;
                }
                // Where it leads is a page, at its URL, a folder's page,
                // with or without its `/`, as a host sends a reader on to
                // it, or any other file, at its own path. A page's URL has
                // no `/` at its end: a host finds nothing there.
                let target = match folders {
                    Some(_) => Some(to),
                    None => redirect.target(),
                };
                let is_there = |target: &str| {
                    paths.contains(&page_file(target))
                        || paths.contains(&redirect_file(target))
                        || paths.contains(target.trim_matches('/'))
                };
                let leads_nowhere = target.is_some_and(|target| !is_there(target));
                if versions.is_some() && (leads_nowhere || taken) {
                    continue;
                }
                if let (true, Some(target)) = (leads_nowhere, target) {
                    let page = (target.strip_suffix('/'))
                        .and_then(|url| pages.iter().find(|page| page.url == url));
                    match page {
                        Some(page) => bail!(
                            "`site.redirects` sends {from} to {to}, but there's no page or file \
                             at {target}: the page is at {}, with no `/` at its end",
                            page.url
                        ),
                        None => bail!(
                            "`site.redirects` sends {from} to {to}, but there's no page or file \
                             at {target}"
                        ),
                    }
                }
                let title = target.and_then(|target| titles.get(target.trim_end_matches('/')));
                let data = PageData {
                    redirect: Some(Target {
                        url: to,
                        title: title.copied(),
                    }),
                    ..PageData::bare(PageType::Redirect, from, homes.as_deref())
                };
                let name = format!("the redirect from {}{from}", place.prefix);
                let path = place.destination.join(&file);
                let rendered = page_value(&data, &bare, &every_page_wants, &place.url)
                    .and_then(|value| {
                        let url = place.url.link(from);
                        render(PageType::Redirect, value, &url, &name, &path)
                    })
                    .with_context(|| format!("rendering {name}"))?;
                if let Some((html, wrote, _)) = rendered {
                    outputs.push(Output {
                        path,
                        url: format!("{}{from}", place.prefix),
                        html,
                        wrote,
                        origin: name.clone(),
                        name,
                        links: Vec::new(),
                    });
                    files.push(file);
                    if redirected.last() != Some(&index) {
                        redirected.push(index);
                    }
                }
            }
        }
        // Only now, so each is checked against the version's pages and
        // files, not the redirects before it: two from one URL clash, below.
        paths.extend(files);
    }
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
    let for_search = format!("/{}/, where little writes files for search", search::DIR);
    let mut written: Vec<(&Path, String)> = (search_files.iter())
        .map(|file| (file.path.as_path(), for_search.clone()))
        .collect();
    // The version at the root of the site has the others in its folders.
    for version in versions.iter().flat_map(|versions| &versions.each) {
        if let Some(folder) = version.prefix.strip_prefix('/').filter(|_| place.is_root()) {
            let title = (version.entry.get("title").and_then(Value::as_str)).unwrap_or_default();
            let name = format!("version `{title}`, built at {}/", version.prefix);
            written.push((Path::new(folder), name));
        }
    }
    for output in &outputs {
        let path = output.path.strip_prefix(&place.destination)?;
        written.push((path, output.name.clone()));
    }
    for file in &static_files {
        written.push((file.path.as_path(), file.origin.clone()));
    }
    // Each page but a folder's own is in a file named for it.
    let named = (pages.iter().filter(|page| !page.url.ends_with('/')))
        .map(|page| page.out_path.strip_prefix(&place.destination))
        .collect::<Result<Vec<_>, _>>()?;
    check_written(written, &named)?;

    built
        .warnings
        .extend(warned(&mut plugins, git_ref.as_deref()));
    Ok(Ready {
        destination: place.destination,
        from_ref: git_ref.is_some(),
        pages: pages.len(),
        redirected,
        outputs,
        static_files,
        search_files,
    })
}

/// Write `version` of a site into the destination, `root`, noting what each
/// file is built from, and, for a page, its URL and links, in `built`. After
/// `last`, the build before it, a file that's there as that wrote it is left.
fn write_version(
    version: Ready,
    root: &Path,
    last: Option<&Built>,
    built: &mut Built,
) -> Result<()> {
    let Ready {
        destination,
        from_ref,
        outputs,
        static_files,
        search_files,
        ..
    } = version;
    // Whether a file with `contents` needs writing, which it's noted with.
    let differs = |path: &Path, contents: &[u8], built: &mut Built| {
        let wrote = Wrote::contents(contents);
        let in_root = path.strip_prefix(root)?.to_path_buf();
        let same = last.and_then(|last| last.files.get(&in_root)) == Some(&wrote);
        let differs = !(same && path.is_file());
        if differs {
            built.written.insert(in_root.clone());
        }
        built.files.insert(in_root, wrote);
        anyhow::Ok(differs)
    };
    let mut written = Vec::new();
    for output in outputs {
        let path = output.path.strip_prefix(root)?.to_path_buf();
        if let Some(html) = &output.html {
            write_file(&output.path, html.as_bytes())?;
            built.written.insert(path.clone());
        }
        built.files.insert(path.clone(), output.wrote);
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
        if differs(&path, &file.contents, built)? {
            write_file(&path, &file.contents)?;
        }
        let path = path.strip_prefix(root)?.to_path_buf();
        built.origins.insert(path.clone(), file.origin);
        written.push(path);
    }
    for file in search_files {
        let path = destination.join(&file.path);
        if differs(&path, &file.contents, built)? {
            write_file(&path, &file.contents)?;
        }
    }
    if from_ref {
        built.from_refs.extend(written);
    }
    Ok(())
}

/// The page built from what `rendered` was rendered from, at `rel` in the
/// source, and what it's built from, `origin`, whether a plugin `added` it,
/// to be written to `destination`. Its frontmatter goes over `every_page`,
/// the settings every page has, from `page` in the config. Settings little
/// can't read, like an `outline` that isn't its settings, are an error.
fn page_from(
    rel: PathBuf,
    origin: String,
    added: bool,
    rendered: render::Rendered,
    every_page: &Mapping,
    destination: &Path,
) -> Result<Page> {
    // A key left blank is as good as not there: a page never holds null,
    // and `title:` shouldn't wipe out the title little finds.
    let mut frontmatter = rendered.frontmatter;
    frontmatter.retain(|_, value| !value.is_null());
    let mut settings = every_page.clone();
    config::merge(&mut settings, frontmatter);
    let wants = PageSettings::read(&settings, "").with_context(|| format!("rendering {origin}"))?;
    // Its table of contents lists its headings at the levels it chooses.
    let listed = (rendered.headings.iter())
        .filter(|heading| wants.outline && wants.depths.contains(&heading.depth));
    let outline = nest_headings(listed);

    // Title: its settings' `title` -> first <h1> -> filename/folder-derived.
    let title = settings
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(rendered.first_h1)
        .unwrap_or_else(|| fallback_title(&rel));

    let url = derive_url(&rel);
    let out_path = out_path_for_url(destination, &url);
    let page_type = page_type(&rel);

    // A diagram or block in a file a plugin reads has no line of the file.
    let at = |thing: &str, line: Option<usize>| match line {
        Some(line) => format!("the {thing} on line {line} of {origin}"),
        None => format!("a {thing} in {origin}"),
    };
    let math = (rendered.unrendered_math.into_iter()).map(|formula| Warning {
        kind: "formula(s) that can't be rendered",
        message: format!(
            "can't render {} in {origin}: {}",
            formula.written, formula.reason
        ),
    });
    let diagrams = (rendered.unrendered_diagrams.into_iter()).map(|diagram| Warning {
        kind: "diagram(s) that can't be drawn",
        message: format!(
            "can't draw {}: {}",
            at("diagram", diagram.line),
            diagram.reason
        ),
    });
    let blocks = (rendered.unrendered_blocks.into_iter()).map(|block| Warning {
        kind: "code block(s) that plugins can't render",
        message: format!(
            "plugin `{}` can't render {}: {}",
            block.plugin,
            at("block", block.line),
            block.reason
        ),
    });
    let warnings = math.chain(diagrams).chain(blocks).collect();

    Ok(Page {
        rel,
        origin,
        added,
        dates: None,
        url,
        out_path,
        title,
        settings,
        wants,
        html: rendered.html,
        outline,
        page_type,
        links: rendered.links,
        warnings,
    })
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

/// When a build is, in UTC, which its theme gets as `site.built`, a diagram
/// as its today, and a plugin as its clock: `SOURCE_DATE_EPOCH`, if it's set,
/// so the build can be reproduced, or now. A malformed `SOURCE_DATE_EPOCH`
/// is an error, as its specification asks.
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

/// `time`, as a theme gets it: in UTC, to the second, as RFC 3339 writes
/// it, like `2026-11-19T02:37:42Z`.
fn timestamp(time: OffsetDateTime) -> Result<String> {
    let time = time.to_offset(UtcOffset::UTC).replace_nanosecond(0)?;
    Ok(time.format(&Rfc3339)?)
}

/// What a template sees as `page`: what little works out for it, `data`,
/// with the page's `settings` merged over it the way each level of the
/// config merges over the one beneath, so the author has the last word:
/// every page's settings, from `page` in the config, and the page's own
/// frontmatter. They change only what the theme sees: a page is built at
/// its own URL, with the template for its own type, whatever they say.
///
/// little acts on what the settings turn off, as `wants` has it, so a theme
/// shows only what it's given: a group that's off has no `items`, and no
/// URLs, whatever the settings write there, and `previous` and `next` are
/// left out, as they are without a page to link to.
///
/// Each of the page's links gets the path of the site's `url`, whoever
/// wrote it, as do the links in its Markdown, unless it's to another site,
/// like its file in the site's repository. Those in `page.breadcrumb`,
/// `page.previous`, and `page.next` have it already, from the nav the theme
/// gets.
fn page_value(
    data: &PageData,
    settings: &Mapping,
    wants: &PageSettings,
    url: &SiteUrl,
) -> Result<Mapping> {
    let Value::Mapping(mut page) = serde_yaml::to_value(data)? else {
        unreachable!("a struct serializes to a mapping");
    };
    link(page.get_mut("url"), url);
    if let Some(items) = page.get_mut("children") {
        link_items(items, url);
    }
    if let Some(items) = within(page.get_mut("outline"), "items") {
        link_items(items, url);
    }
    link(within(page.get_mut("redirect"), "url"), url);
    let mut settings = settings.clone();
    link(settings.get_mut("url"), url);
    for name in ["view", "edit"] {
        let named = within(settings.get_mut("repository"), name);
        link(within(named, "url"), url);
    }
    // A URL the settings write for the page's file gets the path, too:
    // the one little works out has it already.
    for group in ["previous", "next", "source"] {
        link(within(settings.get_mut(group), "url"), url);
    }
    for group in ["breadcrumb", "outline"] {
        if let Some(items) = within(settings.get_mut(group), "items") {
            link_items(items, url);
        }
    }
    if let Some(items) = settings.get_mut("children") {
        link_items(items, url);
    }
    config::merge(&mut page, settings);

    for (group, on) in [("outline", wants.outline), ("breadcrumb", wants.breadcrumb)] {
        if let (false, Some(Value::Mapping(group))) = (on, page.get_mut(group)) {
            group.insert("items".into(), Value::Sequence(Vec::new()));
        }
    }
    for (group, on) in [("previous", wants.previous), ("next", wants.next)] {
        // All it has is its settings, without a page to link to, from the
        // nav or its own.
        let links = matches!(
            page.get(group),
            Some(Value::Mapping(link)) if link.keys().any(|key| key.as_str() != Some("enabled"))
        );
        if !(on && links) {
            page.shift_remove(group);
        }
    }
    for (name, on) in [("view", wants.view), ("edit", wants.edit)] {
        let named = within(page.get_mut("repository"), name);
        if let (false, Some(Value::Mapping(named))) = (on, named) {
            named.shift_remove("url");
        }
    }
    if let (false, Some(Value::Mapping(source))) = (wants.source, page.get_mut("source")) {
        source.shift_remove("url");
    }
    Ok(page)
}

/// The setting at `key` in `group`, a group of a page's settings, to change,
/// if it's there.
fn within<'a>(group: Option<&'a mut Value>, key: &str) -> Option<&'a mut Value> {
    group?.get_mut(key)
}

/// The nav in `site`, what the theme gets as `site.navigation.items`, or
/// why it isn't one.
fn nav_in(site: &Mapping) -> Result<Vec<NavItem>, serde_yaml::Error> {
    let nav = (site.get("navigation")).and_then(|navigation| navigation.get("items"));
    serde_yaml::from_value(nav.cloned().unwrap_or(Value::Sequence(Vec::new())))
}

/// The nav in `site`, its `navigation.items`, to change, if it has one.
fn nav_mut(site: &mut Mapping) -> Option<&mut Value> {
    site.get_mut("navigation")?.get_mut("items")
}

/// Put the site's path before the `url` of each of `items`, a list shaped
/// like `site.navigation.items`, and of each of their `children`.
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

/// Delete the file at `path` in `destination`, which a build no longer
/// writes, and each folder that leaves empty.
fn remove_file(destination: &Path, path: &Path) -> Result<()> {
    let file = destination.join(path);
    match fs::remove_file(&file) {
        Err(e) if e.kind() != ErrorKind::NotFound => {
            return Err(e).with_context(|| format!("deleting {}", file.display()));
        }
        _ => {}
    }
    // A folder with anything in it can't be removed, so stays.
    let folders = path.ancestors().skip(1);
    for folder in folders.take_while(|folder| !folder.as_os_str().is_empty()) {
        if fs::remove_dir(destination.join(folder)).is_err() {
            break;
        }
    }
    Ok(())
}

/// Fail if two files in the source would be the same page, like `README.md`
/// and `readme.md`, where one would silently overwrite the other, or
/// `guide.md` and `guide/README.md`, at `/guide` and `/guide/`, which a
/// host can't serve apart. So would a page a plugin adds, as a file at its
/// path.
/// Files whose URLs differ only in case, like `API.md` and `api.md`, aren't
/// caught, though they clash on a disk that ignores case, as macOS's and
/// Windows' do by default.
fn check_clashes(pages: &[Page]) -> Result<()> {
    let mut files: HashMap<&Path, &Page> = HashMap::new();
    let mut urls: HashMap<&str, &Page> = HashMap::new();
    for page in pages {
        // `guide.md` and `guide/README.md` each have a file of their own,
        // but `/guide` and `/guide/` are one page to a reader, and which of
        // the two a host serves there depends on the host. A page named
        // `index` is its folder's `index.html`, as the folder's `README.md`
        // is.
        let file = files.insert(&page.out_path, page);
        let url = urls.insert(page.url.trim_end_matches('/'), page);
        if let Some(other) = file.or(url) {
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

/// Fail if one of the files a version of a site writes, `files`, each its
/// path in the version's folder and what it's called, is on another, or
/// where another's folder goes, or in it, so one would overwrite the other.
///
/// Fail, too, if one of `pages`, the files among them that are each named
/// for the page it is, like `guide.html`, at `/guide`, is beside a file or
/// a folder of that name, `guide/`. A host can't tell `/guide` from
/// `/guide/`: some serve the page at both, and some look in the folder, and
/// find another page, or none.
fn check_written(files: Vec<(&Path, String)>, pages: &[&Path]) -> Result<()> {
    // Each with its place in `files`, so the later of two is the one named
    // as clashing with the other.
    let mut written: BTreeMap<&Path, (usize, String)> = BTreeMap::new();
    for (place, (path, name)) in files.into_iter().enumerate() {
        // Paths sort by their parts, so the files in a folder sort just after it.
        let under = (written.range(path..).next()).filter(|(file, _)| file.starts_with(path));
        let other = (path.ancestors().find_map(|folder| written.get(folder)))
            .or(under.map(|(_, other)| other));
        if let Some((_, other)) = other {
            bail!("{name} clashes with {other}");
        }
        written.insert(path, (place, name));
    }
    for page in pages {
        let Some((place, name)) = written.get(page) else {
            continue;
        };
        let folder = page.with_extension("");
        // A file of that name, or a version built there, is at the page's
        // URL, and so is the folder's own page, like a redirect from it.
        let index = folder.join("index.html");
        let there = (written.get(folder.as_path())).or_else(|| written.get(index.as_path()));
        if let Some((other_place, other)) = there {
            let (later, earlier) = match place < other_place {
                true => (other, name),
                false => (name, other),
            };
            bail!("{later} clashes with {earlier}");
        }
        let within = (written.range(folder.as_path()..).next())
            .filter(|(file, _)| file.starts_with(&folder));
        if let Some((_, (_, other))) = within {
            bail!(
                "{name} clashes with {other}: a page can't be beside a folder of its name, so \
                 make it the folder's README, or rename one"
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

/// Does `exclude`, a `site.source.exclude`, leave out the file or folder at
/// `rel`, a path in the source folder? One of its paths is the file's, or
/// that of a folder it's in.
pub fn is_excluded(rel: &Path, exclude: &[String]) -> bool {
    let path = from_source_root(rel);
    (exclude.iter()).any(|listed| config::leaves_out(listed, &path))
}

/// The file or folder at `rel`, a path in the source folder, as
/// `site.source.exclude` names it: from the root of the folder, with `/`
/// between its parts, like `/guide/install.md`.
fn from_source_root(rel: &Path) -> String {
    format!("/{}", slashed(rel))
}

/// Is this file a directory's home page? `README.md` is the documentation
/// convention (mirroring how GitHub renders READMEs), as is a `README` in
/// a format a plugin reads, like `README.ipynb`; little maps it to the web's
/// `index.html`.
pub fn is_readme(rel: &Path) -> bool {
    rel.file_stem()
        .is_some_and(|stem| stem.eq_ignore_ascii_case("readme"))
}

/// What type of page the file at `rel` makes, in Markdown or a format a
/// plugin reads.
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

/// The path of the file at `url`, from the root of its version, like
/// `api/openapi.yaml` for `/api/openapi.yaml`, as it's written, and with
/// what's encoded in it decoded, as a file's name can have a `%` of its
/// own. None for a URL that isn't from the root.
fn files_at(url: &str) -> impl Iterator<Item = String> {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let path = path.strip_prefix('/').filter(|path| !path.starts_with('/'));
    let decoded = path.map(|path| percent_decode_str(path).decode_utf8_lossy().into_owned());
    path.map(str::to_string).into_iter().chain(decoded)
}

/// `path`, with `/` between its parts, as plugins' hooks get it, like
/// `guide/install.md`.
fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Derive a page's pretty, extensionless URL: its folder's, then its name,
/// unless it's the folder's `README`, like `README.md`.
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

/// Map a pretty URL to its output file in the destination.
fn out_path_for_url(destination: &Path, url: &str) -> PathBuf {
    destination.join(page_file(url))
}

/// The file the page at `url`, from the root of a version, is built into,
/// from the version's folder, with `/` between its parts: one named for the
/// page, which a host serves at the page's URL, with no `/` at its end and
/// no redirect, or, for a folder's own page, whose URL ends in `/`, the
/// folder's `index.html`.
///
/// - `/`               -> `index.html`
/// - `/guide/`         -> `guide/index.html`
/// - `/guide/install`  -> `guide/install.html`
fn page_file(url: &str) -> String {
    match url.strip_prefix('/').unwrap_or(url) {
        "" => "index.html".to_string(),
        folder if folder.ends_with('/') => format!("{folder}index.html"),
        page => format!("{page}.html"),
    }
}

/// The file a redirect from `url`, from the root of a version, is written
/// to, from the version's folder, with `/` between its parts: the
/// `index.html` of a folder at the URL, which a host serves with or without
/// the `/` at its end, as a reader may have either, or, for a URL that ends
/// in `.html`, that file itself, as a site built by another tool may have
/// published its pages.
///
/// - `/`               -> `index.html`
/// - `/guide/install`  -> `guide/install/index.html`
/// - `/install.html`   -> `install.html`
fn redirect_file(url: &str) -> String {
    match url.trim_matches('/') {
        "" => "index.html".to_string(),
        file if file.ends_with(".html") => file.to_string(),
        folder => format!("{folder}/index.html"),
    }
}

/// Last-resort page title when there's no frontmatter `title` or `<h1>`.
/// Index pages borrow their folder name (root -> "Home").
fn fallback_title(rel: &Path) -> String {
    if is_readme(rel) {
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

/// A folder in the docs tree: its own `README.md` (if any), leaf pages, and
/// subfolders.
#[derive(Default)]
struct DirNode {
    dirs: BTreeMap<String, DirNode>,
    pages: Vec<NavItem>,
    index: Option<NavItem>,
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
    let mut items: Vec<NavItem> = root.index.iter().cloned().collect();
    items.extend(child_items(root));
    items
}

fn insert_into_tree(root: &mut DirNode, page: &Page) {
    let mut node = root;
    for folder in folders(&page.rel) {
        node = node.dirs.entry(folder.to_string()).or_default();
    }

    let item = NavItem {
        title: page.title.clone(),
        url: Some(page.url.clone()),
        children: Vec::new(),
    };
    if is_readme(&page.rel) {
        node.index = Some(item);
    } else {
        node.pages.push(item);
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
    let mut items = node.pages.clone();
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
        Some(index) => (index.title.clone(), index.url.clone()),
        None => (humanize(name), None),
    };
    NavItem {
        title,
        url,
        children: child_items(node),
    }
}

/// A page's table of contents, shaped like `site.navigation.items`, from the
/// `headings` it lists: each heading links to its `#fragment`, and goes in
/// the `children` of the nearest heading above it with fewer `#`s, so a
/// `####` goes under the `###` above it, or under the `##` if there's no
/// `###` between them. A heading with nothing above it, like a `###` before
/// the first `##`, goes at the top.
fn nest_headings<'a>(headings: impl Iterator<Item = &'a render::Heading>) -> Vec<NavItem> {
    nest_under(&mut headings.peekable(), 0)
}

/// The headings that come next, up to the first that isn't deeper than
/// `depth`, each holding the deeper ones that follow it.
fn nest_under<'a>(
    headings: &mut Peekable<impl Iterator<Item = &'a render::Heading>>,
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

/// The pages before and after each page, by its URL: its `page.previous` and
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

/// What's above each page in the nav, by its URL: its
/// `page.breadcrumb.items`. They follow the nav from the top down, so they
/// agree with the sidebar: each item the page is in, as the nav titles it.
/// The nav lists the home page, at `home`, beside the items at its top, but
/// it's above them, so their trails start with it. A page listed twice gets
/// the trail to the last place, where its `page.previous` and `page.next`
/// come from too.
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

/// A version's nav, pruned for each of its pages, for
/// `site.navigation.prune`: a page's nav lists what's in a folder only if
/// the page is in it, or is the folder's own page, so it's as long as the
/// page's way down the nav, not as the site.
///
/// A folder the page isn't in is a link, without its items: to its own page,
/// or, if it's a label, to the first page in it. One that leads nowhere in
/// the site keeps its items, as a reader couldn't reach them otherwise: a
/// folder whose `url` isn't a page's, like a link to another site, and a
/// label with no page in it.
struct PrunedNav<'a> {
    /// `site`, as the theme gets it, but with no items in its nav.
    site: Mapping,
    /// The whole nav, as the theme would get it.
    nav: &'a [Value],
    /// The URL of each of the version's pages, as the nav links to it.
    pages: HashSet<String>,
    /// Where each item with a URL is in the nav, by its URL: the index of
    /// each item it's in, from the top down, then its own. A page listed
    /// twice is at the last place, where its breadcrumbs lead.
    places: HashMap<&'a str, Vec<usize>>,
}

impl<'a> PrunedNav<'a> {
    /// The nav in `site`, what the theme gets as `site`, ready to prune for
    /// each page. `pages` holds each page's URL, as the nav links to it.
    /// `None` if `site` has no nav.
    fn new(site: &'a Mapping, pages: impl IntoIterator<Item = String>) -> Option<Self> {
        let navigation = site.get("navigation")?.as_mapping()?;
        let nav = navigation.get("items")?.as_sequence()?;
        // Each page puts its own nav where the whole one was.
        let emptied = navigation.iter().map(|(key, value)| match key.as_str() {
            Some("items") => (key.clone(), Value::Sequence(Vec::new())),
            _ => (key.clone(), value.clone()),
        });
        let emptied = Value::Mapping(emptied.collect());
        let emptied = site.iter().map(|(key, value)| match key.as_str() {
            Some("navigation") => (key.clone(), emptied.clone()),
            _ => (key.clone(), value.clone()),
        });
        let mut places = HashMap::new();
        find_places(nav, &mut Vec::new(), &mut places);
        Some(PrunedNav {
            site: emptied.collect(),
            nav,
            pages: pages.into_iter().collect(),
            places,
        })
    }

    /// `site`, as the theme gets it for a page, with `nav`, the page's own,
    /// from [`PrunedNav::at`].
    fn site(&self, nav: Vec<Value>) -> MjValue {
        let mut site = self.site.clone();
        if let Some(items) = nav_mut(&mut site) {
            *items = Value::Sequence(nav);
        }
        MjValue::from(Serde(&site))
    }

    /// The nav of the page at `url`, as the nav links to it. A page that
    /// isn't in the nav, like the 404 page, is in none of its folders.
    fn at(&self, url: &str) -> Vec<Value> {
        let place = self.places.get(url).map_or(&[][..], Vec::as_slice);
        self.prune(self.nav, place)
    }

    /// `items`, for a page at `place` in them: the index of each item it's
    /// in, from here down, then its own. It's empty for a page that isn't in
    /// them.
    fn prune(&self, items: &[Value], place: &[usize]) -> Vec<Value> {
        let (here, below) = match place {
            [here, below @ ..] => (Some(*here), below),
            [] => (None, place),
        };
        (items.iter().enumerate())
            .map(|(index, item)| {
                let within = items_in(item);
                if within.is_empty() {
                    return item.clone();
                }
                // The page is in the folder, or is the folder's own.
                if here == Some(index) {
                    return relisted(item, Some(self.prune(within, below)), None);
                }
                let own = url_of(item);
                let first = (own.is_none().then(|| self.first_page(within))).flatten();
                match (own, first) {
                    (Some(url), _) if self.pages.contains(url) => relisted(item, None, None),
                    (None, Some(first)) => relisted(item, None, Some(first)),
                    // It leads nowhere in the site, so it keeps its items.
                    _ => relisted(item, Some(self.prune(within, &[])), None),
                }
            })
            .collect()
    }

    /// The URL of the first page in `items`, in the nav's order, where an
    /// item comes before the items in it.
    fn first_page<'v>(&self, items: &'v [Value]) -> Option<&'v str> {
        items.iter().find_map(|item| {
            (url_of(item).filter(|url| self.pages.contains(*url)))
                .or_else(|| self.first_page(items_in(item)))
        })
    }
}

/// Note where each of `items` with a URL is in the nav, and each item in
/// them, in `places`. `above` holds the index of each item `items` are in,
/// from the top down.
fn find_places<'a>(
    items: &'a [Value],
    above: &mut Vec<usize>,
    places: &mut HashMap<&'a str, Vec<usize>>,
) {
    for (index, item) in items.iter().enumerate() {
        above.push(index);
        if let Some(url) = url_of(item) {
            places.insert(url, above.clone());
        }
        find_places(items_in(item), above, places);
        above.pop();
    }
}

/// The `url` of `item`, an item in a nav, unless it's a label.
fn url_of(item: &Value) -> Option<&str> {
    item.get("url").and_then(Value::as_str)
}

/// The items in `item`, an item in a nav: none, unless it's a folder.
fn items_in(item: &Value) -> &[Value] {
    let items = item.get("children").and_then(Value::as_sequence);
    items.map_or(&[], Vec::as_slice)
}

/// `item`, a folder in a nav, with `items` in place of its own, or, with
/// none, as a link: to `url`, if it has none of its own. Its other keys,
/// like an `icon` for a theme, stay as they are.
fn relisted(item: &Value, mut items: Option<Vec<Value>>, url: Option<&str>) -> Value {
    let Value::Mapping(folder) = item else {
        return item.clone();
    };
    let mut relisted = Mapping::with_capacity(folder.len() + 1);
    for (key, value) in folder {
        match key.as_str() {
            Some("children") => {
                if let Some(items) = items.take() {
                    relisted.insert(key.clone(), Value::Sequence(items));
                }
            }
            // A label's `url`, left blank, which its link takes the place of.
            Some("url") if url.is_some() => {}
            _ => {
                relisted.insert(key.clone(), value.clone());
            }
        }
    }
    if let Some(url) = url {
        relisted.insert("url".into(), url.into());
    }
    Value::Mapping(relisted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{IN_THEME, files_in, render_at, scratch, write};
    use std::io::Read;
    use tempfile::TempDir;

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
            dates: None,
            url,
            out_path,
            title: title.to_string(),
            settings: Mapping::new(),
            wants: PageSettings {
                outline: true,
                depths: 2..=6,
                breadcrumb: true,
                previous: true,
                next: true,
                view: true,
                edit: true,
                source: true,
                search: None,
            },
            html: String::new(),
            outline: Vec::new(),
            page_type,
            links: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// What `built` warns of, as it says it.
    fn warnings_of(built: &Built) -> Vec<String> {
        built.warnings.iter().map(|w| w.message.clone()).collect()
    }

    /// `path`, written with `/`, with the system's separators, as little
    /// names files in what it warns of.
    fn native(path: &str) -> PathBuf {
        path.split('/').collect()
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

    /// The URL of each page search found in the site built into `dist`.
    fn indexed_urls(dist: &Path) -> Vec<String> {
        let pages = indexed(dist).into_iter().map(|page| {
            let page: serde_json::Value = serde_json::from_str(&page).unwrap();
            page["url"].as_str().unwrap().to_string()
        });
        let mut urls: Vec<String> = pages.collect();
        urls.sort();
        urls
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

    /// The nav of the page at `url`, pruned from the nav in `site`, whose
    /// pages are at `pages`: its titles and URLs, nested like the nav.
    fn pruned_at(site: &Mapping, pages: &[&str], url: &str) -> Vec<String> {
        let pages = pages.iter().map(|page| page.to_string());
        let nav = PrunedNav::new(site, pages).unwrap().at(url);
        outline(&serde_yaml::from_value::<Vec<NavItem>>(Value::Sequence(nav)).unwrap())
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
    fn maps_urls_to_the_files_pages_are_built_into() {
        // A page is in a file named for it, and a folder's own page in the
        // folder's `index.html`.
        let dist = Path::new("dist");
        for (url, file) in [
            ("/", "dist/index.html"),
            ("/getting-started", "dist/getting-started.html"),
            ("/guide/", "dist/guide/index.html"),
            ("/guide/install", "dist/guide/install.html"),
            // Named in full, whatever dots it has.
            ("/releases/v1.2", "dist/releases/v1.2.html"),
            ("/guide/index", "dist/guide/index.html"),
        ] {
            assert_eq!(out_path_for_url(dist, url), PathBuf::from(file), "{url}");
        }

        // A redirect is in a folder, with or without the `/` at the end of
        // the URL it's from, or in the `.html` file it names.
        for (url, file) in [
            ("/", "index.html"),
            ("/guide/setup", "guide/setup/index.html"),
            ("/guide/setup/", "guide/setup/index.html"),
            ("/setup.html", "setup.html"),
        ] {
            assert_eq!(redirect_file(url), file, "{url}");
        }
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

        // A page named `index` is its folder's `index.html`, as its README is.
        assert_eq!(
            check("guide/README.md", "guide/index.md").unwrap_err(),
            "docs/guide/README.md and docs/guide/index.md would both be the page at /guide/: \
             rename or move one"
        );

        // A page beside a folder of its name is caught once every file the
        // site writes is known, and URLs that differ only in case aren't
        // caught, for now.
        assert!(check("guide.md", "guide/install.md").is_ok());
        assert!(check("API.md", "api.md").is_ok());
    }

    #[test]
    fn fallback_titles_use_folder_for_index() {
        assert_eq!(fallback_title(Path::new("README.md")), "Home");
        assert_eq!(fallback_title(Path::new("guide/README.md")), "Guide");
        assert_eq!(
            fallback_title(Path::new("getting-started.md")),
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
    fn a_pages_outline_nests_its_headings() {
        let md = "### Before\n\n### Also Before\n\n## One\n\n### One A\n\n#### One A i\n\n\
                  ### One B\n\n## Two\n\n#### Two i\n";
        let headings = render_at(md, "/", &SiteUrl::default(), &config::Markdown::default())
            .unwrap()
            .headings;

        // A heading goes under the nearest one above it with fewer `#`s, even
        // two levels up; before the first `##`, it goes at the top, beside the
        // others.
        assert_eq!(
            outline(&nest_headings(headings.iter())),
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
        // to the last place, where its `previous` and `next` come from too.
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
    fn a_pruned_nav_lists_only_the_folders_a_page_is_in() {
        let site: Mapping = serde_yaml::from_str(
            "navigation:\n  items:\n  \
             - { title: Home, url: / }\n  \
             - { title: Guide, url: /guide/, children: [\
                 { title: Install, url: /guide/install },\
                 { title: Advanced, url: /guide/advanced/, children: [\
                     { title: Tuning, url: /guide/advanced/tuning },\
                     { title: Deep, children: [\
                         { title: Limits, url: /guide/advanced/deep/limits }] }] },\
                 { title: Recipes, children: [\
                     { title: Bake, url: /guide/recipes/bake },\
                     { title: Roast, url: /guide/recipes/roast }] }] }\n  \
             - { title: Reference, children: [\
                 { title: Commands, url: /reference/commands },\
                 { title: Settings, url: /reference/settings }] }\n  \
             - { title: API, url: /api/, children: [{ title: Errors, url: /api/errors }] }\n",
        )
        .unwrap();
        let pages = [
            "/",
            "/guide/",
            "/guide/install",
            "/guide/advanced/",
            "/guide/advanced/tuning",
            "/guide/advanced/deep/limits",
            "/guide/recipes/bake",
            "/guide/recipes/roast",
            "/reference/commands",
            "/reference/settings",
            "/api/",
            "/api/errors",
        ];
        let at = |url: &str| pruned_at(&site, &pages, url);

        // Each folder a page is in lists what's in it, down to the page. Any
        // other is a link: to its own page, or, for a label, to the first
        // page in it.
        assert_eq!(
            at("/guide/advanced/deep/limits"),
            [
                "Home /",
                "Guide /guide/",
                "  Install /guide/install",
                "  Advanced /guide/advanced/",
                "    Tuning /guide/advanced/tuning",
                "    Deep (label)",
                "      Limits /guide/advanced/deep/limits",
                "  Recipes /guide/recipes/bake",
                "Reference /reference/commands",
                "API /api/",
            ]
        );
        // A folder's own page lists what's in it, but not what's in the
        // folders there.
        assert_eq!(
            at("/guide/"),
            [
                "Home /",
                "Guide /guide/",
                "  Install /guide/install",
                "  Advanced /guide/advanced/",
                "  Recipes /guide/recipes/bake",
                "Reference /reference/commands",
                "API /api/",
            ]
        );
        // A label is still one on the pages in it.
        assert_eq!(
            at("/reference/settings"),
            [
                "Home /",
                "Guide /guide/",
                "Reference (label)",
                "  Commands /reference/commands",
                "  Settings /reference/settings",
                "API /api/",
            ]
        );
        // The top of the nav is always whole: it's all a page at the top
        // gets, or one that isn't in the nav, like the 404 page.
        for url in ["/", "/404.html", "/drafts/notes"] {
            assert_eq!(
                at(url),
                [
                    "Home /",
                    "Guide /guide/",
                    "Reference /reference/commands",
                    "API /api/",
                ],
                "{url}"
            );
        }
    }

    #[test]
    fn a_pruned_nav_keeps_what_a_reader_couldnt_reach() {
        let site: Mapping = serde_yaml::from_str(
            "name: Docs\n\
             navigation:\n  prune: true\n  items:\n  \
             - { title: Home, url: / }\n  \
             - { title: Elsewhere, url: \"https://example.com/\", children: [\
                 { title: One, url: /one },\
                 { title: More, url: /more/, children: [{ title: Two, url: /more/two }] }] }\n  \
             - { title: Typo, url: /guide, children: [{ title: Install, url: /guide/install }] }\n  \
             - { title: Links, children: [\
                 { title: GitHub, url: \"https://github.com/withlittle/little\" }] }\n  \
             - { title: Tools, icon: wrench, url: , children: [\
                 { title: Old, children: [{ title: Site, url: \"https://example.com/old\" }] },\
                 { title: New, children: [{ title: Lint, url: /tools/lint }] }] }\n  \
             - { title: Guide, url: /guide/, icon: book, children: [\
                 { title: Setup, url: /guide/setup }] }\n\
             version: \"1.4\"\n",
        )
        .unwrap();
        let pages = [
            "/",
            "/one",
            "/more/",
            "/more/two",
            "/guide/",
            "/guide/install",
            "/guide/setup",
            "/tools/lint",
        ];

        // A folder whose link isn't to one of the site's pages keeps what's
        // in it, as does a label with no page in it: following either would
        // never show its items. The folders in them are still pruned. A
        // label's first page can be in a folder within it.
        assert_eq!(
            pruned_at(&site, &pages, "/"),
            [
                "Home /",
                "Elsewhere https://example.com/",
                "  One /one",
                "  More /more/",
                "Typo /guide",
                "  Install /guide/install",
                "Links (label)",
                "  GitHub https://github.com/withlittle/little",
                "Tools /tools/lint",
                "Guide /guide/",
            ]
        );
        assert_eq!(
            pruned_at(&site, &pages, "/more/two"),
            [
                "Home /",
                "Elsewhere https://example.com/",
                "  One /one",
                "  More /more/",
                "    Two /more/two",
                "Typo /guide",
                "  Install /guide/install",
                "Links (label)",
                "  GitHub https://github.com/withlittle/little",
                "Tools /tools/lint",
                "Guide /guide/",
            ]
        );

        // A folder keeps its other keys, like a theme's `icon`, and a label
        // gets a `url` in place of one left blank. The rest of `site` is as
        // it was, in its order, as is the rest of its `navigation`.
        let pruned = PrunedNav::new(&site, pages.iter().map(|page| page.to_string())).unwrap();
        let nav = pruned.at("/");
        let item = |index: usize| serde_yaml::to_string(&nav[index]).unwrap();
        assert_eq!(item(4), "title: Tools\nicon: wrench\nurl: /tools/lint\n");
        assert_eq!(item(5), "title: Guide\nurl: /guide/\nicon: book\n");
        let keys: Vec<_> = pruned.site.keys().filter_map(Value::as_str).collect();
        assert_eq!(keys, ["name", "navigation", "version"]);
        assert_eq!(pruned.site.get("version"), site.get("version"));
        let navigation = serde_yaml::to_string(&pruned.site["navigation"]).unwrap();
        assert_eq!(navigation, "prune: true\nitems: []\n");

        // A site with no nav has none to prune.
        for site in ["name: Docs\n", "navigation: { prune: true }\n"] {
            let site: Mapping = serde_yaml::from_str(site).unwrap();
            assert!(PrunedNav::new(&site, Vec::new()).is_none());
        }
    }

    #[test]
    fn a_pruned_nav_follows_a_page_to_the_last_place_its_listed() {
        let site: Mapping = serde_yaml::from_str(
            "navigation:\n  items:\n  \
             - { title: Tutorials, children: [\
                 { title: Setup, url: /guide/install },\
                 { title: First Steps, url: /getting-started }] }\n  \
             - { title: Guide, url: /guide/, children: [\
                 { title: Install, url: /guide/install }] }\n",
        )
        .unwrap();
        let pages = ["/guide/", "/guide/install", "/getting-started"];

        // As for its breadcrumbs, a page listed twice is in the folder it's
        // last listed in, so the other is a link, here to the page itself.
        assert_eq!(
            pruned_at(&site, &pages, "/guide/install"),
            [
                "Tutorials /guide/install",
                "Guide /guide/",
                "  Install /guide/install",
            ]
        );
        assert_eq!(
            pruned_at(&site, &pages, "/getting-started"),
            [
                "Tutorials (label)",
                "  Setup /guide/install",
                "  First Steps /getting-started",
                "Guide /guide/",
            ]
        );
    }

    #[test]
    fn builds_with_a_local_theme() {
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                "site:\n  theme:\n    source: ./path/to/my/theme\n",
            ),
            ("docs/README.md", "# Home\n"),
            (
                "path/to/my/theme/_templates/default.html",
                "{{ site.greeting }} from my theme",
            ),
            ("path/to/my/theme/little.yml", "site: { greeting: Hello }\n"),
            ("path/to/my/theme/_static/style.css", "body {}\n"),
        ]);
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
    }

    #[test]
    fn a_site_needs_only_its_docs() {
        // Without a little.yml or a theme, little's built-in theme builds it.
        let (_tmp, site) = scratch(&[("docs/README.md", "# Home\n")]);

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
    }

    #[test]
    fn a_site_can_turn_off_the_built_in_themes_search() {
        let (_tmp, site) = scratch(&[
            ("little.yml", "site: { search: { enabled: false } }\n"),
            ("docs/README.md", "# Home\n"),
        ]);

        build(&site).unwrap();

        let dist = site.join("dist");
        let home = fs::read_to_string(dist.join("index.html")).unwrap();
        assert!(!home.contains("pagefind"), "{home}");
        assert!(!dist.join("pagefind").exists());
    }

    #[test]
    fn a_site_sets_the_built_in_themes_look() {
        // The built-in theme's colours, fonts, and corners are its own,
        // which its template writes into each page for its stylesheet.
        let (_tmp, site) = scratch(&[("docs/README.md", "# Home\n")]);
        let home = || {
            build(&site).unwrap();
            fs::read_to_string(site.join("dist/index.html")).unwrap()
        };
        let page = home();
        assert!(page.contains("--primary: #7c3aed;"), "{page}");
        assert!(page.contains("--radius: 0.5rem;"), "{page}");
        // Headings are in the body's font, as the stylesheet has them.
        assert!(!page.contains("--font-heading"), "{page}");

        // A site sets what it likes, and the rest stay the theme's.
        let look = "site:\n  theme:\n    colors:\n      primary: \"#0d9488\"\n    fonts:\n      \
                    heading: \"'Source Serif 4', serif\"\n    radius: 0\n";
        write(&site, &[("little.yml", look)]);
        let page = home();
        assert!(page.contains("--primary: #0d9488;"), "{page}");
        assert!(page.contains("--info: #2f6fdb;"), "{page}");
        // A font's quotes are written as they are.
        assert!(
            page.contains("--font-heading: 'Source Serif 4', serif;"),
            "{page}"
        );
        // The stylesheet multiplies the radius, so 0 has a unit.
        assert!(page.contains("--radius: 0px;"), "{page}");
    }

    #[test]
    fn builds_with_a_theme_that_builds_on_another() {
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                "site:\n  theme:\n    source: ./company\n  name: My Docs\n",
            ),
            ("docs/README.md", "# Home\n"),
            (
                "company/little.yml",
                "site:\n  theme:\n    source: ../base\n  greeting: Hi\n",
            ),
            ("company/_templates/partials/brand.html", "company"),
            ("company/_static/logo.svg", "company logo"),
            ("base/little.yml", "site: { greeting: Hello, name: Base }\n"),
            (
                "base/_templates/default.html",
                r#"{% include "partials/brand.html" %}: {{ site.greeting }} from {{ site.name }}"#,
            ),
            ("base/_templates/partials/brand.html", "base"),
            ("base/_static/logo.svg", "base logo"),
            ("base/_static/style.css", "body {}\n"),
        ]);

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        assert_eq!(read("index.html"), "company: Hi from My Docs");
        assert_eq!(read("logo.svg"), "company logo");
        assert_eq!(read("style.css"), "body {}\n");
    }

    #[test]
    fn builds_with_a_theme_that_builds_on_a_downloaded_one() {
        let url = "https://example.invalid/fancy-1.0.zip";
        let company_cfg = format!("site:\n  theme:\n    source: {url}\n  greeting: Hi\n");
        let (_tmp, site) = scratch(&[
            ("little.yml", "site:\n  theme:\n    source: ./company\n"),
            ("docs/README.md", "# Home\n"),
            ("company/little.yml", &company_cfg),
            ("company/_static/logo.svg", "company logo"),
        ]);
        // Where it's kept once it's downloaded, so the test needn't download it.
        let fancy = crate::download::folder(&site, url);
        write(
            &fancy,
            &[
                ("little.yml", "site: { greeting: Hello }\n"),
                (
                    "_templates/default.html",
                    "{{ site.greeting }} from {{ page.title }}",
                ),
                ("_static/logo.svg", "fancy logo"),
                ("_static/style.css", "body {}\n"),
            ],
        );

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
    }

    #[test]
    fn a_build_needs_a_source_folder_and_a_theme() {
        let (_tmp, site) = scratch(&[("little.yml", "site: { name: My Docs }\n")]);
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "there's no source folder at {}",
            site.join("docs").display()
        );
        assert_eq!(message, expected);

        fs::create_dir_all(site.join("docs")).unwrap();
        let no_theme = "site: { theme: { enabled: false } }\n";
        fs::write(site.join("little.yml"), no_theme).unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected =
            "little can't build a site without a theme (`site.theme.enabled: false`) yet";
        assert_eq!(message, expected);
        assert!(!site.join("dist").exists());
    }

    #[test]
    fn builds_from_and_into_the_folders_the_config_chooses() {
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                "site:\n  theme:\n    source: ./theme\n  source:\n    directory: ./content\n  \
                 destination: ./public\n",
            ),
            ("content/README.md", "# Home\n"),
            ("content/guide/install.md", "# Install\n"),
            ("docs/README.md", "# Not the source\n"),
            ("theme/_templates/default.html", "{{ page.title }}"),
        ]);

        let destination = build(&site).unwrap().destination;

        assert_eq!(destination, site.join("public"));
        let read = |path: &str| fs::read_to_string(destination.join(path)).unwrap();
        assert_eq!(read("index.html"), "Home");
        assert_eq!(read("guide/install.html"), "Install");
        assert!(!site.join("dist").exists());
    }

    #[test]
    fn hidden_files_and_folders_are_left_out() {
        // Like Jupyter's checkpoints, an editor's swap files, or drafts.
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
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
                     {% endfor %}{% endmacro %}{{ list(site.navigation.items) }}",
            ),
            ("theme/_static/.nojekyll", ""),
        ]);

        build(&site).unwrap();

        // They aren't pages or copied, so they aren't in the nav either. A
        // theme's static files are copied whole, hidden ones too.
        let dist = site.join("dist");
        let built = [
            MARKER_FILE,
            ".nojekyll",
            "README.md",
            "guide/README.md",
            "guide/index.html",
            "guide/install.html",
            "guide/install.md",
            "index.html",
        ];
        assert_eq!(files_in(&dist), built);
        let home = fs::read_to_string(dist.join("index.html")).unwrap();
        assert_eq!(home, "[Home][Guide [Install]]");
    }

    #[test]
    fn only_hidden_names_inside_the_source_count() {
        // So the source can be a hidden folder, and the site can be in one.
        let (_tmp, root) = scratch(&[
            (
                ".site/little.yml",
                "site:\n  theme:\n    source: ./theme\n  source:\n    directory: ./.github\n",
            ),
            (".site/README.md", "# Site\n"),
            (".site/.github/README.md", "# Home\n"),
            (".site/.github/ISSUE_TEMPLATE/bug.md", "# Bug\n"),
            (".site/.github/.drafts/wip.md", "# WIP\n"),
            (".site/.little/themes/fancy/README.md", "# Fancy\n"),
            (".site/theme/_templates/default.html", "{{ page.title }}"),
        ]);
        let site = root.join(".site");
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();

        build(&site).unwrap();

        let built = [
            MARKER_FILE,
            "ISSUE_TEMPLATE/bug.html",
            "ISSUE_TEMPLATE/bug.md",
            "README.md",
            "index.html",
        ];
        assert_eq!(files_in(&dist), built);
        assert_eq!(read("index.html"), "Home");

        // With the site's folder as its source, what's in `.github/` and
        // `.little/` isn't a page. Everything else is published, its config
        // and theme too, but for the destination, which a build never copies
        // into itself.
        let site_cfg = format!("{IN_THEME}  source:\n    directory: .\n");
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();

        let built = [
            MARKER_FILE,
            "README.md",
            "index.html",
            "little.yml",
            "theme/_templates/default.html",
        ];
        assert_eq!(files_in(&dist), built);
        assert_eq!(read("index.html"), "Site");

        // Its `exclude` leaves the rest out.
        let site_cfg =
            format!("{IN_THEME}  source:\n    directory: .\n    exclude: [/little.yml, /theme/]\n");
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();
        assert_eq!(files_in(&dist), [MARKER_FILE, "README.md", "index.html"]);
    }

    #[test]
    fn the_files_a_source_excludes_are_left_out() {
        let site_cfg = format!(
            "{IN_THEME}  source:\n    exclude:\n      - /drafts/\n      - /guide/old.md\n      \
             - /images/plan.png\n      - /notes\n"
        );
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg),
            ("docs/README.md", "# Home\n"),
            ("docs/drafts/README.md", "# Drafts\n"),
            ("docs/drafts/2026/plan.md", "# Plan\n"),
            ("docs/drafts/sketch.png", "png"),
            ("docs/drafts.md", "# About drafts\n"),
            ("docs/guide/README.md", "# Guide\n[Old](old.md)\n"),
            ("docs/guide/old.md", "# Old\n"),
            ("docs/guide/old.md.txt", "kept\n"),
            ("docs/guide/install.md", "# Install\n"),
            ("docs/images/plan.png", "png"),
            ("docs/images/logo.png", "png"),
            ("docs/notes/today.md", "# Today\n"),
            (
                "theme/_templates/default.html",
                "{% macro list(items) %}{% for item in items %}\
                     [{{ item.title }}{% if item.children %} {{ list(item.children) }}{% endif %}]\
                     {% endfor %}{% endmacro %}{{ list(site.navigation.items) }}",
            ),
        ]);

        let built = build(&site).unwrap();

        // They aren't pages or copied, nor is a page's own file, so they
        // aren't in the nav either: a file at each path, and all that's in
        // a folder there, with the `/` at its end or without. A file whose
        // path only starts as one does stays.
        let dist = site.join("dist");
        let kept = [
            MARKER_FILE,
            "README.md",
            "drafts.html",
            "drafts.md",
            "guide/README.md",
            "guide/index.html",
            "guide/install.html",
            "guide/install.md",
            "guide/old.md.txt",
            "images/logo.png",
            "index.html",
        ];
        assert_eq!(files_in(&dist), kept);
        let home = fs::read_to_string(dist.join("index.html")).unwrap();
        assert_eq!(home, "[Home][About drafts][Guide [Install]]");
        // Each left something out, so there's nothing to warn of.
        assert_eq!(warnings_of(&built), Vec::<String>::new());
    }

    #[test]
    fn a_path_a_source_excludes_that_leaves_nothing_out_is_warned_of() {
        let site_cfg = format!(
            "{IN_THEME}  source:\n    \
             exclude: [/drafts/, /Guide/, /typo.md, /.hidden/, /drafts/wip.md]\n"
        );
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg),
            ("docs/README.md", "# Home\n"),
            ("docs/drafts/wip.md", "# WIP\n"),
            ("docs/guide/README.md", "# Guide\n"),
            ("docs/.hidden/secret.md", "# Secret\n"),
            ("theme/_templates/default.html", "{{ page.title }}"),
        ]);

        let built = build(&site).unwrap();

        // A path is matched as it's written, so `/Guide/` leaves `guide/`
        // in, and one that leaves nothing out may be a typo: there's no
        // file there, or it's left out already, as a hidden one is, or one
        // in a folder that's listed.
        let dist = site.join("dist");
        assert!(dist.join("guide/index.html").is_file());
        assert!(!dist.join("drafts").exists());
        let warning = |path: &str| {
            format!(
                "`site.source.exclude` lists {path}, but the source has no file at that path, \
                 or under it, to leave out"
            )
        };
        assert_eq!(
            warnings_of(&built),
            [
                warning("/Guide/"),
                warning("/typo.md"),
                warning("/.hidden/"),
                warning("/drafts/wip.md"),
            ]
        );
    }

    #[test]
    fn a_build_only_wipes_a_folder_it_built() {
        let files = [
            ("site/docs/README.md", "# Home\n"),
            ("site/theme/_templates/default.html", "{{ page.title }}"),
            ("site/theme/_static/style.css", "body {}\n"),
            ("other/notes.txt", "mine\n"),
        ];
        let (_tmp, root) = scratch(&files);
        let site = root.join("site");
        let build_into = |destination: &str| {
            let site_cfg = format!("{IN_THEME}  destination: {destination}\n");
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
    }

    #[test]
    fn every_other_file_in_the_source_is_copied() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/logo.svg", "<svg/>"),
            ("docs/guide/install.md", "# Install\n"),
            ("docs/guide/videos/demo.mp4", "mp4"),
            ("docs/assets/style.css", "site"),
            ("theme/_templates/default.html", "{{ page.title }}"),
            ("theme/_static/assets/style.css", "theme"),
            ("theme/_static/assets/theme.js", "theme"),
        ]);
        let png = [
            0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0x00, 0xff,
        ];
        let image = site.join("docs/guide/images/step 1.png");
        fs::create_dir_all(image.parent().unwrap()).unwrap();
        fs::write(&image, png).unwrap();

        build(&site).unwrap();

        // To the same path, beside the pages, and each page's own file.
        let dist = site.join("dist");
        let built = [
            MARKER_FILE,
            "README.md",
            "assets/style.css",
            "assets/theme.js",
            "guide/images/step 1.png",
            "guide/install.html",
            "guide/install.md",
            "guide/videos/demo.mp4",
            "index.html",
            "logo.svg",
        ];
        assert_eq!(files_in(&dist), built);
        assert_eq!(fs::read(dist.join("guide/images/step 1.png")).unwrap(), png);
        // Where the theme has a file at the same path, the site's wins.
        let style = fs::read_to_string(dist.join("assets/style.css")).unwrap();
        assert_eq!(style, "site");
    }

    #[test]
    fn sass_in_the_theme_is_compiled_and_in_the_source_copied() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/examples/button.scss", "$x: 1;"),
            ("docs/examples/_mixins.scss", "$y: 2;"),
            ("theme/_templates/default.html", "{{ page.title }}"),
            (
                "theme/_static/assets/style.scss",
                "@use 'colors';\na { color: colors.$link; }",
            ),
            ("theme/_sass/_colors.scss", "$link: teal;"),
        ]);
        build(&site).unwrap();

        let dist = site.join("dist");
        let built = [
            MARKER_FILE,
            "README.md",
            "assets/style.css",
            "examples/_mixins.scss",
            "examples/button.scss",
            "index.html",
        ];
        assert_eq!(files_in(&dist), built);
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
    }

    #[test]
    fn a_destination_in_the_source_isnt_copied_into_itself() {
        let (_tmp, site) = scratch(&[
            ("docs/README.md", "# Home\n"),
            ("docs/logo.svg", "<svg/>"),
            ("theme/_templates/default.html", "{{ page.title }}"),
        ]);
        // However its path is written. The second build finds the first's.
        for destination in ["./docs/_build", "./docs/../docs/_build"] {
            let site_cfg = format!("{IN_THEME}  destination: {destination}\n");
            fs::write(site.join("little.yml"), site_cfg).unwrap();
            build(&site).unwrap();
            build(&site).unwrap();
            let built = [MARKER_FILE, "README.md", "index.html", "logo.svg"];
            let files = files_in(&site.join("docs/_build"));
            assert_eq!(files, built, "{destination}");
        }
    }

    #[test]
    fn links_lead_from_the_folder_of_the_file_theyre_in() {
        // As on GitHub, whatever URL the page is served at, with or without a
        // `/` at its end, and under the site's path.
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                "site:\n  theme:\n    source: ./theme\n  \
                 url: https://withlittle.github.io/little\n",
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
        ]);

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
            read("guide/install.html"),
            format!("<p>{diagram} {logo}</p>\n")
        );
        // And each image is where it leads.
        for image in ["assets/logo.svg", "guide/images/diagram.svg"] {
            assert!(dist.join(image).is_file(), "{image}");
        }
    }

    /// A site's config that has the test plugin, as `test`, reading the
    /// files with the extension `test`.
    fn reading_tests() -> String {
        format!(
            "{IN_THEME}  plugins:\n    test:\n      wasm: {}\n      extensions: [test]\n",
            crate::plugin::tests::test_plugin().display()
        )
    }

    #[test]
    fn the_files_a_plugin_reads_are_pages() {
        let parts = |parts: serde_json::Value| parts.to_string();
        let analysis = parts(serde_json::json!([
            {
                "markdown": "---\ntitle: The analysis\n---\n# Analysis\n\n## Results\n\n\
                             [Home](../README.md)",
            },
            { "code": { "language": "python", "text": "print(6 * 7)" } },
            { "html": "<p class=\"table\">A table</p>" },
            "given",
        ]));
        let data = parts(serde_json::json!([{ "markdown": "What we have." }]));
        let (_tmp, site) = scratch(&[
            ("little.yml", &reading_tests()),
            (
                "docs/README.md",
                "[Analysis](guide/analysis.test#results)\n",
            ),
            ("docs/guide/analysis.test", &analysis),
            ("docs/data/README.test", &data),
            // Its extension, as it's written, is the plugin's.
            ("docs/data/notes.TEST", "Notes"),
            (
                "theme/_templates/default.html",
                "{{ page.title }}\n{{ page.content | safe }}",
            ),
        ]);
        let built = build(&site).unwrap();

        // Each is built into its page, as a Markdown file is, and its file
        // is copied beside it, as a Markdown file's is.
        let dist = site.join("dist");
        let files = [
            MARKER_FILE,
            "README.md",
            "data/README.test",
            "data/index.html",
            "data/notes.TEST",
            "guide/analysis.html",
            "guide/analysis.test",
            "index.html",
        ];
        assert_eq!(files_in(&dist), files);
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();
        assert_eq!(read("data/README.test"), data);
        let analysis = read("guide/analysis.html");
        assert!(analysis.starts_with("The analysis\n<h1"), "{analysis}");
        assert!(analysis.contains("<a href=\"/\">Home</a>"), "{analysis}");
        let code = "<pre class=\"highlight\"><code class=\"language-python\">";
        assert!(analysis.contains(code), "{analysis}");
        let html = "</code></pre>\n<p class=\"table\">A table</p>\n";
        assert!(analysis.ends_with(html), "{analysis}");
        assert_eq!(read("data/index.html"), "Data\n<p>What we have.</p>\n");
        // A link to its file leads to its page.
        let home = read("index.html");
        let link = "<a href=\"/guide/analysis#results\">Analysis</a>";
        assert!(home.contains(link), "{home}");
        // The plugin is given the page's file, and the site's `markdown`
        // settings, as `little config` prints them.
        let warnings = warnings_of(&built);
        let given = warnings[0].strip_prefix("plugin `test`: docs/guide/analysis.test ");
        let given: serde_json::Value = serde_json::from_str(given.unwrap()).unwrap();
        assert_eq!(given["raw_html"], true);
        assert_eq!(given["highlight"], "pygments");
        assert_eq!(warnings.len(), 1, "{warnings:?}");

        // It can be the same page as a Markdown file.
        let markdown = site.join(native("docs/guide/analysis.md"));
        fs::write(&markdown, "# Analysis\n").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "{} and {} would both be the page at /guide/analysis: rename or move one",
            markdown.display(),
            site.join(native("docs/guide/analysis.test")).display()
        );
        assert_eq!(message, expected);
        fs::remove_file(&markdown).unwrap();

        // One the plugin can't read stops the build, naming it.
        let broken = site.join(native("docs/broken.test"));
        fs::write(&broken, "[").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "rendering {}: plugin `test`'s `formats` hook failed: EOF while parsing a list at \
             line 1 column 1",
            broken.display()
        );
        assert_eq!(message, expected);
    }

    #[test]
    fn each_formula_diagram_and_block_that_fails_is_noted() {
        // Each case has the home page's Markdown, and that of a file the
        // plugin reads. Then a page that works, and how it starts. Then
        // what's noted of each file, with `{}` for the file.
        let cases = [
            // The math in a file a plugin reads is MathML, as a Markdown
            // file's is. Each formula that isn't is noted, with the file it's
            // in, in the order of the pages.
            (
                "$$\\qty{1}{m}$$\n",
                "$x^2$ and $\\dv{f}{x}$",
                "analysis.html",
                "<p><math><msup>",
                [
                    "can't render $$\\qty{1}{m}$$ in {}: Unknown command \"\\qty\".",
                    "can't render $\\dv{f}{x}$ in {}: Unknown command \"\\dv\".",
                ],
            ),
            // Each diagram that isn't drawn is noted, with the file it's in,
            // and the line its block starts on in a Markdown file, in the
            // order of the pages.
            (
                "---\ntitle: Home\n---\n```mermaid\nflowchart LR\n  A --> B\n```\n\n\
                 ```mermaid\nflowchat LR\n```\n",
                "```mermaid\nflowchat\n```",
                "index.html",
                "<svg id=\"mermaid-1\" ",
                [
                    "can't draw the diagram on line 9 of {}: No diagram type detected",
                    "can't draw a diagram in {}: No diagram type detected",
                ],
            ),
            // A plugin is told the file of each block's page from the site's
            // folder, wherever the site is. Each block it can't render is
            // noted, with the file it's in, and the line it starts on in a
            // Markdown file, in the order of the pages.
            (
                "---\ntitle: Home\n---\n```test\nblock\n```\n\n```test\nerror it is broken\n```\n",
                "```test\nerror it is lost\n```",
                "index.html",
                "<p>test | test | docs/README.md</p>\n",
                [
                    "plugin `test` can't render the block on line 8 of {}: it is broken",
                    "plugin `test` can't render a block in {}: it is lost",
                ],
            ),
        ];
        for (readme, analysis, page, start, noted) in cases {
            let parts = serde_json::json!([{ "markdown": analysis }]);
            let (_tmp, site) = scratch(&[
                ("little.yml", &reading_tests()),
                ("docs/README.md", readme),
                ("docs/analysis.test", &parts.to_string()),
                ("theme/_templates/default.html", "{{ page.content | safe }}"),
            ]);
            let built = build(&site).unwrap();

            let html = fs::read_to_string(site.join("dist").join(page)).unwrap();
            assert!(html.starts_with(start), "{html}");
            let docs = site.join("docs");
            let files = [docs.join("README.md"), docs.join("analysis.test")];
            let expected: Vec<String> = noted
                .iter()
                .zip(files)
                .map(|(note, file)| note.replace("{}", &file.display().to_string()))
                .collect();
            assert_eq!(warnings_of(&built), expected);
        }
    }

    /// A site's config that has the test plugin, as `api`, beside `config`,
    /// which holds its `site`'s other settings, adding the files `add` lists
    /// to the source, and warning of `warn`.
    fn adding_files(config: &str, add: serde_json::Value, warn: Option<&str>) -> String {
        let mut plugin = serde_json::json!({
            "wasm": crate::plugin::tests::test_plugin(),
            "files": { "add": add },
        });
        if let Some(warn) = warn {
            plugin["warn"] = warn.into();
        }
        format!("{config}  plugins:\n    api: {plugin}\n")
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
            &format!("{IN_THEME}  repository:\n    url: https://github.com/withlittle/little\n"),
            pages,
            Some("can't read pkg/broken.py"),
        );
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg),
            ("docs/README.md", "# Home\n"),
            (
                "docs/guide.md",
                "# Guide\n\nSee [the API](reference/README.md).\n",
            ),
            (
                "theme/_templates/default.html",
                "{{ page.title }} | {{ 'url' in page.repository.view }} | \
                     {{ page.breadcrumb.items | map(attribute='title') | join(', ') }} | \
                     {{ page.previous.title if page.previous }} | {{ page.next.title if page.next }}\n\
                     {{ page.content | safe }}",
            ),
        ]);
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
        let base = read("reference/pkg/base.html");
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
        let guide = read("guide.html");
        assert!(
            guide.starts_with("Guide | True | Home | Home | Reference\n"),
            "{guide}"
        );

        // What it warns of is noted, and its links are checked, naming it.
        let warnings = warnings_of(&built);
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
    }

    #[test]
    fn a_plugins_page_cant_be_the_same_page_as_another() {
        let clash = |files: &[(&str, &str)], pages: serde_json::Value| {
            let site_cfg = adding_files(IN_THEME, pages, None);
            let mut files = files.to_vec();
            files.push(("little.yml", &site_cfg));
            files.push(("theme/_templates/default.html", "{{ page.content | safe }}"));
            let (_tmp, site) = scratch(&files);
            let error = format!("{:#}", build(&site).err().unwrap());
            error.replace(&site.display().to_string(), ".")
        };
        let page = |path: &str| serde_json::json!({ "path": path, "text": "# API\n" });
        assert_eq!(
            clash(
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
            "config": { "merge": { "site": { "title": "Hooked" } } },
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
            format!("{IN_THEME}  url: https://example.com/docs\n  plugins:\n    test: {plugin}\n");
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg),
            ("docs/README.md", "# Home\n\nHello, world.\n"),
            ("docs/old.md", "# Old\n"),
            ("theme/little.yml", "site: { search: { enabled: true } }\n"),
            (
                "theme/_templates/default.html",
                "<html lang=\"en\">{{ site.title }} | {{ site.motto }} | {{ page.badge }} | \
                     {{ page.title }}\n{{ page.content | safe }}</html>",
            ),
        ]);
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
        let added = read("added.html");
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
        // build writes, and it's noted as the plugin's. The home page's
        // file is among them, copied as it's written, whatever `source`
        // made of its text. The page `files` added has no file to copy.
        let files = read("files.txt");
        let paths: Vec<_> = files.lines().collect();
        let first = ["README.md", "added.html", "index.html"];
        assert_eq!(paths[..3], first, "{files}");
        assert!(paths.contains(&"pagefind/pagefind.js"), "{files}");
        assert!(
            paths[3..].iter().all(|path| path.starts_with("pagefind/")),
            "{files}"
        );
        assert_eq!(read("README.md"), "# Home\n\nHello, world.\n");
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
        assert!(built.warnings.is_empty(), "{:?}", built.warnings);
    }

    #[test]
    fn a_hook_that_fails_is_warned_of_or_stops_the_build() {
        let (_tmp, site) = scratch(&[
            ("docs/README.md", "# Home\n\nHello, world.\n"),
            ("theme/_templates/default.html", "{{ page.content | safe }}"),
        ]);
        let with = |mut plugin: serde_json::Value| {
            plugin["wasm"] = serde_json::json!(crate::plugin::tests::test_plugin());
            let site_cfg = format!("{IN_THEME}  plugins:\n    test: {plugin}\n");
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
        let warnings = warnings_of(&built);
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
        let nowhere = serde_json::json!({ "navigation": { "items": "nowhere" } });
        let message = error(serde_json::json!({ "site": { "merge": nowhere } }));
        assert!(
            message.starts_with(
                "plugin `test`'s `site` hook gives back a `navigation.items` that isn't a list \
                 of links, each `{ title, url, children }`: "
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
    }

    #[test]
    fn each_type_of_page_gets_its_template_and_data() {
        // Each template says which it is, and what `page` holds.
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
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
        ]);

        build(&site).unwrap();

        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();
        // The first page has no `previous`, and the last no `next`. Each
        // other group is on every page.
        let groups = "outline repository search source title type url";
        assert_eq!(
            read("index.html"),
            format!("home with default.html: breadcrumb children content next {groups}")
        );
        assert_eq!(
            read("guide/index.html"),
            "section with section.html: breadcrumb children content description next \
             outline previous repository search source title type url"
        );
        assert_eq!(
            read("guide/install.html"),
            "page with default.html: breadcrumb content outline previous repository search \
             source title type url"
        );
        // A folder without a README gets no page of its own.
        assert!(!dist.join("api/index.html").exists());
        assert_eq!(
            read("api/reference.html"),
            "page with default.html: breadcrumb content next outline previous repository \
             search source title type url"
        );
        // A 404 page has no content of its own, but a title and a
        // description, from `site.error`, and each group, for its template.
        assert_eq!(
            read(NOT_FOUND_FILE),
            format!("404 with 404.html: breadcrumb description {groups} = /404.html")
        );
    }

    #[test]
    fn every_page_gets_the_settings_under_page() {
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                "site:\n  theme:\n    source: ./theme\npage:\n  author: Site\n  \
                 hero:\n    image: /site.png\n  breadcrumb:\n    enabled: false\n",
            ),
            ("docs/README.md", "# Home\n"),
            ("docs/api/errors.md", "# Errors\n"),
            (
                "docs/guide.md",
                "---\nauthor: Ada\nhero:\n  alt: A guide\nbreadcrumb:\n  enabled: true\n\
                 ---\n# Guide\n",
            ),
            (
                "theme/little.yml",
                "page:\n  author: Theme\n  layout: wide\n",
            ),
            (
                "theme/_templates/default.html",
                "{{ page.title }}: {{ page.author }}, {{ page.layout }}, \
                 {{ page.hero.image | safe }}, {{ page.hero.alt }}, \
                 {{ page.breadcrumb.items | length }} above",
            ),
            (
                "theme/_templates/404.html",
                "{{ page.title }}: {{ page.author }}, {{ page.layout }}, \
                 {{ page.breadcrumb.items | length }} above",
            ),
        ]);

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        // A key under `page`, in the theme's little.yml or the site's, is in
        // every page's `page`, as if each page's frontmatter set it, so here
        // no page has breadcrumbs.
        assert_eq!(
            read("api/errors.html"),
            "Errors: Site, wide, /site.png, , 0 above"
        );
        // A page's own frontmatter goes over it, key by key.
        assert_eq!(
            read("guide.html"),
            "Guide: Ada, wide, /site.png, A guide, 1 above"
        );
        // The 404 page has them too, with its title, from `site.error`.
        assert_eq!(read(NOT_FOUND_FILE), "Page not found: Site, wide, 0 above");
    }

    #[test]
    fn the_404_page_has_the_title_and_description_the_site_sets() {
        let (_tmp, site) = scratch(&[("docs/README.md", "# Home\n")]);
        let not_found = || {
            build(&site).unwrap();
            fs::read_to_string(site.join("dist").join(NOT_FOUND_FILE)).unwrap()
        };

        // little's own, as the built-in theme shows them...
        let page = not_found();
        for shown in [
            "<title>Page not found · Documentation</title>",
            "<h1>Page not found</h1>",
            "<p>The page you are looking for could not be found.</p>",
        ] {
            assert!(page.contains(shown), "{shown} in {page}");
        }
        // ...or the site's.
        let site_cfg = "site:\n  error:\n    title: Lost?\n    description: Try the search box.\n";
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        let page = not_found();
        for shown in [
            "<title>Lost? · Documentation</title>",
            r#"<meta name="description" content="Try the search box.">"#,
            "<h1>Lost?</h1>",
            "<p>Try the search box.</p>",
        ] {
            assert!(page.contains(shown), "{shown} in {page}");
        }
    }

    #[test]
    fn each_redirect_is_a_page_the_theme_writes_where_its_from() {
        let site_cfg = format!(
            "{IN_THEME}  url: https://example.com/proj\n  search:\n    enabled: true\n  \
             redirects:\n    - to: /guide/install#from-source\n      \
             from: [/guide/setup, /setup.html, /Old Guide/]\n    \
             - {{ to: /files/guide.pdf, from: [/guide.pdf] }}\n    \
             - {{ to: \"https://example.org/blog?tag=new\", from: [/news/] }}\n\
             page:\n  author: Site\n"
        );
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg),
            ("docs/README.md", "# Home\n"),
            (
                "docs/guide/install.md",
                "---\ntitle: Installing\n---\n# Install\n\n## From source\n",
            ),
            ("docs/files/guide.pdf", "pdf"),
            (
                "theme/_templates/default.html",
                "<html lang=\"en\">{{ page.title }}: {% for item in site.navigation.items recursive %}\
                 {{ item.title }} {{ loop(item.children) }}{% endfor %}</html>",
            ),
            (
                "theme/_templates/redirect.html",
                "{{ page.type }}: {{ page | list | sort | join(' ') }} = {{ page.url | safe }} to \
                 {{ page.redirect.url | safe }} ({{ page.redirect.title }}), by {{ page.author }}, \
                 {{ page.search.enabled }}",
            ),
        ]);
        let built = build(&site).unwrap();
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();

        // Each is written in a folder at its URL, which a host serves with
        // or without the URL's `/`, or, for a URL that ends in `.html`, at
        // that file's own path. Its `page` has
        // where it leads, with the site's path, and the title of the page
        // there, and what the 404 page has: each group, with nothing in it,
        // and every page's settings.
        let redirect = |from: &str, to: &str| {
            format!(
                "redirect: author breadcrumb outline redirect repository search source type url \
                 = {from} to {to}, by Site, False"
            )
        };
        let install = "/proj/guide/install#from-source (Installing)";
        for (file, from) in [
            ("guide/setup/index.html", "/proj/guide/setup"),
            ("setup.html", "/proj/setup.html"),
            ("Old Guide/index.html", "/proj/Old Guide/"),
        ] {
            assert_eq!(read(file), redirect(from, install), "{file}");
        }
        // One can lead to a file, or to another site, as it's written.
        assert_eq!(
            read("guide.pdf/index.html"),
            redirect("/proj/guide.pdf", "/proj/files/guide.pdf ()")
        );
        assert_eq!(
            read("news/index.html"),
            redirect("/proj/news/", "https://example.org/blog?tag=new ()")
        );

        // It's in no nav, search doesn't index it, and it's not counted
        // among the site's pages, but its URL is known, for a link to it.
        assert_eq!(
            read("index.html"),
            "<html lang=\"en\">Home: Home Guide Installing </html>"
        );
        assert_eq!(indexed_urls(&dist), ["/", "/guide/install"]);
        let setup = Path::new("guide/setup/index.html");
        assert_eq!(built.urls[setup], "/guide/setup");
        assert_eq!(built.origins[setup], "the redirect from /guide/setup");
        assert_eq!(warnings_of(&built), Vec::<String>::new());
    }

    #[test]
    fn a_redirect_leads_somewhere_from_a_url_of_its_own() {
        let (_tmp, site) = scratch(&[
            ("docs/README.md", "# Home\n"),
            ("docs/guide/README.md", "# Guide\n"),
            ("docs/new.md", "# New\n"),
            ("docs/old.html", "<p>Old</p>"),
            ("docs/files/guide.pdf", "pdf"),
            ("theme/_templates/default.html", "{{ page.title }}"),
            (
                "theme/_templates/redirect.html",
                "to {{ page.redirect.url | safe }}",
            ),
        ]);
        let redirects = |redirects: &str| {
            let site_cfg = format!("{IN_THEME}  redirects:\n{redirects}");
            fs::write(site.join("little.yml"), site_cfg).unwrap();
            build(&site).map(drop).map_err(|e| format!("{e:#}"))
        };
        let dist = site.join("dist");
        let docs = site.join("docs");

        // It leads to a page, a folder's with or without the `/` at its end,
        // or to a file, from a URL nothing is at, even beside the files in a
        // folder.
        let ok = redirects(
            "    - { to: /guide, from: [/a] }\n    - { to: /guide/, from: [/b] }\n    \
             - { to: /files/guide.pdf, from: [/files] }\n    - { to: /old.html, from: [/d] }\n    \
             - { to: /, from: [/e/f.html] }\n",
        );
        assert_eq!(ok, Ok(()));
        for (file, to) in [
            ("a/index.html", "/guide"),
            ("b/index.html", "/guide/"),
            ("files/index.html", "/files/guide.pdf"),
            ("d/index.html", "/old.html"),
            ("e/f.html", "/"),
        ] {
            let page = fs::read_to_string(dist.join(file)).unwrap();
            assert_eq!(page, format!("to {to}"), "{file}");
        }

        // One that leads nowhere in the site stops the build...
        for (to, target) in [
            ("/guides", "/guides"),
            ("/guide/install", "/guide/install"),
            ("/guide.md", "/guide.md"),
            ("/files/", "/files/"),
            ("\"/guides/#usage\"", "/guides/"),
        ] {
            let nowhere = redirects(&format!("    - {{ to: {to}, from: [/a] }}\n"));
            let to = to.trim_matches('"');
            let expected = format!(
                "`site.redirects` sends /a to {to}, but there's no page or file at {target}"
            );
            assert_eq!(nowhere, Err(expected), "{to}");
        }
        // ...as does one to a page with a `/` at the end of its URL, where a
        // host finds nothing, and the error says where the page is...
        assert_eq!(
            redirects("    - { to: \"/new/#usage\", from: [/a] }\n"),
            Err(
                "`site.redirects` sends /a to /new/#usage, but there's no page or file at \
                 /new/: the page is at /new, with no `/` at its end"
                    .to_string()
            )
        );
        // ...and so does one from where a page or a file is, or would be in
        // one's way, as for two files that would be one page...
        let page = |file: &str| {
            let file = docs.join(native(file));
            format!("the page built from {}", file.display())
        };
        let file = |file: &str| docs.join(native(file)).display().to_string();
        for (from, clash) in [
            (
                "/",
                format!("the redirect from / clashes with {}", page("README.md")),
            ),
            (
                "/guide",
                format!(
                    "the redirect from /guide clashes with {}",
                    page("guide/README.md")
                ),
            ),
            (
                "/guide/",
                format!(
                    "the redirect from /guide/ clashes with {}",
                    page("guide/README.md")
                ),
            ),
            (
                "/old.html",
                format!(
                    "{} clashes with the redirect from /old.html",
                    file("old.html")
                ),
            ),
            (
                "/files/guide.pdf",
                format!(
                    "{} clashes with the redirect from /files/guide.pdf",
                    file("files/guide.pdf")
                ),
            ),
        ] {
            let clashed = redirects(&format!("    - {{ to: /new, from: [/a, {from}] }}\n"));
            assert_eq!(clashed, Err(clash), "{from}");
        }
        // A page that isn't a folder's is in a file of its own, beside the
        // folder a redirect from its URL would be in, which clashes too.
        for from in ["/new", "/new/"] {
            let clashed = redirects(&format!("    - {{ to: /guide/, from: [{from}] }}\n"));
            let clash = format!("the redirect from {from} clashes with {}", page("new.md"));
            assert_eq!(clashed, Err(clash), "{from}");
        }
        // ...or two from one URL.
        for (twice, clash) in [
            (
                "[/a, /a]",
                "the redirect from /a clashes with the redirect from /a",
            ),
            (
                "[/a, /a/]",
                "the redirect from /a/ clashes with the redirect from /a",
            ),
            (
                "[/a, /a/index.html]",
                "the redirect from /a/index.html clashes with the redirect from /a",
            ),
        ] {
            let clashed = redirects(&format!("    - {{ to: /new, from: {twice} }}\n"));
            assert_eq!(clashed, Err(clash.to_string()), "{twice}");
        }

        // The last build is left in place.
        assert_eq!(
            fs::read_to_string(dist.join("a/index.html")).unwrap(),
            "to /guide"
        );
    }

    #[test]
    fn a_redirect_from_every_page_under_a_folder_leads_to_each_page_there_now() {
        let (_tmp, site) = scratch(&[
            ("docs/README.md", "# Home\n"),
            ("docs/tools/README.md", "# Tools\n"),
            ("docs/tools/this.md", "# This\n"),
            ("docs/tools/deep/that.md", "# That\n"),
            ("docs/tools/kept.md", "# Moved\n"),
            ("docs/tools/data.csv", "a,b\n"),
            ("docs/widgets/kept.md", "# Kept\n"),
            ("docs/toolshed.md", "# Shed\n"),
            ("theme/_templates/default.html", "{{ page.title }}"),
            (
                "theme/_templates/redirect.html",
                "{{ page.url | safe }} to {{ page.redirect.url | safe }} ({{ page.redirect.title }})",
            ),
        ]);
        let redirects = |redirects: &str| {
            let site_cfg =
                format!("{IN_THEME}  url: https://example.com/proj\n  redirects:\n{redirects}");
            fs::write(site.join("little.yml"), site_cfg).unwrap();
            build(&site).map_err(|e| format!("{e:#}"))
        };
        let built = redirects(
            "    - { to: /tools/*, from: [/widgets/*, /gadgets/*] }\n    \
             - { to: /tools/this, from: [/gadgets/deep/that] }\n",
        )
        .unwrap();
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();

        // Each page under the folder it leads to has a redirect, where the
        // page would be under each folder it's from: the folder's own page,
        // and those in its folders, but not its other files.
        for (file, sends) in [
            (
                "widgets/index.html",
                "/proj/widgets/ to /proj/tools/ (Tools)",
            ),
            (
                "widgets/this/index.html",
                "/proj/widgets/this to /proj/tools/this (This)",
            ),
            (
                "widgets/deep/that/index.html",
                "/proj/widgets/deep/that to /proj/tools/deep/that (That)",
            ),
            (
                "gadgets/index.html",
                "/proj/gadgets/ to /proj/tools/ (Tools)",
            ),
            (
                "gadgets/kept/index.html",
                "/proj/gadgets/kept to /proj/tools/kept (Moved)",
            ),
            // It gives way to a page that's still there...
            ("widgets/kept.html", "Kept"),
            // ...and to a redirect from that URL alone.
            (
                "gadgets/deep/that/index.html",
                "/proj/gadgets/deep/that to /proj/tools/this (This)",
            ),
        ] {
            assert_eq!(read(file), sends, "{file}");
        }
        let redirected = |folder: &str| -> Vec<String> {
            let under = |file: &String| file.starts_with(folder);
            files_in(&dist).into_iter().filter(under).collect()
        };
        assert_eq!(
            redirected("widgets/"),
            [
                "widgets/deep/that/index.html",
                "widgets/index.html",
                "widgets/kept.html",
                "widgets/kept.md",
                "widgets/this/index.html",
            ]
        );
        assert_eq!(redirected("toolshed"), ["toolshed.html", "toolshed.md"]);
        let this = Path::new("widgets/this/index.html");
        assert_eq!(built.urls[this], "/widgets/this");
        assert_eq!(built.origins[this], "the redirect from /widgets/this");
        assert_eq!(warnings_of(&built), Vec::<String>::new());

        // One from every page of the site, or to every page of it, is each
        // page's, but for those at a page that's there.
        redirects("    - { to: /tools/*, from: [/*] }\n").unwrap();
        assert_eq!(read("index.html"), "Home");
        assert_eq!(
            read("this/index.html"),
            "/proj/this to /proj/tools/this (This)"
        );
        redirects("    - { to: /*, from: [/old/*] }\n").unwrap();
        assert_eq!(read("old/index.html"), "/proj/old/ to /proj/ (Home)");
        assert_eq!(
            read("old/tools/this/index.html"),
            "/proj/old/tools/this to /proj/tools/this (This)"
        );

        // One that leads to a folder with no pages stops the build.
        for to in ["/tool", "/toolshed", "/tools/data.csv", "/widgets/kept"] {
            assert_eq!(
                redirects(&format!("    - {{ to: {to}/*, from: [/a/*] }}\n")).map(drop),
                Err(format!(
                    "`site.redirects` sends /a/* to {to}/*, but there's no page under {to}/"
                )),
                "{to}"
            );
        }
        // Two from one URL still do.
        assert_eq!(
            redirects("    - { to: /tools/deep/*, from: [/a/*, /a/*] }\n").map(drop),
            Err("the redirect from /a/that clashes with the redirect from /a/that".to_string())
        );
    }

    #[test]
    fn a_theme_without_a_template_writes_no_redirects() {
        let site_cfg = format!(
            "{IN_THEME}  redirects:\n    - {{ to: /guide, from: [/a, /b] }}\n    \
             - {{ to: /nowhere, from: [/c] }}\n"
        );
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg),
            ("docs/README.md", "# Home\n"),
            ("docs/guide.md", "# Guide\n"),
            ("theme/_templates/default.html", "{{ page.title }}"),
        ]);
        let built = build(&site).unwrap();

        // `default.html` has nothing to send a reader on with, so the build
        // warns, once, and writes none.
        let built_files = [
            ".little-build",
            "README.md",
            "guide.html",
            "guide.md",
            "index.html",
        ];
        assert_eq!(files_in(&site.join("dist")), built_files);
        assert_eq!(
            warnings_of(&built),
            [
                "`site.redirects` lists 3 redirect(s), but the theme has no \
              _templates/redirect.html to write them with, so none is written"
            ]
        );
    }

    #[test]
    fn the_builtin_theme_sends_a_redirects_reader_on() {
        let site_cfg = |fragment: &str| {
            format!(
                "site:\n  name: Test\n  url: https://example.com/proj\n  redirects:\n    \
                 - {{ to: \"/guide/install#{fragment}\", from: [/setup] }}\n    \
                 - {{ to: https://example.org/blog, from: [/news] }}\n"
            )
        };
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg("usage")),
            ("docs/README.md", "# Home\n"),
            ("docs/guide/install.md", "# Install\n\n## Usage\n"),
        ]);
        let built = build(&site).unwrap();
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();

        // At once, with a refresh, and with a link, to where it leads, under
        // the site's path, and it tells search engines which page to index.
        let page = read("setup/index.html");
        for shown in [
            "<title>Install · Test</title>",
            r#"<meta name="robots" content="noindex">"#,
            r#"<link rel="canonical" href="https://example.com/proj/guide/install">"#,
            r#"<meta http-equiv="refresh" content="0; url=/proj/guide/install#usage">"#,
            r#"<a href="/proj/guide/install#usage">Install</a>"#,
        ] {
            assert!(page.contains(shown), "{shown} in {page}");
        }
        // It's a page of its own, without the site's nav.
        assert!(!page.contains("<nav"), "{page}");
        // One to another site leads there, as it's written.
        let page = read("news/index.html");
        for shown in [
            "<title>Redirecting · Test</title>",
            r#"<link rel="canonical" href="https://example.org/blog">"#,
            r#"<meta http-equiv="refresh" content="0; url=https://example.org/blog">"#,
            r#"<a href="https://example.org/blog">https://example.org/blog</a>"#,
        ] {
            assert!(page.contains(shown), "{shown} in {page}");
        }

        // Its links are checked, as any page's are, and it's named in what's
        // warned of.
        let broken = |built: &Built| -> Vec<String> {
            let broken = crate::links::check(built, crate::links::Scope::Internal).unwrap();
            broken.iter().map(ToString::to_string).collect()
        };
        assert_eq!(broken(&built), Vec::<String>::new());
        fs::write(site.join("little.yml"), site_cfg("nope")).unwrap();
        assert_eq!(
            broken(&build(&site).unwrap()),
            [
                "broken link /proj/guide/install#nope in the redirect from /setup: Cannot find \
              fragment"
            ]
        );
    }

    #[test]
    fn plugins_hooks_are_called_for_each_redirect() {
        let plugin = serde_json::json!({
            "wasm": crate::plugin::tests::test_plugin(),
            "config": { "merge": { "site": { "redirects": [{ "to": "/", "from": ["/old"] }] } } },
            "site": { "echo": "given" },
            "page": { "only": "/docs/old", "merge": { "note": "Hooked" } },
            "html": { "only": "/docs/old", "append": " at {arg}" },
            "destination": { "list": "files.txt" },
        });
        let site_cfg =
            format!("{IN_THEME}  url: https://example.com/docs\n  plugins:\n    test: {plugin}\n");
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg),
            ("docs/README.md", "# Home\n"),
            ("theme/_templates/default.html", "{{ site.given | safe }}"),
            (
                "theme/_templates/redirect.html",
                "{{ page.note }}: to {{ page.redirect.url | safe }}",
            ),
        ]);
        let built = build(&site).unwrap();
        let read = |file: &str| fs::read_to_string(site.join("dist").join(file)).unwrap();

        // `config` can list redirects, which `site` is given. `page` and
        // `html` are called for each, at its URL under the site's path, and
        // `destination` is given its file.
        let given: serde_json::Value = serde_json::from_str(&read("index.html")).unwrap();
        assert_eq!(
            given["redirects"],
            serde_json::json!([{ "to": "/", "from": ["/old"] }])
        );
        assert_eq!(read("old/index.html"), "Hooked: to /docs/ at /docs/old");
        assert_eq!(read("files.txt"), "README.md\nindex.html\nold/index.html");
        assert!(built.warnings.is_empty(), "{:?}", built.warnings);
    }

    #[test]
    fn each_page_links_to_the_pages_before_and_after_it() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/guide/README.md", "# Guide\n"),
            ("docs/guide/install.md", "# Install\n"),
            (
                "theme/_templates/default.html",
                "{% if page.previous %}{{ page.previous.title }} {{ page.previous.url | safe }} {% endif %}\
                     < {{ page.title }} >\
                     {% if page.next %} {{ page.next.title }} {{ page.next.url | safe }}{% endif %}",
            ),
        ]);

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        assert_eq!(read("index.html"), "< Home > Guide /guide/");
        assert_eq!(
            read("guide/index.html"),
            "Home / < Guide > Install /guide/install"
        );
        assert_eq!(read("guide/install.html"), "Guide /guide/ < Install >");
    }

    #[test]
    fn each_page_lists_the_pages_above_it() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/guide/README.md", "# Guide\n"),
            ("docs/guide/install.md", "# Install\n"),
            ("docs/api/reference.md", "# Reference\n"),
            (
                "docs/api/errors.md",
                "---\nbreadcrumb:\n  enabled: false\n---\n# Errors\n",
            ),
            (
                "docs/api/limits.md",
                "---\nbreadcrumb:\n  items:\n    - { title: Reference, url: /api/reference }\n\
                 ---\n# Limits\n",
            ),
            (
                "theme/_templates/default.html",
                "{% if page.breadcrumb.items %}{% for crumb in page.breadcrumb.items %}\
                     {{ crumb.title }} {{ crumb.url | default('(label)') | safe }} > \
                     {% endfor %}{% endif %}{{ page.title }}",
            ),
        ]);

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        assert_eq!(read("index.html"), "Home");
        assert_eq!(read("guide/index.html"), "Home / > Guide");
        assert_eq!(
            read("guide/install.html"),
            "Home / > Guide /guide/ > Install"
        );
        assert_eq!(
            read("api/reference.html"),
            "Home / > Api (label) > Reference"
        );
        // Frontmatter has the last word: `breadcrumb.enabled` turns them off,
        // and its `items` replace them.
        assert_eq!(read("api/errors.html"), "Errors");
        assert_eq!(read("api/limits.html"), "Reference /api/reference > Limits");

        // They follow a nav set by hand, so a page that isn't in it has none.
        let site_cfg = "site:\n  theme:\n    source: ./theme\n  navigation:\n    items:\n      \
                        - { title: Start, url: / }\n      \
                        - { title: Install, url: /guide/install }\n";
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();
        assert_eq!(read("guide/install.html"), "Start / > Install");
        assert_eq!(read("guide/index.html"), "Guide");
    }

    #[test]
    fn the_builtin_theme_shows_breadcrumbs() {
        let (_tmp, site) = scratch(&[
            ("docs/README.md", "# Home\n"),
            ("docs/guide/README.md", "# Guide\n"),
            ("docs/guide/install.md", "# Install\n"),
            ("docs/api/reference.md", "# Reference\n"),
            (
                "docs/api/errors.md",
                "---\nbreadcrumb:\n  enabled: false\n---\n# Errors\n",
            ),
        ]);

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
            trail("guide/install.html"),
            [
                r#"<li><a href="/">Home</a></li>"#,
                r#"<li><a href="/guide/">Guide</a></li>"#,
                r#"<li><span aria-current="page">Install</span></li>"#,
            ]
        );
        assert_eq!(
            trail("api/reference.html"),
            [
                r#"<li><a href="/">Home</a></li>"#,
                "<li>Api</li>",
                r#"<li><span aria-current="page">Reference</span></li>"#,
            ]
        );
        // The home page has nothing above it, a page can turn them off, and the
        // 404 page has none.
        for path in ["index.html", "api/errors.html", NOT_FOUND_FILE] {
            assert!(!read(path).contains(nav), "{path}");
        }
    }

    #[test]
    fn the_builtin_theme_folds_the_nav() {
        let (_tmp, site) = scratch(&[
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
        ]);

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
            sidebar("guide/advanced/deep/limits.html"),
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
            folds("guide/recipes/bake.html"),
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

        // With `site.navigation.prune`, a folder the current page isn't in
        // is a link, with nothing to fold: to its own page, or, for a label,
        // to the first page in it. The folders the page is in are as before.
        let pruned = "site: { navigation: { prune: true } }\n";
        fs::write(site.join("little.yml"), pruned).unwrap();
        build(&site).unwrap();
        assert_eq!(
            sidebar("guide/advanced/deep/limits.html"),
            [
                r#"<a href="/">Home</a>"#,
                r#"<a href="/api/reference">Api</a>"#,
                r#"<a href="/guide/">Guide</a>"#,
                r#"<a href="/guide/advanced/">Advanced</a>"#,
                "<details open>",
                r#"<summary><span class="visually-hidden">Advanced</span></summary>"#,
                "<details open>",
                r#"<summary class="nav-label">Deep</summary>"#,
                r#"<a href="/guide/advanced/deep/limits" aria-current="page">Limits</a>"#,
                r#"<a href="/guide/advanced/tuning">Tuning</a>"#,
                r#"<a href="/guide/install">Install</a>"#,
                r#"<a href="/guide/recipes/bake">Recipes</a>"#,
            ]
        );
        assert_eq!(
            sidebar("index.html"),
            [
                r#"<a href="/" aria-current="page">Home</a>"#,
                r#"<a href="/api/reference">Api</a>"#,
                r#"<a href="/guide/">Guide</a>"#,
            ]
        );
    }

    #[test]
    fn each_page_links_to_its_file_in_the_repo() {
        let template = "{{ site.repository.url | safe }} \
                        | {% if page.repository.view.url %}{{ page.repository.view.url | safe }}{% endif %} \
                        | {% if page.repository.edit.url %}{{ page.repository.edit.url | safe }}{% endif %}";
        let repo = "https://github.com/withlittle/little";
        let site_cfg = format!(
            "{IN_THEME}  url: https://withlittle.github.io/little\n  \
             repository:\n    url: {repo}\n"
        );
        let (_tmp, root) = scratch(&[
            (".git/HEAD", "ref: refs/heads/main\n"),
            ("site/little.yml", &site_cfg),
            ("site/docs/README.md", "# Home\n"),
            ("site/docs/guide/install.md", "# Install\n"),
            (
                "site/docs/guide/reference.md",
                "---\nrepository:\n  view:\n    url: /api/\n---\n# Reference\n",
            ),
            (
                "site/docs/guide/generated.md",
                "---\nrepository:\n  view:\n    enabled: false\n    url: /api/\n  \
                 edit:\n    enabled: false\n---\n# Generated\n",
            ),
            (
                "site/docs/guide/mirrored.md",
                "---\nrepository:\n  edit:\n    enabled: false\n---\n# Mirrored\n",
            ),
            ("site/theme/_templates/default.html", template),
            ("site/theme/_templates/404.html", template),
        ]);
        let site = root.join("site");
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let links = |path: &str| format!("{repo} | {repo}/blob/{path} | {repo}/edit/{path}");

        // A page's file is found from the root of the repository: the
        // nearest folder above it with a `.git`.
        build(&site).unwrap();
        assert_eq!(
            read("guide/install.html"),
            links("main/site/docs/guide/install.md")
        );
        // A page's frontmatter has the last word, and a link from the root of
        // the site gets its path. Each link's `enabled` turns it off,
        // whatever else it sets, and leaves the other. The 404 page isn't
        // built from a file.
        assert_eq!(
            read("guide/reference.html"),
            format!("{repo} | /little/api/ | {repo}/edit/main/site/docs/guide/reference.md")
        );
        assert_eq!(read("guide/generated.html"), format!("{repo} |  | "));
        assert_eq!(
            read("guide/mirrored.html"),
            format!("{repo} | {repo}/blob/main/site/docs/guide/mirrored.md | ")
        );
        assert_eq!(read(NOT_FOUND_FILE), format!("{repo} |  | "));

        // In a worktree, `.git` is a file. A site can link to another branch.
        fs::remove_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git"), "gitdir: /elsewhere\n").unwrap();
        let branch_cfg = format!("{site_cfg}    branch: release/2.0\n");
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
    }

    #[test]
    fn the_builtin_theme_links_to_the_repo_and_each_pages_file() {
        let repo = "https://github.com/withlittle/little";
        let (_tmp, site) = scratch(&[
            (".git/HEAD", "ref: refs/heads/main\n"),
            (
                "little.yml",
                &format!("site:\n  repository:\n    url: {repo}\n"),
            ),
            ("docs/README.md", "# Home\n"),
            (
                "docs/guide.md",
                "---\nrepository:\n  view:\n    enabled: false\n  edit:\n    enabled: false\n\
                 ---\n# Guide\n",
            ),
            (
                "docs/mirrored.md",
                "---\nrepository:\n  edit:\n    enabled: false\n---\n# Mirrored\n",
            ),
        ]);

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
        // A page can turn its links off, or one of them, and the 404 page
        // has neither.
        let guide = read("guide.html");
        assert!(!guide.contains("Edit this page"), "{guide}");
        assert!(!guide.contains("View source of this page"), "{guide}");
        let mirrored = read("mirrored.html");
        assert!(!mirrored.contains("Edit this page"), "{mirrored}");
        let view = format!(r#"<a href="{repo}/blob/main/docs/mirrored.md">"#);
        assert!(mirrored.contains(&view), "{mirrored}");
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
    }

    #[test]
    fn each_pages_file_is_copied_beside_it() {
        // A theme that links to each page's file.
        let template = "{{ page.source.url | safe }}";
        let site_cfg = format!("{IN_THEME}  url: https://example.com/little\n");
        // As it's written: its frontmatter, its line endings, and what its
        // page doesn't show.
        let install = "---\ntitle: Install\nreviewed: no\n---\n# Install\r\n\n[//]: # (A note)\n";
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg),
            ("docs/README.md", "# Home\n"),
            ("docs/guide/README.md", "# Guide\n"),
            ("docs/guide/install.md", install),
            (
                "docs/guide/private.md",
                "---\nsource:\n  enabled: false\n  url: /mine.md\n---\n# Private\n",
            ),
            (
                "docs/guide/elsewhere.md",
                "---\nsource:\n  url: /files/elsewhere.txt\n---\n# Elsewhere\n",
            ),
            ("theme/_templates/default.html", template),
            ("theme/_templates/404.html", template),
        ]);
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();
        let copies = || -> Vec<String> {
            let copied = |file: &String| file.ends_with(".md");
            files_in(&dist).into_iter().filter(copied).collect()
        };

        // To its path in the source, as any other file is, so beside its
        // page: at the page's URL with `.md`, or, for a folder's own page,
        // in the folder. Its URL has the site's path.
        build(&site).unwrap();
        let copied = [
            "README.md",
            "guide/README.md",
            "guide/elsewhere.md",
            "guide/install.md",
        ];
        assert_eq!(copies(), copied);
        assert_eq!(read("guide/install.md"), install);
        assert_eq!(read("guide/install.html"), "/little/guide/install.md");
        assert_eq!(read("guide/index.html"), "/little/guide/README.md");
        assert_eq!(read("index.html"), "/little/README.md");
        // A page's own settings have the last word on what its theme gets,
        // and a URL they write gets the site's path. Its `enabled` leaves
        // its file out, and its URL, whatever else it sets. The 404 page
        // isn't built from a file.
        assert_eq!(read("guide/elsewhere.html"), "/little/files/elsewhere.txt");
        assert_eq!(read("guide/private.html"), "");
        assert_eq!(read(NOT_FOUND_FILE), "");

        // A site turns it off for every page, and a page back on for itself.
        let site_cfg = format!("{site_cfg}page:\n  source:\n    enabled: false\n");
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        let private = site.join(native("docs/guide/private.md"));
        fs::write(private, "---\nsource:\n  enabled: true\n---\n# Private\n").unwrap();
        build(&site).unwrap();
        assert_eq!(copies(), ["guide/private.md"]);
        assert_eq!(read("guide/private.html"), "/little/guide/private.md");
        assert_eq!(read("guide/install.html"), "");
    }

    #[test]
    fn a_plugins_page_has_only_the_source_it_names() {
        // A page with nothing to say of what it's made from, and three that
        // name the file they're made from: one the plugin adds too, one of
        // the site's own, and one another plugin adds.
        let page = |name: &str, url: &str| {
            serde_json::json!({
                "path": format!("api/{name}.md"),
                "text": format!("---\nsource:\n  url: {url}\n---\n# Made from a file\n"),
            })
        };
        let wasm = crate::plugin::tests::test_plugin();
        let api = serde_json::json!({
            "wasm": wasm,
            "files": { "add": [
                { "path": "api/modules.md", "text": "# Modules\n" },
                page("petstore", "/api/pet%20store.yaml"),
                { "path": "api/pet store.yaml", "text": "openapi: 3.1.0\n" },
                page("events", "/api/events.yaml"),
                page("shared", "/shared.yaml"),
                { "path": "api/logo.svg", "text": "<svg/>" },
            ] },
        });
        // Each renders a language of its own, as two can't render one.
        let other = serde_json::json!({
            "wasm": wasm,
            "languages": ["other"],
            "files": { "add": [{ "path": "shared.yaml", "text": "shared: true\n" }] },
        });
        let site_cfg = format!(
            "{IN_THEME}  url: https://example.com/little\n  plugins:\n    api: {api}\n    \
             other: {other}\n"
        );
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg),
            ("docs/README.md", "# Home\n"),
            ("docs/api/events.yaml", "asyncapi: 3.0.0\n"),
            (
                "theme/_templates/default.html",
                "{{ page.source.url | safe }}",
            ),
        ]);
        build(&site).unwrap();
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();
        let added = || -> Vec<String> {
            let page = |file: &String| file.ends_with(".html");
            files_in(&dist)
                .into_iter()
                .filter(|file| !page(file))
                .collect()
        };

        // The Markdown a plugin gives for a page is made from something
        // else, so it isn't the page's source, and isn't copied.
        assert_eq!(read("api/modules.html"), "");
        // A page made from one file says so in its frontmatter, as any
        // page's settings have the last word on what its theme gets.
        assert_eq!(read("api/petstore.html"), "/little/api/pet%20store.yaml");
        assert_eq!(read("api/pet store.yaml"), "openapi: 3.1.0\n");
        assert_eq!(read("api/events.html"), "/little/api/events.yaml");
        assert_eq!(read("api/shared.html"), "/little/shared.yaml");
        let files = [
            MARKER_FILE,
            "README.md",
            "api/events.yaml",
            "api/logo.svg",
            "api/pet store.yaml",
            "shared.yaml",
        ];
        assert_eq!(added(), files);

        // With its `source` off, a page has no URL, and the file it names
        // is left out, if its plugin added it: that's the page's source, as
        // the home page's file is its own. The site's own file stays, as
        // any file in the source does, and so do another plugin's, and one
        // no page names.
        let off = format!("{site_cfg}page:\n  source:\n    enabled: false\n");
        fs::write(site.join("little.yml"), off).unwrap();
        build(&site).unwrap();
        for page in ["petstore", "events", "shared"] {
            assert_eq!(read(&format!("api/{page}.html")), "", "{page}");
        }
        let files = [
            MARKER_FILE,
            "api/events.yaml",
            "api/logo.svg",
            "shared.yaml",
        ];
        assert_eq!(added(), files);
    }

    #[test]
    fn a_versions_pages_files_are_copied_from_its_ref() {
        let site_cfg = format!(
            "{IN_THEME}  url: https://example.com/proj\n  versions:\n    \
             - {{ title: \"2.0\", url: / }}\n    - {{ title: \"1.4\", url: /1.4, ref: v1.4 }}\n"
        );
        let (_tmp, site) = versioned_site(
            &[
                ("docs/README.md", "# Old home\n"),
                ("docs/old.md", "# Old\n"),
            ],
            &[
                ("little.yml", &site_cfg),
                ("docs/README.md", "# Home\n"),
                (
                    "theme/_templates/default.html",
                    "{{ page.source.url | safe }}",
                ),
                // One of the theme's files, which every version shares at
                // the root of the site, where a version's page has its own.
                ("theme/_static/old.md", "The theme's\n"),
            ],
        );
        build(&site).unwrap();
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();

        // Each version has its pages' files as they are at its ref, in its
        // own folder, at URLs under its own path.
        assert_eq!(read("index.html"), "/proj/README.md");
        assert_eq!(read("README.md"), "# Home\n");
        assert_eq!(read("1.4/index.html"), "/proj/1.4/README.md");
        assert_eq!(read("1.4/README.md"), "# Old home\n");
        assert_eq!(read("1.4/old.html"), "/proj/1.4/old.md");
        assert_eq!(read("1.4/old.md"), "# Old\n");
        assert_eq!(read("old.md"), "The theme's\n");
    }

    #[test]
    fn the_builtin_theme_links_to_a_pages_markdown() {
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                &format!(
                    "site:\n  url: https://example.com/little\n  plugins:\n    test:\n      \
                     wasm: {}\n      extensions: [test]\n",
                    crate::plugin::tests::test_plugin().display()
                ),
            ),
            ("docs/README.md", "# Home\n"),
            (
                "docs/guide.md",
                "---\nsource:\n  enabled: false\n---\n# Guide\n",
            ),
            ("docs/analysis.test", "[{ \"markdown\": \"# Analysis\" }]"),
        ]);
        build(&site).unwrap();
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();

        // In its head, as an alternate to the page, for a page written in
        // Markdown, whose file is copied.
        let alternate = "<link rel=\"alternate\" type=\"text/markdown\"";
        let home = read("index.html");
        let link = format!("{alternate} href=\"/little/README.md\">");
        assert!(home.contains(&link), "{home}");
        // A page whose file isn't copied has none, nor has one in another
        // format, whose file isn't Markdown, nor the 404 page.
        for page in ["guide.html", "analysis.html", NOT_FOUND_FILE] {
            let html = read(page);
            assert!(!html.contains("rel=\"alternate\""), "{page}: {html}");
        }
        assert!(site.join("dist/analysis.test").is_file());
    }

    #[test]
    fn a_site_gets_its_repositorys_owner_and_name() {
        let repository = |settings: &str| format!("{IN_THEME}  repository:\n{settings}");
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                &repository("    url: https://gitlab.com/withlittle/docs/little\n"),
            ),
            ("docs/README.md", "# Home\n"),
            (
                "theme/_templates/default.html",
                "{{ site.repository.owner | safe }} | {{ site.repository.name }} \
                 | {{ site.repository.branch }}",
            ),
        ]);
        let home = || {
            build(&site).unwrap();
            fs::read_to_string(site.join("dist/index.html")).unwrap()
        };

        // From its `url`: the last part of its path is its name, and what's
        // before it its owner, here a group within another.
        assert_eq!(home(), "withlittle/docs | little | main");
        // A site that sets either has the last word.
        let site_cfg =
            repository("    url: https://github.com/withlittle/little\n    owner: With Little\n");
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        assert_eq!(home(), "With Little | little | main");
        // Without a `url`, it has neither.
        fs::write(site.join("little.yml"), IN_THEME).unwrap();
        assert_eq!(home(), " |  | main");
    }

    #[test]
    fn the_builtin_theme_gives_a_page_without_a_description_the_sites() {
        let (_tmp, site) = scratch(&[
            ("little.yml", "site: { description: Docs that just work }\n"),
            ("docs/README.md", "# Home\n"),
            (
                "docs/guide.md",
                "---\ndescription: All about it\n---\n# Guide\n",
            ),
        ]);

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let meta = |text: &str| format!(r#"<meta name="description" content="{text}">"#);
        let guide = read("guide.html");
        assert!(guide.contains(&meta("All about it")), "{guide}");
        assert!(!guide.contains("Docs that just work"), "{guide}");
        // The header doesn't show it, so it's only in the meta tag.
        let home = read("index.html");
        assert!(home.contains(&meta("Docs that just work")), "{home}");
        assert_eq!(home.matches("Docs that just work").count(), 1, "{home}");

        fs::write(site.join("little.yml"), "").unwrap();
        build(&site).unwrap();
        let home = read("index.html");
        assert!(!home.contains(r#"<meta name="description""#), "{home}");
    }

    #[test]
    fn a_page_toc_is_shaped_like_the_site_nav() {
        // So one macro can list both.
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
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
                     {{ list(site.navigation.items) }} / {{ list(page.outline.items) }}",
            ),
        ]);

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let nav = "[Home /][Guide /guide/ [Install /guide/install]]";
        assert_eq!(
            read("index.html"),
            format!("{nav} / [Setup #setup [Requirements #requirements]][Usage #usage]")
        );
        // A page without headings has an empty `page.outline.items`.
        assert_eq!(read("guide/install.html"), format!("{nav} / "));
    }

    #[test]
    fn a_page_can_turn_off_its_toc() {
        // `outline.enabled: false` empties the table of contents, and the
        // built-in theme lists one only `{% if page.outline.items %}`.
        let (_tmp, site) = scratch(&[
            ("docs/README.md", "# Home\n\n## Setup\n"),
            (
                "docs/guide.md",
                "---\noutline:\n  enabled: false\n---\n# Guide\n\n## Setup\n",
            ),
        ]);

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let toc = r#"<nav class="toc""#;
        assert!(read("index.html").contains(toc));
        assert!(!read("guide.html").contains(toc));
    }

    #[test]
    fn a_page_turns_off_what_little_works_out_for_it() {
        // A theme that shows all it's given.
        let template = "{{ page.outline.items | map(attribute='title') | join(', ') }} \
                        | {{ page.breadcrumb.items | map(attribute='title') | join(', ') }} \
                        | {{ page.previous.title if page.previous }} \
                        | {{ page.next.title if page.next }} \
                        | {{ page.repository.view.url | safe }} \
                        {{ page.repository.edit.url | safe }}";
        let repo = "https://github.com/withlittle/little";
        let (_tmp, site) = scratch(&[
            (".git/HEAD", "ref: refs/heads/main\n"),
            (
                "little.yml",
                &format!("{IN_THEME}  repository:\n    url: {repo}\n"),
            ),
            ("docs/README.md", "# Home\n"),
            ("docs/guide/README.md", "# Guide\n\n## Setup\n"),
            ("docs/guide/usage.md", "# Usage\n"),
            ("theme/_templates/default.html", template),
        ]);
        let page = site.join(native("docs/guide/README.md"));
        let guide = |frontmatter: &str| {
            fs::write(
                &page,
                format!("---\n{frontmatter}---\n# Guide\n\n## Setup\n"),
            )
            .unwrap();
            build(&site).map(|_| fs::read_to_string(site.join("dist/guide/index.html")).unwrap())
        };
        let file = "main/docs/guide/README.md";
        let (view, edit) = (format!("{repo}/blob/{file}"), format!("{repo}/edit/{file}"));
        let links = format!("{view} {edit}");

        // Each is on, as little sets it.
        assert_eq!(
            guide("").unwrap(),
            format!("Setup | Home | Home | Usage | {links}")
        );
        // Each group's `enabled` turns its own off, whatever the theme.
        let off = "outline:\n  enabled: false\nbreadcrumb:\n  enabled: false\n\
                   previous:\n  enabled: false\nnext:\n  enabled: false\n\
                   repository:\n  view:\n    enabled: false\n  edit:\n    enabled: false\n";
        assert_eq!(guide(off).unwrap(), " |  |  |  |  ");
        for (group, shown) in [
            ("outline", format!(" | Home | Home | Usage | {links}")),
            ("breadcrumb", format!("Setup |  | Home | Usage | {links}")),
            ("previous", format!("Setup | Home |  | Usage | {links}")),
            ("next", format!("Setup | Home | Home |  | {links}")),
        ] {
            let frontmatter = format!("{group}:\n  enabled: false\n");
            assert_eq!(guide(&frontmatter).unwrap(), shown, "{group}");
        }
        // Each of its links to the site's repository has an `enabled` of
        // its own, in `repository`.
        for (link, shown) in [
            ("view", format!("Setup | Home | Home | Usage |  {edit}")),
            ("edit", format!("Setup | Home | Home | Usage | {view} ")),
        ] {
            let frontmatter = format!("repository:\n  {link}:\n    enabled: false\n");
            assert_eq!(guide(&frontmatter).unwrap(), shown, "{link}");
        }
        // What it turns off, it can't give the theme itself, either.
        let off = "outline:\n  enabled: false\n  items: [{ title: Mine, url: \"#mine\" }]\n\
                   previous:\n  enabled: false\n  title: Mine\n\
                   repository:\n  edit:\n    enabled: false\n    url: https://example.com/edit\n";
        assert_eq!(guide(off).unwrap(), format!(" | Home |  | Usage | {view} "));

        // A value where a group's settings go, like `false`, stops the
        // build, naming the page.
        let groups = ["outline", "breadcrumb", "previous", "next", "source"];
        for group in groups {
            let message = format!("{:#}", guide(&format!("{group}: false\n")).unwrap_err());
            let expected = format!(
                "rendering {}: `{group}` must be its settings, like `enabled: false`",
                page.display()
            );
            assert_eq!(message, expected);
        }
        for (frontmatter, group, like) in [
            (
                "repository: false\n",
                "repository",
                "edit: { enabled: false }",
            ),
            (
                "repository:\n  view: false\n",
                "repository.view",
                "enabled: false",
            ),
        ] {
            let message = format!("{:#}", guide(frontmatter).unwrap_err());
            let expected = format!(
                "rendering {}: `{group}` must be its settings, like `{like}`",
                page.display()
            );
            assert_eq!(message, expected);
        }
        let message = format!("{:#}", guide("next:\n  enabled: no\n").unwrap_err());
        let expected = format!(
            "rendering {}: `next.enabled` must be `true` or `false`",
            page.display()
        );
        assert_eq!(message, expected);
    }

    #[test]
    fn a_page_chooses_the_headings_its_own_toc_lists() {
        let headings = "## Setup\n\n### Install\n\n#### From source\n";
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                &format!("{IN_THEME}page:\n  outline:\n    depth:\n      max: 3\n"),
            ),
            ("docs/README.md", &format!("# Home\n\n{headings}")),
            (
                "docs/deep.md",
                &format!("---\noutline:\n  depth:\n    max: 4\n---\n# Deep\n\n{headings}"),
            ),
            (
                "docs/whole.md",
                &format!(
                    "---\noutline: {{ depth: {{ min: 1, max: 2 }} }}\n---\n# Whole\n\n{headings}"
                ),
            ),
            (
                "theme/_templates/default.html",
                "{% for item in page.outline.items recursive %}{{ item.title }}\
                 {% if item.children %} [{{ loop(item.children) }}]{% endif %}; {% endfor %}\
                 from {{ page.outline.depth.min }} to {{ page.outline.depth.max }}",
            ),
        ]);

        build(&site).unwrap();

        // The site's levels, for every page, unless a page sets its own, in
        // its frontmatter, which the theme gets beside its headings.
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        assert_eq!(read("index.html"), "Setup [Install; ]; from 2 to 3");
        assert_eq!(
            read("deep.html"),
            "Setup [Install [From source; ]; ]; from 2 to 4"
        );
        assert_eq!(read("whole.html"), "Whole [Setup; ]; from 1 to 2");

        // Levels a page can't have stop the build, naming the page.
        let page = site.join("docs").join("deep.md");
        fs::write(&page, "---\noutline:\n  depth:\n    min: 4\n---\n# Deep\n").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "rendering {}: `outline.depth.min` (4) can't be more than `outline.depth.max` (3)",
            page.display()
        );
        assert_eq!(message, expected);
    }

    #[test]
    fn a_page_toc_lists_the_headings_its_config_chooses() {
        // little lists every heading but the title, the built-in theme's
        // `little.yml` only `##` and `###`, and a site can choose otherwise.
        let page = "# Home\n\n## Setup\n\n### Install\n\n#### From Source\n";
        for (label, site_cfg, listed) in [
            ("toc-theme", "", &["#setup", "#install"][..]),
            (
                "toc-max-2",
                "page: { outline: { depth: { max: 2 } } }\n",
                &["#setup"],
            ),
            (
                "toc-max-4",
                "page: { outline: { depth: { max: 4 } } }\n",
                &["#setup", "#install", "#from-source"],
            ),
            (
                "toc-min-1",
                "page: { outline: { depth: { min: 1 } } }\n",
                &["#home", "#setup", "#install"],
            ),
            (
                "toc-min-3",
                "page: { outline: { depth: { min: 3, max: 4 } } }\n",
                &["#install", "#from-source"],
            ),
        ] {
            let (_tmp, site) = scratch(&[("little.yml", site_cfg), ("docs/README.md", page)]);

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
        }
    }

    #[test]
    fn every_frontmatter_key_reaches_the_page() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            (
                "docs/README.md",
                "---\nauthor: Ada\ntags: [setup, cli]\nhero:\n  image: hero.png\ntitle:\ndraft:\n---\n# Home\n",
            ),
            (
                "theme/_templates/default.html",
                r#"{{ page.author }}, {{ page.tags | join("+") }}, {{ page.hero.image }}, {{ page.title }}, {{ page.draft | default("not a draft") }}"#,
            ),
        ]);

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
    }

    #[test]
    fn frontmatter_has_the_last_word_on_the_page() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            (
                "docs/guide.md",
                "---\ntype: tutorial\nurl: /elsewhere\nprevious:\n  title: Start here\n\
                 next:\n  enabled: false\n---\n# Guide\n",
            ),
            ("docs/reference.md", "# Reference\n"),
            (
                "theme/_templates/default.html",
                "{{ page.type }} {{ page.url | safe }}: \
                     {% if page.previous %}{{ page.previous.title }} {{ page.previous.url | safe }} {% endif %}\
                     < {{ page.title }} >\
                     {% if page.next %} {{ page.next.title }} {{ page.next.url | safe }}{% endif %}",
            ),
            ("theme/_templates/tutorial.html", "tutorial.html"),
        ]);

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        // Settings merge key by key, so `previous` keeps its URL, and `next`
        // is turned off.
        assert_eq!(
            read("guide.html"),
            "tutorial /elsewhere: Start here / < Guide >"
        );
        // Only the theme sees it: the page is built at its own URL, with the
        // template for its own type, and the pages beside it link to it there.
        assert!(!site.join("dist/elsewhere").exists());
        assert_eq!(read("index.html"), "home /: < Home > Guide /guide");
        assert_eq!(
            read("reference.html"),
            "page /reference: Guide /guide < Reference >"
        );
    }

    #[test]
    fn a_nav_in_the_config_replaces_littles() {
        let site_cfg = "site:\n  theme:\n    source: ./theme\n  navigation:\n    items:\n      \
                        - { title: Start, url: /guide/install }\n      \
                        - { title: Home, url: /, icon: house }\n      \
                        - { title: GitHub, url: \"https://github.com/withlittle/little\" }\n";
        let (_tmp, site) = scratch(&[
            ("little.yml", site_cfg),
            ("docs/README.md", "# Home\n"),
            ("docs/guide/README.md", "# Guide\n"),
            ("docs/guide/install.md", "# Install\n"),
            (
                "theme/_templates/default.html",
                "{% for item in site.navigation.items %}{{ item.title }}{% if item.icon %} ({{ item.icon }}){% endif %}; {% endfor %}\
                     {% if page.previous %}{{ page.previous.url | safe }} {% endif %}\
                     < {{ page.title }} >\
                     {% if page.next %} {{ page.next.url | safe }}{% endif %}",
            ),
        ]);

        build(&site).unwrap();

        // The theme gets the nav as the config sets it, and `previous` and
        // `next` follow it. A page that isn't in it has neither.
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let nav = "Start; Home (house); GitHub; ";
        assert_eq!(read("guide/install.html"), format!("{nav}< Install > /"));
        assert_eq!(
            read("index.html"),
            format!("{nav}/guide/install < Home > https://github.com/withlittle/little")
        );
        assert_eq!(read("guide/index.html"), format!("{nav}< Guide >"));

        // A nav that isn't a list of links stops the build.
        for nav in ["items: { style: tabs }\n", "items:\n      - url: /\n"] {
            let site_cfg = format!("{IN_THEME}  navigation:\n    {nav}");
            fs::write(site.join("little.yml"), site_cfg).unwrap();
            let message = format!("{:#}", build(&site).unwrap_err());
            let expected = "`site.navigation.items` in little.yml must be a list of links";
            assert!(message.starts_with(expected), "{message}");
        }
    }

    #[test]
    fn nav_prune_gives_each_page_a_nav_of_its_own() {
        let nav = "{% for item in site.navigation.items recursive %}{{ item.title }}\
                   {% if item.url %} {{ item.url | safe }}{% endif %}\
                   {% if item.children %} [{{ loop(item.children) }}]{% endif %}; {% endfor %}";
        let template = format!(
            "{nav}| {{% for crumb in page.breadcrumb.items %}}{{{{ crumb.title }}}} > {{% endfor %}}\
             | {{% if page.previous %}}{{{{ page.previous.url | safe }}}}{{% endif %}} \
             < {{{{ page.title }}}} > \
             {{% if page.next %}}{{{{ page.next.url | safe }}}}{{% endif %}}"
        );
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                "site:\n  theme:\n    source: ./theme\n  navigation:\n    prune: true\n",
            ),
            ("docs/README.md", "# Home\n"),
            ("docs/guide/README.md", "# Guide\n"),
            ("docs/guide/install.md", "# Install\n"),
            ("docs/guide/advanced/tuning.md", "# Tuning\n"),
            ("docs/api/reference.md", "# Reference\n"),
            ("theme/_templates/default.html", &template),
            ("theme/_templates/404.html", nav),
        ]);
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();

        build(&site).unwrap();

        // A page's nav lists what's in the folders it's in. Its breadcrumbs,
        // `previous`, and `next` follow the whole nav, as they do without
        // `prune`.
        assert_eq!(
            read("guide/advanced/tuning.html"),
            "Home /; Api /api/reference; \
             Guide /guide/ [Advanced [Tuning /guide/advanced/tuning; ]; Install /guide/install; ]; \
             | Home > Guide > Advanced > | /guide/ < Tuning > /guide/install"
        );
        assert_eq!(
            read("api/reference.html"),
            "Home /; Api [Reference /api/reference; ]; Guide /guide/; \
             | Home > Api > | / < Reference > /guide/"
        );
        // A folder's own page lists what's in it, with each folder there a
        // link. The home page, and the 404 page, get the top of the nav.
        assert_eq!(
            read("guide/index.html"),
            "Home /; Api /api/reference; \
             Guide /guide/ [Advanced /guide/advanced/tuning; Install /guide/install; ]; \
             | Home > | /api/reference < Guide > /guide/advanced/tuning"
        );
        let top = "Home /; Api /api/reference; Guide /guide/; ";
        assert_eq!(
            read("index.html"),
            format!("{top}| |  < Home > /api/reference")
        );
        assert_eq!(read(NOT_FOUND_FILE), top);

        // Its links have the path of the site's `url`, a label's too.
        let site_cfg = "site:\n  theme:\n    source: ./theme\n  navigation:\n    prune: true\n  \
                        url: https://withlittle.github.io/little\n";
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();
        assert_eq!(
            read(NOT_FOUND_FILE),
            "Home /little/; Api /little/api/reference; Guide /little/guide/; "
        );
        assert!(read("guide/install.html").starts_with(
            "Home /little/; Api /little/api/reference; Guide /little/guide/ \
             [Advanced /little/guide/advanced/tuning; Install /little/guide/install; ]; "
        ));

        // Without it, as little has it, every page gets the whole nav.
        fs::write(site.join("little.yml"), IN_THEME).unwrap();
        build(&site).unwrap();
        let whole = "Home /; Api [Reference /api/reference; ]; \
                     Guide /guide/ [Advanced [Tuning /guide/advanced/tuning; ]; \
                     Install /guide/install; ]; ";
        for path in ["index.html", "guide/install.html", NOT_FOUND_FILE] {
            assert!(read(path).starts_with(whole), "{path}");
        }
    }

    #[test]
    fn every_page_gets_when_the_site_was_built() {
        let template = "{{ site.built }} &copy; {{ site.built | dateformat(format='%Y') }} \
                        {{ site.name }} [{{ site.modified }}{{ page.created }}{{ page.modified }}]";
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                "site:\n  theme:\n    source: ./theme\n  name: Little\n",
            ),
            ("docs/README.md", "# Home\n"),
            ("docs/guide.md", "# Guide\n"),
            ("theme/_templates/default.html", template),
            ("theme/_templates/404.html", template),
        ]);

        // In UTC, to the second, whatever the time's zone, and its fraction.
        // The site isn't in a repository, so git has no dates for it.
        build_as_of(&site, git::tests::when("2026-11-19T13:37:42.5+11:00")).unwrap();
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        for path in ["index.html", "guide.html", "404.html"] {
            let built = "2026-11-19T02:37:42Z &copy; 2026 Little []";
            assert_eq!(read(path), built, "{path}");
        }

        // They're beneath the config, like the nav, so a `built` or a
        // `modified` there replaces little's, as it's written.
        let site_cfg = "site:\n  theme:\n    source: ./theme\n  name: Little\n  \
                        built: 2019-06-01\n  modified: last year\n";
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();
        assert_eq!(
            read("index.html"),
            "2019-06-01 &copy; 2019 Little [last year]"
        );
    }

    #[test]
    fn a_rebuild_keeps_the_time_of_the_build() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/guide.md", "# Guide\n"),
            (
                "theme/_templates/default.html",
                "{{ site.built }} {{ page.content | safe }}",
            ),
        ]);
        let time = git::tests::when("2026-11-19T02:37:42Z");
        let built = build_as_of(&site, time).unwrap();

        // Later, a page's text changes: it alone is rendered again, as of
        // then, as the time is the same in what every page is rendered from.
        write(&site, &[("docs/guide.md", "# Guide\n\nMore.\n")]);
        let rebuilt = rebuild(&site, &built).unwrap();
        assert_eq!(rebuilt.time, time);
        assert!(rebuilt.written.contains(&native("guide.html")));
        assert!(!rebuilt.written.contains(&native("index.html")));
        let guide = fs::read_to_string(site.join("dist/guide.html")).unwrap();
        assert!(guide.starts_with("2026-11-19T02:37:42Z <h1"), "{guide}");
        assert!(guide.contains("<p>More.</p>"), "{guide}");
    }

    #[test]
    fn each_page_gets_its_files_dates_from_git() {
        use git::tests::{commit_all_at, git_in, init};
        let template = "{{ page.created | default('-') }} {{ page.modified | default('-') }} \
                        {{ site.modified | default('-') }}";
        let moved = "# Moved\n\nA page.\n\nWith a few lines.\n\nFor git to know it by.\n";
        let (_tmp, site) = scratch(&[
            (".gitignore", "dist/\n"),
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/guide.md", "# Guide\n"),
            ("docs/old.md", moved),
            (
                "docs/pinned.md",
                "---\nmodified: 2020-02-02\n---\n# Pinned\n",
            ),
            ("theme/_templates/default.html", template),
            ("theme/_templates/404.html", template),
        ]);
        init(&site);
        commit_all_at(&site, "2026-01-05T10:00:00+11:00");
        write(&site, &[("docs/guide.md", "# Guide\n\nMore.\n")]);
        commit_all_at(&site, "2026-02-01T00:00:00Z");
        git_in(&site, &["mv", "docs/old.md", "docs/new.md"]);
        commit_all_at(&site, "2026-03-01T00:00:00Z");
        // Not committed: a new page, and a change to one.
        let home = ("docs/README.md", "# Home\n\nWelcome.\n");
        write(&site, &[("docs/draft.md", "# Draft\n"), home]);
        let (jan, feb, mar, apr) = (
            "2026-01-04T23:00:00Z",
            "2026-02-01T00:00:00Z",
            "2026-03-01T00:00:00Z",
            "2026-04-01T00:00:00Z",
        );

        // Each page's are its file's commits', in UTC, and the site's is its
        // newest commit's. What isn't committed moves none of them.
        let built = build(&site).unwrap();
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        assert_eq!(read("index.html"), format!("{jan} {jan} {mar}"));
        assert_eq!(read("guide.html"), format!("{jan} {feb} {mar}"));
        // A file that's renamed keeps its history.
        assert_eq!(read("new.html"), format!("{jan} {mar} {mar}"));
        // A page's frontmatter replaces its own, as it's written.
        let pinned = format!("{jan} 2020-02-02 {mar}");
        assert_eq!(read("pinned.html"), pinned);
        // A page whose file isn't committed has none, nor has one with no
        // file.
        assert_eq!(read("draft.html"), format!("- - {mar}"));
        assert_eq!(read("404.html"), format!("- - {mar}"));

        // A rebuild has the same dates, so renders nothing again, until
        // there's another commit, which every page has as the site's.
        let rebuilt = rebuild(&site, &built).unwrap();
        for page in ["index.html", "guide.html", "draft.html"] {
            assert!(!rebuilt.written.contains(&native(page)), "{page}");
        }
        commit_all_at(&site, apr);
        rebuild(&site, &rebuilt).unwrap();
        assert_eq!(read("index.html"), format!("{jan} {apr} {apr}"));
        assert_eq!(read("guide.html"), format!("{jan} {feb} {apr}"));
        assert_eq!(read("draft.html"), format!("{apr} {apr} {apr}"));
        assert_eq!(read("404.html"), format!("- - {apr}"));

        // A shallow clone has the commit it's built from, whole, but not the
        // commits each file changed in, so its pages have no dates.
        let (_clone, clone) = scratch(&[]);
        let url = url::Url::from_file_path(&site).unwrap().to_string();
        git_in(&clone, &["clone", "--quiet", "--depth", "1", &url, "."]);
        build(&clone).unwrap();
        let read = |path: &str| fs::read_to_string(clone.join("dist").join(path)).unwrap();
        for path in ["index.html", "guide.html", "draft.html"] {
            assert_eq!(read(path), format!("- - {apr}"), "{path}");
        }
        // Its frontmatter still gives a page one.
        assert_eq!(read("pinned.html"), format!("- 2020-02-02 {apr}"));
    }

    #[test]
    fn a_version_from_a_ref_has_its_pages_dates_from_its_ref() {
        use git::tests::{commit_all_at, git_in, init};
        let template = "{{ page.created | default('-') }} {{ page.modified | default('-') }} \
                        {{ site.modified | default('-') }}";
        let config = format!(
            "{IN_THEME}  versions:\n    - {{ title: \"2.0\", url: / }}\n    \
             - {{ title: \"1.0\", url: /1.0, ref: v1.0 }}\n"
        );
        let (_tmp, site) = scratch(&[
            (".gitignore", "dist/\n"),
            ("little.yml", &config),
            ("docs/README.md", "# Home\n"),
            ("docs/guide.md", "# Guide\n"),
            ("theme/_templates/default.html", template),
        ]);
        init(&site);
        commit_all_at(&site, "2026-01-01T00:00:00Z");
        git_in(&site, &["tag", "v1.0"]);
        let guide = ("docs/guide.md", "# Guide\n\nMore.\n");
        write(&site, &[guide, ("docs/new.md", "# New\n")]);
        commit_all_at(&site, "2026-02-01T00:00:00Z");
        let (jan, feb) = ("2026-01-01T00:00:00Z", "2026-02-01T00:00:00Z");

        build(&site).unwrap();
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        // The site's files, as they are, have the history of `HEAD`.
        assert_eq!(read("index.html"), format!("{jan} {jan} {feb}"));
        assert_eq!(read("guide.html"), format!("{jan} {feb} {feb}"));
        assert_eq!(read("new.html"), format!("{feb} {feb} {feb}"));
        // A version from a ref has its ref's, but the site's own date is the
        // same in every version.
        assert_eq!(read("1.0/index.html"), format!("{jan} {jan} {feb}"));
        assert_eq!(read("1.0/guide.html"), format!("{jan} {jan} {feb}"));
        assert!(!site.join("dist/1.0/new").exists());
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
        let site_cfg = "site:\n  theme:\n    source: ./theme\n  \
                        url: https://withlittle.github.io/little\n";
        let (_tmp, site) = scratch(&[
            ("little.yml", site_cfg),
            (
                "docs/README.md",
                "# Home\n\nRead the [guide](/guide/), or see ![the logo](/logo.png).\n",
            ),
            ("docs/guide/README.md", "# Guide\n\n## Steps\n"),
            (
                "docs/guide/install.md",
                "---\nprevious: { url: /start/ }\nlogo: /logo.png\n---\n# Install\n",
            ),
            ("theme/_templates/home.html", "{{ page.content | safe }}"),
            (
                "theme/_templates/default.html",
                "{% for item in site.navigation.items recursive %}{{ item.url | safe }} \
                     {% if item.children %}{{ loop(item.children) }}{% endif %}{% endfor %}\
                     | {{ page.url | safe }} \
                     | {% if page.previous %}{{ page.previous.url | safe }}{% endif %} \
                     {% if page.next %}{{ page.next.url | safe }}{% endif %} \
                     | {% for crumb in page.breadcrumb.items %}{{ crumb.url | safe }} {% endfor %}\
                     | {% for item in page.children %}{{ item.url | safe }} {% endfor %}\
                     | {% for item in page.outline.items %}{{ item.url | safe }} {% endfor %}\
                     | {{ page.logo | safe }}",
            ),
            (
                "theme/_templates/404.html",
                "{{ page.url | safe }} {{ '/' | url }} {{ site.url | safe }}",
            ),
        ]);

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
        // So does one the author writes where little would, like `previous`,
        // but not a setting of the theme's own, like `logo`.
        assert_eq!(
            read("guide/install.html"),
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
            "{site_cfg}  navigation:\n    items:\n      \
             - {{ title: Start, url: /guide/install }}\n      \
             - {{ title: GitHub, url: \"https://github.com/withlittle/little\" }}\n"
        );
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();
        let github = "https://github.com/withlittle/little";
        assert_eq!(
            read("guide/install.html"),
            format!(
                "/little/guide/install {github} | /little/guide/install \
                 | /little/start/ {github} | | | | /logo.png"
            )
        );
    }

    #[test]
    fn a_failed_build_leaves_the_last_one() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("theme/_templates/default.html", "{{ page.title }}"),
        ]);
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
    }

    /// Each file in `dist`, with its contents.
    fn contents_of(dist: &Path) -> BTreeMap<String, Vec<u8>> {
        (files_in(dist).into_iter())
            .map(|file| {
                let contents = fs::read(dist.join(&file)).unwrap();
                (file, contents)
            })
            .collect()
    }

    #[test]
    fn a_rebuild_leaves_what_a_build_would() {
        // With the built-in theme, so with search, and each kind of nav.
        for nav_prune in [false, true] {
            let (_tmp, site) = scratch(&[
                (
                    "little.yml",
                    &format!(
                        "site:\n  name: Test\n  navigation:\n    prune: {nav_prune}\n  \
                         redirects:\n    - {{ to: /guide/install, from: [/setup, /a.html] }}\n    \
                         - {{ to: /guide/*, from: [/manual/*] }}\n"
                    ),
                ),
                (
                    "docs/README.md",
                    "# Home\n\nTo [install](guide/install.md).\n",
                ),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/guide/install.md", "# Install\n\nFirst.\n"),
                ("docs/guide/old.md", "# Old\n"),
                ("docs/guide/images/diagram.png", "png"),
                ("docs/notes", "A file with no extension.\n"),
                ("docs/api/reference.md", "# Reference\n"),
            ]);
            let dist = site.join("dist");
            let check = |rebuilt: &Built, edit: &str| {
                let left = contents_of(&dist);
                // As of the same time, which each page is rendered from.
                let built = build_as_of(&site, rebuilt.time).unwrap();
                let fresh = contents_of(&dist);
                let names = |files: &BTreeMap<String, Vec<u8>>| -> Vec<String> {
                    files.keys().cloned().collect()
                };
                assert_eq!(names(&left), names(&fresh), "{edit}");
                for (file, contents) in &fresh {
                    assert!(left[file] == *contents, "{file} differs, {edit}");
                }
                assert_eq!(rebuilt.files, built.files, "{edit}");
                assert_eq!(rebuilt.origins, built.origins, "{edit}");
                assert_eq!(rebuilt.urls, built.urls, "{edit}");
                built
            };
            let built = build(&site).unwrap();

            // A page's text changes.
            write(
                &site,
                &[("docs/guide/install.md", "# Install\n\nSecond.\n")],
            );
            let built = check(&rebuild(&site, &built).unwrap(), "after an edit");

            // A page gets another title, so the nav changes, and the
            // redirects to it; a page and a folder go, and a page comes,
            // each with the redirect from where it was in its folder; and
            // a file changes.
            write(
                &site,
                &[
                    ("docs/guide/install.md", "# Installing\n\nSecond.\n"),
                    ("docs/guide/new.md", "# New\n"),
                    ("docs/guide/images/diagram.png", "another png"),
                ],
            );
            fs::remove_file(site.join("docs/guide/old.md")).unwrap();
            fs::remove_dir_all(site.join("docs/api")).unwrap();
            let built = check(&rebuild(&site, &built).unwrap(), "after pages change");

            // A file gives way to a folder of pages, and a page to a file.
            fs::remove_file(site.join("docs/notes")).unwrap();
            fs::remove_file(site.join("docs/guide/new.md")).unwrap();
            write(
                &site,
                &[
                    ("docs/notes/README.md", "# Notes\n"),
                    ("docs/guide/new", "A file with no extension.\n"),
                ],
            );
            let built = check(&rebuild(&site, &built).unwrap(), "after files move");

            // The config changes, and so the theme's files, and the
            // redirects go, and each page's own file with them.
            let config = format!(
                "site:\n  name: Renamed\n  navigation:\n    prune: {nav_prune}\n  \
                 search:\n    enabled: false\npage:\n  source:\n    enabled: false\n"
            );
            write(&site, &[("little.yml", &config)]);
            assert!(dist.join("guide/install.md").is_file());
            check(&rebuild(&site, &built).unwrap(), "after the config changes");
            assert!(!dist.join("guide/install.md").exists());
        }
    }

    #[test]
    fn a_rebuild_writes_only_what_differs() {
        let template = "{{ page.title }}: {{ page.content | safe }} \
                        [{% for item in site.navigation.items %}{{ item.title }} {% endfor %}]";
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/guide.md", "# Guide\n\nFirst.\n"),
            ("docs/about.md", "# About\n"),
            ("docs/logo.svg", "<svg/>"),
            ("theme/_templates/default.html", template),
        ]);
        let dist = site.join("dist");
        let files = [
            "index.html",
            "guide.html",
            "about.html",
            "logo.svg",
            "guide.md",
            "about.md",
        ];
        // Each file that's left as it was, of those a build writes, found by
        // writing over them all first.
        let left = |last: &Built| {
            for file in files {
                fs::write(dist.join(file), "left").unwrap();
            }
            let built = rebuild(&site, last).unwrap();
            let left: Vec<_> = (files.into_iter())
                .filter(|file| fs::read(dist.join(file)).is_ok_and(|read| read == b"left"))
                .collect();
            (built, left)
        };
        let built = build(&site).unwrap();

        // Nothing's written when nothing's changed.
        let (built, unwritten) = left(&built);
        assert_eq!(unwritten, files);

        // A page whose text changes is rendered again, alone, and its file
        // copied again.
        write(&site, &[("docs/guide.md", "# Guide\n\nSecond.\n")]);
        let (built, unwritten) = left(&built);
        let others = ["index.html", "about.html", "logo.svg", "about.md"];
        assert_eq!(unwritten, others);
        let guide = fs::read_to_string(dist.join("guide.html")).unwrap();
        assert!(guide.contains("<p>Second.</p>"), "{guide}");
        let copied = fs::read_to_string(dist.join("guide.md")).unwrap();
        assert_eq!(copied, "# Guide\n\nSecond.\n");

        // Its title is in the nav, so in every page.
        write(&site, &[("docs/guide.md", "# Guiding\n\nSecond.\n")]);
        let (built, unwritten) = left(&built);
        assert_eq!(unwritten, ["logo.svg", "about.md"]);

        // So is the theme's template, but a file is only itself, and so is
        // a page's own.
        let theme = site.join("theme/_templates/default.html");
        fs::write(&theme, "{{ page.title }}").unwrap();
        fs::write(site.join("docs/logo.svg"), "<svg></svg>").unwrap();
        let (built, unwritten) = left(&built);
        assert_eq!(unwritten, ["guide.md", "about.md"]);
        assert_eq!(fs::read(dist.join("logo.svg")).unwrap(), b"<svg></svg>");

        // A page that's gone from the source goes, with its file and its
        // folder, and one that's gone from the destination is written again.
        fs::remove_file(site.join("docs/about.md")).unwrap();
        let built = rebuild(&site, &built).unwrap();
        assert!(!dist.join("about").exists());
        assert!(!dist.join("about.html").exists());
        assert!(!dist.join("about.md").exists());
        fs::remove_file(dist.join("guide.html")).unwrap();
        let built = rebuild(&site, &built).unwrap();
        assert_eq!(
            fs::read_to_string(dist.join("guide.html")).unwrap(),
            "Guiding"
        );

        // Without its destination, or after a build into another, it's a
        // build.
        fs::remove_dir_all(&dist).unwrap();
        let built = rebuild(&site, &built).unwrap();
        assert_eq!(fs::read_to_string(dist.join("index.html")).unwrap(), "Home");
        let config = format!("{IN_THEME}  destination: ./public\n");
        write(&site, &[("little.yml", &config)]);
        rebuild(&site, &built).unwrap();
        let public = fs::read_to_string(site.join("public/index.html")).unwrap();
        assert_eq!(public, "Home");
    }

    #[test]
    fn a_rebuild_renders_every_page_for_a_plugins_html_hook() {
        let plugin = serde_json::json!({
            "wasm": crate::plugin::tests::test_plugin(),
            "html": { "append": "!" },
        });
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                &format!("{IN_THEME}  plugins:\n    test: {plugin}\n"),
            ),
            ("docs/README.md", "# Home\n"),
            ("theme/_templates/default.html", "{{ page.title }}"),
        ]);
        let built = build(&site).unwrap();
        let home = site.join("dist/index.html");
        assert_eq!(fs::read_to_string(&home).unwrap(), "Home!");

        fs::write(&home, "left").unwrap();
        rebuild(&site, &built).unwrap();
        assert_eq!(fs::read_to_string(&home).unwrap(), "Home!");
    }

    #[test]
    fn a_rebuild_parses_only_the_pages_that_differ() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/guide.md", "# Guide\n\nFirst.\n"),
            ("docs/chart.md", "# Chart\n\n```mermaid\ngantt\n```\n"),
            ("theme/_templates/default.html", "{{ page.content | safe }}"),
        ]);
        // How many pages a build keeps the parse of, and how many of those
        // it had from the build before.
        let kept = |built: &Built, last: &Built| {
            let from_last = built
                .parsed
                .keys()
                .filter(|key| last.parsed.contains_key(key));
            (built.parsed.len(), from_last.count())
        };
        let built = build(&site).unwrap();
        // A diagram is drawn as of now, so its page is parsed each time.
        assert_eq!(built.parsed.len(), 2);

        let rebuilt = rebuild(&site, &built).unwrap();
        assert_eq!(kept(&rebuilt, &built), (2, 2));

        // A page's text changes, and it's parsed again, alone.
        write(&site, &[("docs/guide.md", "# Guide\n\nSecond.\n")]);
        let built = rebuild(&site, &rebuilt).unwrap();
        assert_eq!(kept(&built, &rebuilt), (2, 1));
        let guide = fs::read_to_string(site.join("dist/guide.html")).unwrap();
        assert!(guide.contains("<p>Second.</p>"), "{guide}");

        // A page that moves leads its links from another folder.
        fs::create_dir(site.join("docs/more")).unwrap();
        fs::rename(site.join("docs/guide.md"), site.join("docs/more/guide.md")).unwrap();
        let moved = rebuild(&site, &built).unwrap();
        assert_eq!(kept(&moved, &built), (2, 1));

        // What every page has, under `page`, is read once a page is parsed,
        // so a change to it parses none again.
        let config = format!("{IN_THEME}page:\n  outline:\n    depth:\n      max: 2\n");
        write(&site, &[("little.yml", &config)]);
        let outlined = rebuild(&site, &moved).unwrap();
        assert_eq!(kept(&outlined, &moved), (2, 2));

        // The site's settings reach every page.
        let config = format!("{IN_THEME}  markdown:\n    highlight: false\n");
        write(&site, &[("little.yml", &config)]);
        let built = rebuild(&site, &outlined).unwrap();
        assert_eq!(kept(&built, &outlined), (2, 0));

        // A plugin that renders code blocks can make anything of a page.
        let plugin = serde_json::json!({
            "wasm": crate::plugin::tests::test_plugin(),
            "languages": ["test"],
        });
        let config = format!("{IN_THEME}  plugins:\n    test: {plugin}\n");
        write(&site, &[("little.yml", &config)]);
        let hooked = rebuild(&site, &built).unwrap();
        assert_eq!(hooked.parsed.len(), 0);
    }

    #[test]
    fn static_files_cannot_take_the_place_of_pages() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/guide/install.md", "# Install\n"),
            ("theme/_templates/default.html", "{{ page.title }}"),
            ("theme/_templates/404.html", "Not found"),
            ("theme/_static/guide/style.css", "body {}\n"),
        ]);
        // Beside the pages is fine...
        build(&site).unwrap();
        assert!(site.join("dist/guide/style.css").is_file());

        // ...but not on one, or beside one, with its name, whether it's the
        // theme's or one in the source.
        let built_from = |file: &str| {
            let source = site.join("docs").join(native(file));
            format!("the page built from {}", source.display())
        };
        for folder in [site.join("theme").join("_static"), site.join("docs")] {
            for (file, page) in [
                ("index.html", built_from("README.md")),
                ("guide/install", built_from("guide/install.md")),
                ("guide/install.html", built_from("guide/install.md")),
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
    }

    #[test]
    fn pages_cannot_take_each_others_place() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/guide/README.md", "# Guide\n"),
            ("theme/_templates/default.html", "{{ page.title }}"),
        ]);
        build(&site).unwrap();

        // `guide.md` would be the page at `/guide`, which a host can't tell
        // from `/guide/`, so the build stops, and leaves the last one in
        // place.
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
    }

    #[test]
    fn a_page_cant_be_beside_a_folder_of_its_name() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("docs/guide.md", "# Guide\n"),
            ("theme/_templates/default.html", "{{ page.title }}"),
        ]);
        build(&site).unwrap();
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();
        assert_eq!(read("guide.html"), "Guide");

        // The page is at `/guide`, which a host can't tell from `/guide/`,
        // the folder, so anything in a folder of its name stops the build:
        // a page, or any other file, in the source or in the theme.
        let docs = site.join("docs");
        let guide = docs.join("guide.md");
        for (file, name) in [
            ("docs/guide/install.md", "the page built from "),
            ("docs/guide/images/setup.png", ""),
            ("theme/_static/guide/style.css", ""),
        ] {
            let file = site.join(native(file));
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, "# Install\n").unwrap();
            let message = format!("{:#}", build(&site).unwrap_err());
            let expected = format!(
                "the page built from {} clashes with {name}{}: a page can't be beside a folder \
                 of its name, so make it the folder's README, or rename one",
                guide.display(),
                file.display()
            );
            assert_eq!(message, expected);
            fs::remove_file(&file).unwrap();
        }
        // The last build is left in place.
        assert_eq!(read("guide.html"), "Guide");

        // As the folder's README, it's the folder's own page, at `/guide/`.
        fs::rename(&guide, docs.join(native("guide/README.md"))).unwrap();
        fs::write(docs.join(native("guide/install.md")), "# Install\n").unwrap();
        build(&site).unwrap();
        assert_eq!(read("guide/index.html"), "Guide");
        assert_eq!(read("guide/install.html"), "Install");
        assert!(!dist.join("guide.html").exists());
    }

    #[test]
    fn each_page_is_indexed_for_search() {
        let (_tmp, site) = scratch(&[
            (
                "little.yml",
                "site:\n  theme:\n    source: ./theme\n  url: https://example.com/docs\n",
            ),
            ("docs/README.md", "# Home\n\nWelcome aboard.\n"),
            (
                "docs/guide/install.md",
                "---\ntitle: Installing\n---\n# Install\n\n## From source\n\nRun cargo.\n",
            ),
            ("theme/little.yml", "site: { search: { enabled: true } }\n"),
            (
                "theme/_templates/default.html",
                "<html lang=\"fr\"><header>Theme header</header>{{ page.content }}</html>",
            ),
            ("theme/_templates/404.html", "<html lang=\"fr\">Lost</html>"),
        ]);
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
        let site_cfg = format!("{IN_THEME}  search:\n    enabled: false\n");
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        build(&site).unwrap();
        assert!(dist.join("index.html").is_file());
        assert!(!dist.join("pagefind").exists());
    }

    #[test]
    fn a_site_or_a_page_can_leave_pages_out_of_search() {
        let site_cfg = |exclude: &str| {
            format!("{IN_THEME}  search:\n    enabled: true\n    exclude: {exclude}\n")
        };
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg("[/other/, /this/old, /typo/]")),
            ("docs/README.md", "# Home\n"),
            ("docs/this/old.md", "# Old\n"),
            ("docs/this/new.md", "# New\n"),
            (
                "docs/this/secret.md",
                "---\nsearch:\n  enabled: false\n---\n# Secret\n",
            ),
            ("docs/other/README.md", "# Other\n"),
            ("docs/other/a.md", "# A\n"),
            (
                "docs/other/deep/b.md",
                "---\nsearch:\n  enabled: true\n---\n# B\n",
            ),
            ("docs/others.md", "# Others\n"),
            ("theme/_templates/default.html", "{{ page.content }}"),
        ]);
        let built = build(&site).unwrap();
        let dist = site.join("dist");

        // Each page is built, but search leaves out the page at each URL in
        // `exclude`, and those under it, but not one that only starts with
        // the same letters, and a page's own `search.enabled` has the last
        // word.
        assert!(dist.join("other/a.html").is_file());
        let urls = indexed_urls(&dist);
        assert_eq!(urls, ["/", "/other/deep/b", "/others", "/this/new"]);

        // A URL that leaves out no page is warned of, as it may be a typo.
        assert_eq!(
            warnings_of(&built),
            ["`site.search.exclude` lists /typo/, but there's no page at that URL, or under it"]
        );

        // `/` leaves out every page, but one whose own `search` is `true`...
        fs::write(site.join("little.yml"), site_cfg("[/]")).unwrap();
        let built = build(&site).unwrap();
        assert_eq!(warnings_of(&built), Vec::<String>::new());
        assert_eq!(indexed_urls(&dist), ["/other/deep/b"]);

        // ...and with none left, search's files are still written, for the
        // theme's search box, but with no index in any language.
        fs::write(site.join("docs/other/deep/b.md"), "# B\n").unwrap();
        build(&site).unwrap();
        let entry = fs::read_to_string(dist.join("pagefind/pagefind-entry.json")).unwrap();
        assert!(entry.contains(r#""languages":{}"#), "{entry}");
        assert!(dist.join("pagefind/pagefind.js").is_file());

        // A page's `search.enabled` must be `true` or `false`, even with
        // search off, and its `search` its settings, not either of them...
        fs::write(site.join("little.yml"), IN_THEME).unwrap();
        let page = site.join(native("docs/this/new.md"));
        fs::write(&page, "---\nsearch:\n  enabled: no\n---\n# New\n").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "rendering {}: `search.enabled` must be `true`, to index the page for search, or \
             `false`, to leave it out",
            page.display()
        );
        assert_eq!(message, expected);
        fs::write(&page, "---\nsearch: false\n---\n# New\n").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "rendering {}: `search` must be its settings, like `enabled: false`",
            page.display()
        );
        assert_eq!(message, expected);

        // ...and can't turn it on for a site that has it off.
        fs::write(&page, "---\nsearch:\n  enabled: true\n---\n# New\n").unwrap();
        build(&site).unwrap();
        assert!(!dist.join("pagefind").exists());

        // Nor can `exclude`, whose URLs aren't checked while it's off.
        let site_cfg = format!("{IN_THEME}  search:\n    exclude: [/typo/]\n");
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        let built = build(&site).unwrap();
        assert!(!dist.join("pagefind").exists());
        assert_eq!(warnings_of(&built), Vec::<String>::new());
    }

    #[test]
    fn each_page_knows_whether_search_indexes_it() {
        let site_cfg = format!("{IN_THEME}  search:\n    enabled: true\n    exclude: [/old/]\n");
        let (_tmp, site) = scratch(&[
            ("little.yml", &site_cfg),
            ("docs/README.md", "# Home\n"),
            ("docs/old/README.md", "# Old\n"),
            (
                "docs/old/kept.md",
                "---\nsearch:\n  enabled: true\n---\n# Kept\n",
            ),
            (
                "docs/secret.md",
                "---\nsearch:\n  enabled: false\n---\n# Secret\n",
            ),
            (
                "theme/_templates/default.html",
                "<html lang=\"en\">{{ page.search.enabled }}</html>",
            ),
            (
                "theme/_templates/404.html",
                "<html lang=\"en\">{{ page.search.enabled }}</html>",
            ),
        ]);

        build(&site).unwrap();

        // `page.search.enabled` is `false` for a page under a URL in
        // `site.search.exclude`, unless the page's own says otherwise, and
        // for the 404 page, which search never indexes. MiniJinja writes a
        // boolean as Jinja2 does.
        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        for (path, enabled) in [
            ("index.html", "True"),
            ("old/index.html", "False"),
            ("old/kept.html", "True"),
            ("secret.html", "False"),
            (NOT_FOUND_FILE, "False"),
        ] {
            assert_eq!(
                read(path),
                format!("<html lang=\"en\">{enabled}</html>"),
                "{path}"
            );
        }
        assert_eq!(indexed_urls(&site.join("dist")), ["/", "/old/kept"]);
    }

    #[test]
    fn nothing_can_take_the_place_of_search_files() {
        let (_tmp, site) = scratch(&[
            ("little.yml", IN_THEME),
            ("docs/README.md", "# Home\n"),
            ("theme/little.yml", "site: { search: { enabled: true } }\n"),
            ("theme/_templates/default.html", "{{ page.title }}"),
        ]);
        // The page of the folder at /pagefind/, or a file beside search's
        // files, is fine...
        fs::create_dir_all(site.join("docs/pagefind")).unwrap();
        fs::write(site.join("docs/pagefind/README.md"), "# Pagefind\n").unwrap();
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
            let expected =
                format!("{name} clashes with /pagefind/, where little writes files for search");
            assert_eq!(message, expected);
            fs::remove_file(file).unwrap();
        }

        // ...or a page beside their folder, which is named as the folder
        // is, so a host can't tell their URLs apart...
        fs::remove_dir_all(docs.join("pagefind")).unwrap();
        let page = docs.join("pagefind.md");
        fs::write(&page, "# Pagefind\n").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "the page built from {} clashes with /pagefind/, where little writes files for \
             search: a page can't be beside a folder of its name, so make it the folder's \
             README, or rename one",
            page.display()
        );
        assert_eq!(message, expected);
        fs::remove_file(&page).unwrap();

        // ...or one where their folder goes.
        fs::write(docs.join("pagefind"), "a file, not a folder").unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        let expected = format!(
            "{} clashes with /pagefind/, where little writes files for search",
            docs.join("pagefind").display()
        );
        assert_eq!(message, expected);

        // The last build is left in place.
        assert!(dist.join("pagefind/pagefind.js").is_file());
    }

    /// A site in a git repository whose files were `old`, at the tag `v1.4`,
    /// and are now `new`, uncommitted. Its destination is ignored.
    fn versioned_site(old: &[(&str, &str)], new: &[(&str, &str)]) -> (TempDir, PathBuf) {
        let (tmp, site) = scratch(old);
        fs::write(site.join(".gitignore"), "dist/\n").unwrap();
        git::tests::init(&site);
        git::tests::commit_all(&site);
        git::tests::git_in(&site, &["tag", "v1.4"]);
        for (file, _) in old {
            fs::remove_file(site.join(file)).unwrap();
        }
        write(&site, new);
        (tmp, site)
    }

    #[test]
    fn each_version_is_built_from_its_git_ref() {
        use git::tests::{commit_all, git_in};
        let template = "{{ site.name }} {{ site.version }} \
                        | {% for v in site.versions %}{{ v.title }}{{ '*' if v.root }} {{ v.url | safe }} {% endfor %}\
                        | {% for v in page.versions %}{{ v.title }}{{ '*' if v.root }} {{ v.url | safe }} {% endfor %}\
                        | {{ page.repository.view.url | safe if page.repository.view.url }} \
                        | {{ page.repository.edit.url | safe if page.repository.edit.url }}\n{{ page.content | safe }}";
        let site_cfg = |versions: &str| {
            format!(
                "{IN_THEME}  name: New\n  url: https://example.com/proj\n  \
                 repository:\n    url: https://github.com/o/r\n  versions:\n{versions}"
            )
        };
        let (_tmp, site) = versioned_site(
            &[
                // Of its own settings, only its `source` and
                // `navigation.items` count.
                (
                    "little.yml",
                    "site:\n  name: Old\n  url: https://example.com/old\n",
                ),
                ("docs/README.md", "# Home\n\nSee [the old page](old.md).\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/old.md", "# Old\n"),
            ],
            &[
                (
                    "little.yml",
                    &site_cfg(
                        "    - { title: \"2.0\", url: / }\n    \
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
            head("guide.html"),
            format!(
                "New 2.0 | {all}| 2.0* /proj/guide 1.4 /proj/1.4/guide/ | {} | {}",
                file("main", "guide.md"),
                edit("guide.md")
            )
        );
        assert_eq!(
            head("new.html"),
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
            head("1.4/old.html"),
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
        let old = Path::new("1.4/old.html");
        assert_eq!(built.origins[old], "v1.4:docs/old.md");
        assert_eq!(built.urls[old], "/1.4/old");
        // Each page's file is copied beside it, as it was at the ref.
        let from_refs = [
            "1.4/index.html",
            "1.4/README.md",
            "1.4/guide/index.html",
            "1.4/guide/README.md",
            "1.4/old.html",
            "1.4/old.md",
        ];
        let from_refs: HashSet<_> = from_refs.into_iter().map(PathBuf::from).collect();
        assert_eq!(built.from_refs, from_refs);

        // The version at `/` can be from a ref, too, and listed anywhere, and
        // the site's files, as they are, in a folder, like any other, as deep
        // as its `url` says.
        commit_all(&site);
        git_in(&site, &["tag", "v2.0"]);
        let v2_0 = git_in(&site, &["rev-parse", "v2.0^{commit}"]);
        let versions = "    - { title: dev, url: /dev/ }\n    \
                        - { title: \"2.0\", url: /, ref: v2.0 }\n    \
                        - { title: \"1.4\", url: /versions/1.4, ref: v1.4 }\n";
        fs::write(site.join("little.yml"), site_cfg(versions)).unwrap();
        fs::write(site.join("docs/new.md"), "# Newer\n").unwrap();
        let built = build(&site).unwrap();
        let all = "dev /proj/dev/ 2.0* /proj/ 1.4 /proj/versions/1.4/ ";
        let new = "dev /proj/dev/new 2.0* /proj/new 1.4 /proj/versions/1.4/ ";
        assert_eq!(
            head("new.html"),
            format!("New 2.0 | {all}| {new}| {} | ", file(&v2_0, "new.md"))
        );
        assert!(read("new.html").contains("<h1 id=\"new\">New<"));
        assert_eq!(
            head("dev/new.html"),
            format!(
                "New dev | {all}| {new}| {} | {}",
                file("main", "new.md"),
                edit("new.md")
            )
        );
        assert!(read("dev/new.html").contains("<h1 id=\"newer\">Newer<"));
        assert_eq!(
            head("versions/1.4/old.html"),
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
            built.urls[Path::new("versions/1.4/old.html")],
            "/versions/1.4/old"
        );
        assert_eq!(
            read(NOT_FOUND_FILE),
            "/proj/dev/ /proj/ /proj/versions/1.4/ "
        );
        assert!(!dist.join("dev").join(NOT_FOUND_FILE).exists());
        assert!(!dist.join("1.4").exists());
        assert!(built.from_refs.contains(Path::new("new.html")));
        assert!(!built.from_refs.contains(Path::new("dev/new.html")));
        assert!((built.from_refs).contains(Path::new("versions/1.4/old.html")));
    }

    #[test]
    fn a_version_from_a_ref_has_its_own_pages_and_nav_in_the_sites_config() {
        use git::tests::{commit_all, git_in};
        let site_cfg = |version: &str| {
            format!(
                "{IN_THEME}  name: New\n  source:\n    directory: ./content\n  \
                 url: https://example.com/proj\n  \
                 versions:\n    - {{ title: \"2.0\", url: / }}\n    \
                 - {{ title: \"1.4\", url: /1.4, {version} }}\n"
            )
        };
        // At the ref, the site wasn't built with little, so has no
        // little.yml.
        let (_tmp, site) = versioned_site(
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
                     | {% for item in site.navigation.items %}{{ item.title }} {{ item.url | safe }}; {% endfor %}\n\
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
        let moved = site_cfg("ref: v1.4, source: { directory: ./documentation }");
        fs::write(site.join("little.yml"), moved).unwrap();
        build(&site).unwrap();
        assert_eq!(
            head("1.4/index.html"),
            "New /proj/1.4/style.css | Documentation /proj/1.4/; "
        );
        assert_eq!(read("1.4/style.css"), "old {}\n");
        assert_eq!(read("style.css"), "body {}\n");

        // Of a little.yml at the ref, its `site.source` and
        // `site.navigation.items` count, but no other setting.
        let own = "site:\n  name: Old\n  source:\n    directory: ./documentation\n  \
                   navigation:\n    items:\n      - { title: Docs, url: / }\n";
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
        // But its entry's `navigation.items` win, with each URL from its
        // home.
        let own = "ref: v1.5, navigation: { items: [{ title: Start, url: / }] }";
        fs::write(site.join("little.yml"), site_cfg(own)).unwrap();
        build(&site).unwrap();
        assert_eq!(
            head("1.4/index.html"),
            "New /proj/style.css | Start /proj/1.4/; "
        );
    }

    #[test]
    fn a_site_whose_versions_have_no_ref_needs_no_repository() {
        let (_tmp, site) = scratch(&[
            // little says which version is at `/`, whatever an entry
            // says.
            (
                "little.yml",
                "site:\n  theme:\n    source: ./theme\n  \
                 versions:\n    - { title: Latest, url: /, root: false }\n",
            ),
            ("docs/README.md", "# Home\n"),
            (
                "theme/_templates/default.html",
                "{{ site.version }} | {{ site.versions | map(attribute='url') | join(' ') | safe }} \
                     | {{ page.versions | map(attribute='url') | join(' ') | safe }} \
                     | {{ page.versions | map(attribute='root') | join(' ') }}",
            ),
        ]);
        build(&site).unwrap();
        let home = fs::read_to_string(site.join("dist/index.html")).unwrap();
        // MiniJinja writes a boolean as Jinja2 does.
        assert_eq!(home, "Latest | / | / | True");
    }

    #[test]
    fn each_version_leaves_out_the_pages_search_excludes() {
        let (_tmp, site) = versioned_site(
            &[("docs/README.md", "# Home\n"), ("docs/old.md", "# Old\n")],
            &[
                (
                    "little.yml",
                    "site:\n  theme:\n    source: ./theme\n  \
                     search:\n    enabled: true\n    exclude: [/old, /new, /gone/]\n  \
                     versions:\n    - { title: \"2.0\", url: / }\n    \
                     - { title: \"1.4\", url: /1.4, ref: v1.4 }\n",
                ),
                ("docs/README.md", "# Home\n"),
                ("docs/new.md", "# New\n"),
                ("theme/_templates/default.html", "{{ page.content }}"),
            ],
        );
        let built = build(&site).unwrap();

        // Each URL is from the root of each version, and is warned of only
        // if no version has a page there.
        let dist = site.join("dist");
        assert_eq!(indexed_urls(&dist), ["/"]);
        assert_eq!(indexed_urls(&dist.join("1.4")), ["/"]);
        assert_eq!(
            warnings_of(&built),
            ["`site.search.exclude` lists /gone/, but there's no page at that URL, or under it"]
        );
    }

    #[test]
    fn each_version_leaves_out_what_its_own_source_excludes() {
        // The site's `source.exclude`, and what's beside version 1.4's
        // `ref` in its entry.
        let site_cfg = |exclude: &str, own: &str| {
            format!(
                "{IN_THEME}  source:\n    exclude: {exclude}\n  \
                 versions:\n    - {{ title: \"2.0\", url: / }}\n    \
                 - {{ title: \"1.4\", url: /1.4, ref: v1.4{own} }}\n"
            )
        };
        let (_tmp, site) = versioned_site(
            &[
                (
                    "little.yml",
                    "site:\n  source:\n    exclude: [/drafts/, /gone.md]\n",
                ),
                ("docs/README.md", "# Home\n"),
                ("docs/drafts/wip.md", "# WIP\n"),
                ("docs/notes/old.md", "# Old notes\n"),
            ],
            &[
                ("little.yml", &site_cfg("[/notes/, /typo/]", "")),
                ("docs/README.md", "# Home\n"),
                ("docs/drafts/wip.md", "# WIP\n"),
                ("docs/notes/new.md", "# New notes\n"),
                ("theme/_templates/default.html", "{{ page.title }}"),
            ],
        );
        let dist = site.join("dist");
        let warning = |path: &str| {
            format!(
                "`site.source.exclude` lists {path}, but the source has no file at that path, \
                 or under it, to leave out"
            )
        };

        // Each version leaves out what its own little.yml lists: 2.0 its
        // notes, and 1.4, by the one at its ref, its drafts.
        let built = build(&site).unwrap();
        assert!(dist.join("drafts/wip.html").is_file());
        assert!(!dist.join("notes").exists());
        assert!(dist.join("1.4/notes/old.html").is_file());
        assert!(!dist.join("1.4/drafts").exists());
        assert_eq!(
            warnings_of(&built),
            [warning("/typo/"), warning("/gone.md")]
        );

        // Its entry's `source` goes over the one at its ref, so it lists its
        // own. A path is warned of once, and only if it leaves nothing out in
        // every version that lists it: 2.0 has no `notes/old.md`, but 1.4
        // has.
        let own = ", source: { exclude: [/notes/old.md, /typo/] }";
        let site_cfg = site_cfg("[/notes/old.md, /typo/]", own);
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        let built = build(&site).unwrap();
        assert!(dist.join("notes/new.html").is_file());
        assert!(dist.join("1.4/drafts/wip.html").is_file());
        assert!(!dist.join("1.4/notes").exists());
        assert_eq!(warnings_of(&built), [warning("/typo/")]);
    }

    #[test]
    fn each_version_has_a_folders_redirects_for_its_own_pages() {
        let site_cfg = format!(
            "{IN_THEME}  redirects:\n    - {{ to: /tools/*, from: [/widgets/*] }}\n    \
             - {{ to: /kit/*, from: [/bits/*] }}\n  \
             versions:\n    - {{ title: \"2.0\", url: / }}\n    \
             - {{ title: \"1.4\", url: /1.4, ref: v1.4 }}\n"
        );
        let (_tmp, site) = versioned_site(
            &[
                ("docs/README.md", "# Home\n"),
                ("docs/widgets/this.md", "# This\n"),
                ("docs/tools/early.md", "# Early\n"),
            ],
            &[
                ("little.yml", &site_cfg),
                ("docs/README.md", "# Home\n"),
                ("docs/tools/this.md", "# This\n"),
                ("docs/tools/that.md", "# That\n"),
                ("theme/_templates/default.html", "{{ page.title }}"),
                (
                    "theme/_templates/redirect.html",
                    "{{ page.url | safe }} to {{ page.redirect.url | safe }}",
                ),
            ],
        );
        let built = build(&site).unwrap();
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();

        // Each version has one for each of its own pages under the folder:
        // 1.4 still has the page 2.0 has moved, and had one there already.
        assert_eq!(
            read("widgets/this/index.html"),
            "/widgets/this to /tools/this"
        );
        assert_eq!(
            read("widgets/that/index.html"),
            "/widgets/that to /tools/that"
        );
        assert_eq!(read("1.4/widgets/this.html"), "This");
        assert_eq!(
            read("1.4/widgets/early/index.html"),
            "/1.4/widgets/early to /1.4/tools/early"
        );
        assert!(!dist.join("widgets/early").exists());
        assert!(!dist.join("1.4/widgets/that").exists());

        // One that no version has a page for is warned of.
        assert_eq!(
            warnings_of(&built),
            [
                "`site.redirects` sends /bits/* to /kit/*, but no version of the site has a page \
                 under /kit/ without a page or file where it would be under /bits/"
            ]
        );
    }

    #[test]
    fn each_version_has_the_redirects_it_can() {
        let site_cfg = |redirects: &str| {
            format!(
                "{IN_THEME}  redirects:\n    - {{ to: /guide/install, from: [/guide/setup] }}\n    \
                 - {{ to: /guide/install, from: [/faq] }}\n    \
                 - {{ to: https://example.org/, from: [/elsewhere] }}\n{redirects}  \
                 versions:\n    - {{ title: \"2.0\", url: / }}\n    \
                 - {{ title: \"1.4\", url: /1.4, ref: v1.4 }}\n"
            )
        };
        let (_tmp, site) = versioned_site(
            &[
                ("docs/README.md", "# Home\n"),
                ("docs/guide/setup.md", "# Setup\n"),
                ("docs/faq.md", "# FAQ\n"),
            ],
            &[
                (
                    "little.yml",
                    &site_cfg(
                        "    - { to: /gone, from: [/went] }\n    - { to: https://example.org/, from: [/] }\n",
                    ),
                ),
                ("docs/README.md", "# Home\n"),
                ("docs/guide/install.md", "# Install\n"),
                ("theme/_templates/default.html", "{{ page.title }}"),
                (
                    "theme/_templates/redirect.html",
                    "{{ page.url | safe }} to {{ page.redirect.url | safe }} \
                     |{% for v in page.versions %} {{ v.url | safe }}{% endfor %}",
                ),
            ],
        );
        let built = build(&site).unwrap();
        let dist = site.join("dist");
        let read = |path: &str| fs::read_to_string(dist.join(path)).unwrap();

        // Each URL is from the root of each version. A version has a
        // redirect if it has a page or file where it leads, and none where
        // it's from: 1.4 still has the pages 2.0 has moved, where they were.
        assert_eq!(
            read("guide/setup/index.html"),
            "/guide/setup to /guide/install | / /1.4/"
        );
        assert_eq!(read("1.4/guide/setup.html"), "Setup");
        assert_eq!(read("faq/index.html"), "/faq to /guide/install | / /1.4/");
        assert_eq!(read("1.4/faq.html"), "FAQ");
        // One to another site is in each that has nothing where it's from.
        assert_eq!(
            read("elsewhere/index.html"),
            "/elsewhere to https://example.org/ | / /1.4/"
        );
        assert_eq!(
            read("1.4/elsewhere/index.html"),
            "/1.4/elsewhere to https://example.org/ | / /1.4/"
        );
        let old = Path::new("1.4/elsewhere/index.html");
        assert_eq!(built.urls[old], "/1.4/elsewhere");
        assert_eq!(built.origins[old], "the redirect from /1.4/elsewhere");
        assert!(built.from_refs.contains(old));

        // One that no version has is warned of, as it's likely a mistake.
        assert!(!dist.join("went").exists());
        assert!(!dist.join("1.4/went").exists());
        assert_eq!(read("index.html"), "Home");
        assert_eq!(read("1.4/index.html"), "Home");
        assert_eq!(
            warnings_of(&built),
            [
                "`site.redirects` sends /went to /gone, but no version of the site has a page or \
                 file at /gone without one at /went",
                "`site.redirects` sends / to https://example.org/, but every version of the site \
                 has a page or file at /",
            ]
        );

        // Two from one URL still stop the build, as does one where another
        // version is.
        for (redirect, clash) in [
            (
                "{ to: /, from: [/elsewhere/] }",
                "the redirect from /elsewhere/ clashes with the redirect from /elsewhere",
            ),
            (
                "{ to: /, from: [/1.4] }",
                "the redirect from /1.4 clashes with version `1.4`, built at /1.4/",
            ),
        ] {
            let site_cfg = site_cfg(&format!("    - {redirect}\n"));
            fs::write(site.join("little.yml"), site_cfg).unwrap();
            let message = format!("{:#}", build(&site).unwrap_err());
            assert_eq!(message, clash, "{redirect}");
        }
    }

    #[test]
    fn a_version_cant_be_where_a_page_or_search_is() {
        let site_cfg = |url: &str| {
            format!(
                "{IN_THEME}  versions:\n    - {{ title: \"2.0\", url: / }}\n    \
                 - {{ title: \"1.4\", url: {url}, ref: v1.4 }}\n"
            )
        };
        let (_tmp, site) = versioned_site(
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
                    "{name}{} clashes with version `1.4`, built at {url}/",
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
        for page in [
            "versions/index.html",
            "versions/1.html",
            "versions/1.4/index.html",
            "notes/1.4.html",
        ] {
            assert!(dist.join(page).is_file(), "{page}");
        }

        // Nor can a version be where search's files are.
        let searched = "site: { search: { enabled: true } }\n";
        fs::write(site.join("theme/little.yml"), searched).unwrap();
        fs::write(site.join("little.yml"), site_cfg("/pagefind")).unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        assert_eq!(
            message,
            "version `1.4`, built at /pagefind/ clashes with /pagefind/, where little writes \
             files for search"
        );
    }

    #[test]
    fn a_version_from_a_ref_is_named_in_what_goes_wrong() {
        use git::tests::{commit_all, git_in};
        let site_cfg = |version: &str| {
            format!(
                "{IN_THEME}  versions:\n    - {{ title: \"2.0\", url: / }}\n    \
                 - {{ title: 1.4 (Old), url: /1.4, {version} }}\n"
            )
        };
        let (_tmp, site) = versioned_site(
            &[("docs/README.md", "# Old\n\n$\\qty{1}{m}$\n")],
            &[
                ("little.yml", &site_cfg("ref: v1.4")),
                ("docs/README.md", "# Home\n"),
                ("theme/_templates/default.html", "{{ page.content | safe }}"),
            ],
        );
        // What's warned of in its files names each as git does.
        let built = build(&site).unwrap();
        let warnings = warnings_of(&built);
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
        fs::write(
            site.join("little.yml"),
            "site: { source: { directory: content } }\n",
        )
        .unwrap();
        commit_all(&site);
        git_in(&site, &["tag", "v1.3"]);
        fs::write(site.join("little.yml"), site_cfg("ref: v1.3")).unwrap();
        let message = format!("{:#}", build(&site).unwrap_err());
        assert_eq!(
            message,
            "building version `1.4 (Old)`, from v1.3: there's no source folder at v1.3:content: \
             set the version's `source`, in `site.versions`, to the folder its pages are in at \
             v1.3"
        );
        // ...which its `source` can name.
        let fixed = site_cfg("ref: v1.3, source: { directory: ./docs }");
        fs::write(site.join("little.yml"), fixed).unwrap();
        build(&site).unwrap();
        assert!(site.join("dist/1.4/index.html").is_file());
    }

    #[test]
    fn a_versions_plugins_are_the_sites_reading_its_files_at_its_ref() {
        use git::tests::{commit_all, git_in};
        let site_cfg = |version: &str| {
            format!(
                "{IN_THEME}  plugins:\n    api:\n      wasm: api.wasm\n      read: pkg\n      \
                 files: {{ add: [{{ path: reference/hello.md, read: pkg/hello.md }}] }}\n      \
                 warn: \"can't read pkg/broken.py\"\n  \
                 versions:\n    - {{ title: \"2.0\", url: / }}\n    \
                 - {{ title: \"1.4\", url: /1.4, {version} }}\n"
            )
        };
        // At the ref, the site had no plugins, and the plugin's file, which
        // is built, is never committed.
        let (_tmp, site) = versioned_site(
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
        let hello = read("1.4/reference/hello.html");
        assert!(hello.starts_with("<h1 id=\"hello\">Hello<"), "{hello}");
        let hi = read("reference/hello.html");
        assert!(hi.starts_with("<h1 id=\"hi\">Hi<"), "{hi}");
        // What it warns of, and the pages it adds, say which version they're
        // in.
        let warnings = warnings_of(&built);
        assert_eq!(
            warnings,
            [
                "plugin `api`: can't read pkg/broken.py",
                "plugin `api` at v1.4: can't read pkg/broken.py"
            ]
        );
        assert_eq!(
            built.origins[Path::new("1.4/reference/hello.html")],
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
             folder there: set its `read`, in the version's `plugins`, in `site.versions`, to \
             the folders it reads at v1.3, or turn it off there, with `enabled: false`"
        );
        // ...unless the version's `plugins` say where it reads there, over the
        // site's settings...
        let moved = "ref: v1.3, plugins: { api: { read: lib, \
                     files: { add: [{ path: reference/hello.md, read: lib/hello.md }] } } }";
        fs::write(site.join("little.yml"), site_cfg(moved)).unwrap();
        let built = build(&site).unwrap();
        let hi = read("1.4/reference/hello.html");
        assert!(hi.starts_with("<h1 id=\"hi\">Hi<"), "{hi}");
        assert_eq!(built.warnings.len(), 2);
        // ...or turn it off, for that version alone.
        let off = "ref: v1.3, plugins: { api: { enabled: false } }";
        fs::write(site.join("little.yml"), site_cfg(off)).unwrap();
        let built = build(&site).unwrap();
        assert!(!site.join("dist/1.4/reference").exists());
        assert!(site.join("dist/1.4/index.html").is_file());
        assert!(site.join("dist/reference/hello.html").is_file());
        let warnings = warnings_of(&built);
        assert_eq!(warnings, ["plugin `api`: can't read pkg/broken.py"]);
    }

    #[test]
    fn the_builtin_theme_has_a_menu_of_versions() {
        let pages = [
            ("docs/README.md", "# Home\n"),
            ("docs/guide.md", "# Guide\n"),
        ];
        let mut new = vec![(
            "little.yml",
            "site:\n  versions:\n    - title: \"2.0 (Alpha)\"\n      url: /2.0-alpha\n    \
             - title: \"1.4 (Latest)\"\n      url: /\n      ref: v1.4\n",
        )];
        new.extend(pages);
        let (_tmp, site) = versioned_site(&pages, &new);

        build(&site).unwrap();

        let read = |path: &str| fs::read_to_string(site.join("dist").join(path)).unwrap();
        let latest = read("guide.html");
        assert!(
            latest.contains(
                "<button type=\"button\" popovertarget=\"_versions\">\
                 <span class=\"visually-hidden\">Version </span>1.4 (Latest)</button>"
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
        let alpha = read("2.0-alpha/guide.html");
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
            "site:\n  versions:\n    - { title: Latest, url: / }\n",
        )
        .unwrap();
        build(&site).unwrap();
        let only = read("guide.html");
        assert!(!only.contains("version-menu"), "{only}");
        assert!(!only.contains("version-note"), "{only}");
    }
}
