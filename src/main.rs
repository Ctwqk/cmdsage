use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

use cmdsage::config::{self, Config};
use cmdsage::engine::{executor, template};
use cmdsage::history::History;
use cmdsage::knowledge::{indexer::KeywordIndex, loader, platform, CommandEntry};
use cmdsage::matcher::keyword::Tokenizer;
use cmdsage::matcher::semantic::SemanticMatcher;
use cmdsage::matcher::MatchResult;
use cmdsage::model::onnx;
use cmdsage::ui::preview;

#[derive(Parser)]
#[command(name = "cmdsage", about = "Local command-line intelligent executor")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Natural language description of what you want to do
    #[arg(trailing_var_arg = true)]
    query: Vec<String>,

    /// Path to command knowledge base directory
    #[arg(long, default_value = "commands", global = true)]
    commands_dir: PathBuf,

    /// Path to ONNX model directory
    #[arg(long, global = true)]
    model_dir: Option<PathBuf>,

    /// Number of results to show
    #[arg(short = 'n', long, global = true)]
    top_k: Option<usize>,

    /// Skip semantic matching (keyword-only mode)
    #[arg(long, global = true)]
    no_semantic: bool,

    /// Dry run: show matches without executing
    #[arg(long, global = true)]
    dry_run: bool,

    /// Override target platform: linux, macos, windows, auto
    #[arg(long, global = true)]
    platform: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a custom command to the knowledge base
    Add {
        /// Command name (unique identifier)
        #[arg(long)]
        name: String,
        /// Binary/executable name
        #[arg(long)]
        binary: String,
        /// Command template with {placeholders}
        #[arg(long)]
        template: String,
        /// Description (Chinese or English)
        #[arg(long)]
        description: String,
        /// Comma-separated keywords
        #[arg(long)]
        keywords: String,
        /// Risk level: safe, moderate, dangerous
        #[arg(long, default_value = "safe")]
        risk: String,
    },
    /// Show command execution history
    History {
        /// Number of recent entries to show
        #[arg(long, default_value = "20")]
        count: usize,
    },
    /// Show statistics about the command knowledge base
    Stats,
    /// View or modify configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set target platform (linux, macos, windows, auto)
    SetPlatform {
        /// Platform name
        platform: String,
    },
    /// Set default number of results
    SetTopK {
        /// Number of results
        count: usize,
    },
    /// Toggle semantic matching on/off
    SetSemantic {
        /// "on" or "off"
        value: String,
    },
    /// Reset configuration to defaults
    Reset,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load(&Config::default_path());

    // Resolve effective platform
    let effective_platform =
        cfg.resolve_platform(cli.platform.as_deref());

    // Validate platform if explicitly provided
    if let Some(ref p) = cli.platform {
        if !config::is_valid_platform(p) {
            anyhow::bail!(
                "Invalid platform '{}'. Valid options: auto, linux, macos, windows",
                p
            );
        }
    }

    let top_k = cli.top_k.unwrap_or(cfg.top_k);
    let no_semantic = cli.no_semantic || cfg.no_semantic;

    match &cli.command {
        Some(Commands::Add {
            name,
            binary,
            template,
            description,
            keywords,
            risk,
        }) => {
            return cmd_add(name, binary, template, description, keywords, risk, &cli.commands_dir);
        }
        Some(Commands::History { count }) => {
            return cmd_history(*count);
        }
        Some(Commands::Stats) => {
            return cmd_stats(&cli.commands_dir, &effective_platform);
        }
        Some(Commands::Config { action }) => {
            return cmd_config(action.as_ref(), &effective_platform);
        }
        None => {}
    }

    let query = cli.query.join(" ");
    if query.is_empty() {
        println!("{}", "CmdSage - Local Command Intelligence".green().bold());
        let plat_display = if effective_platform == config::detect_platform() {
            format!("{} (native)", effective_platform)
        } else {
            format!("{} (override)", effective_platform).yellow().to_string()
        };
        println!("Platform: {}", plat_display.cyan());
        println!(
            "Type your command description (or 'quit' to exit, 'history' to view history):"
        );
        loop {
            let mut input = String::new();
            print!("{} ", "›".cyan().bold());
            use std::io::Write;
            std::io::stdout().flush()?;
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim();
            if input.is_empty() {
                continue;
            }
            if input == "quit" || input == "exit" || input == "q" {
                break;
            }
            if input == "history" {
                let _ = cmd_history(10);
                continue;
            }
            if let Err(e) = run_query(
                input,
                &cli.commands_dir,
                cli.model_dir.as_deref(),
                top_k,
                no_semantic,
                cli.dry_run,
                &effective_platform,
            ) {
                eprintln!("{}: {}", "Error".red(), e);
            }
        }
        return Ok(());
    }

    run_query(
        &query,
        &cli.commands_dir,
        cli.model_dir.as_deref(),
        top_k,
        no_semantic,
        cli.dry_run,
        &effective_platform,
    )
}

