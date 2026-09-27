//! A theme: the templates that turn a site's pages into HTML, the static files
//! that go with them, and optional config (see `config`).
//!
//! ```text
//! my-theme/
//! ├── _static/      optional: copied as-is to the site's root
//! ├── _templates/   the templates; default.html is required
//! └── little.yml    optional: the theme's config
//! ```

use anyhow::{Result, bail};
use minijinja::{Environment, ErrorKind, Value, path_loader};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The folder of a theme's templates, which can extend and include each other.
pub const TEMPLATES_DIR: &str = "_templates";
/// The folder of a theme's static files, copied as-is to the site's root.
pub const STATIC_DIR: &str = "_static";
/// The template for any type of page without its own. Every theme has one.
const DEFAULT_TEMPLATE: &str = "default.html";

/// The types of page little builds. A theme can give each one its own
/// template, named after the type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageType {
    /// The source folder's `README.md`: the site's home page.
    Home,
    /// A subfolder's `README.md`: that folder's home page.
    Section,
    /// Any other Markdown file.
    Page,
    /// The page a host shows for a URL that has no page.
    NotFound,
}

impl PageType {
    const ALL: [Self; 4] = [Self::Home, Self::Section, Self::Page, Self::NotFound];

    /// What a template sees as `page.type`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Section => "section",
            Self::Page => "page",
            Self::NotFound => "404",
        }
    }

    /// The type's own template.
    pub fn template(self) -> &'static str {
        match self {
            Self::Home => "home.html",
            Self::Section => "section.html",
            Self::Page => "page.html",
            Self::NotFound => "404.html",
        }
    }
}

/// A theme, loaded and ready to render pages.
pub struct Theme {
    env: Environment<'static>,
    /// The template each type of page is rendered with. A type that isn't
    /// here isn't rendered at all.
    templates: HashMap<PageType, &'static str>,
    static_dir: Option<PathBuf>,
}

impl Theme {
    /// Load the theme in `dir`. A build does this before it touches its
    /// destination, so a theme that can't build the site doesn't cost it its
    /// last build.
    pub fn load(dir: &Path) -> Result<Self> {
        let templates_dir = dir.join(TEMPLATES_DIR);
        let mut env = Environment::new();
        // Templates are read as they're needed, by name, so the folder can
        // hold other files (a README, say) without little trying to parse them.
        env.set_loader(path_loader(&templates_dir));
        // Loading a template compiles it, so a broken one fails the build now,
        // rather than when the first page needs it.
        let has_template = |name: &str| match env.get_template(name) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == ErrorKind::TemplateNotFound => Ok(false),
            Err(e) => Err(anyhow::Error::new(e)
                .context(format!("loading {}", templates_dir.join(name).display()))),
        };

        if !has_template(DEFAULT_TEMPLATE)? {
            bail!(
                "theme {} has no {TEMPLATES_DIR}/{DEFAULT_TEMPLATE}, which every theme needs",
                dir.display()
            );
        }
        let mut templates = HashMap::new();
        for page_type in PageType::ALL {
            if has_template(page_type.template())? {
                templates.insert(page_type, page_type.template());
            } else if page_type != PageType::NotFound {
                // A 404 page has no content for default.html to show, so
                // without a 404.html the site gets none, and its host shows
                // its own.
                templates.insert(page_type, DEFAULT_TEMPLATE);
            }
        }

        let static_dir = dir.join(STATIC_DIR);
        Ok(Self {
            env,
            templates,
            static_dir: static_dir.is_dir().then_some(static_dir),
        })
    }

    /// Render a page of type `page_type` from `ctx`, or `None` if the theme
    /// doesn't render that type of page.
    pub fn render(&self, page_type: PageType, ctx: Value) -> Result<Option<String>> {
        let Some(name) = self.templates.get(&page_type) else {
            return Ok(None);
        };
        Ok(Some(self.env.get_template(name)?.render(ctx)?))
    }

    /// The theme's `_static/` folder, if it has one.
    pub fn static_dir(&self) -> Option<&Path> {
        self.static_dir.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::context;

    /// A fresh scratch theme holding `files`, each a (path, contents) pair.
    fn scratch(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("little-theme-test-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for (file, contents) in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        root
    }

    /// What `theme` renders for each type of page, `None` for types it skips.
    fn render_each(theme: &Theme) -> Vec<Option<String>> {
        PageType::ALL
            .iter()
            .map(|&page_type| {
                let ctx = context! { page => context! { type => page_type.name() } };
                theme.render(page_type, ctx).unwrap()
            })
            .collect()
    }

    #[test]
    fn each_type_of_page_uses_its_own_template_or_the_default() {
        let dir = scratch(
            "lookup",
            &[
                ("_templates/default.html", "default:{{ page.type }}"),
                ("_templates/section.html", "section:{{ page.type }}"),
            ],
        );
        let rendered = render_each(&Theme::load(&dir).unwrap());
        let expect = |s: &str| Some(s.to_string());
        assert_eq!(
            rendered,
            [
                expect("default:home"),
                expect("section:section"),
                expect("default:page"),
                None, // no 404.html, so no 404 page: default.html has nothing to show
            ]
        );

        for page_type in PageType::ALL {
            let path = dir.join(TEMPLATES_DIR).join(page_type.template());
            std::fs::write(path, "own:{{ page.type }}").unwrap();
        }
        let rendered = render_each(&Theme::load(&dir).unwrap());
        assert_eq!(
            rendered,
            [
                expect("own:home"),
                expect("own:section"),
                expect("own:page"),
                expect("own:404"),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn templates_can_extend_and_include_each_other() {
        let dir = scratch(
            "inherit",
            &[
                (
                    "_templates/default.html",
                    r#"{% extends "layouts/base.html" %}{% block body %}default{% endblock %}"#,
                ),
                (
                    "_templates/layouts/base.html",
                    r#"<{% include "partials/brand.html" %}>{% block body %}{% endblock %}"#,
                ),
                ("_templates/partials/brand.html", "little"),
                // Other files can sit alongside the templates.
                ("_templates/README.md", "How to use {{ this theme"),
            ],
        );
        let theme = Theme::load(&dir).unwrap();
        let html = theme.render(PageType::Page, context! {}).unwrap();
        assert_eq!(html.as_deref(), Some("<little>default"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_theme_needs_a_working_default_template() {
        let error = |files: &[(&str, &str)]| {
            let dir = scratch("broken", files);
            let message = format!("{:#}", Theme::load(&dir).err().unwrap());
            let _ = std::fs::remove_dir_all(&dir);
            message
        };

        let message = error(&[("_templates/page.html", "")]);
        assert!(message.contains("has no _templates/default.html, which every theme needs"));
        // The layout before `_templates/`, as older themes have it.
        let message = error(&[("templates/page.html", ""), ("assets/style.css", "")]);
        assert!(message.contains("has no _templates/default.html"));

        // A broken template fails the load, not the first page that needs it.
        let message = error(&[
            ("_templates/default.html", ""),
            ("_templates/home.html", "{% if %}"),
        ]);
        assert!(
            message.contains("_templates/home.html: syntax error"),
            "{message}"
        );
    }

    #[test]
    fn static_files_are_optional() {
        let dir = scratch("static", &[("_templates/default.html", "")]);
        assert_eq!(Theme::load(&dir).unwrap().static_dir(), None);

        std::fs::create_dir_all(dir.join(STATIC_DIR)).unwrap();
        let theme = Theme::load(&dir).unwrap();
        assert_eq!(theme.static_dir(), Some(dir.join(STATIC_DIR).as_path()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
