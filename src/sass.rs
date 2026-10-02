//! Sass in a theme, compiled to CSS with grass. A `.scss` or `.sass` file in
//! a theme's `_static/` is a stylesheet, published as the CSS file at its
//! path; what it loads with `@use`, `@forward`, or `@import` comes from the
//! theme's `_sass/` folder, which isn't published.
//!
//! A stylesheet loads only from `_sass/`, never from beside itself, so the
//! files in `_static/` are just what the site gets. In `_sass/`, a file loads
//! from its own folder first, then from the root of `_sass/`, as Sass does
//! with `_sass/` as a load path. Like templates, the files in `_sass/` are
//! layered: each comes from the nearest theme that has it, so a theme can
//! replace any file of the one it builds on, and the other theme's
//! stylesheets load the replacement.

use anyhow::{Context, Result, anyhow};
use codemap::SpanLoc;
use std::collections::BTreeMap;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::theme::StaticFile;

/// The name grass gives a stylesheet compiled from a string. Its loads are
/// then found from the folder it's in, the root of `_sass/`. A file in
/// `_sass/` can't be loaded by it: a load always ends in `.scss`, `.sass`,
/// or `.css`.
const STYLESHEET: &str = "stdin";

/// Whether the file at `path` is Sass, by its extension.
pub fn is_sass(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("scss" | "sass")
    )
}

/// Whether the file at `path` is a Sass partial, named for loading: like
/// `_colors.scss`.
pub fn is_partial(path: &Path) -> bool {
    is_sass(path)
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('_'))
}

/// Compile `stylesheet`, a file in `_static/`, to CSS, loading from `sass`:
/// the files in `_sass/`, by their path from it.
pub fn compile(stylesheet: &StaticFile, sass: &BTreeMap<PathBuf, StaticFile>) -> Result<Vec<u8>> {
    let files = Files {
        stylesheet: &stylesheet.origin,
        sass,
    };
    let syntax = match stylesheet.path.extension().and_then(|ext| ext.to_str()) {
        Some("sass") => grass::InputSyntax::Sass,
        _ => grass::InputSyntax::Scss,
    };
    // The root of `_sass/` is `""`, so a load path of `""` is `_sass/` itself.
    let options = grass::Options::default()
        .fs(&files)
        .logger(&files)
        .input_syntax(syntax)
        .load_path("");
    let compiled = std::str::from_utf8(&stylesheet.contents)
        .map_err(anyhow::Error::new)
        .and_then(|source| {
            grass::from_string(source, &options).map_err(|error| files.error(*error))
        });
    let css = compiled.with_context(|| format!("compiling {}", stylesheet.origin))?;
    Ok(css.into_bytes())
}

/// All grass can see while it compiles a stylesheet: the files in `_sass/`,
/// so that a theme, even a downloaded one, can't load anything else on disk.
/// They're at their paths from `_sass/`, and errors and warnings name each
/// by the file it was read from.
#[derive(Debug)]
struct Files<'a> {
    /// The file the stylesheet was read from.
    stylesheet: &'a str,
    sass: &'a BTreeMap<PathBuf, StaticFile>,
}

impl Files<'_> {
    /// The file in `_sass/` at `path`, if there's one.
    fn get(&self, path: &Path) -> Option<&StaticFile> {
        self.sass.get(&within(path)?)
    }

    /// Where `location` is, as `file:line:column`, which editors and
    /// terminals can follow.
    fn locate(&self, location: &SpanLoc) -> String {
        let name = location.file.name();
        let file = if name == STYLESHEET {
            self.stylesheet
        } else {
            self.get(Path::new(name)).map_or(name, |file| &file.origin)
        };
        let (line, column) = (location.begin.line + 1, location.begin.column + 1);
        format!("{file}:{line}:{column}")
    }

    fn error(&self, error: grass::Error) -> anyhow::Error {
        let message = error.to_string();
        match error.kind() {
            grass::ErrorKind::ParseError { message, loc, .. } => {
                anyhow!("{}: {message}", self.locate(&loc))
            }
            _ => anyhow!("{}", message.trim().trim_start_matches("Error: ")),
        }
    }
}

impl grass::Fs for Files<'_> {
    fn is_dir(&self, path: &Path) -> bool {
        within(path).is_some_and(|dir| {
            self.sass
                .keys()
                .any(|file| file.starts_with(&dir) && *file != dir)
        })
    }

    fn is_file(&self, path: &Path) -> bool {
        self.get(path).is_some()
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        match self.get(path) {
            Some(file) => Ok(file.contents.clone()),
            None => Err(io::ErrorKind::NotFound.into()),
        }
    }

    // grass knows a file by this path, so it loads one module once however
    // it's reached, as Sass does.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        within(path).ok_or_else(|| io::ErrorKind::NotFound.into())
    }
}

impl grass::Logger for Files<'_> {
    fn debug(&self, location: SpanLoc, message: &str) {
        eprintln!("debug: {}: {message}", self.locate(&location));
    }

    fn warn(&self, location: SpanLoc, message: &str) {
        eprintln!("warning: {}: {message}", self.locate(&location));
    }
}

