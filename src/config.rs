//! The 3-level cascading config: little -> theme -> site, each a `little.yml`.
//! Each level deep-merges over the previous, so the *last* writer wins.
//! little's own file is compiled in; the theme's and the site's are optional.
//!
//! Which theme fills level 2 is itself a setting, `theme`. A theme's config
//! can only be found once the theme has been chosen, so only the site gets a
//! say in it; a site that doesn't choose gets little's built-in theme.
//!
//! Two more settings name the folders a site is built from and into, `source`
//! and `destination`. Any level can set them, but they're always relative to
//! the site.

use anyhow::{Context, Result, bail};
use serde_yaml::Value;
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};

/// The config file a theme and a site may each provide.
pub const CONFIG_FILE: &str = "little.yml";

/// A site's merged config, and the folders it chose.
#[derive(Debug)]
pub struct Config {
    /// The merged cascade, which the theme sees as `site`.
    pub values: Value,
    pub theme_dir: PathBuf,
    /// The folder of Markdown files the site is built from.
    pub source_dir: PathBuf,
    /// The folder the site is built into, which every build wipes.
    pub destination_dir: PathBuf,
}

/// little's built-in defaults: level 1 of the cascade. They live in
/// `src/little.yml`, compiled into the binary so little stays a single file
/// with nothing to install alongside it.
fn builtin_defaults() -> Value {
    serde_yaml::from_str(include_str!("little.yml")).expect("src/little.yml is valid YAML")
}

/// Choose a site's theme, then load and merge the cascade.
pub fn load(site_dir: &Path) -> Result<Config> {
    let site_cfg = read_yaml(&site_dir.join(CONFIG_FILE))?;
    let theme_dir = choose_theme(site_dir, site_cfg.as_ref())?;
    let mut values = builtin_defaults();

    // Level 2: the theme's config.
    let theme_cfg_path = theme_dir.join(CONFIG_FILE);
    if let Some(theme_cfg) = read_yaml(&theme_cfg_path)? {
        // Honouring it would make the theme choose itself, and ignoring it
        // would break "last writer wins", so say so instead.
        if theme_cfg.get("theme").is_some() {
            bail!(
                "{} sets `theme`, but only a site's {CONFIG_FILE} can choose the theme",
                theme_cfg_path.display()
            );
        }
        deep_merge(&mut values, theme_cfg);
    }

    // Level 3: the site's config (the final word).
    if let Some(site_cfg) = site_cfg {
        deep_merge(&mut values, site_cfg);
    }

    let (source, source_dir) = folder_setting(site_dir, &values, "source")?;
    if !source_dir.is_dir() {
        bail!(
            "source `{source}` not found: no folder at {}",
            source_dir.display()
        );
    }
    let (_, destination_dir) = folder_setting(site_dir, &values, "destination")?;

    Ok(Config {
        values,
        theme_dir,
        source_dir,
        destination_dir,
    })
}

/// Resolve the site's `theme` to the theme's folder, or to little's built-in
/// theme if the site doesn't choose one.
fn choose_theme(site_dir: &Path, site_cfg: Option<&Value>) -> Result<PathBuf> {
    match site_cfg.and_then(|cfg| cfg.get("theme")) {
        // `theme:` with nothing after it is as good as leaving it out.
        None | Some(Value::Null) => {
            // little's built-in theme lives in the site's `theme/` folder
            // until it's built into little.
            let dir = site_dir.join("theme");
            if !dir.is_dir() {
                bail!(
                    "no theme: point `theme` in {CONFIG_FILE} at a theme folder, or add one at {}",
                    dir.display()
                );
            }
            Ok(dir)
        }
        Some(Value::String(theme)) => {
            let dir = site_path(site_dir, theme);
            if !dir.is_dir() {
                bail!("theme `{theme}` not found: no folder at {}", dir.display());
            }
            Ok(dir)
        }
        Some(_) => bail!("`theme` must be the path to a theme folder, like `./my-theme`"),
    }
}

/// The folder a setting names, and the setting as written. It's relative to
/// the site even when a theme sets it: it's the site that's built.
fn folder_setting<'a>(site_dir: &Path, values: &'a Value, key: &str) -> Result<(&'a str, PathBuf)> {
    match values.get(key) {
        Some(Value::String(path)) => Ok((path, site_path(site_dir, path))),
        _ => {
            let defaults = builtin_defaults();
            let example = defaults[key].as_str().unwrap_or_default();
            bail!("`{key}` must be the path to a folder, like `{example}`")
        }
    }
}

/// Resolve a path a config names against the site's folder: it's relative to
/// the site, not to wherever little runs, and an absolute path is just that
/// path. Tidied, so a site in `.` that sets `./docs` shows it as `./docs`.
fn site_path(site_dir: &Path, path: &str) -> PathBuf {
    site_dir.join(path).components().collect()
}

