use crate::api::GenerationStats;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{stdout, Write};
use std::time::{Duration, Instant};

pub fn print_banner() {
    println!(
        "{}",
        r#"
   ___               ____            _      ____ _     ___ 
  / _ \___ ___ ___  / __/__ ___ k   | |    / ___| |   |_ _|
 / // / -_) -_) _ \_\ \/ -_) -_) k  | |   | |   | |    | | 
/____/\__/\__/_//_/___/\__/\__/_/_/ |_|   |_|   |___| |___|
"#.bright_blue().bold()
    );
    println!(
        "  {} {} │ {} {}",
        "DeepSeek CLI".bold().white(),
        "v0.1.0".dimmed(),
        "Models:".dimmed(),
        "deepseek-chat (V3), deepseek-reasoner (R1)".cyan()
    );
    println!(
        "  {} for commands, {} to copy, {} to exit.\n",
        "/help".yellow(),
        "/copy".yellow(),
        "/exit".yellow()
    );
}

pub fn create_spinner(msg: &'static str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(70));
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan.bold} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.set_message(msg);
    pb
}

pub struct StreamRenderer {
    in_reasoning: bool,
    reasoning_start: Option<Instant>,
    reasoning_duration: Option<Duration>,
    reasoning_char_count: usize,
}

impl StreamRenderer {
    pub fn new() -> Self {
        Self {
            in_reasoning: false,
            reasoning_start: None,
            reasoning_duration: None,
            reasoning_char_count: 0,
        }
    }

    pub fn on_reasoning_chunk(&mut self, text: &str) {
        if !self.in_reasoning {
            self.in_reasoning = true;
            self.reasoning_start = Some(Instant::now());
            println!("{}", "╭─── 🧠 DeepThink (Reasoning Process) ──────────────────".dimmed().cyan());
        }
        self.reasoning_char_count += text.chars().count();
        print!("{}", text.dimmed());
        let _ = stdout().flush();
    }

    pub fn on_content_chunk(&mut self, text: &str) {
        if self.in_reasoning {
            self.in_reasoning = false;
            let duration_text = if let Some(start) = self.reasoning_start {
                let d = start.elapsed();
                self.reasoning_duration = Some(d);
                format!("Thought for {:.1}s", d.as_secs_f64())
            } else {
                "Thinking finished".to_string()
            };
            let token_est = (self.reasoning_char_count / 4).max(1);
            println!(
                "\n{}",
                format!(
                    "╰─── {} (~{} tokens) ──────────────────────────",
                    duration_text, token_est
                )
                .dimmed()
                .cyan()
            );
            println!("{}", "╭─── 💬 Response ───────────────────────────────────────".dimmed().blue());
        }
        print!("{}", text);
        let _ = stdout().flush();
    }

    pub fn finish(&mut self, stats: Option<&GenerationStats>) {
        if self.in_reasoning {
            println!("\n{}", "╰──────────────────────────────────────────────────────".dimmed().cyan());
            self.in_reasoning = false;
        } else {
            println!();
        }

        if let Some(s) = stats {
            print_stats_bar(s);
        }
        println!();
        let _ = stdout().flush();
    }
}

pub fn print_stats_bar(stats: &GenerationStats) {
    let mut parts = Vec::new();

    if stats.completion_tokens > 0 {
        parts.push(format!("⚡ {} tokens", stats.completion_tokens).white().bold().to_string());
    }

    if stats.tokens_per_sec > 0.0 {
        parts.push(format!("{:.1} tok/s", stats.tokens_per_sec).cyan().to_string());
    }

    parts.push(format!("⏱ {:.2}s", stats.elapsed.as_secs_f64()).dimmed().to_string());

    if stats.cache_hit_tokens > 0 || stats.cache_miss_tokens > 0 {
        let total_cache = stats.cache_hit_tokens + stats.cache_miss_tokens;
        if total_cache > 0 {
            let hit_percent = (stats.cache_hit_tokens as f64 / total_cache as f64) * 100.0;
            parts.push(
                format!("💾 Cache: {:.0}% hit ({} tok)", hit_percent, stats.cache_hit_tokens)
                    .green()
                    .to_string(),
            );
        }
    }

    if !parts.is_empty() {
        println!("{}", format!("  [{}]", parts.join(" · ")).dimmed());
    }
}

pub fn copy_to_clipboard(text: &str) -> bool {
    // 1. Try system clipboard via arboard
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if cb.set_text(text.to_string()).is_ok() {
            return true;
        }
    }

    // 2. Fallback to OSC 52 sequence (works over SSH, Tmux, Wayland/X11, Ghostty, Alacritty, Kitty)
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;
    let b64 = BASE64.encode(text.as_bytes());
    let osc = format!("\x1b]52;c;{}\x07", b64);
    print!("{}", osc);
    let _ = stdout().flush();
    true
}

#[allow(dead_code)]
pub fn render_markdown(text: &str) {
    termimad::print_text(text);
}