fn run_query(
    query: &str,
    commands_dir: &PathBuf,
    model_dir: Option<&std::path::Path>,
    top_k: usize,
    no_semantic: bool,
    dry_run: bool,
    target_platform: &str,
) -> Result<()> {
    // 1. Load commands filtered by target platform
    let commands_path = resolve_commands_path(commands_dir);
    let all_commands = loader::load_commands(&commands_path)?;
    let commands = platform::filter_for_platform(all_commands, target_platform);

    if commands.is_empty() {
        anyhow::bail!(
            "No commands found for platform '{}' in {}.",
            target_platform,
            commands_path.display()
        );
    }

    let history_path = History::default_path();
    let history = History::load(&history_path);
    let boosts = history.score_boosts();

    // 2. Tokenize query
    let tokenizer = Tokenizer::new();
    let query_tokens = tokenizer.tokenize(query);

    // 3. Keyword search (BM25)
    let keyword_index = KeywordIndex::build(&commands, &|text| tokenizer.tokenize(text));
    let keyword_results = keyword_index.search(&query_tokens, 20);

    // 4. Semantic re-ranking
    let final_results = if !no_semantic {
        let model_path = model_dir
            .map(PathBuf::from)
            .unwrap_or_else(onnx::default_model_dir);

        if onnx::model_exists(&model_path) {
            match SemanticMatcher::load(&model_path) {
                Ok(mut matcher) => {
                    let candidate_indices: Vec<usize> =
                        keyword_results.iter().map(|&(idx, _)| idx).collect();
                    let descriptions: Vec<String> =
                        commands.iter().map(|c| c.description.clone()).collect();

                    if matcher.precompute_embeddings(&descriptions).is_ok() {
                        match matcher.rank_candidates(query, &candidate_indices, top_k * 2) {
                            Ok(semantic_ranked) => blend_scores(
                                &keyword_results,
                                &semantic_ranked,
                                &candidate_indices,
                                &commands,
                                &boosts,
                                top_k,
                            ),
                            Err(_) => apply_boosts(keyword_results, &commands, &boosts, top_k),
                        }
                    } else {
                        apply_boosts(keyword_results, &commands, &boosts, top_k)
                    }
                }
                Err(_) => {
                    eprintln!(
                        "{}",
                        "Note: semantic model not loaded, using keyword matching only.".dimmed()
                    );
                    apply_boosts(keyword_results, &commands, &boosts, top_k)
                }
            }
        } else {
            eprintln!(
                "{}",
                "Note: no semantic model found, using keyword matching only.".dimmed()
            );
            apply_boosts(keyword_results, &commands, &boosts, top_k)
        }
    } else {
        apply_boosts(keyword_results, &commands, &boosts, top_k)
    };

    // 5. Build match results
    let match_results: Vec<MatchResult> = final_results
        .iter()
        .map(|&(idx, score)| {
            let cmd = &commands[idx];
            let filled = template::fill_template(cmd, query);
            MatchResult {
                command: cmd.clone(),
                score,
                filled_template: Some(filled),
            }
        })
        .collect();

    // 6. Preview and execute
    if dry_run {
        println!();
        println!(
            "  {} {}  {}",
            "Query:".dimmed(),
            query,
            format!("[{}]", target_platform).cyan().dimmed()
        );
        println!();
        for (i, m) in match_results.iter().enumerate() {
            let cmd_str = m.filled_template.as_deref().unwrap_or(&m.command.template);
            println!("  [{}] {} (score: {:.2})", i + 1, cmd_str, m.score);
            println!("      {}", m.command.description.dimmed());
        }
        if match_results.is_empty() {
            println!("  {}", "No matching commands found.".red());
        }
    } else if let Some((command_str, _risk, command_name)) =
        preview::preview_and_select(query, &match_results)
    {
        let exit_code = executor::execute_command(&command_str)?;
        preview::show_result(exit_code);

        let mut history = History::load(&history_path);
        history.record(query, &command_name, &command_str, exit_code);
        let _ = history.save(&history_path);
    }

    Ok(())
}

