use std::path::Path;
use anyhow::{Context, Result};
use walkdir::WalkDir;

use super::{CommandEntry, CommandFile};

/// Load all command entries from TOML files in the given directory (recursive)
pub fn load_commands(commands_dir: &Path) -> Result<Vec<CommandEntry>> {
    let mut all_commands = Vec::new();

    for entry in WalkDir::new(commands_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().map_or(false, |ext| ext == "toml")
                && !e.file_name().to_string_lossy().starts_with('_')
        })
    {
        let content = std::fs::read_to_string(entry.path())
            .with_context(|| format!("Failed to read {}", entry.path().display()))?;

        let file: CommandFile = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", entry.path().display()))?;

        all_commands.extend(file.command);
    }

    Ok(all_commands)
}
