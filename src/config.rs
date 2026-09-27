//! The 3-level cascading config: little -> theme -> site, each a `little.yml`.
//! Each level deep-merges over the previous, so the *last* writer wins.
//! little's own file is compiled in; the theme's and the site's are optional.

use anyhow::{Context, Result};
use serde_yaml::Value;
use std::io::ErrorKind;
use std::path::Path;

/// The config file a theme and a site may each provide.
pub const CONFIG_FILE: &str = "little.yml";

/// little's built-in defaults: level 1 of the cascade. They live in
/// `src/little.yml`, compiled into the binary so little stays a single file
/// with nothing to install alongside it.
fn builtin_defaults() -> Value {
    serde_yaml::from_str(include_str!("little.yml")).expect("src/little.yml is valid YAML")
}

/// Load and merge the cascade for a site.
pub fn load_cascade(theme_dir: &Path, site_dir: &Path) -> Result<Value> {
    let mut cfg = builtin_defaults();

    // Level 2: the theme's config.
    if let Some(theme_cfg) = read_yaml(&theme_dir.join(CONFIG_FILE))? {
        deep_merge(&mut cfg, theme_cfg);
    }

    // Level 3: the site's config (the final word).
    if let Some(site_cfg) = read_yaml(&site_dir.join(CONFIG_FILE))? {
        deep_merge(&mut cfg, site_cfg);
    }

    Ok(cfg)
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

    /// Run the real cascade against a scratch site. Each `Some` is written
    /// as that level's config file; each `None` leaves the file out entirely.
    fn cascade(label: &str, theme_cfg: Option<&str>, site_cfg: Option<&str>) -> Value {
        let site =
            std::env::temp_dir().join(format!("little-config-test-{}-{label}", std::process::id()));
        let theme = site.join("theme");
        let _ = std::fs::remove_dir_all(&site);
        std::fs::create_dir_all(&theme).unwrap();
        for (dir, cfg) in [(&theme, theme_cfg), (&site, site_cfg)] {
            if let Some(yaml) = cfg {
                std::fs::write(dir.join(CONFIG_FILE), yaml).unwrap();
            }
        }

        let cfg = load_cascade(&theme, &site);
        let _ = std::fs::remove_dir_all(&site);
        cfg.expect("cascade loads")
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
