use std::collections::HashMap;

use crate::knowledge::CommandEntry;

/// Extract arguments from user input and fill the command template
pub fn fill_template(cmd: &CommandEntry, user_input: &str) -> String {
    // First: try exact/close example matching (highest confidence)
    for example in &cmd.examples {
        let input_lower = user_input.to_lowercase();
        let example_lower = example.input.to_lowercase();
        // Fuzzy match: input contains example or example contains input
        if input_lower.contains(&example_lower) || example_lower.contains(&input_lower) {
            return example.filled.clone();
        }
    }

    let mut params: HashMap<String, String> = HashMap::new();
    let tokens: Vec<&str> = user_input.split_whitespace().collect();

    for token in &tokens {
        let t = *token;

        // Skip pure Chinese/natural-language tokens — they are not parameter values
        if is_natural_language_token(t) {
            continue;
        }

        // Path detection: /xxx, ./xxx, ~/xxx, ..
        if t.starts_with('/') || t.starts_with("./") || t.starts_with("~/") || t.starts_with("..") {
            assign_to_arg(&mut params, &cmd.args, t, &["path", "dir", "file", "source", "dest", "mountpoint"]);
            continue;
        }

        // Glob pattern: *.py, *.log, etc.
        if t.starts_with("*.") || t.contains("*") {
            assign_to_arg(&mut params, &cmd.args, t, &["pattern", "glob", "name", "query"]);
            continue;
        }

        // URL detection
        if t.starts_with("http://") || t.starts_with("https://") || t.starts_with("git@") {
            assign_to_arg(&mut params, &cmd.args, t, &["url", "host", "target"]);
            continue;
        }

        // user@host detection (SSH-style)
        if t.contains('@') && !t.starts_with('@') && !t.ends_with('@') {
            if let Some((user, host)) = t.split_once('@') {
                let _ = try_assign(&mut params, &cmd.args, "user", user);
                let _ = try_assign(&mut params, &cmd.args, "host", host);
                continue;
            }
        }

        // Hostname/domain detection: google.com, 192.168.1.1
        // Must come before file extension detection
        if t.contains('.') && !t.starts_with('.') && !t.ends_with('.')
            && t.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
            && t.split('.').count() >= 2
        {
            // Distinguish hostname from filename: known file extensions → file, else → host
            let ext = t.rsplit('.').next().unwrap_or("");
            let is_file_ext = matches!(
                ext,
                "txt" | "log" | "json" | "yaml" | "yml" | "toml" | "xml" | "csv"
                | "py" | "rs" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h"
                | "sh" | "bash" | "zsh" | "fish"
                | "md" | "html" | "css" | "sql"
                | "tar" | "gz" | "zip" | "7z" | "xz" | "bz2"
                | "png" | "jpg" | "jpeg" | "gif" | "svg" | "pdf"
                | "conf" | "cfg" | "ini" | "env"
            );
            if is_file_ext {
                assign_to_arg(&mut params, &cmd.args, t, &["file", "path", "source", "package", "image"]);
            } else {
                assign_to_arg(&mut params, &cmd.args, t, &["host", "url", "target"]);
            }
            continue;
        }

        // File extension detection: file.txt, script.py (contains dot, no slash)
        if t.contains('.') && !t.contains('/') && t.len() > 2 {
            assign_to_arg(&mut params, &cmd.args, t, &["file", "path", "source", "package", "image"]);
            continue;
        }

        // Size with unit: 100M, 1G, 500K
        if t.len() >= 2 && t.ends_with(|c: char| "MmGgKk".contains(c)) {
            let num_part = &t[..t.len() - 1];
            if num_part.parse::<u64>().is_ok() {
                assign_to_arg(&mut params, &cmd.args, t, &["size"]);
                continue;
            }
        }

        // Pure number: port, pid, count, days
        if t.parse::<u64>().is_ok() {
            let num: u64 = t.parse().unwrap();
            if num <= 65535 && num > 0 {
                assign_to_arg(&mut params, &cmd.args, t, &["port", "pid", "count", "number", "days", "size"]);
            }
            continue;
        }

        // Known flags: -r, -f, --recursive, etc
        if t.starts_with('-') {
            if !params.contains_key("flags") {
                let existing = params.entry("flags".to_string()).or_default();
                if !existing.is_empty() {
                    existing.push(' ');
                }
                existing.push_str(t);
            }
            continue;
        }

        // Generic token: try to match against any unfilled required arg
        for arg in &cmd.args {
            if arg.required && !params.contains_key(&arg.name) {
                // Don't assign generic tokens to path/file args (those need path-like tokens)
                if !arg.name.contains("path") && !arg.name.contains("file") && !arg.name.contains("dir") {
                    params.insert(arg.name.clone(), t.to_string());
                    break;
                }
            }
        }
    }

    // Fill template with extracted + default values
    let mut result = cmd.template.clone();
    for arg in &cmd.args {
        let placeholder = format!("{{{}}}", arg.name);
        if let Some(value) = params.get(&arg.name) {
            result = result.replace(&placeholder, value);
        } else if let Some(ref default) = arg.default {
            result = result.replace(&placeholder, default);
        }
        // If required and no value: leave placeholder for interactive prompt
    }

    // Clean up double spaces from empty defaults
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }
    result.trim().to_string()
}

