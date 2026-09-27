//! The 3-level cascading config: little -> theme -> site, each a `little.yml`.
//! Each level deep-merges over the previous, so the *last* writer wins.
//! little's own file is compiled in; the theme's and the site's are optional.
//!
//! Which theme fills level 2 is itself a setting, `theme`. A theme's config
//! can only be found once the theme has been chosen, so only little and the
//! site get a say in it.

use anyhow::{Context, Result, bail};
use serde_yaml::Value;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

/// The config file a theme and a site may each provide.
pub const CONFIG_FILE: &str = "little.yml";

/// A site's merged config, and the folder of the theme it chose.
#[derive(Debug)]
pub struct Config {
    /// The merged cascade, which the theme sees as `site`.
    pub values: Value,
    pub theme_dir: PathBuf,
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

    Ok(Config { values, theme_dir })
}

/// The folder of the theme a site chooses, without loading the whole cascade.
pub fn theme_dir(site_dir: &Path) -> Result<PathBuf> {
    let site_cfg = read_yaml(&site_dir.join(CONFIG_FILE))?;
    choose_theme(site_dir, site_cfg.as_ref())
}

/// Resolve the site's `theme` (or little's default) to the theme's folder.
fn choose_theme(site_dir: &Path, site_cfg: Option<&Value>) -> Result<PathBuf> {
    let builtin = builtin_defaults();
    let setting = site_cfg
        .and_then(|cfg| cfg.get("theme"))
        .or(builtin.get("theme"));
    let Some(theme) = setting.and_then(Value::as_str) else {
        bail!("`theme` must be a string, like `./my-theme`");
    };

    let dir = if theme == "default" {
        // little's default theme lives in the site's `theme/` folder until
        // it's built into little.
        site_dir.join("theme")
    } else if is_path(theme) {
        // Relative paths start from the site, like the config that sets them.
        // Joining an absolute path just gives that path.
        let path = Path::new(theme);
        site_dir.join(path.strip_prefix(".").unwrap_or(path))
    } else {
        bail!(
            "unknown theme `{theme}`: `theme` must be `default` or the path to a \
             theme folder, either absolute or starting with ./ or ../"
        );
    };

    if !dir.is_dir() {
        bail!("theme `{theme}` not found: no folder at {}", dir.display());
    }
    Ok(dir)
}

/// Is this `theme` value a path? Relative paths start with `./` or `../`,
/// which keeps them apart from theme names like `default`. Absolute paths are
/// the OS's own, so `C:\themes\mine` is one on Windows but not elsewhere.
fn is_path(theme: &str) -> bool {
    matches!(
        Path::new(theme).components().next(),
        Some(Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_))
    )
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
        let site = scratch(label, &[]);
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
                ("theme/little.yml", "font: mono\n"),
                ("path/to/my/theme/little.yml", "name: Mine\nfont: serif\n"),
            ],
        );

        // Left unset, `theme` is little's `default`: the site's `theme/` folder.
        let cfg = load(&site).unwrap();
        assert_eq!(cfg.theme_dir, site.join("theme"));
        assert_eq!(get(&cfg.values, "font"), Some("mono"));

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
                ("shared/theme/templates/page.html", ""),
            ],
        );
        let (site, shared) = (root.join("site"), root.join("shared/theme"));
        // Unquoted, as people usually write them.
        let chosen = |theme: &str| {
            std::fs::write(site.join(CONFIG_FILE), format!("theme: {theme}\n")).unwrap();
            theme_dir(&site).unwrap().canonicalize().unwrap()
        };
        let expected = shared.canonicalize().unwrap();

        // Relative to the site, not to wherever little runs.
        assert_eq!(chosen("../shared/theme"), expected);

        // Absolute: the temp dir is /… on macOS and Linux, C:\… on Windows.
        let absolute = shared.display().to_string();
        assert!(shared.is_absolute(), "`{absolute}` should be absolute");
        assert_eq!(chosen(&absolute), expected);

        // Windows takes either separator.
        if cfg!(windows) {
            assert_eq!(chosen(r"..\shared\theme"), expected);
            assert_eq!(chosen(&absolute.replace('/', r"\")), expected);
            assert_eq!(chosen(&absolute.replace('\\', "/")), expected);
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn theme_values_are_paths_or_names() {
        let mut paths = vec!["./theme", "../themes/fancy", "/srv/themes/fancy", ".", ".."];
        if cfg!(windows) {
            paths.extend([
                r"C:\themes\fancy",
                "C:/themes/fancy",
                r"\\server\share\fancy",
                r".\theme",
                r"..\themes\fancy",
            ]);
        }
        for path in paths {
            assert!(is_path(path), "`{path}` is a path");
        }
        for name in [
            "default",
            "fancy",
            "themes/fancy",
            ".fancy",
            "https://x.io/t.zip",
            "",
        ] {
            assert!(!is_path(name), "`{name}` isn't a path");
        }
    }

    #[test]
    fn bad_theme_choices_explain_themselves() {
        let site = scratch("bad-theme", &[("theme/templates/page.html", "")]);
        let error = |site_cfg: &str| {
            std::fs::write(site.join(CONFIG_FILE), site_cfg).unwrap();
            format!("{:#}", load(&site).unwrap_err())
        };

        assert!(error("theme: fancy\n").contains("unknown theme `fancy`"));
        assert!(error("theme: ./missing\n").contains("theme `./missing` not found"));
        assert!(error("theme: [a, b]\n").contains("`theme` must be a string"));

        // A theme can't choose the theme; only the site can.
        std::fs::write(site.join("theme").join(CONFIG_FILE), "theme: ./other\n").unwrap();
        let message = error("name: My Docs\n");
        assert!(message.contains("only a site's little.yml can choose the theme"));

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
        assert_eq!(get(&cfg, "theme"), Some("default"));

        let cfg = cascade("site-only", None, site);
        assert_eq!(get(&cfg, "name"), Some("My Docs"));
        assert_eq!(get(&cfg, "theme"), Some("default"));

        let cfg = cascade("both", theme, site);
        assert_eq!(get(&cfg, "name"), Some("My Docs"));
        assert_eq!(get(&cfg, "font"), Some("serif"));
        assert_eq!(get(&cfg, "theme"), Some("default"));
    }

    #[test]
    fn empty_config_files_count_as_absent() {
        // Placeholder files must not wipe out the levels beneath them.
        let cfg = cascade("empty", Some(""), Some("# nothing to override yet\n"));
        assert_eq!(cfg, builtin_defaults());
    }

    #[test]
    fn builtin_defaults_leave_presentation_to_theme_and_site() {
        let cfg = builtin_defaults();
        assert_eq!(get(&cfg, "theme"), Some("default"));
        for key in ["name", "lang"] {
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
        assert_eq!(get(&cfg, "theme"), Some("default")); // builtin survives
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
