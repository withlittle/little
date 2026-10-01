//! The build pipeline: the source (`docs/`) + the theme + config -> the
//! destination (`dist/`).

use anyhow::{Context, Result, bail};
use minijinja::{Value as MjValue, context};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::iter::Peekable;
use std::path::{Component, Path, PathBuf};
use std::slice;
use walkdir::WalkDir;

use crate::config::SiteUrl;
use crate::theme::{PageType, StaticFile, Theme};
use crate::{config, render};

/// Where a site's 404 page is written: the root of the destination, which is
/// where static hosts look for it.
pub const NOT_FOUND_FILE: &str = "404.html";

/// The file a build leaves in its destination, so the next build knows the
/// folder is little's to wipe.
const MARKER_FILE: &str = ".little-build";

/// A rendered content page.
struct Page {
    /// Path relative to the source folder (e.g. `guide/install.md`).
    rel: PathBuf,
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
}

impl Page {
    /// Is this a folder's `README.md` (its home page)?
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
    html: String,
    /// Which page it is, for an error: "the page built from docs/README.md".
    name: String,
}

/// A site that's been built.
#[derive(Clone, Debug)]
pub struct Built {
    /// The folder it was built into.
    pub destination: PathBuf,
    /// Where it's published, which `little serve` serves it under.
    pub url: SiteUrl,
}

/// Build the site rooted at `site_dir`.
///
/// Pages, and the nav and links between them, are worked out at their URLs
/// from the root of the site, like `/guide/`. Only what the theme gets has
/// the path of the site's `url` before them, like `/my-project/guide/`.
pub fn build(site_dir: &Path) -> Result<Built> {
    let config::Config {
        values: site_cfg,
        themes,
        source_dir,
        destination_dir,
        toc_depths,
        url: site_url,
        ..
    } = config::load(site_dir)?;
    if !source_dir.is_dir() {
        bail!("there's no source folder at {}", source_dir.display());
    }
    let theme = Theme::load(&themes, &site_url)?;

    // Collect and render every Markdown file in the source folder but hidden
    // ones, without walking hidden folders. The source folder itself (depth 0)
    // can be hidden.
    let mut pages = Vec::new();
    let entries = WalkDir::new(&source_dir)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !is_hidden(Path::new(entry.file_name())));
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let rel = path.strip_prefix(&source_dir)?.to_path_buf();
        let raw =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let rendered = render::parse_document(&raw, &toc_depths, &site_url)
            .with_context(|| format!("rendering {}", path.display()))?;

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
        let out_path = out_path_for_url(&destination_dir, &url);
        let page_type = page_type(&rel);

        pages.push(Page {
            rel,
            url,
            out_path,
            title,
            frontmatter,
            html: rendered.html,
            toc: nest_headings(&rendered.toc),
            page_type,
        });
    }

    // Deterministic output order (nav has its own ordering), and so a
    // deterministic error for files that would be the same page.
    pages.sort_by(|a, b| (&a.url, &a.rel).cmp(&(&b.url, &b.rel)));
    check_clashes(&pages, &source_dir)?;

    // What little works out is the bottom of the cascade: the nav it builds
    // from the folders goes beneath the config, so a theme or site that sets
    // `nav` replaces it. `page.breadcrumbs`, `page.prev`, and `page.next`
    // follow the nav the theme gets, so they agree with its sidebar.
    let tree = build_tree(&pages);
    let mut site = Mapping::new();
    site.insert("nav".into(), serde_yaml::to_value(build_nav(&tree))?);
    config::merge(&mut site, site_cfg);
    let nav: Vec<NavItem> = serde_yaml::from_value(site["nav"].clone())
        .context("`nav` in little.yml must be a list of links, each `{ title, url, children }`")?;
    link_items(&mut site["nav"], &site_url);
    let site_value = MjValue::from_serialize(&site);
    let neighbours = neighbours_in(&nav);
    let trails = breadcrumbs_in(&nav);

    // The theme owns 100% of the markup; little only hands it structured data.
    // Every page is rendered before the destination is touched, so a build
    // that fails leaves the last one in place.
    let render = |page_type: PageType, data: &PageData, frontmatter: &Mapping| {
        let page = page_value(data, frontmatter, &site_url)?;
        theme.render(
            page_type,
            context! {
                site => site_value.clone(),
                page => page,
            },
        )
    };
    let mut outputs = Vec::new();
    for page in &pages {
        let children = page.is_index().then(|| children_of(&tree, &page.rel));
        let (prev, next) = neighbours
            .get(page.url.as_str())
            .copied()
            .unwrap_or_default();
        // A page that isn't in the nav has nothing above it there.
        let breadcrumbs = trails
            .get(page.url.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
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
        };
        let source = source_dir.join(&page.rel);
        if let Some(html) = render(page.page_type, &data, &page.frontmatter)
            .with_context(|| format!("rendering {}", source.display()))?
        {
            let path = page.out_path.clone();
            let name = format!("the page built from {}", source.display());
            outputs.push(Output { path, html, name });
        }
    }

    // A 404 page has no content: all `page` has is its type and URL.
    let not_found_url = format!("/{NOT_FOUND_FILE}");
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
    };
    if let Some(html) =
        render(PageType::NotFound, &data, &Mapping::new()).context("rendering the 404 page")?
    {
        let path = destination_dir.join(NOT_FOUND_FILE);
        let name = "the 404 page".to_string();
        outputs.push(Output { path, html, name });
    }

    // The theme's static files (css/js/images) go verbatim to the site's root.
    let static_files = theme.static_files()?;
    check_static_files(&static_files, &destination_dir, &outputs)?;

    clear_destination(&destination_dir)?;
    for output in &outputs {
        write_file(&output.path, output.html.as_bytes())?;
    }
    // Written from what was read, not with `fs::copy`: on macOS that clones
    // the file, and FSEvents then reports the *source* as changed, so `little
    // serve` would rebuild forever. A build must only ever read its sources.
    for file in &static_files {
        write_file(&destination_dir.join(&file.path), &file.contents)?;
    }

    println!(
        "built {} page(s) -> {}",
        pages.len(),
        destination_dir.display()
    );
    Ok(Built {
        destination: destination_dir,
        url: site_url,
    })
}

