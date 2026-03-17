use super::CommandEntry;

/// Detect the current platform name (compile-time)
pub fn current_platform() -> &'static str {
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

/// Filter commands to only those available on the given platform
pub fn filter_for_platform(commands: Vec<CommandEntry>, platform: &str) -> Vec<CommandEntry> {
    commands
        .into_iter()
        .filter(|cmd| cmd.platforms.iter().any(|p| p == platform))
        .collect()
}

/// Filter commands to only those available on the current (compile-time) platform
pub fn filter_for_current_platform(commands: Vec<CommandEntry>) -> Vec<CommandEntry> {
    filter_for_platform(commands, current_platform())
}
