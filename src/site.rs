//! The build pipeline: docs/ + the theme + config -> dist/.

use anyhow::{Context, Result};
use minijinja::{Environment, Value as MjValue, context};
use serde::Serialize;
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

use crate::{config, render};

/// A rendered content page.
struct Page {
    /// Path relative to `docs/` (e.g. `guide/install.md`).
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
    /// True when this file is a directory's `README.md` (its home page).
    is_index: bool,
}

/// One node in the site-wide navigation tree (see `build_nav`).
#[derive(Debug, Serialize)]
struct NavItem {
    title: String,
    /// `None` for a section folder that has no `README.md` (a plain label).
    url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<NavItem>,
}

/// Build the site rooted at `site_dir`.
pub fn build(site_dir: &Path) -> Result<()> {
    let docs_dir = site_dir.join("docs");
    let dist_dir = site_dir.join("dist");

    let config::Config {
        values: cfg,
        theme_dir,
    } = config::load(site_dir)?;

    if dist_dir.exists() {
        fs::remove_dir_all(&dist_dir).context("clearing dist/")?;
    }
    fs::create_dir_all(&dist_dir)?;

    // Collect and render every Markdown file under docs/.
    let mut pages = Vec::new();
    for entry in WalkDir::new(&docs_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let rel = path.strip_prefix(&docs_dir)?.to_path_buf();
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

        // Description: frontmatter `description` -> null.
        let description = rendered
            .frontmatter
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);

        let url = derive_url(&rel);
        let out_path = out_path_for_url(&dist_dir, &url);

        pages.push(Page {
            rel,
            url,
            out_path,
            title,
            description,
            html: rendered.html,
            toc: rendered.toc,
            is_index,
        });
    }

    // Deterministic output order (nav has its own ordering).
    pages.sort_by(|a, b| a.url.cmp(&b.url));

    // The main nav is derived once from the folder structure and shared across
    // every page; the theme marks the active item by matching `page.url`.
    let nav = build_nav(&pages);
    let mut site_cfg = cfg;
    if let Value::Mapping(map) = &mut site_cfg {
        map.insert(Value::String("nav".into()), serde_yaml::to_value(&nav)?);
    }
    let site_value = MjValue::from_serialize(&site_cfg);

    // The theme owns 100% of the markup; little only hands it structured data.
    let mut env = Environment::new();
    let template_path = theme_dir.join("templates/page.html");
    let template_src = fs::read_to_string(&template_path)
        .with_context(|| format!("reading {}", template_path.display()))?;
    env.add_template_owned("page.html".to_string(), template_src)?;
    let template = env.get_template("page.html")?;

    for page in &pages {
        let rendered = template.render(context! {
            site => site_value.clone(),
            page => context! {
                title => page.title.clone(),
                url => page.url.clone(),
                description => MjValue::from_serialize(&page.description),
                content => page.html.clone(),
                nav => MjValue::from_serialize(&page.toc),
            },
        })?;

        if let Some(parent) = page.out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&page.out_path, rendered)?;
    }

    // Copy theme assets (css/js/images) verbatim into dist/assets.
    let assets_src = theme_dir.join("assets");
    if assets_src.exists() {
        copy_dir(&assets_src, &dist_dir.join("assets"))?;
    }

    println!("built {} page(s) -> {}", pages.len(), dist_dir.display());
    Ok(())
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

