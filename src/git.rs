use crate::api::{DeepSeekClient, Message, StreamChunk};
use crate::config::Config;
use crate::ui::{create_spinner, StreamRenderer};
use anyhow::{bail, Context, Result};
use colored::*;
use std::io::{stdin, stdout, Write};
use std::process::Command;

pub struct GitAssistant<'a> {
    client: &'a DeepSeekClient,
    model: String,
    temperature: f32,
}

impl<'a> GitAssistant<'a> {
    pub fn new(client: &'a DeepSeekClient, cfg: &Config, model_override: Option<String>) -> Self {
        let model = model_override
            .map(|m| Config::canonicalize_model(&m))
            .unwrap_or_else(|| cfg.default_model.clone());

        Self {
            client,
            model,
            temperature: 0.2, // Low temperature for deterministic, high quality commit messages
        }
    }

    pub fn get_diff() -> Result<(String, bool)> {
        // Check git repository
        let repo_check = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .output();

        if repo_check.is_err() || !repo_check.unwrap().status.success() {
            bail!("Current directory is not inside a git repository");
        }

        // Try staged diff first
        let staged = Command::new("git")
            .args(["diff", "--cached"])
            .output()
            .context("Failed to run git diff --cached")?;

        let staged_diff = String::from_utf8_lossy(&staged.stdout).to_string();
        if !staged_diff.trim().is_empty() {
            return Ok((staged_diff, true));
        }

        // Fall back to unstaged diff
        let unstaged = Command::new("git")
            .args(["diff"])
            .output()
            .context("Failed to run git diff")?;

        let unstaged_diff = String::from_utf8_lossy(&unstaged.stdout).to_string();
        if !unstaged_diff.trim().is_empty() {
            return Ok((unstaged_diff, false));
        }

        bail!("No staged or unstaged changes detected to commit.");
    }

    pub async fn run_commit(&self) -> Result<()> {
        let (diff, is_staged) = Self::get_diff()?;

        if !is_staged {
            println!(
                "{} No staged changes found, analyzing unstaged diff ({} bytes)...",
                "ℹ".yellow(),
                diff.len()
            );
        } else {
            println!(
                "{} Analyzing staged diff ({} bytes)...",
                "ℹ".cyan(),
                diff.len()
            );
        }

        // Truncate diff if gigantic to avoid blowing token context
        let truncated_diff = if diff.len() > 30000 {
            format!("{}\n\n[Diff truncated to 30KB]", &diff[..30000])
        } else {
            diff
        };

        let system_prompt = "You are an expert Git commit assistant. \
Generate a clean, conventional commit message based on the provided diff. \
Use standard conventional commit types (feat, fix, refactor, docs, chore, test, perf). \
The first line MUST be concise (under 72 chars): `<type>(<scope>): <summary>`. \
If needed, include an empty line and bullet points for key details. \
Output ONLY the raw commit message. Do NOT wrap in quotes or markdown code blocks.";

        let user_prompt = format!("Generate a commit message for this git diff:\n\n```diff\n{}\n```", truncated_diff);

        loop {
            let messages = vec![
                Message::system(system_prompt),
                Message::user(&user_prompt),
            ];

            let spinner = create_spinner("Generating commit message with DeepSeek...");
            let mut proposed_message = String::new();
            let mut first_token = true;

            let result = self
                .client
                .stream_chat(
                    &self.model,
                    &messages,
                    Some(self.temperature),
                    None,
                    |chunk| {
                        if first_token {
                            first_token = false;
                            spinner.finish_and_clear();
                        }
                        if let StreamChunk::Content(text) = chunk {
                            proposed_message.push_str(&text);
                        }
                    },
                )
                .await;

            spinner.finish_and_clear();
            result.context("Failed to generate commit message")?;

            let trimmed_message = proposed_message.trim().to_string();

            println!("\n{}", "Proposed Commit Message:".bold().cyan());
            println!("──────────────────────────────────────────────");
            println!("{}", trimmed_message.green().bold());
            println!("──────────────────────────────────────────────");

            print!(
                "\n{} [c] Commit | [e] Edit | [r] Regenerate | [q] Cancel: ",
                "Action?".bold().yellow()
            );
            let _ = stdout().flush();

            let mut choice = String::new();
            stdin().read_line(&mut choice)?;
            let action = choice.trim().to_lowercase();

            match action.as_str() {
                "c" | "" => {
                    if !is_staged {
                        println!("{} Staging all modified files (git add -u)...", "→".cyan());
                        let _ = Command::new("git").args(["add", "-u"]).status();
                    }
                    let status = Command::new("git")
                        .args(["commit", "-m", &trimmed_message])
                        .status()
                        .context("Failed to execute git commit")?;

                    if status.success() {
                        println!("{} Commit created successfully!", "✓".green().bold());
                    } else {
                        eprintln!("{} Git commit failed.", "✗".red());
                    }
                    return Ok(());
                }
                "e" => {
                    // Open in temporary file with EDITOR
                    let temp_path = std::env::temp_dir().join("COMMIT_EDITMSG");
                    std::fs::write(&temp_path, &trimmed_message)?;
                    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
                    let _ = Command::new(editor).arg(&temp_path).status();
                    let edited = std::fs::read_to_string(&temp_path)?;
                    let final_msg = edited.trim();
                    if final_msg.is_empty() {
                        println!("Empty commit message. Cancelled.");
                        return Ok(());
                    }
                    if !is_staged {
                        let _ = Command::new("git").args(["add", "-u"]).status();
                    }
                    let status = Command::new("git")
                        .args(["commit", "-m", final_msg])
                        .status()?;
                    if status.success() {
                        println!("{} Commit created successfully!", "✓".green().bold());
                    }
                    return Ok(());
                }
                "r" => {
                    println!("Regenerating...\n");
                    continue;
                }
                "q" => {
                    println!("Commit cancelled.");
                    return Ok(());
                }
                _ => {
                    println!("Invalid option.");
                }
            }
        }
    }

    pub async fn run_review(&self) -> Result<()> {
        let (diff, _) = Self::get_diff()?;

        println!(
            "{} Reviewing git diff ({} bytes) with {}...",
            "🔍".cyan(),
            diff.len(),
            self.model.bold()
        );

        let system_prompt = "You are a principal software engineer and security auditor conducting a thorough code review. \
Analyze the provided git diff. Focus on: \
1. Bugs, race conditions, memory issues, or edge cases. \
2. Security vulnerabilities. \
3. Performance optimizations and architectural feedback. \
Format your response in clean Markdown with clear section headings. If the code is solid, praise good practices.";

        let user_prompt = format!("Please review this diff:\n\n```diff\n{}\n```", diff);
        let messages = vec![
            Message::system(system_prompt),
            Message::user(&user_prompt),
        ];

        let mut renderer = StreamRenderer::new();
        let spinner = create_spinner("Analyzing code diff...");
        let mut first = true;

        let result = self
            .client
            .stream_chat(
                &self.model,
                &messages,
                Some(self.temperature),
                None,
                |chunk| {
                    if first {
                        first = false;
                        spinner.finish_and_clear();
                    }
                    match chunk {
                        StreamChunk::Reasoning(text) => renderer.on_reasoning_chunk(&text),
                        StreamChunk::Content(text) => renderer.on_content_chunk(&text),
                        StreamChunk::Done => {}
                        _ => {}
                    }
                },
            )
            .await;

        spinner.finish_and_clear();
        let (_, _, stats) = result?;
        renderer.finish(Some(&stats));

        Ok(())
    }
}
