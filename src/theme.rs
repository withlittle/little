//! A theme: the templates that turn a site's pages into HTML, the static files
//! that go with them, and optional config (see `config`).
//!
//! ```text
//! my-theme/
//! ├── _sass/        optional: Sass for its stylesheets to load
//! ├── _static/      optional: copied to the site's root, Sass compiled to CSS
//! ├── _templates/   the templates; default.html is required
//! └── little.yml    optional: the theme's config
//! ```
//!
//! A theme can build on another (see `config`). Its files then go over the
//! other theme's, file by file, so it only needs the ones it changes: a
//! stylesheet over the CSS file it's compiled to, and the other way round. A
//! theme is built into little, in a folder, or downloaded (see `download`).
//! Once its files are read, and layered, a plugin's `theme` hook can change
//! them (see `plugin`).
//!
//! little adds two filters to a theme's templates, for URLs the theme writes
//! itself: `url` puts the path of the site's `url` before a URL from the root
//! of the site, and `absolute_url` its scheme and host before one that has
//! the path.

use anyhow::{Context, Result, bail};
use minijinja::{AutoEscape, Environment, ErrorKind, HtmlEscape, State, Value};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::config::SiteUrl;
use crate::plugin::{HookedFile, Plugins};
use crate::sass;

/// The folder of a theme's templates, which can extend and include each other.
const TEMPLATES_DIR: &str = "_templates";
/// The folder of a theme's static files, copied to the site's root, but for
/// its Sass stylesheets, which are compiled to CSS (see `sass`).
const STATIC_DIR: &str = "_static";
/// The folder of the Sass a theme's stylesheets load. It isn't published.
const SASS_DIR: &str = "_sass";
/// The template for any type of page without its own. Every theme has one.
const DEFAULT_TEMPLATE: &str = "default.html";

/// little's built-in theme: the `theme/` folder, compiled into the binary so a
/// site needs nothing but its Markdown. Every file in `theme/` is listed here.
pub mod builtin {
    /// Its config.
    pub const CONFIG: &str = include_str!("../theme/little.yml");
    /// Its templates, by name.
    pub const TEMPLATES: &[(&str, &str)] = &[
        ("404.html", include_str!("../theme/_templates/404.html")),
        (
            "default.html",
            include_str!("../theme/_templates/default.html"),
        ),
    ];
    /// Its static files, by their path from `_static/`.
    pub const STATIC: &[(&str, &[u8])] = &[
        (
            "assets/script.js",
            include_bytes!("../theme/_static/assets/script.js"),
        ),
        (
            "assets/style.css",
            include_bytes!("../theme/_static/assets/style.css"),
        ),
    ];
}

/// The types of page little builds. A theme can give each one its own
/// template, named after the type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageType {
    /// The source folder's `README`, like `README.md`: the site's home page.
    Home,
    /// A subfolder's `README`, like `README.md`: that folder's home page.
    Section,
    /// Any other Markdown file, or file a plugin's `formats` hook reads.
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

    /// The type's own template, named after it, like `home.html`.
    pub fn template(self) -> String {
        format!("{}.html", self.name())
    }
}

/// Where a theme is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Location {
    /// little's built-in theme.
    BuiltIn,
    /// A theme in a folder.
    Folder(PathBuf),
    /// A theme downloaded from `url` and unpacked into `dir`.
    Downloaded { url: String, dir: PathBuf },
}

impl Location {
    /// The theme's folder, unless it's built in.
    pub fn folder(&self) -> Option<&Path> {
        match self {
            Self::BuiltIn => None,
            Self::Folder(dir) | Self::Downloaded { dir, .. } => Some(dir),
        }
    }

    /// Name the file at `path` in the theme, for an error.
    pub fn describe(&self, path: &str) -> String {
        match self {
            Self::BuiltIn => format!("{path} in little's built-in theme"),
            Self::Folder(dir) => dir.join(path).display().to_string(),
            // Its folder is named with a hash, so its URL says more.
            Self::Downloaded { url, .. } => format!("{path} in the theme from {url}"),
        }
    }

