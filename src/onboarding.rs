use crate::auth;
use crate::config::Config;
use anyhow::Result;
use colored::*;
use std::io::{stdin, stdout, Write};

pub async fn run_onboarding(cfg: &mut Config) -> Result<String> {
    print_welcome_banner();

    // Step 1: Authentication (Web browser login, manual key, or custom endpoint)
    let api_key = auth::prompt_authentication(cfg).await?;
    cfg.api_key = Some(api_key.clone());

    // Step 2: Default Model Selection
    let chosen_model = prompt_model_selection();
    cfg.default_model = chosen_model.clone();

    // Step 3: Save configuration
    cfg.save()?;

    // Step 4: Setup Complete Box & Quick Tips
    print_completion_box(cfg, &chosen_model)?;

    Ok(api_key)
}

fn print_welcome_banner() {
    let logo = r#"
  ____                  ____            _       ____ _     ___ 
 |  _ \  ___  ___ _ __ / ___|  ___  ___| | __  / ___| |   |_ _|
 | | | |/ _ \/ _ \ '_ \\___ \ / _ \/ _ \ |/ / | |   | |    | | 
 | |_| |  __/  __/ |_) |___) |  __/  __/   <  | |___| |___ | | 
 |____/ \___|\___| .__/|____/ \___|\___|_|\_\  \____|_____|___|
                 |_|                                           
"#;
    println!("{}", logo.bright_cyan().bold());
    println!(
        "  {} {}",
        "⚡".yellow(),
        "DeepSeek CLI — Intelligent Terminal Coding Partner".bold()
    );
    println!(
        "  {} {}",
        "Version:".dimmed(),
        env!("CARGO_PKG_VERSION").cyan()
    );
    println!(
        "  {} {}",
        "Repository:".dimmed(),
        "https://github.com/surf1k/deepseek-cli".underline().blue()
    );
    println!(
        "\n  {}\n",
        "Welcome! Let's configure your DeepSeek environment in a few quick steps."
            .dimmed()
    );
}

fn prompt_model_selection() -> String {
    println!(
        "\n{}",
        "╭── Step 2: Select Default Model ──────────────────────────────────────╮".cyan().bold()
    );
    println!(
        "│ {} {}                                  │",
        "[1]".yellow().bold(),
        "DeepSeek-V3  (deepseek-chat) - Recommended".bold()
    );
    println!("│     • Ultra-fast inference, top coding & architecture benchmarks    │");
    println!("│     • Optimal for daily REPL chat, agents, and git workflows         │");
    println!("│                                                                      │");
    println!(
        "│ {} {}                                  │",
        "[2]".yellow().bold(),
        "DeepSeek-R1  (deepseek-reasoner)          ".bold()
    );
    println!("│     • DeepThink reasoning model (shows internal thought tokens)      │");
    println!("│     • Excels at complex math, algorithms, and deep logic analysis    │");
    println!(
        "{}",
        "╰──────────────────────────────────────────────────────────────────────╯".cyan().bold()
    );

    print!("\n{} ", "❯ Select default model [1 or 2, default: 1]:".bold());
    let _ = stdout().flush();

    let mut choice = String::new();
    let _ = stdin().read_line(&mut choice);

    match choice.trim() {
        "2" | "r1" | "reasoner" => {
            println!("  Selected: {}", "DeepSeek-R1 (deepseek-reasoner)".green().bold());
            "deepseek-reasoner".to_string()
        }
        _ => {
            println!("  Selected: {}", "DeepSeek-V3 (deepseek-chat)".green().bold());
            "deepseek-chat".to_string()
        }
    }
}

fn print_completion_box(cfg: &Config, model: &str) -> Result<()> {
    let cfg_path = Config::config_path()?;

    println!(
        "\n{}",
        "╭── ✓ Setup Complete! ──────────────────────────────────────────────────╮".green().bold()
    );
    println!(
        "│ Config saved:  {}",
        cfg_path.display().to_string().cyan()
    );
    println!("│ Default Model: {}", model.bold().green());
    println!("│ Auto-compact:  {} (threshold: {} chars)", "enabled".green(), cfg.compact_threshold_chars);
    println!("│ Base URL:      {}", cfg.base_url.dimmed());
    println!(
        "{}",
        "╰───────────────────────────────────────────────────────────────────────╯".green().bold()
    );

    println!("\n{}", "Quick Start Guide:".bold());
    println!(
        "  • Interactive REPL:    {} (type /help or /model anytime)",
        "deepseek".cyan().bold()
    );
    println!(
        "  • Autonomous Agent:    {} (like Claude Code)",
        "deepseek agent \"<task>\"".cyan().bold()
    );
    println!(
        "  • Conventional Commit: {} (generates commits from git diff)",
        "deepseek commit".cyan()
    );
    println!(
        "  • Deep Code Review:    {} (reviews unstaged/staged changes with R1)",
        "deepseek review".cyan()
    );
    println!(
        "  • Track Cost & Savings:{} (calculates $ vs Claude 3.5 Sonnet)",
        "deepseek cost".yellow()
    );
    println!(
        "  • Re-login / Switch:   {}",
        "deepseek login".cyan()
    );
    println!(
        "\nType {} inside interactive mode for more commands.\n",
        "/help".yellow().bold()
    );

    Ok(())
}
