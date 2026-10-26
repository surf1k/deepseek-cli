mod api;
mod config;
mod files;
mod git;
mod repl;
mod shell;
mod ui;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use colored::*;
use config::Config;
use git::GitAssistant;
use repl::ReplSession;
use shell::ShellHelper;
use std::io::{self, IsTerminal, Read};

#[derive(Parser, Debug)]
#[command(
    name = "deepseek",
    author = "surf1k",
    version = "0.1.0",
    about = "⚡ Blazing-fast CLI for DeepSeek V3 & R1 reasoning models written in pure Rust",
    long_about = "A sleek, feature-rich command-line client for DeepSeek AI.\n\
Supports real-time streaming, DeepThink (R1 reasoning) visualization, interactive REPL, \
Git commit & review assistant, natural language shell execution, file attachments, and Unix piping."
)]
struct Cli {
    /// Prompt / question to ask DeepSeek (leave empty for interactive REPL)
    #[arg(trailing_var_arg = true)]
    prompt: Vec<String>,

    /// Model to use: 'v3' (deepseek-chat) or 'r1' (deepseek-reasoner)
    #[arg(short, long)]
    model: Option<String>,

    /// Custom system prompt
    #[arg(short, long)]
    system: Option<String>,

    /// Attach file(s), line ranges, or globs (e.g. -f src/main.rs:10-50 -f "src/**/*.rs")
    #[arg(short, long, value_name = "SPEC")]
    file: Vec<String>,

    /// Sampling temperature (0.0 to 2.0)
    #[arg(short, long)]
    temperature: Option<f32>,

    /// Maximum tokens to generate
    #[arg(long)]
    max_tokens: Option<u32>,

    /// Raw plain-text output (disables styling, ideal for scripts and piping)
    #[arg(long)]
    raw: bool,

    /// Hide the token and speed statistics bar
    #[arg(long)]
    no_stats: bool,

    /// Automatically copy final answer to system clipboard
    #[arg(short = 'c', long)]
    copy: bool,

    /// Custom API base URL (default: https://api.deepseek.com)
    #[arg(long)]
    base_url: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate a Conventional Commit message from current git diff
    #[command(alias = "c")]
    Commit {
        /// Override model for commit generation (defaults to v3)
        #[arg(short, long)]
        model: Option<String>,
    },

    /// Perform a deep code review of staged/unstaged git changes
    #[command(alias = "r")]
    Review {
        /// Override model for review (defaults to r1)
        #[arg(short, long)]
        model: Option<String>,
    },

    /// Generate and optionally execute a shell command from natural language
    #[command(alias = "run")]
    Sh {
        /// What you want the terminal command to do
        #[arg(trailing_var_arg = true)]
        task: Vec<String>,
    },

    /// Manage configuration, models, and API keys
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Set your DeepSeek API key
    SetKey {
        /// The API key (sk-...)
        key: String,
    },
    /// Set default model ('v3' or 'r1')
    SetModel {
        /// Model name or alias
        model: String,
    },
    /// Set custom API base URL
    SetUrl {
        /// Base URL (e.g. https://api.deepseek.com or http://localhost:11434/v1)
        url: String,
    },
    /// Show current configuration
    Show,
    /// Print path to the config file
    Path,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut cfg = Config::load();

    if let Some(ref base_url) = cli.base_url {
        cfg.base_url = base_url.clone();
    }

    // Handle subcommands
    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Config { action } => return handle_config_action(action, cfg),
            Commands::Commit { model } => {
                let api_key = require_api_key(&cfg)?;
                let client = api::DeepSeekClient::new(cfg.base_url.clone(), api_key);
                let assistant = GitAssistant::new(&client, &cfg, model);
                return assistant.run_commit().await;
            }
            Commands::Review { model } => {
                let api_key = require_api_key(&cfg)?;
                let client = api::DeepSeekClient::new(cfg.base_url.clone(), api_key);
                let assistant = GitAssistant::new(&client, &cfg, model.or(Some("r1".to_string())));
                return assistant.run_review().await;
            }
            Commands::Sh { task } => {
                let api_key = require_api_key(&cfg)?;
                let client = api::DeepSeekClient::new(cfg.base_url.clone(), api_key);
                let helper = ShellHelper::new(&client, &cfg);
                let query = task.join(" ");
                if query.trim().is_empty() {
                    bail!("Please provide a task description for the shell helper, e.g. 'deepseek sh find files > 50mb'");
                }
                return helper.run(&query).await;
            }
        }
    }

    // Resolve API key
    let api_key = require_api_key(&cfg)?;

    // Check if stdin has piped data
    let mut stdin_content = String::new();
    if !io::stdin().is_terminal() {
        let _ = io::stdin().read_to_string(&mut stdin_content);
    }

    let has_prompt = !cli.prompt.is_empty();
    let has_files = !cli.file.is_empty();
    let has_stdin = !stdin_content.trim().is_empty();

    // If no prompt, no files, and no piped stdin -> launch interactive REPL
    if !has_prompt && !has_files && !has_stdin {
        let mut session = ReplSession::new(cfg, cli.model);
        return session.run().await;
    }

    // Otherwise, execute one-shot mode
    run_one_shot(cli, cfg, api_key, stdin_content).await
}