    /// The theme's own files in `folder`, like `_static`, each by its path
    /// from there, like `assets/style.css`, with `/` between its parts. The
    /// built-in theme has no `_sass/`.
    fn files_in(&self, folder: &str) -> Result<Vec<(String, ThemeFile)>> {
        let Some(dir) = self.folder() else {
            let file = |path: &str, contents: &[u8]| {
                let origin = self.describe(&format!("{folder}/{path}"));
                let contents = contents.to_vec();
                (path.to_string(), ThemeFile { origin, contents })
            };
            return Ok(match folder {
                TEMPLATES_DIR => (builtin::TEMPLATES.iter())
                    .map(|&(path, source)| file(path, source.as_bytes()))
                    .collect(),
                STATIC_DIR => (builtin::STATIC.iter())
                    .map(|&(path, contents)| file(path, contents))
                    .collect(),
                _ => Vec::new(),
            });
        };
        let root = dir.join(folder);
        let mut files = Vec::new();
        if !root.is_dir() {
            return Ok(files);
        }
        for entry in WalkDir::new(&root).sort_by_file_name() {
            let entry = entry?;
            if entry.file_type().is_dir() {
                continue;
            }
            let path = entry.path();
            let rel = path.strip_prefix(&root)?;
            let file = ThemeFile {
                origin: self.describe(&Path::new(folder).join(rel).display().to_string()),
                contents: std::fs::read(path)
                    .with_context(|| format!("reading {}", path.display()))?,
            };
            files.push((rel.to_string_lossy().replace('\\', "/"), file));
        }
        Ok(files)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::BuiltIn => f.write_str("little's built-in theme"),
            Self::Folder(dir) => write!(f, "{}", dir.display()),
            Self::Downloaded { url, .. } => f.write_str(url),
        }
    }
}

/// A file, read: a static file, ready to copy to the site's root, or one in
/// a theme's `_sass/`, for stylesheets to load.
#[derive(Clone, Debug)]
pub struct StaticFile {
    /// Its path from `_static/`, and so from the site's root, or `_sass/`.
    pub path: PathBuf,
    /// The file it's read from, to name it in errors.
    pub origin: String,
    pub contents: Vec<u8>,
}

/// One of a theme's files, read.
#[derive(Clone, Debug)]
struct ThemeFile {
    /// The file it's read from, or the plugin that gave it, to name it in
    /// errors.
    origin: String,
    contents: Vec<u8>,
}

/// The files of the theme a site chose, and of each it builds on, layered,
/// each by its path from a theme's folder, like `_templates/default.html`:
/// each template, and each file in `_sass/`, from the nearest theme that has
/// it, and each static file from the nearest theme with one published at its
/// path, so a stylesheet replaces a CSS file, and a CSS file a stylesheet.
#[derive(Clone, Debug)]
pub struct ThemeFiles {
    files: BTreeMap<String, ThemeFile>,
}

impl ThemeFiles {
    /// Read the files of the first of `themes`, which builds on the rest in
    /// turn. Fails on a Sass partial in a theme's `_static/`, or two files
    /// there published at one path, or if none of them has
    /// `_templates/default.html`.
    pub fn read(themes: &[Location]) -> Result<ThemeFiles> {
        let Some(top) = themes.first() else {
            bail!("little can't build a site without a theme (`theme: false`) yet");
        };
        let mut files = BTreeMap::new();
        let mut published = HashSet::new();
        for theme in themes {
            for folder in [TEMPLATES_DIR, SASS_DIR] {
                for (path, file) in theme.files_in(folder)? {
                    files.entry(format!("{folder}/{path}")).or_insert(file);
                }
            }
            let own = theme.files_in(STATIC_DIR)?;
            let at = publish(
                own.iter()
                    .map(|(path, file)| (path.as_str(), file.origin.as_str())),
            )?;
            for ((path, file), at) in own.into_iter().zip(at) {
                if published.insert(at) {
                    files.insert(format!("{STATIC_DIR}/{path}"), file);
                }
            }
        }
        if !files.contains_key(&default_path()) {
            bail!(
                "theme {top} has no {}, which every theme needs",
                default_path()
            );
        }
        Ok(ThemeFiles { files })
    }

