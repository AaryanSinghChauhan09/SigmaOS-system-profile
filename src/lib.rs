//! SigmaOS system profile — declarative whole-system configuration.
//!
//! A `system.toml` profile composes per-component configs into one
//! declarative system definition, NixOS-style. The profile names which
//! components are enabled and where each component's config lives;
//! `resolve` merges them into a single effective view for validation
//! and generation.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Errors produced by this component.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("profile invalid: {0}")]
    Invalid(String),
    #[error("component not enabled: {0}")]
    NotEnabled(String),
}

/// A single component entry inside a system profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentEntry {
    /// Whether the component is part of this system.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Path to the component's own config file (TOML/JSON).
    #[serde(default)]
    pub config: Option<PathBuf>,
    /// Free-form component options merged over its defaults.
    #[serde(default)]
    pub options: BTreeMap<String, toml::Value>,
}

fn default_true() -> bool {
    true
}

/// The declarative system profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemProfile {
    /// Profile name (e.g. "sigmaos-desktop").
    pub name: String,
    /// Profile version.
    #[serde(default = "default_version")]
    pub version: String,
    /// Global options applied to every component.
    #[serde(default)]
    pub globals: BTreeMap<String, toml::Value>,
    /// Component name -> entry.
    #[serde(default)]
    pub components: BTreeMap<String, ComponentEntry>,
}

fn default_version() -> String {
    "0.1.0".into()
}

/// One resolved component view: entry + its parsed config file.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedComponent {
    pub name: String,
    pub enabled: bool,
    pub config_path: Option<PathBuf>,
    pub config: Option<toml::Value>,
    pub options: BTreeMap<String, toml::Value>,
}

/// The fully resolved system view.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedSystem {
    pub name: String,
    pub version: String,
    pub globals: BTreeMap<String, toml::Value>,
    pub components: Vec<ResolvedComponent>,
}

impl SystemProfile {
    /// Load a profile from a TOML file.
    pub fn load(path: &Path) -> Result<Self, ProfileError> {
        let text = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }

    /// Save the profile to a TOML file atomically.
    pub fn save(&self, path: &Path) -> Result<(), ProfileError> {
        let text = toml::to_string_pretty(self)
            .map_err(|e| ProfileError::Invalid(e.to_string()))?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Resolve the profile into an effective system view, reading each
    /// enabled component's config file when present.
    pub fn resolve(&self) -> Result<ResolvedSystem, ProfileError> {
        let mut components = Vec::new();
        for (name, entry) in &self.components {
            let config = match &entry.config {
                Some(p) if entry.enabled && p.exists() => {
                    let text = std::fs::read_to_string(p)?;
                    Some(toml::from_str::<toml::Value>(&text)?)
                }
                _ => None,
            };
            components.push(ResolvedComponent {
                name: name.clone(),
                enabled: entry.enabled,
                config_path: entry.config.clone(),
                config,
                options: entry.options.clone(),
            });
        }
        Ok(ResolvedSystem {
            name: self.name.clone(),
            version: self.version.clone(),
            globals: self.globals.clone(),
            components,
        })
    }

    /// Validate the profile: name required, component names unique by
    /// map semantics, enabled components with missing config paths are
    /// warnings not errors (config may be optional).
    pub fn validate(&self) -> Result<Vec<String>, ProfileError> {
        let mut warnings = Vec::new();
        if self.name.trim().is_empty() {
            return Err(ProfileError::Invalid("profile name is empty".into()));
        }
        for (name, entry) in &self.components {
            if entry.enabled && entry.config.is_none() {
                warnings.push(format!("{name}: enabled but no config path"));
            }
            if !entry.enabled && entry.options.is_empty() && entry.config.is_some() {
                warnings.push(format!("{name}: disabled but has config path"));
            }
        }
        Ok(warnings)
    }

    /// List enabled component names in stable (sorted) order.
    pub fn enabled_components(&self) -> Vec<String> {
        self.components
            .iter()
            .filter(|(_, e)| e.enabled)
            .map(|(n, _)| n.clone())
            .collect()
    }
}

/// Default profile search path under XDG config.
pub fn default_profile_path() -> PathBuf {
    directories::ProjectDirs::from("org", "sigmaos", "sigmaos")
        .map(|d| d.config_dir().join("system.toml"))
        .unwrap_or_else(|| PathBuf::from("~/.config/sigmaos/system.toml"))
}

/// Write the resolved system view as JSON.
pub fn to_json(system: &ResolvedSystem) -> Result<String, ProfileError> {
    Ok(serde_json::to_string_pretty(system)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_profile() {
        let text = r#"
name = "test"
[components.shell]
enabled = true
config = "/tmp/shell.toml"
"#;
        let p: SystemProfile = toml::from_str(text).unwrap();
        assert_eq!(p.name, "test");
        assert!(p.components["shell"].enabled);
        assert_eq!(p.enabled_components(), vec!["shell"]);
    }

    #[test]
    fn validate_warns_on_missing_config() {
        let p = SystemProfile {
            name: "t".into(),
            version: "0.1.0".into(),
            globals: BTreeMap::new(),
            components: BTreeMap::from([(
                "x".to_string(),
                ComponentEntry {
                    enabled: true,
                    config: None,
                    options: BTreeMap::new(),
                },
            )]),
        };
        let w = p.validate().unwrap();
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn resolve_reads_component_config() {
        let dir = std::env::temp_dir().join("sigmaos-profile-test");
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("c.toml");
        std::fs::write(&cfg, "key = \"value\"\n").unwrap();
        let p = SystemProfile {
            name: "t".into(),
            version: "0.1.0".into(),
            globals: BTreeMap::new(),
            components: BTreeMap::from([(
                "comp".to_string(),
                ComponentEntry {
                    enabled: true,
                    config: Some(cfg),
                    options: BTreeMap::new(),
                },
            )]),
        };
        let r = p.resolve().unwrap();
        assert!(r.components[0].config.is_some());
    }
}