fn blend_scores(
    keyword_results: &[(usize, f64)],
    semantic_ranked: &[(usize, f64)],
    candidate_indices: &[usize],
    commands: &[CommandEntry],
    boosts: &std::collections::HashMap<String, f64>,
    top_k: usize,
) -> Vec<(usize, f64)> {
    let bm25_max = keyword_results
        .iter()
        .map(|&(_, s)| s)
        .fold(0.0f64, f64::max);
    let bm25_map: std::collections::HashMap<usize, f64> = keyword_results
        .iter()
        .map(|&(idx, score)| (idx, if bm25_max > 0.0 { score / bm25_max } else { 0.0 }))
        .collect();
    let sem_map: std::collections::HashMap<usize, f64> =
        semantic_ranked.iter().copied().collect();

    let mut blended: Vec<(usize, f64)> = candidate_indices
        .iter()
        .map(|&idx| {
            let bm25_norm = bm25_map.get(&idx).copied().unwrap_or(0.0);
            let sem_score = sem_map.get(&idx).copied().unwrap_or(0.0);
            let history_boost = boosts.get(&commands[idx].name).copied().unwrap_or(0.0);
            (idx, 0.55 * bm25_norm + 0.35 * sem_score + 0.1 * history_boost)
        })
        .collect();
    blended.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    blended.truncate(top_k);
    blended
}

fn apply_boosts(
    mut results: Vec<(usize, f64)>,
    commands: &[CommandEntry],
    boosts: &std::collections::HashMap<String, f64>,
    top_k: usize,
) -> Vec<(usize, f64)> {
    for (idx, score) in &mut results {
        let boost = boosts.get(&commands[*idx].name).copied().unwrap_or(0.0);
        *score += boost * *score;
    }
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    results
}

fn resolve_commands_path(commands_dir: &PathBuf) -> PathBuf {
    if commands_dir.is_absolute() {
        return commands_dir.clone();
    }

    // 1. Try relative to CWD (for development: cargo run)
    if commands_dir.exists() {
        return commands_dir.clone();
    }

    // 2. Try ~/.cmdsage/commands/ (installed location)
    if let Some(home) = dirs::home_dir() {
        let home_cmds = home.join(".cmdsage").join("commands");
        if home_cmds.exists() {
            return home_cmds;
        }
    }

    // 3. Try relative to executable
    let exe_relative = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join(commands_dir)));
    if let Some(ref p) = exe_relative {
        if p.exists() {
            return p.clone();
        }
    }

    commands_dir.clone()
}

// --- Subcommands ---