/// What a template sees as `page`: what little works out for it, with the
/// page's frontmatter merged over it the way each level of the config merges
/// over the one beneath, so the author has the last word. It changes only
/// what the theme sees: a page is built at its own URL, with the template for
/// its own type, whatever its frontmatter says.
///
/// Then each of the page's links gets the path of the site's `url`, whoever
/// wrote it, as do the links in its Markdown.
fn page_value(data: &PageData, frontmatter: &Mapping, url: &SiteUrl) -> Result<Mapping> {
    let Value::Mapping(mut page) = serde_yaml::to_value(data)? else {
        unreachable!("a struct serializes to a mapping");
    };
    config::merge(&mut page, frontmatter.clone());
    link(page.get_mut("url"), url);
    for key in ["prev", "next"] {
        if let Some(Value::Mapping(item)) = page.get_mut(key) {
            link(item.get_mut("url"), url);
        }
    }
    for key in ["breadcrumbs", "children", "toc"] {
        if let Some(items) = page.get_mut(key) {
            link_items(items, url);
        }
    }
    Ok(page)
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
/// overwrite the other. Files whose URLs differ only in case, like `API.md`
/// and `api/README.md`, aren't caught, though they clash on a disk that
/// ignores case, as macOS's and Windows' do by default.
fn check_clashes(pages: &[Page], source_dir: &Path) -> Result<()> {
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
                source_dir.join(&other.rel).display(),
                source_dir.join(&page.rel).display(),
            );
        }
    }
    Ok(())
}

