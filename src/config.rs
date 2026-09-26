//! The 3-level cascading config: little defaults -> theme -> site.
//! Each level deep-merges over the previous, so the *last* writer wins.

use anyhow::{Context, Result};
use serde_yaml::Value;
use std::path::Path;

/// little's built-in defaults. This is level 1 of the cascade.
fn builtin_defaults() -> Value {
    serde_yaml::from_str(
        r##"
title: "A little site"
lang: en
theme: default
accent: "#2563eb"
"##,
    )
    .expect("built-in defaults are valid YAML")
}

/// Load and merge the cascade for a project.
pub fn load_cascade(theme_dir: &Path, project_dir: &Path) -> Result<Value> {
    let mut cfg = builtin_defaults();

    // Level 2: the theme's config.
    if let Some(theme_cfg) = read_yaml(&theme_dir.join("little.yml"))? {
        deep_merge(&mut cfg, theme_cfg);
    }

    // Level 3: the site's config (the final word).
    if let Some(site_cfg) = read_yaml(&project_dir.join("little.yml"))? {
        deep_merge(&mut cfg, site_cfg);
    }

    Ok(cfg)
}

fn read_yaml(path: &Path) -> Result<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let value = serde_yaml::from_str(&raw)
        .with_context(|| format!("parsing {}", path.display()))?;
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
