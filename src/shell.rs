use crate::api::{DeepSeekClient, Message, StreamChunk};
use crate::config::Config;
use crate::ui::{copy_to_clipboard, create_spinner};
use anyhow::{Context, Result};
use colored::*;
use std::io::{stdin, stdout, Write};
use std::process::Command;

pub struct ShellHelper<'a> {
    client: &'a DeepSeekClient,
    model: String,
}

impl<'a> ShellHelper<'a> {
    pub fn new(client: &'a DeepSeekClient, cfg: &Config) -> Self {
        Self {
            client,
            model: cfg.default_model.clone(),
        }
    }

    pub async fn run(&self, query: &str) -> Result<()> {
        let os_info = std::env::consts::OS;
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string());
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        let system_prompt = format!(
            "You are a terminal command assistant for {} running {}. Working directory is {}.\n\
             Generate the exact shell command that satisfies the user's request.\n\
             Respond in strictly this two-line format:\n\
             COMMAND: <one-liner command>\n\
             EXPLANATION: <one clear sentence explaining the command>\n\
             Do NOT add markdown, quotes, or code fences.",
            os_info, shell, cwd
        );

        let messages = vec![
            Message::system(system_prompt),
            Message::user(query),
        ];

        let spinner = create_spinner("Formulating shell command...");
        let mut full_output = String::new();
        let mut first = true;

        let result = self
            .client
            .stream_chat(
                &self.model,
                &messages,
                Some(0.1),
                None,
                |chunk| {
                    if first {
                        first = false;
                        spinner.finish_and_clear();
                    }
                    if let StreamChunk::Content(text) = chunk {
                        full_output.push_str(&text);
                    }
                },
            )
            .await;

        spinner.finish_and_clear();
        result.context("Failed to generate shell command")?;

        let mut cmd_str = String::new();
        let mut explanation = String::new();

        for line in full_output.lines() {
            let trimmed = line.trim();
            if let Some(c) = trimmed.strip_prefix("COMMAND:") {
                cmd_str = c.trim().to_string();
            } else if let Some(e) = trimmed.strip_prefix("EXPLANATION:") {
                explanation = e.trim().to_string();
            } else if cmd_str.is_empty() && !trimmed.is_empty() {
                cmd_str = trimmed.to_string();
            }
        }

        if cmd_str.is_empty() {
            cmd_str = full_output.trim().to_string();
        }

        println!("\n{}", "╭─── Suggested Command ──────────────────────────────────".dimmed().cyan());
        println!("│  {}", cmd_str.bold().bright_green());
        if !explanation.is_empty() {
            println!("╰── {}", explanation.dimmed());
        } else {
            println!("{}", "╰────────────────────────────────────────────────────────".dimmed().cyan());
        }

        print!(
            "\n{} [y] Execute | [c] Copy to clipboard | [n] Cancel: ",
            "Action?".bold().yellow()
        );
        let _ = stdout().flush();

        let mut choice = String::new();
        stdin().read_line(&mut choice)?;
        let action = choice.trim().to_lowercase();

        match action.as_str() {
            "y" | "yes" => {
                println!("{} Executing...\n", "⚡".yellow());
                let mut child = Command::new("bash")
                    .arg("-c")
                    .arg(&cmd_str)
                    .spawn()
                    .context("Failed to spawn command")?;

                let status = child.wait()?;
                if status.success() {
                    println!("\n{} Command completed successfully.", "✓".green());
                } else {
                    println!("\n{} Command exited with status: {}", "✗".red(), status);
                }
            }
            "c" | "copy" => {
                copy_to_clipboard(&cmd_str);
                println!("{} Copied to clipboard!", "✓".green());
            }
            _ => {
                println!("Cancelled.");
            }
        }

        Ok(())
    }
}
