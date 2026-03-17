use colored::Colorize;
use dialoguer::{Input, Select};

use crate::engine::template;
use crate::knowledge::Risk;
use crate::matcher::MatchResult;

/// Display matched commands and let the user choose one
pub fn preview_and_select(
    query: &str,
    matches: &[MatchResult],
) -> Option<(String, Risk, String)> {
    if matches.is_empty() {
        println!("{}", "No matching commands found.".red());
        return None;
    }

    println!();
    println!("  {} {}", "Query:".dimmed(), query.white().bold());
    println!();

    let mut items: Vec<String> = Vec::new();
    for (i, m) in matches.iter().enumerate() {
        let cmd_str = m
            .filled_template
            .as_deref()
            .unwrap_or(&m.command.template);

        let risk_badge = match m.command.risk {
            Risk::Safe => "".to_string(),
            Risk::Moderate => " [⚠ moderate]".yellow().to_string(),
            Risk::Dangerous => " [⚠ DANGEROUS]".red().bold().to_string(),
        };

        let display = format!(
            "[{}] {}{}\n      {}  (score: {:.2})",
            i + 1,
            cmd_str,
            risk_badge,
            m.command.description.dimmed(),
            m.score,
        );
        items.push(display);
    }
    items.push("Cancel".dimmed().to_string());

    let selection = Select::new()
        .with_prompt("Select command to execute")
        .items(&items)
        .default(0)
        .interact_opt()
        .ok()
        .flatten();

    match selection {
        Some(idx) if idx < matches.len() => {
            let m = &matches[idx];
            let mut cmd_str = m
                .filled_template
                .clone()
                .unwrap_or_else(|| m.command.template.clone());

            // Check for unfilled parameters
            let unfilled = template::has_unfilled_params(&cmd_str);
            for param_name in &unfilled {
                let value: String = Input::new()
                    .with_prompt(format!("Enter value for '{}'", param_name))
                    .interact_text()
                    .unwrap_or_default();
                cmd_str = cmd_str.replace(&format!("{{{}}}", param_name), &value);
            }

            // Dangerous command: double confirm
            if m.command.risk == Risk::Dangerous {
                println!();
                println!(
                    "  {} {}",
                    "⚠ WARNING:".red().bold(),
                    "This is a dangerous command!".red()
                );
                println!("  Command: {}", cmd_str.yellow());
                let confirm: String = Input::new()
                    .with_prompt("Type 'yes' to confirm execution")
                    .interact_text()
                    .unwrap_or_default();
                if confirm.to_lowercase() != "yes" {
                    println!("{}", "Cancelled.".dimmed());
                    return None;
                }
            }

            // Allow editing
            println!();
            println!("  {} {}", "Command:".green().bold(), cmd_str.white().bold());
            let edit_prompt: String = Input::new()
                .with_prompt("Press Enter to execute, or type modified command")
                .default(cmd_str.clone())
                .interact_text()
                .unwrap_or(cmd_str.clone());

            Some((edit_prompt, m.command.risk.clone(), m.command.name.clone()))
        }
        _ => {
            println!("{}", "Cancelled.".dimmed());
            None
        }
    }
}

/// Show execution result
pub fn show_result(exit_code: i32) {
    println!();
    if exit_code == 0 {
        println!("  {} (exit code: {})", "Done".green().bold(), exit_code);
    } else {
        println!(
            "  {} (exit code: {})",
            "Command failed".red().bold(),
            exit_code
        );
    }
}