/// `path` without its `.`s and `..`s, or `None` if it leaves `_sass/`, with
/// a `..` too many or by being absolute.
fn within(path: &Path) -> Option<PathBuf> {
    let mut within = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => within.push(name),
            Component::CurDir => {}
            Component::ParentDir => {
                if !within.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(within)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file read from `origin`, at `path`.
    fn file(path: &str, origin: &str, contents: &str) -> StaticFile {
        StaticFile {
            path: PathBuf::from(path),
            origin: origin.to_string(),
            contents: contents.into(),
        }
    }

    /// The files in `_sass/`, each a (path, contents) pair, read from
    /// `theme/_sass/`.
    fn sass(files: &[(&str, &str)]) -> BTreeMap<PathBuf, StaticFile> {
        files
            .iter()
            .map(|&(path, contents)| {
                let origin = format!("theme/_sass/{path}");
                (PathBuf::from(path), file(path, &origin, contents))
            })
            .collect()
    }

    /// Compile `source`, from `_static/style.scss` or `.sass`, with `sass`.
    fn compile_from(path: &str, source: &str, sass: &[(&str, &str)]) -> Result<String> {
        let stylesheet = file(path, &format!("theme/_static/{path}"), source);
        let css = compile(&stylesheet, &self::sass(sass))?;
        Ok(String::from_utf8(css).unwrap())
    }

    #[test]
    fn a_stylesheet_loads_from_sass() {
        let library = [
            ("_colors.scss", "$primary: teal !default;"),
            (
                "buttons/_index.scss",
                "@use 'shape'; .button { color: red; }",
            ),
            ("buttons/_shape.scss", ".round { border-radius: 1em; }"),
            ("_type.sass", "$font: serif"),
            ("print.css", "@media print { a { color: black; } }"),
        ];
        let css = compile_from(
            "style.scss",
            "@use 'colors' with ($primary: navy);
             @use 'type.sass';
             @use 'buttons';
             @use 'print';
             body { color: colors.$primary; font-family: type.$font; }",
            &library,
        )
        .unwrap();
        assert_eq!(
            css,
            ".round {\n  border-radius: 1em;\n}\n\n.button {\n  color: red;\n}\n\n\
             @media print {\n  a {\n    color: black;\n  }\n}\n\
             body {\n  color: navy;\n  font-family: serif;\n}\n"
        );

        // The indented syntax, by the stylesheet's extension.
        let css = compile_from(
            "style.sass",
            "@use 'colors'\nbody\n  color: colors.$primary\n",
            &library,
        );
        assert_eq!(css.unwrap(), "body {\n  color: teal;\n}\n");
    }

    #[test]
    fn a_file_in_sass_loads_from_its_folder_then_the_root() {
        let css = compile_from(
            "style.scss",
            "@import 'vendor/library';",
            &[
                (
                    "vendor/library.scss",
                    "@import 'variables'; @import 'reset';",
                ),
                ("vendor/_variables.scss", "$gap: 1rem;"),
                ("_variables.scss", "$gap: 2rem;"),
                ("_reset.scss", "p { margin: $gap; }"),
            ],
        );
        assert_eq!(css.unwrap(), "p {\n  margin: 1rem;\n}\n");
    }

    #[test]
    fn nothing_outside_sass_can_be_loaded() {
        let error = |load: &str| {
            let source = format!("@import '{load}';");
            let message = compile_from("style.scss", &source, &[("a/_b.scss", "")]);
            format!("{:#}", message.unwrap_err())
        };
        let outside = [
            "../_static/other",
            "a/../../outside",
            "/etc/hosts",
            "style",
            "stdin",
        ];
        for load in outside {
            let message = error(load);
            assert_eq!(
                message,
                "compiling theme/_static/style.scss: \
                 theme/_static/style.scss:1:9: Can't find stylesheet to import.",
                "{load}"
            );
        }
        // A `..` that stays inside is fine.
        assert!(compile_from("style.scss", "@use 'a/../a/b';", &[("a/_b.scss", "")]).is_ok());
    }

    #[test]
    fn an_error_names_the_file_its_in() {
        let message = compile_from(
            "style.scss",
            "@use 'colors';\nbody { color: colors.$primary; }",
            &[("_colors.scss", "$primary: $missing;")],
        )
        .unwrap_err();
        assert_eq!(
            format!("{message:#}"),
            "compiling theme/_static/style.scss: \
             theme/_sass/_colors.scss:1:11: Undefined variable."
        );

        let message = compile_from("style.scss", "body {", &[]).unwrap_err();
        assert_eq!(
            format!("{message:#}"),
            "compiling theme/_static/style.scss: theme/_static/style.scss:1:6: expected \"}\"."
        );
    }

    #[test]
    fn sass_knows_its_files_by_their_extension() {
        assert!(is_sass(Path::new("assets/style.scss")));
        assert!(is_sass(Path::new("style.sass")));
        assert!(!is_sass(Path::new("style.css")));
        assert!(!is_sass(Path::new(".scss")));
        assert!(is_partial(Path::new("assets/_colors.scss")));
        assert!(is_partial(Path::new("_type.sass")));
        assert!(!is_partial(Path::new("_assets/colors.scss")));
        assert!(!is_partial(Path::new("_reset.css")));
    }
}
