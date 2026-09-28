//! The 3-level cascading config: little -> theme -> site, each a `little.yml`.
//! Each level deep-merges over the previous, so the *last* writer wins.
//! little's own file is compiled in; the theme's and the site's are optional.
//!
//! `theme` chooses the theme level: `true` for little's built-in theme,
//! `false` for none, or the path to a theme's folder. A theme can set it too,
//! to build on another theme, whose settings go beneath its own: a site whose
//! theme builds on another cascades little -> that theme -> its theme -> site.
//!
//! `source` and `destination` name the folders a site is built from and
//! into. Any level can set them, but they're always relative to the site.

use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::theme::{self, Location};

/// The config file a theme and a site may each provide.
pub const CONFIG_FILE: &str = "little.yml";

/// A site's merged config, and what it chose.
#[derive(Debug)]
pub struct Config {
    /// The merged cascade: what `little config` prints, and what the theme
    /// sees as `site`.
    pub values: Mapping,
    /// The theme the site chose, then each theme it builds on in turn. Empty
    /// for `theme: false`.
    pub themes: Vec<Location>,
    /// The folder of Markdown files the site is built from.
    pub source_dir: PathBuf,
    /// The folder the site is built into, which every build wipes.
    pub destination_dir: PathBuf,
}

/// little's built-in defaults: level 1 of the cascade. They live in
/// `src/little.yml`, compiled into the binary so little stays a single file
/// with nothing to install alongside it.
fn builtin_defaults() -> Mapping {
    parse_config(include_str!("little.yml")).expect("src/little.yml holds settings")
}

/// Follow the chain of themes from the one the site chooses, then merge the
/// cascade.
pub fn load(site_dir: &Path) -> Result<Config> {
    let defaults = builtin_defaults();
    let site_file = site_dir.join(CONFIG_FILE);
    let site_cfg = read_config(&site_file)?;

    let mut next = match site_cfg.get("theme").or(defaults.get("theme")) {
        Some(value) => choose_theme(value, site_dir, &site_file)?,
        None => None,
    };
    let mut themes = Vec::new();
    let mut theme_cfgs = Vec::new();
    // Each theme folder's real path, to catch themes that build on each other
    // in a loop however their paths are written.
    let mut seen = Vec::new();
    while let Some(location) = next {
        let cfg = match &location {
            Location::BuiltIn => {
                next = None; // it builds on nothing
                parse_config(theme::builtin::CONFIG).expect("theme/little.yml holds settings")
            }
            Location::Folder(dir) => {
                let real = dir
                    .canonicalize()
                    .with_context(|| format!("reading {}", dir.display()))?;
                if seen.contains(&real) {
                    let chain: Vec<String> = themes
                        .iter()
                        .chain([&location])
                        .map(ToString::to_string)
                        .collect();
                    bail!(
                        "themes build on each other in a loop: {}",
                        chain.join(" -> ")
                    );
                }
                seen.push(real);
                let file = dir.join(CONFIG_FILE);
                let cfg = read_config(&file)?;
                next = match cfg.get("theme") {
                    Some(value) => choose_theme(value, dir, &file)?,
                    None => None,
                };
                cfg
            }
        };
        themes.push(location);
        theme_cfgs.push(cfg);
    }

    // The theme at the end of the chain goes beneath the ones that build on it.
    let mut values = defaults;
    for cfg in theme_cfgs.into_iter().rev() {
        merge(&mut values, cfg);
    }
    merge(&mut values, site_cfg);

    let source_dir = folder(site_dir, &values, "source")?;
    let destination_dir = folder(site_dir, &values, "destination")?;
    Ok(Config {
        values,
        themes,
        source_dir,
        destination_dir,
    })
}

