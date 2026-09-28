//! The build pipeline: the source (`docs/`) + the theme + config -> the
//! destination (`dist/`).

use anyhow::{Context, Result, bail};
use minijinja::{Value as MjValue, context};
use serde::Serialize;
use serde_yaml::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

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
    description: Option<String>,
    html: String,
    /// In-page table of contents (h2–h3).
    toc: Vec<render::TocItem>,
    page_type: PageType,
}

impl Page {
    /// Is this a folder's `README.md` (its home page)?
    fn is_index(&self) -> bool {
        matches!(self.page_type, PageType::Home | PageType::Section)
    }
}

/// What a template sees as `page`. Anything a page doesn't have is left out
/// rather than set to null, which a template would print as "None".
#[derive(Serialize)]
struct PageData<'a> {
    #[serde(rename = "type")]
    page_type: &'static str,
    url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<&'a str>,
    /// The page's table of contents.
    #[serde(skip_serializing_if = "Option::is_none")]
    nav: Option<&'a [render::TocItem]>,
    /// A home or section page's branch of the nav: what's in its folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<&'a [NavItem]>,
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

/// One node in the site-wide navigation tree (see `build_nav`).
#[derive(Debug, Serialize)]
struct NavItem {
    title: String,
    /// `None` for a section folder that has no `README.md` (a plain label).
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<NavItem>,
}

/// A page the build writes.
struct Output {
    path: PathBuf,
    html: String,
    /// Which page it is, for an error: "the page built from docs/README.md".
    name: String,
}

/// Build the site rooted at `site_dir`, and return the folder it was built
/// into.
pub fn build(site_dir: &Path) -> Result<PathBuf> {
    let config::Config {
        values: mut site_cfg,
        themes,
        source_dir,
        destination_dir,
    } = config::load(site_dir)?;
    if !source_dir.is_dir() {
        bail!("there's no source folder at {}", source_dir.display());
    }
    let theme = Theme::load(&themes)?;

    // Collect and render every Markdown file in the source folder.
    let mut pages = Vec::new();
    for entry in WalkDir::new(&source_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let rel = path.strip_prefix(&source_dir)?.to_path_buf();
        let raw =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let rendered = render::parse_document(&raw)
            .with_context(|| format!("rendering {}", path.display()))?;

        let is_index = is_readme(&rel);

        // Title: frontmatter `title` -> first <h1> -> filename/folder-derived.
        let title = rendered
            .frontmatter
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(rendered.first_h1)
            .unwrap_or_else(|| fallback_title(&rel, is_index));

        // Description: frontmatter `description`, if there is one.
        let description = rendered
            .frontmatter
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);

        let url = derive_url(&rel);
        let out_path = out_path_for_url(&destination_dir, &url);
        let page_type = page_type(&rel);

        pages.push(Page {
            rel,
            url,
            out_path,
            title,
            description,
            html: rendered.html,
            toc: rendered.toc,
            page_type,
        });
    }

    // Deterministic output order (nav has its own ordering).
    pages.sort_by(|a, b| a.url.cmp(&b.url));

    // The main nav is derived once from the folder structure and shared across
    // every page; the theme marks the active item by matching `page.url`.
    let tree = build_tree(&pages);
    let nav = build_nav(&tree);
    site_cfg.insert("nav".into(), serde_yaml::to_value(&nav)?);
    let site_value = MjValue::from_serialize(&site_cfg);
    let neighbours = neighbours_in(&nav);

    // The theme owns 100% of the markup; little only hands it structured data.
    // Every page is rendered before the destination is touched, so a build
    // that fails leaves the last one in place.
    let render = |page_type: PageType, page: &PageData| {
        theme.render(
            page_type,
            context! {
                site => site_value.clone(),
                page => MjValue::from_serialize(page),
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
        let data = PageData {
            page_type: page.page_type.name(),
            url: &page.url,
            title: Some(&page.title),
            description: page.description.as_deref(),
            content: Some(&page.html),
            nav: Some(&page.toc),
            children: children.as_deref(),
            prev,
            next,
        };
        let source = source_dir.join(&page.rel);
        if let Some(html) = render(page.page_type, &data)
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
        description: None,
        content: None,
        nav: None,
        children: None,
        prev: None,
        next: None,
    };
    if let Some(html) = render(PageType::NotFound, &data).context("rendering the 404 page")? {
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
    Ok(destination_dir)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn page(rel: &str, title: &str) -> Page {
        let rel = PathBuf::from(rel);
        let url = derive_url(&rel);
        let page_type = page_type(&rel);
        Page {
            rel,
            url,
            out_path: PathBuf::new(),
            title: title.to_string(),
            description: None,
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

        let destination = build(&site).unwrap();

        assert_eq!(destination, site.join("public"));
        let read = |path: &str| fs::read_to_string(destination.join(path)).unwrap();
        assert_eq!(read("index.html"), "Home");
        assert_eq!(read("guide/install/index.html"), "Install");
        assert!(!site.join("dist").exists());
        let _ = fs::remove_dir_all(&site);
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
            let dir = build_into(destination).unwrap();
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
            "home with default.html: children content nav next title type url"
        );
        assert_eq!(
            read("guide/index.html"),
            "section with section.html: children content description nav next prev title type url"
        );
        assert_eq!(
            read("guide/install/index.html"),
            "page with default.html: content nav prev title type url"
        );
        // A folder without a README gets no page of its own.
        assert!(!dist.join("api/index.html").exists());
        assert_eq!(
            read("api/reference/index.html"),
            "page with default.html: content nav next prev title type url"
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
}