/// Fail if a theme's static file would land on a page in the destination: on
/// the page itself, or on the folder it's in.
fn check_static_files(files: &[StaticFile], destination: &Path, outputs: &[Output]) -> Result<()> {
    for file in files {
        let target = destination.join(&file.path);
        if let Some(page) = outputs.iter().find(|page| page.path.starts_with(&target)) {
            bail!("{} clashes with {}", file.origin, page.name);
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
/// convention (mirroring how GitHub renders READMEs); little maps it to the
/// web's `index.html`.
fn is_readme(rel: &Path) -> bool {
    rel.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("readme"))
        .unwrap_or(false)
}

/// What type of page the Markdown file at `rel` makes.
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

/// Directory segments of `rel` (the parent folders), `/`-joined.
fn dir_segments(rel: &Path) -> Option<String> {
    let folders = folders(rel);
    (!folders.is_empty()).then(|| folders.join("/"))
}

/// Derive a page's pretty, extensionless URL.
///
/// - `README.md`           -> `/`
/// - `getting-started.md`  -> `/getting-started`
/// - `guide/README.md`     -> `/guide/`
/// - `guide/install.md`    -> `/guide/install`
fn derive_url(rel: &Path) -> String {
    let dir = dir_segments(rel);
    if is_readme(rel) {
        match dir {
            Some(d) => format!("/{d}/"),
            None => "/".to_string(),
        }
    } else {
        let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("index");
        match dir {
            Some(d) => format!("/{d}/{stem}"),
            None => format!("/{stem}"),
        }
    }
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
/// title. A folder's own README is rendered by its parent as the section link,
/// so it's excluded here.
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
    items.sort_by_key(|item| item.title.to_lowercase());
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
/// the page is in, as the nav titles it. The nav lists the home page beside the
/// items at its top, but it's above them, so their trails start with it. A
/// page listed twice gets the trail to the last place, where its `page.prev`
/// and `page.next` come from too.
fn breadcrumbs_in(nav: &[NavItem]) -> HashMap<&str, Vec<Breadcrumb<'_>>> {
    let is_home = |item: &NavItem| item.url.as_deref() == Some("/");
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

    fn page(rel: &str, title: &str) -> Page {
        let rel = PathBuf::from(rel);
        let url = derive_url(&rel);
        let out_path = out_path_for_url(Path::new("dist"), &url);
        let page_type = page_type(&rel);
        Page {
            rel,
            url,
            out_path,
            title: title.to_string(),
            frontmatter: Mapping::new(),
            html: String::new(),
            toc: Vec::new(),
            page_type,
        }
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
            check_clashes(&pages, Path::new("docs")).map_err(|e| e.to_string())
        };
        assert_eq!(
            check("guide.md", "guide/README.md").unwrap_err(),
            "docs/guide.md and docs/guide/README.md would both be the page at /guide/: rename or move one"
        );
        assert_eq!(
            check("README.md", "readme.md").unwrap_err(),
            "docs/README.md and docs/readme.md would both be the page at /: rename or move one"
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
        let toc = render::parse_document(md, &(2..=6), &SiteUrl::default())
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
        let trails = breadcrumbs_in(&nav);

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
        let trails = breadcrumbs_in(&nav);
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
        let trails = breadcrumbs_in(&nav);

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
        let trails = breadcrumbs_in(&nav);
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
            "_static/index.html in the theme from {url} clashes with the page built from {}",
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

        // They aren't pages, so they aren't in the nav either. A theme's
        // static files are copied whole, hidden ones too.
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
        // `.little/` isn't a page.
        fs::write(site.join("little.yml"), "theme: ./theme\nsource: .\n").unwrap();
        build(&site).unwrap();

        assert_eq!(
            files_in(&dist),
            [MARKER_FILE, "index.html"].map(PathBuf::from)
        );
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
        let toc = r#"<aside class="toc""#;
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
            let (_, toc) = html.split_once(r#"<aside class="toc""#).unwrap();
            let (toc, _) = toc.split_once("</aside>").unwrap();
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

        // ...but not on one, or where one's folder goes.
        let static_dir = site.join("theme").join("_static");
        let built_from = |file: &str| {
            let source = site.join("docs").join(file);
            format!("the page built from {}", source.display())
        };
        for (file, page) in [
            ("index.html", built_from("README.md")),
            ("guide/install", built_from("guide/install.md")),
            ("guide/install/index.html", built_from("guide/install.md")),
            ("404.html", "the 404 page".to_string()),
        ] {
            let path = static_dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "static").unwrap();
            let message = format!("{:#}", build(&site).unwrap_err());
            let expected = format!("{} clashes with {page}", path.display());
            assert_eq!(message, expected);
            fs::remove_file(&path).unwrap();
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
}