    /// The files, as each of `plugins`' `theme` hooks changes them, in turn,
    /// with each a plugin changes named for it. Stops at the first that
    /// fails, or gives back files that aren't a theme's: one outside
    /// `_templates/`, `_static/`, and `_sass/`, a Sass partial in `_static/`,
    /// two published at one path, or none at `_templates/default.html`.
    pub fn hooked(self, plugins: &mut Plugins) -> Result<ThemeFiles> {
        let mut origins = HashMap::new();
        let files = (self.files.into_iter())
            .map(|(path, ThemeFile { origin, contents })| {
                origins.insert(path.clone(), origin);
                HookedFile {
                    path,
                    contents,
                    plugin: None,
                }
            })
            .collect();
        let origin = |file: &HookedFile| match &file.plugin {
            Some(plugin) => format!("plugin `{plugin}`'s {}", file.path),
            None => (origins.get(&file.path).cloned()).unwrap_or_else(|| file.path.clone()),
        };
        let hooked = plugins.theme(files, |plugin, files| {
            let mut statics = Vec::new();
            for file in files {
                match file.path.split_once('/') {
                    Some((STATIC_DIR, path)) => statics.push((path, origin(file))),
                    Some((TEMPLATES_DIR | SASS_DIR, _)) => {}
                    _ => bail!(
                        "plugin `{plugin}`'s `theme` hook gives back a file at `{}`, but a \
                         theme's files are in {TEMPLATES_DIR}/, {STATIC_DIR}/, or {SASS_DIR}/",
                        file.path
                    ),
                }
            }
            if !files.iter().any(|file| file.path == default_path()) {
                bail!(
                    "plugin `{plugin}`'s `theme` hook removes {}, which every theme needs",
                    default_path()
                );
            }
            publish(
                statics
                    .iter()
                    .map(|(path, origin)| (*path, origin.as_str())),
            )?;
            Ok(())
        })?;
        let files = (hooked.into_iter())
            .map(|file| {
                let origin = origin(&file);
                let contents = file.contents;
                (file.path, ThemeFile { origin, contents })
            })
            .collect();
        Ok(ThemeFiles { files })
    }

    /// The theme's static files, by their path from `_static/`, and so from
    /// the site's root, with each Sass stylesheet compiled to the CSS file at
    /// its path, loading from `_sass/`.
    pub fn static_files(&self) -> Result<Vec<StaticFile>> {
        let read = |(path, file): (&str, &ThemeFile)| StaticFile {
            path: PathBuf::from(path),
            origin: file.origin.clone(),
            contents: file.contents.clone(),
        };
        let sass: BTreeMap<PathBuf, StaticFile> = (self.in_folder(SASS_DIR).map(read))
            .map(|file| (file.path.clone(), file))
            .collect();
        let mut files = BTreeMap::new();
        for mut file in self.in_folder(STATIC_DIR).map(read) {
            if sass::is_sass(&file.path) {
                file.contents = sass::compile(&file, &sass)?;
                file.path.set_extension("css");
            }
            files.insert(file.path.clone(), file);
        }
        Ok(files.into_values().collect())
    }

    /// Each file in `folder`, like `_templates`, by its path from there.
    fn in_folder<'a>(
        &'a self,
        folder: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a ThemeFile)> + 'a {
        (self.files.iter()).filter_map(move |(path, file)| {
            let path = path.strip_prefix(folder)?.strip_prefix('/')?;
            Some((path, file))
        })
    }
}

/// The path of the template every theme has, from the theme's folder.
fn default_path() -> String {
    format!("{TEMPLATES_DIR}/{DEFAULT_TEMPLATE}")
}