/// The theme a `theme` setting chooses, if any. A path is relative to `dir`,
/// the folder of the `file` that sets it: a theme can't know where the sites
/// that use it are.
fn choose_theme(value: &Value, dir: &Path, file: &Path) -> Result<Option<Location>> {
    match value {
        Value::Bool(true) => Ok(Some(Location::BuiltIn)),
        Value::Bool(false) => Ok(None),
        Value::String(url) if url.starts_with("https://") || url.starts_with("http://") => bail!(
            "{} sets `theme` to `{url}`, but little can't download themes yet",
            file.display()
        ),
        Value::String(path) => {
            let folder = resolve(dir, path);
            if !folder.is_dir() {
                bail!(
                    "{} sets `theme` to `{path}`, but there's no folder at {}",
                    file.display(),
                    folder.display()
                );
            }
            Ok(Some(Location::Folder(folder)))
        }
        _ => bail!(
            "`theme` in {} must be `true`, `false`, or the path to a theme folder",
            file.display()
        ),
    }
}

/// The folder a setting names. It's relative to the site even when a theme
/// sets it: it's the site that's built.
fn folder(site_dir: &Path, values: &Mapping, key: &str) -> Result<PathBuf> {
    match values.get(key) {
        Some(Value::String(path)) => Ok(resolve(site_dir, path)),
        _ => bail!("`{key}` must be the path to a folder"),
    }
}

/// Resolve a path a config names against `dir`: an absolute path is just that
/// path. Tidied, so a site in `.` that sets `./docs` shows it as `./docs`.
fn resolve(dir: &Path, path: &str) -> PathBuf {
    dir.join(path).components().collect()
}

