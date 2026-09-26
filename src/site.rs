//! The build pipeline: docs/ + theme/ + config -> dist/.

use anyhow::{Context, Result};
use minijinja::{context, Environment, Value as MjValue};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::{config, render};

struct Page {
    url: String,
    out_path: PathBuf,
    title: String,
    html: String,
}

#[derive(Serialize)]
struct NavLink {
    title: String,
    url: String,
    active: bool,
}

/// Build the site rooted at `project_dir`.
pub fn build(project_dir: &Path) -> Result<()> {
    let docs_dir = project_dir.join("docs");
    let theme_dir = project_dir.join("theme");
    let dist_dir = project_dir.join("dist");

    let cfg = config::load_cascade(&theme_dir, project_dir)?;

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
        let rel = path.strip_prefix(&docs_dir)?;
        let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let (frontmatter, html) = render::parse_document(&raw)?;

        let title = frontmatter
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| derive_title(rel));

        let url = rel.with_extension("html").to_string_lossy().replace('\\', "/");
        let out_path = dist_dir.join(&url);
        pages.push(Page { url, out_path, title, html });
    }

    // index.html first, then alphabetical by title.
    pages.sort_by(|a, b| {
        let a_index = a.url == "index.html";
        let b_index = b.url == "index.html";
        b_index.cmp(&a_index).then_with(|| a.title.cmp(&b.title))
    });

    // The theme owns 100% of the markup; little only hands it structured data.
    let mut env = Environment::new();
    let template_src = fs::read_to_string(theme_dir.join("templates/page.html"))
        .context("theme/templates/page.html not found")?;
    env.add_template_owned("page.html".to_string(), template_src)?;
    let template = env.get_template("page.html")?;

    let site_value = MjValue::from_serialize(&cfg);

    for page in &pages {
        let nav: Vec<NavLink> = pages
            .iter()
            .map(|p| NavLink {
                title: p.title.clone(),
                url: format!("/{}", p.url),
                active: p.url == page.url,
            })
            .collect();

        let rendered = template.render(context! {
            site => site_value.clone(),
            page => context! {
                title => page.title.clone(),
                content => page.html.clone(),
                url => format!("/{}", page.url),
            },
            nav => MjValue::from_serialize(&nav),
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

/// "getting-started" -> "Getting Started".
fn derive_title(rel: &Path) -> String {
    let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled");
    stem.split(['-', '_'])
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
            fs::copy(path, &target)?;
        }
    }
    Ok(())
}
