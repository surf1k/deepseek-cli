use crate::agent::compact::compact_history;
use crate::agent::Agent;
use crate::api::{AgentMessage, DeepSeekClient, GenerationStats, StreamChunk};
use crate::config::Config;
use crate::cost::{format_cost_line, print_detailed_cost};
use crate::session::{list_sessions, save_session, SessionData};
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
use std::io::{stdin, stdout, Write};

struct CommandCompleter {
    commands: Vec<&'static str>,
}

impl CommandCompleter {
    fn new() -> Self {
        Self {
            commands: vec![
                "/help", "/model", "/models", "/compact", "/autocompact", "/cost",
                "/auto", "/status", "/clear", "/copy", "/tokens", "/sessions",
                "/save", "/load", "/system", "/temp", "/retry", "/info", "/exit", "/quit",
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
    messages: Vec<AgentMessage>,
    last_assistant_response: Option<String>,
    session_stats: GenerationStats,
    auto_approve: bool,
    session_data: SessionData,
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
            messages.push(AgentMessage::system(sys.clone()));
        }

        let session_data = SessionData::new(canonical_model.clone(), messages.clone());

        Self {
            config,
            client,
            model: canonical_model,
            messages,
            last_assistant_response: None,
            session_stats: GenerationStats::default(),
            auto_approve: false,
            session_data,
        }
    }

    pub fn from_session(config: Config, session: SessionData) -> Self {
        let api_key = config
            .resolve_api_key()
            .expect("API key must be resolved before starting REPL");
        let client = DeepSeekClient::new(config.base_url.clone(), api_key);
        let model = session.model.clone();
        let messages = session.messages.clone();
        let stats = session.stats.clone();

        Self {
            config,
            client,
            model,
            messages,
            last_assistant_response: None,
            session_stats: stats,
            auto_approve: false,
            session_data: session,
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

        println!("  Active: {} · Type {} for cost, {} for tools\n", model_badge, "/cost".yellow(), "/help".yellow());

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

                    if input.starts_with('/') {
                        let should_continue = self.handle_command(&input).await?;
                        if !should_continue {
                            break;
                        }
                        continue;
                    }

                    // Check if input is a coding action request that should run via Agent
                    if self.should_route_to_agent(&input) {
                        self.process_agent_turn(&input).await;
                    } else {
                        self.process_chat_turn(&input).await;
                    }

                    // Save session state
                    self.session_data.messages = self.messages.clone();
                    self.session_data.stats = self.session_stats.clone();
                    let _ = save_session(&self.session_data);
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

    fn should_route_to_agent(&self, input: &str) -> bool {
        let lower = input.to_lowercase();
        lower.starts_with("fix ")
            || lower.starts_with("edit ")
            || lower.starts_with("create ")
            || lower.starts_with("run ")
            || lower.starts_with("test ")
            || lower.starts_with("build ")
            || lower.starts_with("refactor ")
            || lower.starts_with("add ")
            || lower.starts_with("delete ")
            || lower.starts_with("search ")
            || lower.starts_with("find ")
    }

    async fn handle_command(&mut self, input: &str) -> Result<bool> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "/exit" | "/quit" | "/q" => {
                println!("Bye! 👋");
                return Ok(false);
            }
            "/cost" => {
                print_detailed_cost(&self.session_stats, &self.model);
            }
            "/compact" => {
                let _ = compact_history(&self.client, &self.model, &mut self.messages).await;
            }
            "/autocompact" => {
                if parts.len() > 1 {
                    match parts[1].to_lowercase().as_str() {
                        "on" | "true" | "1" => self.config.auto_compact = true,
                        "off" | "false" | "0" => self.config.auto_compact = false,
                        _ => println!("Usage: /autocompact [on|off]"),
                    }
                } else {
                    self.config.auto_compact = !self.config.auto_compact;
                }
                let status = if self.config.auto_compact {
                    "enabled".green().bold()
                } else {
                    "disabled".yellow().bold()
                };
                println!(
                    "{} Auto-compact is now {} (threshold: {} chars).",
                    "🗜️".cyan(),
                    status,
                    self.config.compact_threshold_chars
                );
                let _ = self.config.save();
            }
            "/auto" => {
                self.auto_approve = !self.auto_approve;
                if self.auto_approve {
                    println!("{} Auto-approval enabled for tool actions.", "🛡️".green());
                } else {
                    println!("{} Interactive approval enabled.", "🛡️".cyan());
                }
            }
            "/sessions" => {
                println!("{}", "Saved Sessions:".bold().cyan());
                if let Ok(sessions) = list_sessions() {
                    for s in sessions.iter().take(10) {
                        println!("  {} [{}] - {} messages", s.id.cyan(), s.model, s.messages.len());
                    }
                }
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
            "/model" | "/models" | "/m" => {
                if parts.len() < 2 {
                    self.interactive_model_select().await;
                } else {
                    let new_model = Config::canonicalize_model(parts[1]);
                    self.model = new_model.clone();
                    self.session_data.model = new_model;
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
                println!("  {}", format_cost_line(&self.session_stats, &self.model));
            }
            "/system" => {
                if parts.len() < 2 {
                    let current = self
                        .messages
                        .iter()
                        .find(|m| m.role == "system")
                        .and_then(|m| m.content.as_deref())
                        .unwrap_or("None");
                    println!("Current system prompt: {}", current.dimmed());
                } else {
                    let sys_prompt = parts[1..].join(" ");
                    self.messages.retain(|m| m.role != "system");
                    self.messages.insert(0, AgentMessage::system(&sys_prompt));
                    println!("{}", "✓ System prompt updated.".green());
                }
            }
            "/save" => {
                let filename = parts.get(1).unwrap_or(&"deepseek-session.md");
                let mut out = String::new();
                out.push_str(&format!("# DeepSeek Chat Session ({})\n\n", self.model));
                for msg in &self.messages {
                    let text = msg.content.as_deref().unwrap_or("");
                    out.push_str(&format!("### {}\n\n{}\n\n", msg.role.to_uppercase(), text));
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
                            self.messages.push(AgentMessage::user(format!(
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
                        self.messages.pop();
                        if let Some(user_msg) = self.messages.last().cloned() {
                            if user_msg.role == "user" {
                                self.messages.pop();
                                if let Some(ref text) = user_msg.content {
                                    println!("{} Retrying last prompt...", "↻".yellow());
                                    self.process_chat_turn(text).await;
                                }
                            }
                        }
                    }
                } else {
                    println!("Nothing to retry.");
                }
            }
            "/status" | "/info" => {
                println!("{}", "DeepSeek CLI Session Status:".bold().cyan());
                println!("  Active Model:   {}", self.model.bold().green());
                println!(
                    "  Auto-compact:   {}",
                    if self.config.auto_compact {
                        "enabled".green()
                    } else {
                        "disabled".yellow()
                    }
                );
                println!(
                    "  Context Limit:  {} chars threshold",
                    self.config.compact_threshold_chars
                );
                println!("  Base URL:       {}", self.config.base_url.dimmed());
                println!("  Temperature:    {}", self.config.temperature);
                println!("  Messages:       {} turns", self.messages.len());
                println!("  Tokens Used:    {} total", self.session_stats.total_tokens);
                println!("  Cost:           {}", format_cost_line(&self.session_stats, &self.model));
            }
            "/help" | "/h" => {
                println!("{}", "Available REPL Commands:".bold());
                println!("  {}                Show this help message", "/help".yellow());
                println!("  {}       Interactive model picker (v3 / r1)", "/model, /models".yellow());
                println!("  {}                Show exact session cost in $ and savings", "/cost".yellow());
                println!("  {}             Compact history immediately", "/compact".yellow());
                println!("  {}         Toggle automatic context compaction", "/autocompact".yellow());
                println!("  {}                Show session status & context usage", "/status".yellow());
                println!("  {}                Toggle auto-approval for coding actions", "/auto".yellow());
                println!("  {}            List saved sessions", "/sessions".yellow());
                println!("  {}                Copy last response to clipboard", "/copy".yellow());
                println!("  {}               Show token usage stats", "/tokens".yellow());
                println!("  {}               Regenerate last response", "/retry".yellow());
                println!("  {}               Clear conversation context", "/clear".yellow());
                println!("  {}       Set temperature (0.0 to 2.0)", "/temp <num>".yellow());
                println!("  {}       Update system prompt", "/system <text>".yellow());
                println!("  {}        Export session to Markdown", "/save [file]".yellow());
                println!("  {}        Load file into context", "/load <file>".yellow());
                println!("  {}                Exit REPL", "/exit".yellow());
            }
            _ => {
                println!("{}: Unknown command '{}'. Type {} for help.", "Notice".yellow(), cmd, "/help".cyan());
            }
        }

        Ok(true)
    }

    async fn interactive_model_select(&mut self) {
        println!("\n{}", "Select Active Model:".bold().cyan());
        let v3_active = self.model.contains("chat");
        let r1_active = self.model.contains("reasoner");

        println!(
            "  {} DeepSeek-V3  {} - Blazing fast, top coding skills{}",
            "[1]".yellow().bold(),
            "(deepseek-chat)".bold(),
            if v3_active { " [active]".green().bold() } else { "".normal() }
        );
        println!(
            "  {} DeepSeek-R1  {} - DeepThink reasoning, complex math & logic{}",
            "[2]".yellow().bold(),
            "(deepseek-reasoner)".bold(),
            if r1_active { " [active]".green().bold() } else { "".normal() }
        );
        print!("\n{} ", "Select [1 or 2, Enter to keep current]:".bold());
        let _ = stdout().flush();

        let mut choice = String::new();
        let _ = stdin().read_line(&mut choice);

        match choice.trim() {
            "1" | "v3" | "chat" => {
                self.model = "deepseek-chat".to_string();
                self.session_data.model = self.model.clone();
                println!("Switched to {}", self.model.bold().green());
            }
            "2" | "r1" | "reasoner" => {
                self.model = "deepseek-reasoner".to_string();
                self.session_data.model = self.model.clone();
                println!("Switched to {}", self.model.bold().cyan());
            }
            _ => {
                println!("Keeping current model: {}", self.model.bold().cyan());
            }
        }
    }

    async fn check_and_auto_compact(&mut self) {
        if !self.config.auto_compact {
            return;
        }

        let total_chars: usize = self
            .messages
            .iter()
            .filter_map(|m| m.content.as_ref())
            .map(|c| c.len())
            .sum();

        if total_chars > self.config.compact_threshold_chars {
            println!(
                "{}",
                format!(
                    "🗜️ Context threshold reached (~{} chars) — auto-compacting history...",
                    total_chars
                )
                .dimmed()
            );
            let _ = compact_history(&self.client, &self.model, &mut self.messages).await;
        }
    }

    async fn process_agent_turn(&mut self, prompt: &str) {
        self.check_and_auto_compact().await;
        let mut agent = Agent::new(&self.client, self.model.clone(), self.auto_approve);
        agent.session.messages = self.messages.clone();

        match agent.run_task(prompt).await {
            Ok(answer) => {
                self.messages = agent.session.messages;
                self.last_assistant_response = Some(answer);
                self.session_stats.prompt_tokens += agent.session.stats.prompt_tokens;
                self.session_stats.completion_tokens += agent.session.stats.completion_tokens;
                self.session_stats.total_tokens += agent.session.stats.total_tokens;
                self.session_stats.cache_hit_tokens += agent.session.stats.cache_hit_tokens;
                self.session_stats.cache_miss_tokens += agent.session.stats.cache_miss_tokens;
            }
            Err(e) => {
                eprintln!("\n{}: {}", "Agent error".red().bold(), e);
            }
        }
    }

    async fn process_chat_turn(&mut self, prompt: &str) {
        self.check_and_auto_compact().await;
        self.messages.push(AgentMessage::user(prompt));

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
                println!("  {}", format_cost_line(&stats, &self.model));
                self.last_assistant_response = Some(content.clone());
                self.messages.push(AgentMessage::assistant(content));

                self.session_stats.prompt_tokens += stats.prompt_tokens;
                self.session_stats.completion_tokens += stats.completion_tokens;
                self.session_stats.total_tokens += stats.total_tokens;
                self.session_stats.cache_hit_tokens += stats.cache_hit_tokens;
                self.session_stats.cache_miss_tokens += stats.cache_miss_tokens;
            }
            Err(e) => {
                renderer.finish(None);
                eprintln!("\n{}: {}", "Error".red().bold(), e);
                self.messages.pop();
            }
        }
    }
}
