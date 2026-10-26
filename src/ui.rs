use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{stdout, Write};
use std::time::Duration;

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
        "  {} {} | {} {}",
        "DeepSeek CLI".bold().white(),
        "v0.1.0".dimmed(),
        "Models:".dimmed(),
        "deepseek-chat (V3), deepseek-reasoner (R1)".cyan()
    );
    println!("  Type {} for command list, {} to exit.\n", "/help".yellow(), "/exit".yellow());
}

pub fn create_spinner(msg: &'static str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.set_message(msg);
    pb
}

pub struct StreamRenderer {
    in_reasoning: bool,
}

impl StreamRenderer {
    pub fn new() -> Self {
        Self {
            in_reasoning: false,
        }
    }

    pub fn on_reasoning_chunk(&mut self, text: &str) {
        if !self.in_reasoning {
            self.in_reasoning = true;
            println!("{}", "┌─── DeepThink (Reasoning Process) ────────────".dimmed().cyan());
        }
        print!("{}", text.dimmed());
        let _ = stdout().flush();
    }

    pub fn on_content_chunk(&mut self, text: &str) {
        if self.in_reasoning {
            self.in_reasoning = false;
            println!("\n{}", "└─── Final Answer ─────────────────────────────".dimmed().cyan());
        }
        print!("{}", text);
        let _ = stdout().flush();
    }

    pub fn finish(&mut self) {
        if self.in_reasoning {
            println!("\n{}", "└──────────────────────────────────────────────".dimmed().cyan());
            self.in_reasoning = false;
        }
        println!();
        let _ = stdout().flush();
    }
}
