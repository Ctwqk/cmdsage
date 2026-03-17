use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Persistent configuration stored in ~/.cmdsage/config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Target platform: "linux", "macos", "windows", or "auto" (detect at runtime)
    #[serde(default = "default_platform")]
    pub platform: String,

    /// Default number of results to show
    #[serde(default = "default_top_k")]
    pub top_k: usize,

    /// Whether to disable semantic matching by default
    #[serde(default)]
    pub no_semantic: bool,
}

fn default_platform() -> String {
    "auto".to_string()
}

fn default_top_k() -> usize {
    3
}

impl Default for Config {
    fn default() -> Self {
        Self {
            platform: default_platform(),
            top_k: default_top_k(),
            no_semantic: false,
        }
    }
}

impl Config {
    /// Default config file path
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cmdsage")
            .join("config.toml")
    }

    /// Load config from file, or return defaults if file doesn't exist
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }
        std::fs::read_to_string(path)
            .ok()
            .and_then(|content| toml::from_str(&content).ok())
            .unwrap_or_default()
    }

    /// Save config to file
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir: {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config: {}", path.display()))?;
        Ok(())
    }

    /// Resolve the effective platform name.
    /// Priority: CLI flag > config file > auto-detect
    pub fn resolve_platform(&self, cli_override: Option<&str>) -> String {
        if let Some(p) = cli_override {
            if p != "auto" {
                return p.to_string();
            }
        }
        if self.platform != "auto" {
            return self.platform.clone();
        }
        // Auto-detect
        detect_platform().to_string()
    }
}

/// Compile-time platform detection
pub fn detect_platform() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    }
}

/// All supported platform names
pub const PLATFORMS: &[&str] = &["linux", "macos", "windows"];

/// Validate a platform string
pub fn is_valid_platform(platform: &str) -> bool {
    platform == "auto" || PLATFORMS.contains(&platform)
}
