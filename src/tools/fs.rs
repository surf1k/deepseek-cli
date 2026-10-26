use super::diff::render_diff;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

pub fn read_file(path_str: &str, start_line: Option<usize>, end_line: Option<usize>) -> Result<String> {
    let path = Path::new(path_str);
    if !path.exists() {
        bail!("File not found: {}", path_str);
    }
    if !path.is_file() {
        bail!("Path is not a regular file: {}", path_str);
    }

    let bytes = fs::read(path).with_context(|| format!("Failed to read {}", path_str))?;
    if bytes.iter().take(1024).any(|&b| b == 0) {
        bail!("Cannot read binary file: {}", path_str);
    }

    let content = String::from_utf8_lossy(&bytes);
    let all_lines: Vec<&str> = content.lines().collect();

    let total_lines = all_lines.len();
    let start = start_line.unwrap_or(1).saturating_sub(1);
    let end = end_line.unwrap_or(total_lines).min(total_lines);

    if start >= total_lines {
        bail!(
            "Start line {} exceeds total lines in file ({})",
            start + 1,
            total_lines
        );
    }

    let mut out = String::new();
    let max_lines_to_return = 800;
    let actual_end = end.min(start + max_lines_to_return);

    for (idx, line) in all_lines[start..actual_end].iter().enumerate() {
        let line_num = start + 1 + idx;
        out.push_str(&format!("{:4} | {}\n", line_num, line));
    }

    if actual_end < end {
        out.push_str(&format!(
            "\n[Output truncated: showing lines {}-{} of {}. Total file lines: {}]\n",
            start + 1,
            actual_end,
            end,
            total_lines
        ));
    }

    Ok(out)
}

pub fn write_file(path_str: &str, content: &str, overwrite: bool) -> Result<String> {
    let path = Path::new(path_str);

    if path.exists() && !overwrite {
        bail!(
            "File already exists: {}. Pass overwrite: true or use edit_file instead.",
            path_str
        );
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directories for {}", path_str))?;
    }

    fs::write(path, content).with_context(|| format!("Failed to write to {}", path_str))?;

    Ok(format!(
        "Successfully wrote {} bytes to {}",
        content.len(),
        path_str
    ))
}

pub fn edit_file(path_str: &str, target: &str, replacement: &str) -> Result<(String, String)> {
    let path = Path::new(path_str);
    if !path.exists() {
        bail!("File not found: {}", path_str);
    }

    let original = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file for editing: {}", path_str))?;

    if target.is_empty() {
        bail!("Target string to replace cannot be empty");
    }

    if !original.contains(target) {
        bail!(
            "Target string was not found in {}. Ensure whitespace and indentation match exactly.",
            path_str
        );
    }

    let matches_count = original.matches(target).count();
    let new_content = original.replacen(target, replacement, 1);
    let diff = render_diff(&original, &new_content, path_str);

    fs::write(path, &new_content)
        .with_context(|| format!("Failed to write edits to {}", path_str))?;

    let msg = if matches_count > 1 {
        format!(
            "Successfully replaced 1 of {} occurrences of target in {}",
            matches_count, path_str
        )
    } else {
        format!("Successfully edited {}", path_str)
    };

    Ok((msg, diff))
}
