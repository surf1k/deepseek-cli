use crate::api::{DeepSeekClient, Message, StreamChunk};
use crate::config::Config;
use crate::ui::{create_spinner, print_banner, StreamRenderer};
use anyhow::Result;
use colored::*;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::fs;

pub struct ReplSession {
    config: Config,
    client: DeepSeekClient,
    model: String,
    messages: Vec<Message>,
}

impl ReplSession {
    pub fn new(config: Config, model: Option<String>) -> Self {
        let api_key = config
            .resolve_api_key()
            .expect("API key must be resolved before starting REPL");
        let client = DeepSeekClient::new(config.base_url.clone(), api_key);
        let selected_model = model.unwrap_or_else(|| config.default_model.clone());
        let canonical_model = Config::canonicalize_model(&selected_model);

        let mut messages = Vec::new();
        if let Some(sys) = &config.system_prompt {
            messages.push(Message::system(sys.clone()));
        }

        Self {
            config,
            client,
            model: canonical_model,
            messages,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        print_banner();

        let history_file = Config::history_path().ok();
        let mut rl = DefaultEditor::new()?;
        if let Some(ref path) = history_file {
            let _ = rl.load_history(path);
        }

        println!(
            "{} Active model: {}\n",
            "⚡".yellow(),
            self.model.bold().green()
        );

        let mut buffer = String::new();

        loop {
            let prompt = if buffer.is_empty() {
                format!("{} ", "❯".bright_blue().bold())
            } else {
                format!("{} ", "...".dimmed())
            };

            let readline = rl.readline(&prompt);
            match readline {
                Ok(line) => {
                    let trimmed = line.trim();

                    // Check for line continuation with trailing backslash
                    if trimmed.ends_with('\\') {
                        let without_slash = &trimmed[..trimmed.len() - 1];
                        buffer.push_str(without_slash);
                        buffer.push('\n');
                        continue;
                    }

                    buffer.push_str(trimmed);
                    let input = buffer.trim().to_string();
                    buffer.clear();

                    if input.is_empty() {
                        continue;
                    }

                    let _ = rl.add_history_entry(&input);

                    // Check for REPL commands
                    if input.starts_with('/') {
                        let should_continue = self.handle_command(&input).await?;
                        if !should_continue {
                            break;
                        }
                        continue;
                    }

                    // Process chat input
                    self.process_turn(&input).await;
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C (Press Ctrl+D or type /exit to quit)");
                    buffer.clear();
                }
                Err(ReadlineError::Eof) => {
                    println!("\nBye! 👋");
                    break;
                }
                Err(err) => {
                    eprintln!("{}: {:?}", "Readline error".red().bold(), err);
                    break;
                }
            }
        }

        if let Some(ref path) = history_file {
            let _ = rl.save_history(path);
        }

        Ok(())
    }

    async fn handle_command(&mut self, input: &str) -> Result<bool> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "/exit" | "/quit" | "/q" => {
                println!("Bye! 👋");
                return Ok(false);
            }
            "/clear" | "/c" => {
                self.messages.retain(|m| m.role == "system");
                println!("{}", "Conversation history cleared ✨".green());
            }
            "/model" | "/m" => {
                if parts.len() < 2 {
                    println!(
                        "Current model: {}. Options: {} or {}",
                        self.model.bold().green(),
                        "v3 (deepseek-chat)".cyan(),
                        "r1 (deepseek-reasoner)".cyan()
                    );
                } else {
                    let new_model = Config::canonicalize_model(parts[1]);
                    self.model = new_model;
                    println!("Model switched to {}", self.model.bold().green());
                }
            }
            "/system" => {
                if parts.len() < 2 {
                    println!("Usage: /system <prompt>");
                } else {
                    let sys_prompt = parts[1..].join(" ");
                    self.messages.retain(|m| m.role != "system");
                    self.messages.insert(0, Message::system(&sys_prompt));
                    println!("{}", "System prompt updated.".green());
                }
            }
            "/save" => {
                let filename = parts.get(1).unwrap_or(&"deepseek-session.md");
                let mut out = String::new();
                out.push_str(&format!("# DeepSeek Chat Session ({})\n\n", self.model));
                for msg in &self.messages {
                    out.push_str(&format!("### {}\n\n{}\n\n", msg.role.to_uppercase(), msg.content));
                }
                fs::write(filename, out)?;
                println!("{} Saved session to {}", "✓".green(), filename.bold());
            }
            "/info" => {
                println!("DeepSeek CLI session info:");
                println!("  Model:       {}", self.model.cyan());
                println!("  Base URL:    {}", self.config.base_url.dimmed());
                println!("  Turns:       {}", self.messages.len());
            }
            "/help" | "/h" => {
                println!("{}", "Available commands:".bold());
                println!("  {}                Show this help message", "/help".yellow());
                println!("  {}       Switch model (v3 / r1)", "/model <name>".yellow());
                println!("  {}               Clear conversation context", "/clear".yellow());
                println!("  {}       Set custom system prompt", "/system <text>".yellow());
                println!("  {}        Save conversation to markdown file", "/save [file]".yellow());
                println!("  {}                Show session info", "/info".yellow());
                println!("  {}                Exit CLI", "/exit".yellow());
                println!("\n  {} End a line with {} to write multiline input.", "Tip:".cyan(), "\\".bold());
            }
            _ => {
                println!("{}: Unknown command '{}'. Type {} for help.", "Notice".yellow(), cmd, "/help".cyan());
            }
        }

        Ok(true)
    }

    async fn process_turn(&mut self, prompt: &str) {
        self.messages.push(Message::user(prompt));

        let spinner = create_spinner("Connecting to DeepSeek...");
        let mut renderer = StreamRenderer::new();
        let mut has_received_tokens = false;

        let result = self
            .client
            .stream_chat(
                &self.model,
                &self.messages,
                Some(self.config.temperature),
                |chunk| {
                    if !has_received_tokens {
                        has_received_tokens = true;
                        spinner.finish_and_clear();
                    }
                    match chunk {
                        StreamChunk::Reasoning(text) => renderer.on_reasoning_chunk(&text),
                        StreamChunk::Content(text) => renderer.on_content_chunk(&text),
                        StreamChunk::Done => renderer.finish(),
                    }
                },
            )
            .await;

        if !has_received_tokens {
            spinner.finish_and_clear();
        }

        match result {
            Ok((_, content)) => {
                renderer.finish();
                self.messages.push(Message::assistant(content));
            }
            Err(e) => {
                renderer.finish();
                eprintln!("\n{}: {}", "Error".red().bold(), e);
                // Remove the last user message so they can re-try
                self.messages.pop();
            }
        }
    }
}
