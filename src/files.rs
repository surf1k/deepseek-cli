use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct FileAttachment {
    pub path: PathBuf,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
}

impl FileAttachment {
    pub fn parse(spec: &str) -> Result<Self> {
        let parts: Vec<&str> = spec.split(':').collect();
        let path = PathBuf::from(parts[0]);

        if parts.len() == 1 {
            return Ok(Self {
                path,
                line_start: None,
                line_end: None,
            });
        }

        let range_str = parts[1];
        if range_str.contains('-') {
            let rparts: Vec<&str> = range_str.split('-').collect();
            let start = rparts[0].parse::<usize>().ok();
            let end = rparts.get(1).and_then(|s| s.parse::<usize>().ok());
            Ok(Self {
                path,
                line_start: start,
                line_end: end,
            })
        } else if let Ok(line) = range_str.parse::<usize>() {
            Ok(Self {
                path,
                line_start: Some(line),
                line_end: Some(line),
            })
        } else {
            Ok(Self {
                path,
                line_start: None,
                line_end: None,
            })
        }
    }
}

pub fn load_file_contexts(specs: &[String]) -> Result<String> {
    let mut out = String::new();

    for spec in specs {
        // Check if glob
        if spec.contains('*') || spec.contains('?') {
            for entry in glob::glob(spec)? {
                let path = entry?;
                if path.is_file() {
                    append_file(&path, None, None, &mut out)?;
                }
            }
            continue;
        }

        let attachment = FileAttachment::parse(spec)?;
        if attachment.path.is_dir() {
            append_directory(&attachment.path, &mut out)?;
        } else if attachment.path.is_file() {
            append_file(
                &attachment.path,
                attachment.line_start,
                attachment.line_end,
                &mut out,
            )?;
        } else {
            bail!("Path not found: {}", attachment.path.display());
        }
    }

    Ok(out)
}

fn append_file(
    path: &Path,
    start: Option<usize>,
    end: Option<usize>,
    out: &mut String,
) -> Result<()> {
    let bytes = fs::read(path)?;
    // Check if binary (contains null byte in first 1024 bytes)
    if bytes.iter().take(1024).any(|&b| b == 0) {
        return Ok(());
    }

    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = content.lines().collect();

    let s = start.unwrap_or(1).saturating_sub(1);
    let e = end.unwrap_or(lines.len()).min(lines.len());

    let header = if start.is_some() || end.is_some() {
        format!(
            "\n--- File: {} (lines {}-{}) ---\n",
            path.display(),
            s + 1,
            e
        )
    } else {
        format!("\n--- File: {} ---\n", path.display())
    };

    out.push_str(&header);
    for (idx, line) in lines[s..e].iter().enumerate() {
        let line_num = s + 1 + idx;
        out.push_str(&format!("{:4} | {}\n", line_num, line));
    }
    out.push_str("--- End of file ---\n");

    Ok(())
}

fn append_directory(dir: &Path, out: &mut String) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }

        if path.is_dir() {
            append_directory(&path, out)?;
        } else if path.is_file() {
            append_file(&path, None, None, out)?;
        }
    }
    Ok(())
}
