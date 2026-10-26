use crate::api::{AgentMessage, DeepSeekClient, GenerationStats};
use crate::tools::{execute_tool, get_tool_definitions};
use crate::ui::print_stats_bar;
use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::io::{stdin, stdout, Write};
use std::path::Path;

pub struct Agent<'a> {
    client: &'a DeepSeekClient,
    model: String,
    auto_approve: bool,
    max_turns: usize,
    cumulative_stats: GenerationStats,
}

impl<'a> Agent<'a> {
    pub fn new(client: &'a DeepSeekClient, model: String, auto_approve: bool) -> Self {
        Self {
            client,
            model,
            auto_approve,
            max_turns: 30,
            cumulative_stats: GenerationStats::default(),
        }
    }

    pub fn load_project_memory() -> Option<String> {
        let candidates = ["DEEPSEEK.md", "CLAUDE.md", "AGENTS.md", "rules.md"];
        for name in candidates {
            let path = Path::new(name);
            if path.exists() && path.is_file() {
                if let Ok(content) = fs::read_to_string(path) {
                    println!("{} Loaded project memory from {}", "🧠".cyan(), name.bold());
                    return Some(format!(
                        "\n--- Project Instructions ({}) ---\n{}\n--- End Instructions ---\n",
                        name, content
                    ));
                }
            }
        }
        None
    }

    pub async fn run_task(&mut self, task: &str) -> Result<()> {
        let mut system_prompt = "You are DeepSeek Code, an expert autonomous coding assistant and pair programmer.\n\
You have tools to read, edit, and write files, run shell commands, and search the codebase.\n\n\
GUIDELINES:\n\
1. Read before writing: use read_file or grep to inspect code before modifying it.\n\
2. Be surgical: prefer edit_file with exact targets over rewriting entire files.\n\
3. Verify your work: run bash commands (cargo test, pytest, npm test, etc.) to confirm your changes.\n\
4. Explain concisely what you are doing before calling tools.\n\
5. When the user's task is fully resolved, conclude with a brief wrap-up summary.".to_string();

        if let Some(mem) = Self::load_project_memory() {
            system_prompt.push_str(&mem);
        }

        let mut messages = vec![
            AgentMessage::system(system_prompt),
            AgentMessage::user(task),
        ];

        let tools = get_tool_definitions();

        println!("\n{} Starting autonomous task: {}", "⚡".yellow().bold(), task.bold());
        if self.auto_approve {
            println!("{} Auto-approval enabled (--auto)", "🛡️".green());
        } else {
            println!("{} Interactive approval active for mutating actions (edit/write/bash)", "🛡️".cyan());
        }
        println!();

        for turn in 1..=self.max_turns {
            print!("{} Turn {}/{} ... ", "❯".bright_blue().bold(), turn, self.max_turns);
            let _ = stdout().flush();

            let (response_msg, stats) = self
                .client
                .chat_with_tools(&self.model, &messages, Some(tools.clone()), Some(0.2))
                .await
                .context("Failed during agent communication turn")?;

            self.cumulative_stats.prompt_tokens += stats.prompt_tokens;
            self.cumulative_stats.completion_tokens += stats.completion_tokens;
            self.cumulative_stats.total_tokens += stats.total_tokens;
            self.cumulative_stats.cache_hit_tokens += stats.cache_hit_tokens;
            self.cumulative_stats.cache_miss_tokens += stats.cache_miss_tokens;

            println!("{}", "done".dimmed());

            // Print textual response if present
            if let Some(ref text) = response_msg.content {
                if !text.trim().is_empty() {
                    println!("\n{}\n", text);
                }
            }

            // Check if model called any tools
            if let Some(ref tool_calls) = response_msg.tool_calls {
                if tool_calls.is_empty() {
                    // No tool calls, agent has finished
                    println!("\n{} Task completed successfully!", "✓".green().bold());
                    print_stats_bar(&self.cumulative_stats);
                    return Ok(());
                }

                // Append assistant's turn with tool_calls to conversation history
                messages.push(response_msg.clone());

                for call in tool_calls {
                    let fn_name = &call.function.name;
                    let fn_args = &call.function.arguments;

                    println!("  {} {} {}", "⚙".yellow(), fn_name.cyan().bold(), fn_args.dimmed());

                    // Permission check
                    let approved = self.check_permission(fn_name, fn_args)?;

                    let tool_result_content = if approved {
                        match execute_tool(fn_name, fn_args) {
                            Ok(res) => {
                                if let Some(ref diff) = res.diff {
                                    println!("\n{}\n", diff);
                                }
                                println!("    {} {}", "✓".green(), res.summary.dimmed());
                                res.content
                            }
                            Err(e) => {
                                eprintln!("    {} Tool failed: {}", "✗".red(), e);
                                format!("Tool execution error: {}", e)
                            }
                        }
                    } else {
                        println!("    {} Action rejected by user.", "✗".red());
                        "Action rejected by user.".to_string()
                    };

                    messages.push(AgentMessage::tool_result(
                        &call.id,
                        fn_name,
                        tool_result_content,
                    ));
                }
            } else {
                // No tool calls, task complete
                println!("\n{} Task completed!", "✓".green().bold());
                print_stats_bar(&self.cumulative_stats);
                return Ok(());
            }
        }

        println!("\n{} Reached maximum turns limit ({}). Stopping agent loop.", "⚠".yellow(), self.max_turns);
        print_stats_bar(&self.cumulative_stats);
        Ok(())
    }

    fn check_permission(&mut self, tool_name: &str, _args_json: &str) -> Result<bool> {
        if self.auto_approve {
            return Ok(true);
        }

        // Read-only tools require no interactive confirmation
        if matches!(tool_name, "read_file" | "glob" | "grep") {
            return Ok(true);
        }

        // Mutating tools require confirmation
        print!(
            "    {} Approve {}? [y] Yes | [n] No | [a] Always allow: ",
            "Permission?".bold().yellow(),
            tool_name.cyan()
        );
        let _ = stdout().flush();

        let mut input = String::new();
        stdin().read_line(&mut input)?;
        let choice = input.trim().to_lowercase();

        match choice.as_str() {
            "y" | "yes" | "" => Ok(true),
            "a" | "all" => {
                self.auto_approve = true;
                println!("    {} Auto-approval enabled for remaining actions.", "✓".green());
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