/// Try to assign a value to the first matching arg by name pattern
fn assign_to_arg(
    params: &mut HashMap<String, String>,
    args: &[crate::knowledge::CommandArg],
    value: &str,
    name_patterns: &[&str],
) {
    for pattern in name_patterns {
        for arg in args {
            if !params.contains_key(&arg.name) && arg.name.contains(pattern) {
                params.insert(arg.name.clone(), value.to_string());
                return;
            }
        }
    }
}

/// Try to assign a specific value to a specific arg name
fn try_assign(
    params: &mut HashMap<String, String>,
    args: &[crate::knowledge::CommandArg],
    name: &str,
    value: &str,
) -> bool {
    for arg in args {
        if arg.name == name && !params.contains_key(&arg.name) {
            params.insert(arg.name.clone(), value.to_string());
            return true;
        }
    }
    false
}

/// Check if a token is a natural language word (Chinese or common English words)
/// rather than a parameter value like a path, number, or filename
fn is_natural_language_token(token: &str) -> bool {
    // Contains any CJK character → natural language
    if token.chars().any(|c| {
        ('\u{4e00}'..='\u{9fff}').contains(&c)
            || ('\u{3400}'..='\u{4dbf}').contains(&c)
            || ('\u{f900}'..='\u{faff}').contains(&c)
    }) {
        return true;
    }

    // Common English natural language words (not parameter values)
    const NL_WORDS: &[&str] = &[
        "find", "search", "show", "list", "view", "get", "check", "look",
        "kill", "stop", "start", "restart", "run", "build", "test", "install",
        "create", "delete", "remove", "copy", "move", "rename", "compress", "extract",
        "ping", "ssh", "docker", "git", "npm", "pip", "cargo", "make", "sudo",
        "all", "the", "in", "on", "at", "to", "for", "from", "with", "by",
        "of", "and", "or", "not", "is", "are", "was", "do", "does",
        "files", "directories", "folder", "folders", "file", "directory",
        "recent", "large", "big", "small", "new", "old", "modified",
        "running", "active", "current", "local", "remote",
        "than", "more", "less", "over", "under", "above", "below",
    ];

    NL_WORDS.contains(&token.to_lowercase().as_str())
}

/// Check if a filled template still has unfilled placeholders
pub fn has_unfilled_params(filled: &str) -> Vec<String> {
    let mut unfilled = Vec::new();
    let mut chars = filled.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                name.push(c);
            }
            if !name.is_empty() {
                unfilled.push(name);
            }
        }
    }
    unfilled
}