/// Read one level's config. A missing file isn't an error: that level just
/// has nothing to add.
fn read_config(path: &Path) -> Result<Mapping> {
    match std::fs::read_to_string(path) {
        Ok(raw) => parse_config(&raw).with_context(|| format!("parsing {}", path.display())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Mapping::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// Parse a config file's settings. An empty or comment-only file has none.
/// Anything but settings is an error: merged, it would replace every level
/// beneath it.
fn parse_config(raw: &str) -> serde_yaml::Result<Mapping> {
    Ok(serde_yaml::from_str::<Option<Mapping>>(raw)?.unwrap_or_default())
}

/// Merge `over` into `base`, key by key. Where both set a key to settings,
/// those merge the same way; any other value replaces the one beneath it.
fn merge(base: &mut Mapping, over: Mapping) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(Value::Mapping(below)), Value::Mapping(above)) => merge(below, above),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(cfg: &'a Mapping, key: &str) -> Option<&'a str> {
        cfg.get(key).and_then(Value::as_str)
    }

    /// A fresh scratch folder holding `files`, each a (path, contents) pair.
    fn scratch(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("little-config-test-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for (file, contents) in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        root
    }

    /// Why the site's config doesn't load once its little.yml is `site_cfg`.
    fn error(site: &Path, site_cfg: &str) -> String {
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
        format!("{:#}", load(site).unwrap_err())
    }

    /// little's defaults, with the built-in theme's settings over them.
    fn with_builtin_theme() -> Mapping {
        let mut values = builtin_defaults();
        merge(&mut values, parse_config(theme::builtin::CONFIG).unwrap());
        values
    }

    #[test]
    fn a_site_gets_the_builtin_theme_unless_it_chooses_another() {
        let site = scratch(
            "choose",
            &[
                ("theme/little.yml", "font: mono\n"),
                ("path/to/my/theme/little.yml", "name: Mine\nfont: serif\n"),
            ],
        );
        let load_with = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            load(&site).unwrap()
        };

        // Without a `theme`, or with `theme: true`, the built-in theme, even
        // for a site with a `theme/` folder.
        assert_eq!(load(&site).unwrap().themes, [Location::BuiltIn]);
        for site_cfg in ["name: My Docs\n", "theme: true\n"] {
            let cfg = load_with(site_cfg);
            assert_eq!(cfg.themes, [Location::BuiltIn], "with {site_cfg:?}");
            assert_eq!(get(&cfg.values, "font"), get(&with_builtin_theme(), "font"));
        }

        // With `theme: false`, no theme, so no theme's settings either.
        let cfg = load_with("theme: false\n");
        assert!(cfg.themes.is_empty());
        assert_eq!(get(&cfg.values, "font"), None);

        // A path chooses the theme in that folder.
        let cfg = load_with("theme: ./path/to/my/theme\nname: My Docs\n");
        let chosen = Location::Folder(site.join("path/to/my/theme"));
        assert_eq!(cfg.themes, [chosen]);
        assert_eq!(get(&cfg.values, "font"), Some("serif")); // the chosen theme's
        assert_eq!(get(&cfg.values, "name"), Some("My Docs")); // the site still wins
        assert_eq!(get(&cfg.values, "theme"), Some("./path/to/my/theme"));

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn theme_paths_are_relative_to_the_file_that_sets_them() {
        let root = scratch(
            "paths",
            &[
                ("site/themes/mine/_templates/default.html", ""),
                ("shared/theme/_templates/default.html", ""),
                ("shared/company/little.yml", "theme: ../theme\n"),
            ],
        );
        let site = root.join("site");
        // Unquoted, as people usually write them.
        let chosen = |theme: &str| {
            std::fs::write(site.join(CONFIG_FILE), format!("theme: {theme}\n")).unwrap();
            let themes = load(&site).unwrap().themes;
            themes
                .iter()
                .map(|theme| theme.folder().unwrap().canonicalize().unwrap())
                .collect::<Vec<_>>()
        };
        let real = |path: &str| root.join(path).canonicalize().unwrap();

        // A site's are relative to the site, not to wherever little runs.
        assert_eq!(chosen("themes/mine"), [real("site/themes/mine")]);
        assert_eq!(chosen("./themes/mine"), [real("site/themes/mine")]);
        assert_eq!(chosen("../shared/theme"), [real("shared/theme")]);

        // Absolute: the temp dir is /… on macOS and Linux, C:\… on Windows.
        let absolute = root.join("shared/theme").display().to_string();
        assert_eq!(chosen(&absolute), [real("shared/theme")]);

        // A theme's are relative to the theme: it can't know where sites are.
        let chain = [real("shared/company"), real("shared/theme")];
        assert_eq!(chosen("../shared/company"), chain);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_theme_can_build_on_another() {
        let site = scratch(
            "chain",
            &[
                ("little.yml", "theme: ./company\nname: My Docs\n"),
                (
                    "company/little.yml",
                    "theme: ../fancy\nfont: Inter\ncolors:\n  accent: navy\n",
                ),
                (
                    "fancy/little.yml",
                    "name: Fancy\nfont: serif\nlang: en\ncolors:\n  accent: pink\n  text: black\n",
                ),
            ],
        );
        let company = site.join("company");

        // little -> fancy -> company -> site, each over the one before, as
        // `little config` prints them.
        let cfg = load(&site).unwrap();
        let fancy = Location::Folder(company.join("../fancy"));
        assert_eq!(cfg.themes, [Location::Folder(company.clone()), fancy]);
        assert_eq!(
            serde_yaml::to_string(&cfg.values).unwrap(),
            "source: ./docs\n\
             destination: ./dist\n\
             theme: ./company\n\
             name: My Docs\n\
             font: Inter\n\
             lang: en\n\
             colors:\n  accent: navy\n  text: black\n"
        );

        // A theme can build on little's built-in theme, too.
        std::fs::write(company.join(CONFIG_FILE), "theme: true\nfont: Inter\n").unwrap();
        let cfg = load(&site).unwrap();
        assert_eq!(cfg.themes, [Location::Folder(company), Location::BuiltIn]);
        assert_eq!(get(&cfg.values, "font"), Some("Inter"));
        assert_eq!(get(&cfg.values, "lang"), get(&with_builtin_theme(), "lang"));

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn themes_cannot_build_on_each_other_in_a_loop() {
        let site = scratch(
            "loop",
            &[
                ("a/little.yml", "theme: ../b\n"),
                ("b/little.yml", "theme: ../a\n"),
            ],
        );
        let (a, b) = (site.join("a"), site.join("a/../b"));
        let message = error(&site, "theme: ./a\n");
        let expected = format!(
            "themes build on each other in a loop: {} -> {} -> {}",
            a.display(),
            b.display(),
            b.join("../a").display()
        );
        assert_eq!(message, expected);

        // Even a theme that builds on itself.
        std::fs::write(a.join(CONFIG_FILE), "theme: .\n").unwrap();
        let message = error(&site, "theme: ./a\n");
        assert!(
            message.contains("themes build on each other in a loop"),
            "{message}"
        );

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn bad_theme_choices_explain_themselves() {
        let site = scratch("bad-theme", &[("theme/_templates/default.html", "")]);
        let file = site.join(CONFIG_FILE);
        let file = file.display();

        let message = error(&site, "theme: ./missing\n");
        let expected = format!(
            "{file} sets `theme` to `./missing`, but there's no folder at {}",
            site.join("missing").display()
        );
        assert_eq!(message, expected);
        // `default` is a folder name like any other, and only unquoted `true`
        // and `false` are booleans.
        for theme in ["default", "\"true\"", "no"] {
            let message = error(&site, &format!("theme: {theme}\n"));
            assert!(message.contains("but there's no folder at"), "{message}");
        }

        // Nothing, or anything but `true`, `false`, or text, isn't a theme.
        for theme in ["", " [a, b]", " 1"] {
            let message = error(&site, &format!("theme:{theme}\n"));
            let expected =
                format!("`theme` in {file} must be `true`, `false`, or the path to a theme folder");
            assert_eq!(message, expected);
        }

        let url = "https://github.com/withlittle/fancy_theme/archive/refs/tags/5.0.4.zip";
        let message = error(&site, &format!("theme: {url}\n"));
        let expected =
            format!("{file} sets `theme` to `{url}`, but little can't download themes yet");
        assert_eq!(message, expected);

        // A theme's own `theme` is checked the same way, and named as its.
        let theme_file = site.join("theme").join(CONFIG_FILE);
        std::fs::write(&theme_file, "theme: [a, b]\n").unwrap();
        let message = error(&site, "theme: ./theme\n");
        let expected = format!(
            "`theme` in {} must be `true`, `false`, or the path to a theme folder",
            theme_file.display()
        );
        assert_eq!(message, expected);

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn folders_are_relative_to_the_site_wherever_theyre_set() {
        let root = scratch(
            "folders",
            &[("site/themes/mine/_templates/default.html", "")],
        );
        let site = root.join("site");
        let folders = |theme_cfg: &str, site_cfg: &str| {
            std::fs::write(site.join("themes/mine").join(CONFIG_FILE), theme_cfg).unwrap();
            let site_cfg = format!("theme: ./themes/mine\n{site_cfg}");
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            let cfg = load(&site).unwrap();
            (cfg.source_dir, cfg.destination_dir)
        };

        // little's own defaults.
        assert_eq!(folders("", ""), (site.join("docs"), site.join("dist")));

        // A theme can set them, but they're still relative to the site...
        let theme = "source: content\ndestination: ./public\n";
        let expected = (site.join("content"), site.join("public"));
        assert_eq!(folders(theme, ""), expected);

        // ...and the site has the last word, of course.
        let site_cfg = "source: ../shared/docs\n";
        let expected = (site.join("../shared/docs"), site.join("public"));
        assert_eq!(folders(theme, site_cfg), expected);

        // Absolute paths work too.
        let out = root.join("out");
        let site_cfg = format!("destination: {}\n", out.display());
        assert_eq!(folders("", &site_cfg), (site.join("docs"), out));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bad_folders_explain_themselves() {
        let site = scratch("bad-folders", &[]);
        for key in ["source", "destination"] {
            // A list, or no value at all, isn't a path.
            for value in [" [a, b]", ""] {
                let message = error(&site, &format!("{key}:{value}\n"));
                assert_eq!(message, format!("`{key}` must be the path to a folder"));
            }
        }

        // Whether the source is there is for the build to say: the config
        // doesn't read it.
        std::fs::write(site.join(CONFIG_FILE), "source: ./missing\n").unwrap();
        assert!(load(&site).is_ok());

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn config_files_hold_settings() {
        let site = scratch("settings", &[]);
        let parsing = format!("parsing {}: ", site.join(CONFIG_FILE).display());

        // Settings are keys and values: a list or a single value would replace
        // them all.
        for site_cfg in ["- source: ./docs\n", "My Docs\n"] {
            let message = error(&site, site_cfg);
            assert!(message.starts_with(&parsing), "{message}");
            assert!(message.contains("expected a YAML mapping"), "{message}");
        }

        // YAML that doesn't parse says where.
        let message = error(&site, "name: \"My Docs\n");
        assert!(message.starts_with(&parsing), "{message}");
        assert!(message.contains("at line"), "{message}");

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn config_files_are_optional_and_can_be_empty() {
        let site = scratch("optional", &[("theme/_templates/default.html", "")]);

        // Without a little.yml, or with one that's empty or only comments, a
        // site gets little's defaults and the built-in theme's.
        assert_eq!(load(&site).unwrap().values, with_builtin_theme());
        for site_cfg in ["", "# nothing to set yet\n"] {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            assert_eq!(load(&site).unwrap().values, with_builtin_theme());
        }

        // The same goes for a theme's little.yml.
        std::fs::write(site.join(CONFIG_FILE), "theme: ./theme\n").unwrap();
        let mut expected = builtin_defaults();
        expected.insert("theme".into(), "./theme".into());
        assert_eq!(load(&site).unwrap().values, expected);
        std::fs::write(site.join("theme").join(CONFIG_FILE), "# nothing\n").unwrap();
        assert_eq!(load(&site).unwrap().values, expected);

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn little_defaults_only_its_own_settings() {
        let cfg = builtin_defaults();
        assert_eq!(get(&cfg, "source"), Some("./docs"));
        assert_eq!(get(&cfg, "destination"), Some("./dist"));
        assert_eq!(cfg.get("theme"), Some(&Value::Bool(true)));
        // A site's name, language, and look are the theme's to default.
        for key in ["name", "lang"] {
            assert_eq!(cfg.get(key), None, "little shouldn't default `{key}`");
        }
    }

    #[test]
    fn the_builtin_theme_builds_on_nothing() {
        // The chain ends with the built-in theme, so a `theme` in its config
        // would be ignored.
        let cfg = parse_config(theme::builtin::CONFIG).unwrap();
        assert_eq!(cfg.get("theme"), None);
    }

    #[test]
    fn later_levels_override_earlier_ones() {
        let mut cfg = parse_config("name: Documentation\nfont: serif\n").unwrap();
        merge(&mut cfg, parse_config("name: My Docs\n").unwrap());

        assert_eq!(get(&cfg, "name"), Some("My Docs")); // last writer wins
        assert_eq!(get(&cfg, "font"), Some("serif")); // the earlier value survives
    }

    #[test]
    fn merging_recurses_into_settings_of_settings() {
        let mut cfg = parse_config("a:\n  x: 1\n  y: 2\nb: [1, 2]\n").unwrap();
        merge(
            &mut cfg,
            parse_config("a:\n  y: 20\n  z: 30\nb: [3]\n").unwrap(),
        );

        let a = cfg.get("a").unwrap();
        assert_eq!(a.get("x").and_then(Value::as_u64), Some(1));
        assert_eq!(a.get("y").and_then(Value::as_u64), Some(20));
        assert_eq!(a.get("z").and_then(Value::as_u64), Some(30));
        // Anything but settings, like a list, is replaced whole.
        let b: Value = serde_yaml::from_str("[3]").unwrap();
        assert_eq!(cfg.get("b"), Some(&b));
    }
}
