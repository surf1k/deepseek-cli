use anyhow::{Context, Result};
use std::process::Command;
use std::time::Instant;

pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub elapsed_ms: u128,
}

pub fn run_bash(command_str: &str, _timeout_ms: Option<u64>) -> Result<BashOutput> {
    let start = Instant::now();

    let output = Command::new("bash")
        .arg("-c")
        .arg(command_str)
        .output()
        .with_context(|| format!("Failed to execute bash command: {}", command_str))?;

    let elapsed_ms = start.elapsed().as_millis();
    let exit_code = output.status.code().unwrap_or(-1);

    let raw_stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let raw_stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Truncate output if it exceeds 30,000 characters to prevent context window explosion
    let max_len = 30000;
    let stdout = if raw_stdout.len() > max_len {
        let lines: Vec<&str> = raw_stdout.lines().collect();
        let tail_lines = lines.iter().rev().take(150).rev().cloned().collect::<Vec<&str>>().join("\n");
        format!(
            "[Output truncated to avoid overflowing context. Total length: {} bytes]\n... (truncated) ...\n{}",
            raw_stdout.len(),
            tail_lines
        )
    } else {
        raw_stdout
    };

    let stderr = if raw_stderr.len() > max_len {
        let lines: Vec<&str> = raw_stderr.lines().collect();
        let tail_lines = lines.iter().rev().take(100).rev().cloned().collect::<Vec<&str>>().join("\n");
        format!(
            "[Stderr truncated. Total length: {} bytes]\n... (truncated) ...\n{}",
            raw_stderr.len(),
            tail_lines
        )
    } else {
        raw_stderr
    };

    Ok(BashOutput {
        stdout,
        stderr,
        exit_code,
        elapsed_ms,
    })
}
