use crate::api::{DeepSeekClient, GenerationStats, Message, StreamChunk};
use crate::config::Config;
use crate::ui::{copy_to_clipboard, create_spinner, print_banner, StreamRenderer};
use anyhow::Result;
use colored::*;
use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use std::fs;

struct CommandCompleter {
    commands: Vec<&'static str>,
}

impl CommandCompleter {
    fn new() -> Self {
        Self {
            commands: vec![
                "/help", "/model", "/clear", "/copy", "/tokens", "/save", "/load",
                "/system", "/temp", "/retry", "/info", "/exit", "/quit",
            ],
        }
    }
}

impl Completer for CommandCompleter {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        if line.starts_with('/') {
            let prefix = &line[..pos];
            let matches: Vec<String> = self
                .commands
                .iter()
                .filter(|cmd| cmd.starts_with(prefix))
                .map(|s| s.to_string())
                .collect();
            return Ok((0, matches));
        }
        Ok((pos, Vec::new()))
    }
}

impl Hinter for CommandCompleter {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        None
    }
}

impl Highlighter for CommandCompleter {}
impl Validator for CommandCompleter {}
impl Helper for CommandCompleter {}

pub struct ReplSession {
    config: Config,
    client: DeepSeekClient,
    model: String,
    messages: Vec<Message>,
    last_assistant_response: Option<String>,
    session_stats: GenerationStats,
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
            last_assistant_response: None,
            session_stats: GenerationStats::default(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        print_banner();

        let history_file = Config::history_path().ok();
        let mut rl: Editor<CommandCompleter, rustyline::history::DefaultHistory> =
            Editor::new()?;
        rl.set_helper(Some(CommandCompleter::new()));

        if let Some(ref path) = history_file {
            let _ = rl.load_history(path);
        }

        let is_r1 = self.model.contains("reasoner");
        let model_badge = if is_r1 {
            format!("[DeepSeek-R1 🧠]").cyan().bold()
        } else {
            format!("[DeepSeek-V3 ⚡]").green().bold()
        };

        println!("  Active: {} (Tab for autocompletion)\n", model_badge);

        let mut multiline_buffer = String::new();

        loop {
            let prompt = if multiline_buffer.is_empty() {
                let badge = if self.model.contains("reasoner") {
                    "R1".cyan()
                } else {
                    "V3".green()
                };
                format!("{} ❯ ", badge)
            } else {
                format!("{} ", "...".dimmed())
            };

            let readline = rl.readline(&prompt);
            match readline {
                Ok(line) => {
                    let trimmed = line.trim();

                    // Check for multiline trailing slash continuation
                    if trimmed.ends_with('\\') {
                        let without_slash = &trimmed[..trimmed.len() - 1];
                        multiline_buffer.push_str(without_slash);
                        multiline_buffer.push('\n');
                        continue;
                    }

                    multiline_buffer.push_str(trimmed);
                    let input = multiline_buffer.trim().to_string();
                    multiline_buffer.clear();

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
                    println!("^C (type /exit or press Ctrl+D to quit)");
                    multiline_buffer.clear();
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
                self.last_assistant_response = None;
                println!("{}", "✨ Conversation history cleared.".green());
            }
            "/copy" | "/cp" => {
                if let Some(ref text) = self.last_assistant_response {
                    if copy_to_clipboard(text) {
                        println!("{} Last answer copied to clipboard!", "✓".green());
                    } else {
                        eprintln!("{} Failed to copy to clipboard.", "✗".red());
                    }
                } else {
                    println!("{} No answer to copy yet.", "ℹ".yellow());
                }
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
                    println!("Model switched to {}", self.model.bold().cyan());
                }
            }
            "/temp" | "/t" => {
                if parts.len() < 2 {
                    println!("Current temperature: {}", self.config.temperature);
                } else if let Ok(val) = parts[1].parse::<f32>() {
                    self.config.temperature = val.clamp(0.0, 2.0);
                    println!("Temperature set to {}", self.config.temperature);
                } else {
                    println!("Usage: /temp <0.0 - 2.0>");
                }
            }
            "/tokens" => {
                println!("{}", "Session Token Statistics:".bold().cyan());
                println!("  Total prompt tokens:     {}", self.session_stats.prompt_tokens);
                println!("  Total completion tokens: {}", self.session_stats.completion_tokens);
                println!("  Total tokens used:       {}", self.session_stats.total_tokens);
                println!("  Context cache hits:      {} tokens", self.session_stats.cache_hit_tokens);
                println!("  Messages in context:     {}", self.messages.len());
            }
            "/system" => {
                if parts.len() < 2 {
                    let current = self
                        .messages
                        .iter()
                        .find(|m| m.role == "system")
                        .map(|m| m.content.as_str())
                        .unwrap_or("None");
                    println!("Current system prompt: {}", current.dimmed());
                } else {
                    let sys_prompt = parts[1..].join(" ");
                    self.messages.retain(|m| m.role != "system");
                    self.messages.insert(0, Message::system(&sys_prompt));
                    println!("{}", "✓ System prompt updated.".green());
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
            "/load" => {
                if parts.len() < 2 {
                    println!("Usage: /load <file_path>");
                } else {
                    let file_path = parts[1];
                    match fs::read_to_string(file_path) {
                        Ok(content) => {
                            self.messages.push(Message::user(format!(
                                "Loaded context from {}:\n\n{}",
                                file_path, content
                            )));
                            println!("{} Loaded {} into context.", "✓".green(), file_path);
                        }
                        Err(e) => {
                            eprintln!("{}: {}", "Error reading file".red(), e);
                        }
                    }
                }
            }
            "/retry" => {
                if let Some(last_msg) = self.messages.last().cloned() {
                    if last_msg.role == "assistant" {
                        self.messages.pop(); // Remove assistant
                        if let Some(user_msg) = self.messages.last().cloned() {
                            if user_msg.role == "user" {
                                self.messages.pop(); // Remove user to re-process
                                println!("{} Retrying last prompt...", "↻".yellow());
                                self.process_turn(&user_msg.content).await;
                            }
                        }
                    }
                } else {
                    println!("Nothing to retry.");
                }
            }
            "/info" => {
                println!("DeepSeek CLI Session:");
                println!("  Model:       {}", self.model.cyan());
                println!("  Base URL:    {}", self.config.base_url.dimmed());
                println!("  Temperature: {}", self.config.temperature);
                println!("  History:     {} turns", self.messages.len());
            }
            "/help" | "/h" => {
                println!("{}", "Available REPL Commands:".bold());
                println!("  {}                Show this help message", "/help".yellow());
                println!("  {}       Switch model (v3 / r1)", "/model <name>".yellow());
                println!("  {}                Copy last response to clipboard", "/copy".yellow());
                println!("  {}               Show token usage stats", "/tokens".yellow());
                println!("  {}               Regenerate last response", "/retry".yellow());
                println!("  {}               Clear conversation context", "/clear".yellow());
                println!("  {}       Set temperature (0.0 to 2.0)", "/temp <num>".yellow());
                println!("  {}       Update system prompt", "/system <text>".yellow());
                println!("  {}        Export session to Markdown", "/save [file]".yellow());
                println!("  {}        Load file into context", "/load <file>".yellow());
                println!("  {}                Show session info", "/info".yellow());
                println!("  {}                Exit REPL", "/exit".yellow());
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
                None,
                |chunk| {
                    if !has_received_tokens {
                        has_received_tokens = true;
                        spinner.finish_and_clear();
                    }
                    match chunk {
                        StreamChunk::Reasoning(text) => renderer.on_reasoning_chunk(&text),
                        StreamChunk::Content(text) => renderer.on_content_chunk(&text),
                        StreamChunk::Usage(_) => {}
                        StreamChunk::Done => {}
                    }
                },
            )
            .await;

        if !has_received_tokens {
            spinner.finish_and_clear();
        }

        match result {
            Ok((_, content, stats)) => {
                renderer.finish(Some(&stats));
                self.last_assistant_response = Some(content.clone());
                self.messages.push(Message::assistant(content));

                // Accumulate session stats
                self.session_stats.prompt_tokens += stats.prompt_tokens;
                self.session_stats.completion_tokens += stats.completion_tokens;
                self.session_stats.total_tokens += stats.total_tokens;
                self.session_stats.cache_hit_tokens += stats.cache_hit_tokens;
                self.session_stats.cache_miss_tokens += stats.cache_miss_tokens;
            }
            Err(e) => {
                renderer.finish(None);
                eprintln!("\n{}: {}", "Error".red().bold(), e);
                // Remove the last user message so they can re-try
                self.messages.pop();
            }
        }
    }
}