fn require_api_key(cfg: &Config) -> Result<String> {
    match cfg.resolve_api_key() {
        Some(k) => Ok(k),
        None => {
            eprintln!("{}", "Error: No DeepSeek API key found.".red().bold());
            eprintln!(
                "Set it via environment variable: {} or run:\n  {}",
                "export DEEPSEEK_API_KEY=sk-...".cyan(),
                "deepseek config set-key <YOUR_KEY>".yellow()
            );
            std::process::exit(1);
        }
    }
}

fn handle_config_action(action: ConfigAction, mut cfg: Config) -> Result<()> {
    match action {
        ConfigAction::SetKey { key } => {
            cfg.api_key = Some(key.trim().to_string());
            cfg.save()?;
            println!("{} API key saved successfully.", "✓".green());
        }
        ConfigAction::SetModel { model } => {
            let canonical = Config::canonicalize_model(&model);
            cfg.default_model = canonical.clone();
            cfg.save()?;
            println!("{} Default model set to {}.", "✓".green(), canonical.bold().cyan());
        }
        ConfigAction::SetUrl { url } => {
            cfg.base_url = url.trim().to_string();
            cfg.save()?;
            println!("{} API base URL set to {}.", "✓".green(), cfg.base_url.bold().cyan());
        }
        ConfigAction::Show => {
            println!("{}", "Current DeepSeek CLI Configuration:".bold());
            println!("  Config path:   {}", Config::config_path()?.display().to_string().cyan());
            println!("  Base URL:      {}", cfg.base_url);
            println!("  Default model: {}", cfg.default_model);
            println!("  Temperature:   {}", cfg.temperature);
            let masked_key = match cfg.resolve_api_key() {
                Some(k) if k.len() > 8 => format!("{}...{}", &k[..4], &k[k.len() - 4..]),
                Some(_) => "******".to_string(),
                None => "None (not set)".red().to_string(),
            };
            println!("  API Key:       {}", masked_key);
        }
        ConfigAction::Path => {
            println!("{}", Config::config_path()?.display());
        }
    }
    Ok(())
}

async fn run_one_shot(
    cli: Cli,
    cfg: Config,
    api_key: String,
    stdin_content: String,
) -> Result<()> {
    let client = api::DeepSeekClient::new(cfg.base_url, api_key);
    let selected_model = cli
        .model
        .unwrap_or_else(|| cfg.default_model.clone());
    let model = Config::canonicalize_model(&selected_model);

    // Build context
    let mut messages = Vec::new();

    // 1. System prompt
    let system_prompt = cli
        .system
        .or(cfg.system_prompt)
        .unwrap_or_else(|| "You are DeepSeek, a helpful, precise, and thoughtful AI assistant.".to_string());
    messages.push(api::Message::system(system_prompt));

    // 2. Attached files content with lines & glob support
    let files_context = if !cli.file.is_empty() {
        files::load_file_contexts(&cli.file)?
    } else {
        String::new()
    };

    // 3. User prompt + Stdin
    let user_text = if cli.prompt.is_empty() {
        if !stdin_content.is_empty() {
            "Please analyze the following input.".to_string()
        } else {
            String::new()
        }
    } else {
        cli.prompt.join(" ")
    };

    let mut full_user_content = String::new();
    if !files_context.is_empty() {
        full_user_content.push_str(&files_context);
        full_user_content.push('\n');
    }
    if !stdin_content.is_empty() {
        full_user_content.push_str("--- Piped Input ---\n");
        full_user_content.push_str(&stdin_content);
        full_user_content.push_str("\n--- End Piped Input ---\n\n");
    }
    full_user_content.push_str(&user_text);

    messages.push(api::Message::user(full_user_content));

    // 4. Stream response
    let raw = cli.raw || !io::stdout().is_terminal();
    let mut renderer = ui::StreamRenderer::new();
    let mut spinner = if !raw {
        Some(ui::create_spinner("Connecting to DeepSeek..."))
    } else {
        None
    };

    let mut first_token = true;
    let mut accumulated_answer = String::new();

    let res = client
        .stream_chat(
            &model,
            &messages,
            cli.temperature.or(Some(cfg.temperature)),
            cli.max_tokens.or(cfg.max_tokens),
            |chunk| {
                if first_token {
                    first_token = false;
                    if let Some(sp) = spinner.take() {
                        sp.finish_and_clear();
                    }
                }

                if raw {
                    if let api::StreamChunk::Content(text) = chunk {
                        accumulated_answer.push_str(&text);
                        print!("{}", text);
                        let _ = io::Write::flush(&mut io::stdout());
                    }
                } else {
                    match chunk {
                        api::StreamChunk::Reasoning(text) => renderer.on_reasoning_chunk(&text),
                        api::StreamChunk::Content(text) => {
                            accumulated_answer.push_str(&text);
                            renderer.on_content_chunk(&text);
                        }
                        api::StreamChunk::Usage(_) => {}
                        api::StreamChunk::Done => {}
                    }
                }
            },
        )
        .await;

    if let Some(sp) = spinner.take() {
        sp.finish_and_clear();
    }

    match res {
        Ok((_, _, stats)) => {
            if !raw {
                let stats_ref = if !cli.no_stats && cfg.show_stats {
                    Some(&stats)
                } else {
                    None
                };
                renderer.finish(stats_ref);
            } else {
                println!();
            }

            if cli.copy && !accumulated_answer.is_empty() {
                ui::copy_to_clipboard(&accumulated_answer);
                if !raw {
                    println!("{}", "  [✓ Copied to clipboard]".dimmed());
                }
            }

            Ok(())
        }
        Err(e) => {
            if !raw {
                renderer.finish(None);
            }
            bail!(e);
        }
    }
}