/// Directory segments of `rel` (the parent folders), `/`-joined.
fn dir_segments(rel: &Path) -> Option<String> {
    let parent = rel.parent()?;
    let joined = parent
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
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

/// Map a pretty URL to its directory-index output file under `dist/`.
fn out_path_for_url(dist_dir: &Path, url: &str) -> PathBuf {
    let trimmed = url.trim_matches('/');
    if trimmed.is_empty() {
        dist_dir.join("index.html")
    } else {
        dist_dir.join(trimmed).join("index.html")
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

/// Build the site-wide navigation tree from the folder structure under `docs/`.
fn build_nav(pages: &[Page]) -> Vec<NavItem> {
    let mut root = DirNode::default();
    for page in pages {
        insert_into_tree(&mut root, page);
    }

    // Top-level: the site home (root README) first, then pages/sections.
    let mut items = Vec::new();
    if let Some(home) = &root.index {
        items.push(NavItem {
            title: home.title.clone(),
            url: Some(home.url.clone()),
            children: Vec::new(),
        });
    }
    items.extend(child_items(&root));
    items
}

fn insert_into_tree(root: &mut DirNode, page: &Page) {
    let segments: Vec<String> = page
        .rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str().map(str::to_string),
            _ => None,
        })
        .collect();

    let dir_count = segments.len().saturating_sub(1); // last segment is the file
    let mut node = root;
    for segment in &segments[..dir_count] {
        node = node.dirs.entry(segment.clone()).or_default();
    }

    let leaf = NavLeaf {
        title: page.title.clone(),
        url: page.url.clone(),
    };
    if page.is_index {
        node.index = Some(leaf);
    } else {
        node.pages.push(leaf);
    }
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

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let rel = path.strip_prefix(src)?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            // Not `fs::copy`: on macOS it clones the file, and FSEvents then
            // reports the *source* as changed, so `little serve` would rebuild
            // forever. A build must only ever read its sources.
            let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
            fs::write(&target, bytes).with_context(|| format!("writing {}", target.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(rel: &str, title: &str, is_index: bool) -> Page {
        let rel = PathBuf::from(rel);
        let url = derive_url(&rel);
        Page {
            rel,
            url,
            out_path: PathBuf::new(),
            title: title.to_string(),
            description: None,
            html: String::new(),
            toc: Vec::new(),
            is_index,
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
            page("README.md", "Home", true),
            page("getting-started.md", "Getting Started", false),
            page("guide/README.md", "Guide", true),
            page("guide/install.md", "Install", false),
        ];
        let nav = build_nav(&pages);

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
            page("README.md", "Home", true),
            page("api/reference.md", "Reference", false),
        ];
        let nav = build_nav(&pages);

        let api = nav.iter().find(|i| i.title == "Api").expect("api section");
        assert_eq!(api.url, None); // no README -> plain label
        assert_eq!(api.children.len(), 1);
        assert_eq!(api.children[0].title, "Reference");
        assert_eq!(api.children[0].url.as_deref(), Some("/api/reference"));
    }

    #[test]
    fn copies_nested_assets_byte_for_byte() {
        let root =
            std::env::temp_dir().join(format!("little-site-test-assets-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (src, dst) = (root.join("assets"), root.join("dist/assets"));
        let png: &[u8] = &[0x89, b'P', b'N', b'G', 0x00, 0xff];
        fs::create_dir_all(src.join("img")).unwrap();
        fs::write(src.join("style.css"), "body {}\n").unwrap();
        fs::write(src.join("img/logo.png"), png).unwrap();

        copy_dir(&src, &dst).unwrap();

        assert_eq!(fs::read(dst.join("style.css")).unwrap(), b"body {}\n");
        assert_eq!(fs::read(dst.join("img/logo.png")).unwrap(), png);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn builds_with_a_local_theme() {
        let site =
            std::env::temp_dir().join(format!("little-site-test-theme-{}", std::process::id()));
        let _ = fs::remove_dir_all(&site);
        for (file, contents) in [
            ("little.yml", "theme: ./path/to/my/theme\n"),
            ("docs/README.md", "# Home\n"),
            ("theme/templates/page.html", "the default theme"),
            (
                "path/to/my/theme/templates/page.html",
                "{{ site.greeting }} from my theme",
            ),
            ("path/to/my/theme/little.yml", "greeting: Hello\n"),
            ("path/to/my/theme/assets/style.css", "body {}\n"),
        ] {
            let path = site.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
        }

        build(&site).unwrap();

        let home = fs::read_to_string(site.join("dist/index.html")).unwrap();
        assert_eq!(home, "Hello from my theme");
        let css = fs::read(site.join("dist/assets/style.css")).unwrap();
        assert_eq!(css, b"body {}\n");
        let _ = fs::remove_dir_all(&site);
    }
}
