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
//!
//! little adds two filters to a theme's templates, for URLs the theme writes
//! itself: `url` puts the path of the site's `url` before a URL from the root
//! of the site, and `absolute_url` its scheme and host before one that has
//! the path.

use anyhow::{Context, Result, bail};
use minijinja::{AutoEscape, Environment, ErrorKind, HtmlEscape, State, Value, path_loader};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::config::SiteUrl;
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
    pub const STATIC: &[(&str, &[u8])] = &[(
        "assets/style.css",
        include_bytes!("../theme/_static/assets/style.css"),
    )];
}

/// The types of page little builds. A theme can give each one its own
/// template, named after the type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageType {
    /// The source folder's `README.md`, or `README.ipynb`: the site's home
    /// page.
    Home,
    /// A subfolder's `README.md`, or `README.ipynb`: that folder's home page.
    Section,
    /// Any other Markdown file or notebook.
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

    /// The source of the theme's template `name`, if it has one.
    fn template(&self, name: &str) -> Result<Option<String>, minijinja::Error> {
        match self {
            Self::BuiltIn => Ok(builtin::TEMPLATES
                .iter()
                .find(|(builtin, _)| *builtin == name)
                .map(|(_, source)| source.to_string())),
            Self::Folder(dir) | Self::Downloaded { dir, .. } => {
                path_loader(dir.join(TEMPLATES_DIR))(name)
            }
        }
    }

    /// The theme's own static files.
    fn static_files(&self) -> Result<Vec<StaticFile>> {
        match self {
            Self::BuiltIn => {
                let file = |&(path, contents): &(&str, &[u8])| StaticFile {
                    path: PathBuf::from(path),
                    origin: self.describe(&format!("{STATIC_DIR}/{path}")),
                    contents: contents.to_vec(),
                };
                Ok(builtin::STATIC.iter().map(file).collect())
            }
            Self::Folder(dir) | Self::Downloaded { dir, .. } => self.files_in(dir, STATIC_DIR),
        }
    }

    /// The theme's own Sass in `_sass/`, by its path from there. The built-in
    /// theme has none.
    fn sass_files(&self) -> Result<Vec<StaticFile>> {
        match self.folder() {
            Some(dir) => self.files_in(dir, SASS_DIR),
            None => Ok(Vec::new()),
        }
    }

    /// Every file in `folder` of the theme in `dir`, by its path from there.
    fn files_in(&self, dir: &Path, folder: &str) -> Result<Vec<StaticFile>> {
        let root = dir.join(folder);
        let mut files = Vec::new();
        if !root.is_dir() {
            return Ok(files);
        }
        for entry in WalkDir::new(&root) {
            let entry = entry?;
            if entry.file_type().is_dir() {
                continue;
            }
            let path = entry.path();
            let rel = path.strip_prefix(&root)?;
            files.push(StaticFile {
                path: rel.to_path_buf(),
                origin: self.describe(&Path::new(folder).join(rel).display().to_string()),
                contents: std::fs::read(path)
                    .with_context(|| format!("reading {}", path.display()))?,
            });
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

/// One of a theme's files, read: a static file, ready to copy to the site's
/// root, or one in `_sass/`, for stylesheets to load.
#[derive(Debug)]
pub struct StaticFile {
    /// Its path from `_static/`, and so from the site's root, or `_sass/`.
    pub path: PathBuf,
    /// The file it's read from, to name it in errors.
    pub origin: String,
    pub contents: Vec<u8>,
}

/// A theme, loaded and ready to render pages.
pub struct Theme {
    env: Environment<'static>,
    /// The template each type of page is rendered with. A type that isn't
    /// here isn't rendered at all.
    templates: HashMap<PageType, &'static str>,
    /// The theme the site chose, then each theme it builds on in turn.
    themes: Vec<Location>,
}

impl Theme {
    /// Load the first of `themes`, which builds on the rest in turn, for a
    /// site published at `url`. A build does this before it touches its
    /// destination, so a theme that can't build the site doesn't cost it its
    /// last build.
    pub fn load(themes: &[Location], url: &SiteUrl) -> Result<Self> {
        let Some(top) = themes.first() else {
            bail!("little can't build a site without a theme (`theme: false`) yet");
        };
        let mut env = Environment::new();
        let site = url.clone();
        env.add_filter("url", move |state: &State, value: Value| {
            url_filter(state, value, |url| site.link(url))
        });
        let site = url.clone();
        env.add_filter("absolute_url", move |state: &State, value: Value| {
            url_filter(state, value, |url| site.absolute(url))
        });
        // Templates are read as they're needed, by name, so a theme's folder
        // can hold other files (a README, say) without little trying to parse
        // them. Each comes from the first theme that has it, so a theme can
        // replace any of the templates of the one it builds on.
        let chain = themes.to_vec();
        env.set_loader(move |name| {
            for theme in &chain {
                if let Some(source) = theme.template(name)? {
                    return Ok(Some(source));
                }
            }
            Ok(None)
        });
        // Loading a template compiles it, so a broken one fails the build now,
        // rather than when the first page needs it.
        let has_template = |name: &str| match env.get_template(name) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == ErrorKind::TemplateNotFound => Ok(false),
            Err(e) => {
                let origin = themes
                    .iter()
                    .find(|theme| !matches!(theme.template(name), Ok(None)))
                    .unwrap_or(top);
                let file = origin.describe(&format!("{TEMPLATES_DIR}/{name}"));
                Err(anyhow::Error::new(e).context(format!("loading {file}")))
            }
        };

        if !has_template(DEFAULT_TEMPLATE)? {
            bail!("theme {top} has no {TEMPLATES_DIR}/{DEFAULT_TEMPLATE}, which every theme needs");
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

        Ok(Self {
            env,
            templates,
            themes: themes.to_vec(),
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

    /// The static files of every theme, merged, with each Sass stylesheet
    /// compiled to the CSS file at its path. Where two themes have a file at
    /// the same path, the one nearer the site wins, so a stylesheet replaces a
    /// CSS file, and a CSS file a stylesheet. The stylesheets load from the
    /// `_sass/` of every theme, merged the same way.
    pub fn static_files(&self) -> Result<Vec<StaticFile>> {
        let mut files = BTreeMap::new();
        let mut sass = BTreeMap::new();
        for theme in &self.themes {
            let mut own: BTreeMap<PathBuf, StaticFile> = BTreeMap::new();
            for file in theme.static_files()? {
                let path = if sass::is_partial(&file.path) {
                    bail!(
                        "{} is a Sass partial, for stylesheets to load: move it to {SASS_DIR}/, \
                         where they load from",
                        file.origin
                    );
                } else if sass::is_sass(&file.path) {
                    file.path.with_extension("css")
                } else {
                    file.path.clone()
                };
                if let Some(other) = own.insert(path.clone(), file) {
                    let mut both = [&other.origin, &own[&path].origin];
                    both.sort();
                    let url = path.to_string_lossy().replace('\\', "/");
                    bail!(
                        "{} and {} would both be /{url}: remove or rename one",
                        both[0],
                        both[1]
                    );
                }
            }
            for (path, file) in own {
                files.entry(path).or_insert(file);
            }
            for file in theme.sass_files()? {
                sass.entry(file.path.clone()).or_insert(file);
            }
        }
        files
            .into_iter()
            .map(|(path, file)| {
                if !sass::is_sass(&file.path) {
                    return Ok(file);
                }
                Ok(StaticFile {
                    contents: sass::compile(&file, &sass)?,
                    path,
                    origin: file.origin,
                })
            })
            .collect()
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

    /// Load the theme in `dir`, which builds on nothing, for a site at the
    /// root of its domain.
    fn load(dir: &Path) -> Result<Theme> {
        Theme::load(&[Location::Folder(dir.to_path_buf())], &SiteUrl::default())
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
            let theme = Theme::load(&[Location::Folder(dir.clone())], url).unwrap();
            let page_url = format!("{}/guide/", url.path);
            let ctx = context! { page => context! { url => page_url } };
            theme.render(PageType::Page, ctx).unwrap().unwrap()
        };

        let project = SiteUrl {
            origin: "https://withlittle.github.io".into(),
            path: "/little".into(),
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
        let theme = Theme::load(&chain, &SiteUrl::default()).unwrap();

        let ctx = context! { page => context! { type => "page" } };
        let html = theme.render(PageType::Page, ctx).unwrap();
        assert_eq!(html.as_deref(), Some("<company>page"));
        let files = theme.static_files().unwrap();
        assert_eq!(
            contents(&files),
            [
                ("css/style.css".into(), "base style".into()),
                ("logo.svg".into(), "company logo".into()),
            ]
        );
        let origin = |dir: &Path, file: &str| dir.join(STATIC_DIR).join(file).display().to_string();
        assert_eq!(files[0].origin, origin(&base, "css/style.css"));
        assert_eq!(files[1].origin, origin(&company, "logo.svg"));

        // A broken template is named as the file it's read from.
        std::fs::write(company.join("_templates/home.html"), "{% if %}").unwrap();
        let message = format!(
            "{:#}",
            Theme::load(&chain, &SiteUrl::default()).err().unwrap()
        );
        let expected = format!("loading {}", company.join("_templates/home.html").display());
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
        assert!(
            message.contains("_templates/home.html: syntax error"),
            "{message}"
        );
    }

    #[test]
    fn static_files_are_optional() {
        let dir = scratch("static", &[("_templates/default.html", "")]);
        assert!(load(&dir).unwrap().static_files().unwrap().is_empty());

        let png: &[u8] = &[0x89, b'P', b'N', b'G', 0x00, 0xff];
        std::fs::create_dir_all(dir.join("_static/img")).unwrap();
        std::fs::write(dir.join("_static/img/logo.png"), png).unwrap();
        let files = load(&dir).unwrap().static_files().unwrap();
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
        let files = load(&dir).unwrap().static_files().unwrap();
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
        let origin = dir.join("_static/assets/style.scss").display().to_string();
        assert_eq!(files[2].origin, origin);
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
        let theme = Theme::load(&chain, &SiteUrl::default()).unwrap();
        assert_eq!(
            contents(&theme.static_files().unwrap()),
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
            let message = format!("{:#}", load(&dir).unwrap().static_files().unwrap_err());
            (dir, message)
        };

        // A partial is for stylesheets to load, and they load from `_sass/`.
        let (dir, message) = error(&[
            ("_templates/default.html", ""),
            ("_static/assets/style.scss", "@use 'colors';"),
            ("_static/assets/_colors.scss", "$link: teal;"),
        ]);
        let partial = dir.join("_static/assets/_colors.scss");
        let expected = format!(
            "{} is a Sass partial, for stylesheets to load: move it to _sass/, where they load from",
            partial.display()
        );
        assert_eq!(message, expected);

        // Two files in one theme that would be the same file.
        for other in ["_static/style.css", "_static/style.sass"] {
            let (dir, message) = error(&[
                ("_templates/default.html", ""),
                ("_static/style.scss", ""),
                (other, ""),
            ]);
            let mut both = [dir.join(other), dir.join("_static/style.scss")];
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