fn cmd_add(
    name: &str,
    binary: &str,
    tmpl: &str,
    description: &str,
    keywords: &str,
    risk: &str,
    commands_dir: &PathBuf,
) -> Result<()> {
    let custom_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cmdsage")
        .join("commands");
    std::fs::create_dir_all(&custom_dir)?;
    let custom_file = custom_dir.join("custom.toml");

    let keywords_arr: Vec<String> = keywords.split(',').map(|s| s.trim().to_string()).collect();
    let keywords_toml: Vec<String> = keywords_arr.iter().map(|k| format!("\"{}\"", k)).collect();

    let entry = format!(
        r#"
[[command]]
name = "{name}"
binary = "{binary}"
template = "{tmpl}"
description = "{description}"
keywords = [{keywords}]
platforms = ["linux", "macos", "windows"]
risk = "{risk}"
args = []
"#,
        name = name,
        binary = binary,
        tmpl = tmpl,
        description = description,
        keywords = keywords_toml.join(", "),
        risk = risk,
    );

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&custom_file)?;
    file.write_all(entry.as_bytes())?;

    println!(
        "{} Added command '{}' to {}",
        "✓".green().bold(),
        name,
        custom_file.display()
    );

    let bundled_custom = commands_dir.join("custom.toml");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&bundled_custom)
    {
        let _ = f.write_all(entry.as_bytes());
        println!(
            "{} Also added to {}",
            "✓".green().bold(),
            bundled_custom.display()
        );
    }

    Ok(())
}

fn cmd_history(count: usize) -> Result<()> {
    let history = History::load(&History::default_path());
    let entries = history.recent(count);

    if entries.is_empty() {
        println!("{}", "No command history yet.".dimmed());
        return Ok(());
    }

    println!("{}", "Recent command history:".green().bold());
    println!();
    for entry in entries {
        let status = if entry.exit_code == 0 {
            "✓".green().to_string()
        } else {
            format!("✗({})", entry.exit_code).red().to_string()
        };
        let time = chrono_format(entry.timestamp);
        println!(
            "  {} {} {} {}",
            status,
            time.dimmed(),
            entry.filled_command.white(),
            format!("← {}", entry.query).dimmed(),
        );
    }

    Ok(())
}

fn cmd_stats(commands_dir: &PathBuf, effective_platform: &str) -> Result<()> {
    let commands_path = resolve_commands_path(commands_dir);
    let all_commands = loader::load_commands(&commands_path)?;

    let native_platform = config::detect_platform();
    let platform_commands =
        platform::filter_for_platform(all_commands.clone(), effective_platform);

    // Per-platform breakdown
    let linux_count = all_commands
        .iter()
        .filter(|c| c.platforms.iter().any(|p| p == "linux"))
        .count();
    let macos_count = all_commands
        .iter()
        .filter(|c| c.platforms.iter().any(|p| p == "macos"))
        .count();
    let windows_count = all_commands
        .iter()
        .filter(|c| c.platforms.iter().any(|p| p == "windows"))
        .count();

    println!("{}", "CmdSage Knowledge Base Stats".green().bold());
    println!();
    println!(
        "  Total commands:      {}",
        all_commands.len().to_string().white().bold()
    );
    println!(
        "  Active platform:     {} ({})",
        effective_platform.cyan().bold(),
        if effective_platform == native_platform {
            "native".to_string()
        } else {
            format!("override, native={}", native_platform).yellow().to_string()
        }
    );
    println!(
        "  Active commands:     {}",
        platform_commands.len().to_string().white().bold()
    );
    println!();
    println!("  {}", "Per-platform breakdown:".dimmed());
    println!(
        "    Linux:    {}{}",
        format!("{:>4}", linux_count).white(),
        if effective_platform == "linux" { "  ◀".cyan().to_string() } else { String::new() }
    );
    println!(
        "    macOS:    {}{}",
        format!("{:>4}", macos_count).white(),
        if effective_platform == "macos" { "  ◀".cyan().to_string() } else { String::new() }
    );
    println!(
        "    Windows:  {}{}",
        format!("{:>4}", windows_count).white(),
        if effective_platform == "windows" { "  ◀".cyan().to_string() } else { String::new() }
    );
    println!();
    println!(
        "  Commands dir:        {}",
        commands_path.display().to_string().dimmed()
    );

    let model_path = onnx::default_model_dir();
    if onnx::model_exists(&model_path) {
        println!("  Semantic model:      {}", "available".green());
    } else {
        println!("  Semantic model:      {}", "not found".yellow());
    }

    let history = History::load(&History::default_path());
    println!(
        "  History entries:     {}",
        history.entries.len().to_string().white()
    );

    Ok(())
}