/// Where each of a theme's static `files`, each by its path from `_static/`
/// and the file it's read from, is published, from the site's root: at its
/// own path, or, for a Sass stylesheet, at the CSS file it's compiled to.
/// Fails on a Sass partial, which is for stylesheets to load, or two files
/// published at one path.
fn publish<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Result<Vec<String>> {
    let mut origins = HashMap::new();
    let mut published = Vec::new();
    for (path, origin) in files {
        let path = Path::new(path);
        let at = if sass::is_partial(path) {
            bail!(
                "{origin} is a Sass partial, for stylesheets to load: move it to {SASS_DIR}/, \
                 where they load from"
            );
        } else if sass::is_sass(path) {
            path.with_extension("css")
        } else {
            path.to_path_buf()
        };
        let at = at.to_string_lossy().replace('\\', "/");
        if let Some(other) = origins.insert(at.clone(), origin) {
            let mut both = [other, origin];
            both.sort();
            bail!(
                "{} and {} would both be /{at}: remove or rename one",
                both[0],
                both[1]
            );
        }
        published.push(at);
    }
    Ok(published)
}

/// A theme, loaded and ready to render pages.
pub struct Theme {
    env: Environment<'static>,
    /// The template each type of page is rendered with. A type that isn't
    /// here isn't rendered at all.
    templates: HashMap<PageType, String>,
}

impl Theme {
    /// Load the theme whose `files` are read, for a site published at `url`.
    /// A build does this before it touches its destination, so a theme that
    /// can't build the site doesn't cost it its last build.
    pub fn load(files: &ThemeFiles, url: &SiteUrl) -> Result<Self> {
        let mut env = Environment::new();
        let site = url.clone();
        env.add_filter("url", move |state: &State, value: Value| {
            url_filter(state, value, |url| site.link(url))
        });
        let site = url.clone();
        env.add_filter("absolute_url", move |state: &State, value: Value| {
            url_filter(state, value, |url| site.absolute(url))
        });
        // Templates are compiled as they're needed, by name, so a theme's
        // `_templates/` can hold other files (a README, say) without little
        // trying to parse them.
        let templates: HashMap<String, Vec<u8>> = (files.in_folder(TEMPLATES_DIR))
            .map(|(name, file)| (name.to_string(), file.contents.clone()))
            .collect();
        env.set_loader(move |name| match templates.get(name) {
            None => Ok(None),
            Some(contents) => String::from_utf8(contents.clone()).map(Some).map_err(|_| {
                minijinja::Error::new(ErrorKind::InvalidOperation, "a template must be UTF-8")
            }),
        });
        // Loading a template compiles it, so a broken one fails the build now,
        // rather than when the first page needs it.
        let has_template = |name: &str| match env.get_template(name) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == ErrorKind::TemplateNotFound => Ok(false),
            Err(e) => {
                let path = format!("{TEMPLATES_DIR}/{name}");
                let file = (files.files.get(&path)).map_or(path, |file| file.origin.clone());
                Err(anyhow::Error::new(e).context(format!("loading {file}")))
            }
        };

        if !has_template(DEFAULT_TEMPLATE)? {
            bail!(
                "the theme has no {}, which every theme needs",
                default_path()
            );
        }
        let mut templates = HashMap::new();
        for page_type in PageType::ALL {
            let template = page_type.template();
            if has_template(&template)? {
                templates.insert(page_type, template);
            } else if page_type != PageType::NotFound {
                // A 404 page has no content for default.html to show, so
                // without a 404.html the site gets none, and its host shows
                // its own.
                templates.insert(page_type, DEFAULT_TEMPLATE.to_string());
            }
        }

        Ok(Self { env, templates })
    }

    /// Whether it renders pages of type `page_type`, as it doesn't the 404
    /// page without a `404.html`.
    pub fn renders(&self, page_type: PageType) -> bool {
        self.templates.contains_key(&page_type)
    }

    /// Render a page of type `page_type` from `ctx`, or `None` if the theme
    /// doesn't render that type of page.
    pub fn render(&self, page_type: PageType, ctx: Value) -> Result<Option<String>> {
        let Some(name) = self.templates.get(&page_type) else {
            return Ok(None);
        };
        Ok(Some(self.env.get_template(name)?.render(ctx)?))
    }
}

