use crate::config::Config;
use crate::ui::create_spinner;
use anyhow::Result;
use colored::*;
use std::io::{stdin, stdout, Write};

pub async fn run_onboarding(cfg: &mut Config) -> Result<String> {
    print_welcome_banner();

    // Step 1: Prompt API Key with online verification
    let api_key = prompt_api_key(&cfg.base_url).await?;
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

async fn prompt_api_key(base_url: &str) -> Result<String> {
    println!(
        "{}",
        "╭── Step 1: DeepSeek API Key ──────────────────────────────────────────╮".cyan().bold()
    );
    println!(
        "│ Get your API key from:                                               │\n│   {}",
        "https://platform.deepseek.com/api_keys".underline().blue()
    );
    println!("│                                                                      │");
    println!("│ DeepSeek V3 & R1 offer state-of-the-art coding and reasoning at      │");
    println!("│ ~95% lower cost than Claude 3.5 Sonnet or OpenAI o1.                 │");
    println!(
        "{}",
        "╰──────────────────────────────────────────────────────────────────────╯".cyan().bold()
    );

    loop {
        print!("\n{} ", "❯ Enter your API Key (sk-...):".bold().yellow());
        let _ = stdout().flush();

        let mut input = String::new();
        stdin().read_line(&mut input)?;
        let key = clean_key_input(&input);

        if key.is_empty() {
            println!("  {} API key cannot be empty. Please try again.", "✗".red());
            continue;
        }

        // Validate API Key
        let spinner = create_spinner("Verifying API key with DeepSeek servers...");
        let is_valid = validate_key(base_url, &key).await;
        spinner.finish_and_clear();

        match is_valid {
            Some(true) => {
                println!(
                    "  {} API key verified successfully! Connected to DeepSeek Platform.",
                    "✓".green().bold()
                );
                return Ok(key);
            }
            Some(false) => {
                println!(
                    "  {} DeepSeek server rejected this key (401 Unauthorized).",
                    "⚠".yellow().bold()
                );
                print!("  Do you want to retry entering your key? [Y/n]: ");
                let _ = stdout().flush();
                let mut retry_input = String::new();
                let _ = stdin().read_line(&mut retry_input);
                let choice = retry_input.trim();
                if choice.eq_ignore_ascii_case("n") || choice.eq_ignore_ascii_case("no") {
                    println!("  Using provided key without verification.");
                    return Ok(key);
                }
            }
            None => {
                println!(
                    "  {} Could not reach DeepSeek server (network unreachable/custom host).",
                    "ℹ".cyan()
                );
                println!("  Proceeding with the entered key.");
                return Ok(key);
            }
        }
    }
}

fn clean_key_input(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c| c == '\'' || c == '"' || c == '`')
        .trim()
        .to_string()
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
        "╭── ✓ Onboarding Complete! ─────────────────────────────────────────────╮".green().bold()
    );
    println!(
        "│ Config saved:  {}",
        cfg_path.display().to_string().cyan()
    );
    println!("│ Default Model: {}", model.bold().green());
    println!("│ Base URL:      {}", cfg.base_url.dimmed());
    println!(
        "{}",
        "╰───────────────────────────────────────────────────────────────────────╯".green().bold()
    );

    println!("\n{}", "Quick Start Guide:".bold());
    println!(
        "  • Interactive REPL:    {} (syntax highlighting, slash commands)",
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
        "  • Natural Language Shell: {} (e.g. 'find files > 50mb')",
        "deepseek sh \"<task>\"".cyan()
    );
    println!(
        "  • Track Cost & Savings:{} (calculates $ vs Claude 3.5 Sonnet)",
        "deepseek cost".yellow()
    );
    println!(
        "\nType {} inside interactive mode for more commands.\n",
        "/help".yellow().bold()
    );

    Ok(())
}

async fn validate_key(base_url: &str, key: &str) -> Option<bool> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .ok()?;

    let res = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", key))
        .send()
        .await
        .ok()?;

    Some(res.status().is_success())
}