fn cmd_config(action: Option<&ConfigAction>, effective_platform: &str) -> Result<()> {
    let config_path = Config::default_path();
    let mut cfg = Config::load(&config_path);

    match action {
        None | Some(ConfigAction::Show) => {
            let native = config::detect_platform();
            println!("{}", "CmdSage Configuration".green().bold());
            println!("  Config file: {}", config_path.display().to_string().dimmed());
            println!();
            println!(
                "  platform:      {} {}",
                cfg.platform.cyan().bold(),
                if cfg.platform == "auto" {
                    format!("(detected: {})", native).dimmed().to_string()
                } else if cfg.platform == native {
                    "(same as native)".dimmed().to_string()
                } else {
                    format!("(native: {})", native).yellow().to_string()
                }
            );
            println!(
                "  effective:     {}",
                effective_platform.white().bold()
            );
            println!("  top_k:         {}", cfg.top_k.to_string().white());
            println!(
                "  no_semantic:   {}",
                if cfg.no_semantic {
                    "true".yellow().to_string()
                } else {
                    "false".green().to_string()
                }
            );
            println!();
            println!("{}", "Usage:".dimmed());
            println!("  cmdsage config set-platform macos    # switch to macOS commands");
            println!("  cmdsage config set-platform auto      # auto-detect platform");
            println!("  cmdsage --platform windows \"...\"      # one-time override");
        }
        Some(ConfigAction::SetPlatform { platform }) => {
            if !config::is_valid_platform(platform) {
                anyhow::bail!(
                    "Invalid platform '{}'. Valid options: auto, linux, macos, windows",
                    platform
                );
            }
            let old = cfg.platform.clone();
            cfg.platform = platform.clone();
            cfg.save(&config_path)?;
            let resolved = cfg.resolve_platform(None);
            println!(
                "{} Platform changed: {} -> {} (effective: {})",
                "✓".green().bold(),
                old.dimmed(),
                platform.cyan().bold(),
                resolved.white().bold(),
            );
        }
        Some(ConfigAction::SetTopK { count }) => {
            cfg.top_k = *count;
            cfg.save(&config_path)?;
            println!(
                "{} top_k set to {}",
                "✓".green().bold(),
                count.to_string().white().bold()
            );
        }
        Some(ConfigAction::SetSemantic { value }) => {
            let enabled = match value.to_lowercase().as_str() {
                "on" | "true" | "yes" | "1" => false,  // no_semantic = false means ON
                "off" | "false" | "no" | "0" => true,
                _ => anyhow::bail!("Invalid value '{}'. Use 'on' or 'off'", value),
            };
            cfg.no_semantic = enabled;
            cfg.save(&config_path)?;
            println!(
                "{} Semantic matching: {}",
                "✓".green().bold(),
                if !enabled { "on".green().to_string() } else { "off".yellow().to_string() }
            );
        }
        Some(ConfigAction::Reset) => {
            let cfg = Config::default();
            cfg.save(&config_path)?;
            println!(
                "{} Configuration reset to defaults",
                "✓".green().bold()
            );
        }
    }

    Ok(())
}

fn chrono_format(timestamp: u64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let time = UNIX_EPOCH + Duration::from_secs(timestamp);
    let now = SystemTime::now();
    let ago = now
        .duration_since(time)
        .unwrap_or(Duration::from_secs(0));

    if ago.as_secs() < 60 {
        "just now".to_string()
    } else if ago.as_secs() < 3600 {
        format!("{}m ago", ago.as_secs() / 60)
    } else if ago.as_secs() < 86400 {
        format!("{}h ago", ago.as_secs() / 3600)
    } else {
        format!("{}d ago", ago.as_secs() / 86400)
    }
}