/// Apply `change` to `value`, if it's text. In an HTML template the result is
/// escaped now, as the template would, but for its `/`s, which needn't be, so
/// a URL reads as written in the page's source without `| safe`.
fn url_filter(state: &State, value: Value, change: impl Fn(&str) -> Cow<'_, str>) -> Value {
    let Some(url) = value.as_str() else {
        return value; // nothing to change, like an undefined value
    };
    let changed = change(url);
    if value.is_safe() {
        Value::from_safe_string(changed.into_owned())
    } else if matches!(state.auto_escape(), AutoEscape::Html) {
        let escaped = HtmlEscape(&changed).to_string();
        Value::from_safe_string(escaped.replace("&#x2f;", "/"))
    } else {
        Value::from(changed.into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::context;
    use serde_json::json;

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

    /// Load the first of `themes`, which builds on the rest, for a site at
    /// `url`.
    fn load_chain(themes: &[Location], url: &SiteUrl) -> Result<Theme> {
        Theme::load(&ThemeFiles::read(themes)?, url)
    }

    /// Load the theme in `dir`, which builds on nothing, for a site at the
    /// root of its domain.
    fn load(dir: &Path) -> Result<Theme> {
        load_chain(&[Location::Folder(dir.to_path_buf())], &SiteUrl::default())
    }

    /// The static files of the first of `themes`, which builds on the rest.
    fn static_files(themes: &[Location]) -> Result<Vec<StaticFile>> {
        ThemeFiles::read(themes)?.static_files()
    }

    /// The static files of the theme in `dir`, which builds on nothing.
    fn static_files_in(dir: &Path) -> Result<Vec<StaticFile>> {
        static_files(&[Location::Folder(dir.to_path_buf())])
    }

    /// The test plugin, started as `test`, with `theme` as its `theme`
    /// hook's settings.
    fn plugin(theme: serde_json::Value) -> Plugins {
        let plugin = crate::config::Plugin {
            name: "test".to_string(),
            wasm: crate::config::Wasm::File(crate::plugin::tests::test_plugin().to_path_buf()),
            read: Vec::new(),
            settings: serde_json::json!({ "theme": theme }).to_string(),
        };
        let now = time::OffsetDateTime::UNIX_EPOCH;
        Plugins::start(&[plugin], Path::new("."), now).unwrap()
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

    /// Each static file's path and contents, as text.
    fn contents(files: &[StaticFile]) -> Vec<(String, String)> {
        files
            .iter()
            .map(|file| {
                let path = file.path.to_string_lossy().replace('\\', "/");
                (path, String::from_utf8_lossy(&file.contents).into_owned())
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
        let rendered = render_each(&load(&dir).unwrap());
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
        let rendered = render_each(&load(&dir).unwrap());
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
        let theme = load(&dir).unwrap();
        let html = theme.render(PageType::Page, context! {}).unwrap();
        assert_eq!(html.as_deref(), Some("<little>default"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn url_filters_put_the_sites_path_and_address_before_a_url() {
        let dir = scratch(
            "url-filters",
            &[(
                "_templates/default.html",
                "{{ '/assets/style.css' | url }} {{ page.url | absolute_url }} \
                 {{ '/og.png' | url | absolute_url }} {{ 'https://example.com/' | url }} \
                 {{ '/search?q=a&b' | url }} {{ '/a&amp;b' | safe | url }} [{{ missing | url }}]",
            )],
        );
        let render = |url: &SiteUrl| {
            let theme = load_chain(&[Location::Folder(dir.clone())], url).unwrap();
            let page_url = format!("{}/guide/", url.path);
            let ctx = context! { page => context! { url => page_url } };
            theme.render(PageType::Page, ctx).unwrap().unwrap()
        };

        let project = SiteUrl {
            origin: "https://withlittle.github.io".into(),
            path: "/little".into(),
            shared: None,
        };
        assert_eq!(
            render(&project),
            "/little/assets/style.css https://withlittle.github.io/little/guide/ \
             https://withlittle.github.io/little/og.png https://example.com/ \
             /little/search?q=a&amp;b /little/a&amp;b []"
        );

        // Without a url, URLs are left as they are, escaped but for their `/`s.
        assert_eq!(
            render(&SiteUrl::default()),
            "/assets/style.css /guide/ /og.png https://example.com/ \
             /search?q=a&amp;b /a&amp;b []"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_theme_replaces_the_files_of_the_one_it_builds_on() {
        let root = scratch(
            "layers",
            &[
                (
                    "base/_templates/default.html",
                    r#"<{% include "partials/brand.html" %}>{{ page.type }}"#,
                ),
                ("base/_templates/partials/brand.html", "base"),
                ("base/_static/logo.svg", "base logo"),
                ("base/_static/css/style.css", "base style"),
                // Only what it changes.
                ("company/_templates/partials/brand.html", "company"),
                ("company/_static/logo.svg", "company logo"),
            ],
        );
        let (company, base) = (root.join("company"), root.join("base"));
        let chain = [
            Location::Folder(company.clone()),
            Location::Folder(base.clone()),
        ];
        let theme = load_chain(&chain, &SiteUrl::default()).unwrap();

        let ctx = context! { page => context! { type => "page" } };
        let html = theme.render(PageType::Page, ctx).unwrap();
        assert_eq!(html.as_deref(), Some("<company>page"));
        let files = static_files(&chain).unwrap();
        assert_eq!(
            contents(&files),
            [
                ("css/style.css".into(), "base style".into()),
                ("logo.svg".into(), "company logo".into()),
            ]
        );
        let css = base.join(STATIC_DIR).join("css").join("style.css");
        let logo = company.join(STATIC_DIR).join("logo.svg");
        assert_eq!(files[0].origin, css.display().to_string());
        assert_eq!(files[1].origin, logo.display().to_string());

        // A broken template is named as the file it's read from.
        let home = company.join("_templates").join("home.html");
        std::fs::write(&home, "{% if %}").unwrap();
        let message = format!(
            "{:#}",
            load_chain(&chain, &SiteUrl::default()).err().unwrap()
        );
        let expected = format!("loading {}", home.display());
        assert!(message.starts_with(&expected), "{message}");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_theme_needs_a_working_default_template() {
        let error = |files: &[(&str, &str)]| {
            let dir = scratch("broken", files);
            let message = format!("{:#}", load(&dir).err().unwrap());
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
        let home = Path::new("_templates").join("home.html");
        assert!(
            message.contains(&format!("{}: syntax error", home.display())),
            "{message}"
        );

        // A template is text.
        let dir = scratch("binary", &[("_templates/default.html", "")]);
        let file = dir.join("_templates").join("home.html");
        std::fs::write(&file, [0xff, 0xfe]).unwrap();
        let message = format!("{:#}", load(&dir).err().unwrap());
        assert!(
            message.starts_with(&format!("loading {}: ", file.display())),
            "{message}"
        );
        assert!(message.contains("a template must be UTF-8"), "{message}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn static_files_are_optional() {
        let dir = scratch("static", &[("_templates/default.html", "")]);
        assert!(static_files_in(&dir).unwrap().is_empty());

        let png: &[u8] = &[0x89, b'P', b'N', b'G', 0x00, 0xff];
        std::fs::create_dir_all(dir.join("_static/img")).unwrap();
        std::fs::write(dir.join("_static/img/logo.png"), png).unwrap();
        let files = static_files_in(&dir).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, Path::new("img").join("logo.png"));
        assert_eq!(files[0].contents, png); // byte for byte
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stylesheets_are_compiled_with_what_they_load_from_sass() {
        let dir = scratch(
            "sass",
            &[
                ("_templates/default.html", ""),
                (
                    "_static/assets/style.scss",
                    "@use 'colors';\na { color: colors.$link; }",
                ),
                (
                    "_static/assets/print.sass",
                    "@use 'colors'\nbody\n  color: black\n",
                ),
                ("_static/assets/notes.md", "Not Sass"),
                ("_sass/_colors.scss", "$link: teal;"),
                ("_sass/README.md", "Not published"),
            ],
        );
        let files = static_files_in(&dir).unwrap();
        assert_eq!(
            contents(&files),
            [
                ("assets/notes.md".into(), "Not Sass".into()),
                (
                    "assets/print.css".into(),
                    "body {\n  color: black;\n}\n".into()
                ),
                ("assets/style.css".into(), "a {\n  color: teal;\n}\n".into()),
            ]
        );
        // Named as the stylesheet it's compiled from, in a broken link, say.
        let origin = dir.join("_static").join("assets").join("style.scss");
        assert_eq!(files[2].origin, origin.display().to_string());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_theme_replaces_the_sass_of_the_one_it_builds_on() {
        let root = scratch(
            "sass-layers",
            &[
                ("base/_templates/default.html", ""),
                (
                    "base/_static/style.scss",
                    "@use 'colors';\na { color: colors.$link; }",
                ),
                ("base/_static/print.css", "base print"),
                ("base/_static/extra.scss", "a {"), // replaced, so never compiled
                ("base/_sass/_colors.scss", "$link: teal;"),
                ("company/_static/print.scss", "body { color: black; }"),
                ("company/_static/extra.css", "company extra"),
                ("company/_sass/_colors.scss", "$link: navy;"),
            ],
        );
        let chain = [
            Location::Folder(root.join("company")),
            Location::Folder(root.join("base")),
        ];
        assert_eq!(
            contents(&static_files(&chain).unwrap()),
            [
                ("extra.css".into(), "company extra".into()),
                ("print.css".into(), "body {\n  color: black;\n}\n".into()),
                ("style.css".into(), "a {\n  color: navy;\n}\n".into()),
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sass_in_the_wrong_place_stops_the_build() {
        let error = |files: &[(&str, &str)]| {
            let dir = scratch("sass-misplaced", files);
            let message = format!("{:#}", static_files_in(&dir).unwrap_err());
            (dir, message)
        };

        // A partial is for stylesheets to load, and they load from `_sass/`.
        let (dir, message) = error(&[
            ("_templates/default.html", ""),
            ("_static/assets/style.scss", "@use 'colors';"),
            ("_static/assets/_colors.scss", "$link: teal;"),
        ]);
        let partial = dir.join("_static").join("assets").join("_colors.scss");
        let expected = format!(
            "{} is a Sass partial, for stylesheets to load: move it to _sass/, where they load from",
            partial.display()
        );
        assert_eq!(message, expected);

        // Two files in one theme that would be the same file.
        for other in ["style.css", "style.sass"] {
            let (dir, message) = error(&[
                ("_templates/default.html", ""),
                ("_static/style.scss", ""),
                (&format!("_static/{other}"), ""),
            ]);
            let static_dir = dir.join("_static");
            let mut both = [static_dir.join(other), static_dir.join("style.scss")];
            both.sort();
            let expected = format!(
                "{} and {} would both be /style.css: remove or rename one",
                both[0].display(),
                both[1].display()
            );
            assert_eq!(message, expected);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_plugins_theme_hook_is_given_the_layered_files_by_their_paths() {
        let root = scratch(
            "hook-layers",
            &[
                ("company/_static/assets/style.scss", "a { color: navy; }"),
                ("company/_sass/_colors.scss", "$link: navy;"),
            ],
        );
        let chain = [Location::Folder(root.join("company")), Location::BuiltIn];
        let mut plugins = plugin(json!({ "list": "_static/paths.txt" }));
        let files = ThemeFiles::read(&chain).unwrap().hooked(&mut plugins);
        let files = files.unwrap().static_files().unwrap();
        let paths = files
            .iter()
            .find(|file| file.path == Path::new("paths.txt"));
        assert_eq!(
            String::from_utf8_lossy(&paths.unwrap().contents),
            // The company's style.scss replaces the built-in style.css.
            "_sass/_colors.scss\n_static/assets/script.js\n_static/assets/style.scss\n\
             _templates/404.html\n_templates/default.html"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_plugins_theme_hook_changes_the_theme() {
        let dir = scratch(
            "hook",
            &[
                (
                    "_templates/default.html",
                    "{% include 'og.html' %}{{ page.type }}",
                ),
                ("_static/logo.svg", "logo"),
                ("_static/style.css", "a {}"),
            ],
        );
        let read = || ThemeFiles::read(&[Location::Folder(dir.clone())]).unwrap();
        let mut plugins = plugin(json!({
            "remove": ["_static/style.css"],
            "add": [
                { "path": "_templates/og.html", "text": "<meta>" },
                { "path": "_static/style.scss", "text": "@use 'colors';\na { color: colors.$link; }" },
                { "path": "_sass/_colors.scss", "text": "$link: teal;" },
            ],
        }));
        let hooked = read().hooked(&mut plugins).unwrap();
        let theme = Theme::load(&hooked, &SiteUrl::default()).unwrap();
        let ctx = context! { page => context! { type => "page" } };
        let html = theme.render(PageType::Page, ctx).unwrap();
        assert_eq!(html.as_deref(), Some("<meta>page"));
        let files = hooked.static_files().unwrap();
        assert_eq!(
            contents(&files),
            [
                ("logo.svg".into(), "logo".into()),
                ("style.css".into(), "a {\n  color: teal;\n}\n".into()),
            ]
        );
        // A file the plugin gives back as it was is still the theme's.
        let logo = dir.join(STATIC_DIR).join("logo.svg");
        assert_eq!(files[0].origin, logo.display().to_string());
        assert_eq!(files[1].origin, "plugin `test`'s _static/style.scss");

        // A template it breaks is named for it.
        let broken = json!({ "add": [{ "path": "_templates/home.html", "text": "{% if %}" }] });
        let hooked = read().hooked(&mut plugin(broken)).unwrap();
        let message = format!(
            "{:#}",
            Theme::load(&hooked, &SiteUrl::default()).err().unwrap()
        );
        assert!(
            message.starts_with("loading plugin `test`'s _templates/home.html: syntax error"),
            "{message}"
        );

        // What it gives back must be a theme's files.
        let error = |theme: serde_json::Value| {
            let hooked = read().hooked(&mut plugin(theme));
            format!("{:#}", hooked.unwrap_err())
        };
        let add = |path: &str| json!({ "add": [{ "path": path, "text": "" }] });
        for path in ["style.css", "_assets/style.css"] {
            assert_eq!(
                error(add(path)),
                format!(
                    "plugin `test`'s `theme` hook gives back a file at `{path}`, but a theme's \
                     files are in _templates/, _static/, or _sass/"
                )
            );
        }
        assert_eq!(
            error(json!({ "remove": ["_templates/default.html"] })),
            "plugin `test`'s `theme` hook removes _templates/default.html, which every theme \
             needs"
        );
        assert_eq!(
            error(add("_static/_colors.scss")),
            "plugin `test`'s _static/_colors.scss is a Sass partial, for stylesheets to load: \
             move it to _sass/, where they load from"
        );
        let css = dir.join(STATIC_DIR).join("style.css");
        assert_eq!(
            error(add("_static/style.sass")),
            format!(
                "{} and plugin `test`'s _static/style.sass would both be /style.css: remove or \
                 rename one",
                css.display()
            )
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_builtin_theme_is_the_theme_folder() {
        // Compiled in, its files are listed by hand: this catches one that's
        // added to theme/ but not to `builtin`.
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("theme");
        let mut on_disk: Vec<(PathBuf, Vec<u8>)> = WalkDir::new(&dir)
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| {
                let path = entry.path().strip_prefix(&dir).unwrap().to_path_buf();
                (path, std::fs::read(entry.path()).unwrap())
            })
            .collect();
        let mut listed = vec![(PathBuf::from("little.yml"), builtin::CONFIG.into())];
        for (name, source) in builtin::TEMPLATES {
            listed.push((
                Path::new(TEMPLATES_DIR).join(name),
                source.as_bytes().into(),
            ));
        }
        for (path, contents) in builtin::STATIC {
            listed.push((Path::new(STATIC_DIR).join(path), contents.to_vec()));
        }
        on_disk.sort();
        listed.sort();

        let paths = |files: &[(PathBuf, Vec<u8>)]| -> Vec<PathBuf> {
            files.iter().map(|(path, _)| path.clone()).collect()
        };
        assert_eq!(paths(&listed), paths(&on_disk), "list every file in theme/");
        assert!(listed == on_disk, "a listed file includes another file");
    }
}
