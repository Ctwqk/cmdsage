use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Execute a shell command and stream output to stdout/stderr
pub fn execute_command(command_str: &str) -> Result<i32> {
    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };
    let flag = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let mut child = Command::new(shell)
        .arg(flag)
        .arg(command_str)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to execute: {}", command_str))?;

    // Stream stdout
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                println!("{}", line);
            }
        }
    }

    // Stream stderr
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("{}", line);
            }
        }
    }

    let status = child.wait().context("Failed to wait for command")?;
    Ok(status.code().unwrap_or(-1))
}
