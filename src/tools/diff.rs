use colored::*;
use similar::{ChangeTag, TextDiff};

pub fn render_diff(old: &str, new: &str, path: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    let mut out = String::new();

    out.push_str(&format!("--- a/{}\n", path).red().to_string());
    out.push_str(&format!("+++ b/{}\n", path).green().to_string());

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                out.push_str(&format!("-{}", change).red().to_string());
            }
            ChangeTag::Insert => {
                out.push_str(&format!("+{}", change).green().to_string());
            }
            ChangeTag::Equal => {
                out.push_str(&format!(" {}", change).dimmed().to_string());
            }
        }
    }

    out
}