/// The absolute path `path` really is, even if it doesn't exist yet: what
/// exists is resolved as the system would (following `..` and symlinks, and
/// spelled as on disk), and the rest is taken as written.
pub fn real_path(path: &Path) -> io::Result<PathBuf> {
    if let Ok(real) = path.canonicalize() {
        return Ok(real);
    }
    let mut real = PathBuf::new();
    for component in std::path::absolute(path)?.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                real.pop();
            }
            Component::Normal(name) => {
                real.push(name);
                if let Ok(canonical) = real.canonicalize() {
                    real = canonical;
                }
            }
            root => real.push(root),
        }
    }
    Ok(real)
}

/// Read one level's config. A missing file isn't an error — that level just
/// has nothing to add — and neither is an empty or comment-only one.
fn read_yaml(path: &Path) -> Result<Option<Value>> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let value: Value =
        serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    // An empty or comment-only file parses as null, and merging null would
    // replace (wipe out) every level beneath it.
    if value.is_null() {
        return Ok(None);
    }
    // Anything else but settings would replace them all, too.
    if !value.is_mapping() {
        bail!(
            "{} must hold settings, like `name: My Docs`",
            path.display()
        );
    }
    Ok(Some(value))
}

/// Recursively merge `over` into `base`. Mappings merge key-by-key; every other
/// kind of value replaces what was there before.
fn deep_merge(base: &mut Value, over: Value) {
    match over {
        Value::Mapping(over_map) => {
            if let Value::Mapping(base_map) = base {
                for (key, over_val) in over_map {
                    if let Some(base_val) = base_map.get_mut(&key) {
                        deep_merge(base_val, over_val);
                    } else {
                        base_map.insert(key, over_val);
                    }
                }
            } else {
                *base = Value::Mapping(over_map);
            }
        }
        other => *base = other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(cfg: &'a Value, key: &str) -> Option<&'a str> {
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

    /// Run the real cascade against a scratch site. Each `Some` is written
    /// as that level's config file; each `None` leaves the file out entirely.
    fn cascade(label: &str, theme_cfg: Option<&str>, site_cfg: Option<&str>) -> Value {
        let site = scratch(label, &[("docs/README.md", "")]);
        let theme = site.join("theme");
        std::fs::create_dir_all(&theme).unwrap();
        for (dir, cfg) in [(&theme, theme_cfg), (&site, site_cfg)] {
            if let Some(yaml) = cfg {
                std::fs::write(dir.join(CONFIG_FILE), yaml).unwrap();
            }
        }

        let cfg = load(&site);
        let _ = std::fs::remove_dir_all(&site);
        cfg.expect("cascade loads").values
    }

    #[test]
    fn the_site_chooses_its_theme_folder() {
        let site = scratch(
            "choose",
            &[
                ("docs/README.md", ""),
                ("theme/little.yml", "font: mono\n"),
                ("path/to/my/theme/little.yml", "name: Mine\nfont: serif\n"),
            ],
        );

        // Without a `theme`, little uses its built-in theme: for now, the
        // site's `theme/` folder. `theme:` with no value counts as without.
        for site_cfg in [None, Some("name: My Docs\n"), Some("theme:\n")] {
            if let Some(yaml) = site_cfg {
                std::fs::write(site.join(CONFIG_FILE), yaml).unwrap();
            }
            let cfg = load(&site).unwrap();
            assert_eq!(cfg.theme_dir, site.join("theme"), "with {site_cfg:?}");
            assert_eq!(get(&cfg.values, "font"), Some("mono"));
        }

        let site_cfg = "theme: ./path/to/my/theme\nname: My Docs\n";
        std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
        let cfg = load(&site).unwrap();
        assert_eq!(cfg.theme_dir, site.join("path/to/my/theme"));
        assert_eq!(get(&cfg.values, "font"), Some("serif")); // the chosen theme's
        assert_eq!(get(&cfg.values, "name"), Some("My Docs")); // the site still wins
        assert_eq!(get(&cfg.values, "theme"), Some("./path/to/my/theme"));

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn theme_paths_can_be_relative_or_absolute() {
        let root = scratch(
            "paths",
            &[
                ("site/docs/README.md", ""),
                ("site/themes/mine/_templates/default.html", ""),
                ("shared/theme/_templates/default.html", ""),
            ],
        );
        let (site, shared) = (root.join("site"), root.join("shared/theme"));
        // Unquoted, as people usually write them.
        let chosen = |theme: &str| {
            std::fs::write(site.join(CONFIG_FILE), format!("theme: {theme}\n")).unwrap();
            load(&site).unwrap().theme_dir.canonicalize().unwrap()
        };
        let mine = site.join("themes/mine").canonicalize().unwrap();
        let theirs = shared.canonicalize().unwrap();

        // Relative to the site, not to wherever little runs.
        assert_eq!(chosen("themes/mine"), mine);
        assert_eq!(chosen("./themes/mine"), mine);
        assert_eq!(chosen("../shared/theme"), theirs);

        // Absolute: the temp dir is /… on macOS and Linux, C:\… on Windows.
        assert!(shared.is_absolute());
        assert_eq!(chosen(&shared.display().to_string()), theirs);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bad_theme_choices_explain_themselves() {
        let site = scratch("bad-theme", &[("theme/_templates/default.html", "")]);
        let error = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            format!("{:#}", load(&site).unwrap_err())
        };

        assert!(error("theme: ./missing\n").contains("theme `./missing` not found"));
        assert!(error("theme: [a, b]\n").contains("`theme` must be the path to a theme folder"));
        // `default` is a folder name like any other, not little's built-in theme.
        assert!(error("theme: default\n").contains("theme `default` not found"));

        // A theme can't choose the theme; only the site can.
        std::fs::write(site.join("theme").join(CONFIG_FILE), "theme: ./other\n").unwrap();
        let message = error("name: My Docs\n");
        assert!(message.contains("only a site's little.yml can choose the theme"));

        // Neither a `theme` nor the built-in theme: nothing to build with.
        std::fs::remove_dir_all(site.join("theme")).unwrap();
        let message = error("name: My Docs\n");
        assert!(message.contains("no theme: point `theme` in little.yml at a theme folder"));

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn folders_are_relative_to_the_site_wherever_theyre_set() {
        let root = scratch(
            "folders",
            &[
                ("site/docs/README.md", ""),
                ("site/content/README.md", ""),
                ("site/themes/mine/_templates/default.html", ""),
                ("shared/docs/README.md", ""),
            ],
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
        let site = scratch(
            "bad-folders",
            &[
                ("docs/README.md", ""),
                ("theme/_templates/default.html", ""),
            ],
        );
        let error = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            format!("{:#}", load(&site).unwrap_err())
        };

        assert!(error("source: ./missing\n").contains("source `./missing` not found"));
        let message = error("source: [a, b]\n");
        assert!(message.contains("`source` must be the path to a folder, like `./docs`"));
        // With no value, it's not a path at all: nothing to build from or into.
        let message = error("destination:\n");
        assert!(message.contains("`destination` must be the path to a folder, like `./dist`"));

        // Settings are keys and values: anything else would replace them all.
        let message = error("- source: ./docs\n");
        assert!(
            message.contains("little.yml must hold settings"),
            "{message}"
        );

        let _ = std::fs::remove_dir_all(&site);
    }

    #[test]
    fn theme_and_site_configs_are_optional() {
        let theme = Some("font: serif\n");
        let site = Some("name: My Docs\n");

        // No config files at all: little's defaults stand on their own.
        assert_eq!(cascade("neither", None, None), builtin_defaults());

        let cfg = cascade("theme-only", theme, None);
        assert_eq!(get(&cfg, "font"), Some("serif"));

        let cfg = cascade("site-only", None, site);
        assert_eq!(get(&cfg, "name"), Some("My Docs"));

        let cfg = cascade("both", theme, site);
        assert_eq!(get(&cfg, "name"), Some("My Docs"));
        assert_eq!(get(&cfg, "font"), Some("serif"));
    }

    #[test]
    fn empty_config_files_count_as_absent() {
        // Placeholder files must not wipe out the levels beneath them.
        let cfg = cascade("empty", Some(""), Some("# nothing to override yet\n"));
        assert_eq!(cfg, builtin_defaults());
    }

    #[test]
    fn builtin_defaults_set_no_theme_or_presentation() {
        let cfg = builtin_defaults();
        assert!(cfg.is_mapping(), "every other level merges into it");
        // No `theme`, so a site that doesn't choose one gets the built-in theme.
        for key in ["theme", "name", "lang"] {
            assert_eq!(cfg.get(key), None, "little shouldn't default `{key}`");
        }
    }

    #[test]
    fn later_level_overrides_earlier() {
        let mut cfg = builtin_defaults();
        // Theme sets name + font; site overrides the name.
        deep_merge(
            &mut cfg,
            serde_yaml::from_str("name: Documentation\nfont: serif\n").unwrap(),
        );
        deep_merge(&mut cfg, serde_yaml::from_str("name: My Docs\n").unwrap());

        assert_eq!(get(&cfg, "name"), Some("My Docs")); // last writer wins
        assert_eq!(get(&cfg, "font"), Some("serif")); // theme value survives
    }

    #[test]
    fn deep_merge_recurses_into_mappings() {
        let mut base = serde_yaml::from_str("a:\n  x: 1\n  y: 2\n").unwrap();
        deep_merge(
            &mut base,
            serde_yaml::from_str("a:\n  y: 20\n  z: 30\n").unwrap(),
        );
        let a = base.get("a").unwrap();
        assert_eq!(a.get("x").and_then(Value::as_u64), Some(1));
        assert_eq!(a.get("y").and_then(Value::as_u64), Some(20));
        assert_eq!(a.get("z").and_then(Value::as_u64), Some(30));
    }
}
