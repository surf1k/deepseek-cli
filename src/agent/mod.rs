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
    depth: usize,
    role: String,
    cumulative_stats: GenerationStats,
}

impl<'a> Agent<'a> {
    pub fn new(client: &'a DeepSeekClient, model: String, auto_approve: bool) -> Self {
        Self {
            client,
            model,
            auto_approve,
            max_turns: 30,
            depth: 0,
            role: "Primary Agent".to_string(),
            cumulative_stats: GenerationStats::default(),
        }
    }

    pub fn new_subagent(
        client: &'a DeepSeekClient,
        model: String,
        auto_approve: bool,
        depth: usize,
        role: String,
    ) -> Self {
        Self {
            client,
            model,
            auto_approve,
            max_turns: 15,
            depth,
            role,
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

    pub async fn run_task(&mut self, task: &str) -> Result<String> {
        let indent = "  ".repeat(self.depth);

        let mut system_prompt = format!(
            "You are DeepSeek Code ({}), an expert autonomous coding assistant and pair programmer.\n\
You have tools to read, edit, and write files, run shell commands, search the codebase, and delegate to subagents.\n\n\
GUIDELINES:\n\
1. Read before writing: use read_file or grep to inspect code before modifying it.\n\
2. Be surgical: prefer edit_file with exact targets over rewriting entire files.\n\
3. Verify your work: run bash commands (cargo test, pytest, npm test, etc.) to confirm your changes.\n\
4. Explain concisely what you are doing before calling tools.\n\
5. If a subtask is complex or requires broad codebase scanning, delegate to a focused subagent with spawn_subagent.\n\
6. When the user's task is fully resolved, conclude with a brief wrap-up summary.",
            self.role
        );

        if self.depth == 0 {
            if let Some(mem) = Self::load_project_memory() {
                system_prompt.push_str(&mem);
            }
        }

        let mut messages = vec![
            AgentMessage::system(system_prompt),
            AgentMessage::user(task),
        ];

        let allow_subagents = self.depth < 2;
        let tools = get_tool_definitions(allow_subagents);

        if self.depth == 0 {
            println!("\n{} Starting autonomous task: {}", "⚡".yellow().bold(), task.bold());
            if self.auto_approve {
                println!("{} Auto-approval enabled (--auto)", "🛡️".green());
            } else {
                println!("{} Interactive approval active for mutating actions (edit/write/bash)", "🛡️".cyan());
            }
            println!();
        }

        let mut final_answer = String::new();

        for turn in 1..=self.max_turns {
            print!("{}{} Turn {}/{} ... ", indent, "❯".bright_blue().bold(), turn, self.max_turns);
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
                    println!("\n{}{}\n", indent, text.replace('\n', &format!("\n{}", indent)));
                    final_answer = text.clone();
                }
            }

            // Check if model called any tools
            if let Some(ref tool_calls) = response_msg.tool_calls {
                if tool_calls.is_empty() {
                    // No tool calls, agent has finished
                    if self.depth == 0 {
                        println!("\n{} Task completed successfully!", "✓".green().bold());
                        print_stats_bar(&self.cumulative_stats);
                    }
                    return Ok(final_answer);
                }

                // Append assistant's turn with tool_calls to conversation history
                messages.push(response_msg.clone());

                for call in tool_calls {
                    let fn_name = &call.function.name;
                    let fn_args = &call.function.arguments;

                    println!("{}{} {} {}", indent, "⚙".yellow(), fn_name.cyan().bold(), fn_args.dimmed());

                    // Handle spawn_subagent specially
                    if fn_name == "spawn_subagent" {
                        let args: serde_json::Value = serde_json::from_str(fn_args).unwrap_or_default();
                        let sub_task = args.get("task").and_then(|v| v.as_str()).unwrap_or("");
                        let sub_role = args.get("role").and_then(|v| v.as_str()).unwrap_or("Subagent");

                        println!(
                            "\n{}╭─── 🤖 Spawning Subagent [{}] ───────────────────",
                            indent,
                            sub_role.purple().bold()
                        );
                        println!("{}│ Task: {}", indent, sub_task.dimmed());

                        let mut subagent = Agent::new_subagent(
                            self.client,
                            self.model.clone(),
                            self.auto_approve,
                            self.depth + 1,
                            sub_role.to_string(),
                        );

                        let sub_res = Box::pin(subagent.run_task(sub_task)).await;
                        let report = match sub_res {
                            Ok(text) => {
                                println!(
                                    "{}╰── ✓ Subagent [{}] completed successfully.\n",
                                    indent,
                                    sub_role.purple()
                                );
                                text
                            }
                            Err(e) => {
                                eprintln!(
                                    "{}╰── ✗ Subagent [{}] failed: {}\n",
                                    indent,
                                    sub_role.purple(),
                                    e
                                );
                                format!("Subagent error: {}", e)
                            }
                        };

                        // Accumulate subagent stats into parent stats
                        self.cumulative_stats.prompt_tokens += subagent.cumulative_stats.prompt_tokens;
                        self.cumulative_stats.completion_tokens += subagent.cumulative_stats.completion_tokens;
                        self.cumulative_stats.total_tokens += subagent.cumulative_stats.total_tokens;
                        self.cumulative_stats.cache_hit_tokens += subagent.cumulative_stats.cache_hit_tokens;
                        self.cumulative_stats.cache_miss_tokens += subagent.cumulative_stats.cache_miss_tokens;

                        messages.push(AgentMessage::tool_result(
                            &call.id,
                            fn_name,
                            report,
                        ));
                        continue;
                    }

                    // Permission check for local tools
                    let approved = self.check_permission(fn_name, fn_args, &indent)?;

                    let tool_result_content = if approved {
                        match execute_tool(fn_name, fn_args) {
                            Ok(res) => {
                                if let Some(ref diff) = res.diff {
                                    println!("\n{}\n", diff);
                                }
                                println!("{}{} {}", indent, "✓".green(), res.summary.dimmed());
                                res.content
                            }
                            Err(e) => {
                                eprintln!("{}{} Tool failed: {}", indent, "✗".red(), e);
                                format!("Tool execution error: {}", e)
                            }
                        }
                    } else {
                        println!("{}{} Action rejected by user.", indent, "✗".red());
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
                if self.depth == 0 {
                    println!("\n{} Task completed!", "✓".green().bold());
                    print_stats_bar(&self.cumulative_stats);
                }
                return Ok(final_answer);
            }
        }

        if self.depth == 0 {
            println!(
                "\n{} Reached maximum turns limit ({}). Stopping agent loop.",
                "⚠".yellow(),
                self.max_turns
            );
            print_stats_bar(&self.cumulative_stats);
        }
        Ok(final_answer)
    }

    fn check_permission(&mut self, tool_name: &str, _args_json: &str, indent: &str) -> Result<bool> {
        if self.auto_approve {
            return Ok(true);
        }

        // Read-only tools require no interactive confirmation
        if matches!(tool_name, "read_file" | "glob" | "grep") {
            return Ok(true);
        }

        // Mutating tools require confirmation
        print!(
            "{}{} Approve {}? [y] Yes | [n] No | [a] Always allow: ",
            indent,
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
                println!("{}{} Auto-approval enabled for remaining actions.", indent, "✓".green());
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
