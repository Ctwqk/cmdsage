use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// A single history entry recording a successfully executed command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub query: String,
    pub command_name: String,
    pub filled_command: String,
    pub timestamp: u64,
    pub exit_code: i32,
}

/// History store: tracks command usage for scoring boost
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

impl History {
    /// Get the default history file path
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cmdsage")
            .join("history.json")
    }

    /// Load history from file, or return empty history if file doesn't exist
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }
        std::fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    /// Save history to file
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create history dir: {}", parent.display()))?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write history: {}", path.display()))?;
        Ok(())
    }

    /// Record a command execution
    pub fn record(
        &mut self,
        query: &str,
        command_name: &str,
        filled_command: &str,
        exit_code: i32,
    ) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.entries.push(HistoryEntry {
            query: query.to_string(),
            command_name: command_name.to_string(),
            filled_command: filled_command.to_string(),
            timestamp,
            exit_code,
        });

        // Keep only last 1000 entries
        if self.entries.len() > 1000 {
            self.entries.drain(..self.entries.len() - 1000);
        }
    }

    /// Compute a score boost map: command_name → boost factor
    /// Commands used more frequently and more recently get higher boosts
    pub fn score_boosts(&self) -> HashMap<String, f64> {
        let mut counts: HashMap<String, (usize, u64)> = HashMap::new(); // (count, latest_timestamp)

        for entry in &self.entries {
            if entry.exit_code == 0 {
                let e = counts.entry(entry.command_name.clone()).or_insert((0, 0));
                e.0 += 1;
                e.1 = e.1.max(entry.timestamp);
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        counts
            .into_iter()
            .map(|(name, (count, latest))| {
                // Frequency boost: log(count + 1) capped at 0.3
                let freq_boost = (count as f64 + 1.0).ln().min(3.0) / 10.0;
                // Recency boost: exponential decay, half-life = 1 day
                let age_hours = (now.saturating_sub(latest)) as f64 / 3600.0;
                let recency_boost = (-age_hours / 24.0).exp() * 0.1;
                (name, freq_boost + recency_boost)
            })
            .collect()
    }

    /// Show recent history entries
    pub fn recent(&self, count: usize) -> &[HistoryEntry] {
        let start = self.entries.len().saturating_sub(count);
        &self.entries[start..]
    }
}
