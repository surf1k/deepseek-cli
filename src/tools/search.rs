use anyhow::{bail, Result};
use regex::RegexBuilder;
use std::fs;
use std::path::Path;

pub fn glob_files(pattern: &str) -> Result<Vec<String>> {
    let mut results = Vec::new();

    for entry in glob::glob(pattern)? {
        let path = entry?;
        let path_str = path.display().to_string();

        if path_str.contains("/.git/")
            || path_str.contains("/target/")
            || path_str.contains("/node_modules/")
        {
            continue;
        }

        if path.is_file() {
            results.push(path_str);
        }
    }

    Ok(results)
}

pub fn grep_search(
    query_str: &str,
    target_dir: Option<&str>,
    case_insensitive: bool,
) -> Result<String> {
    let regex = match RegexBuilder::new(query_str)
        .case_insensitive(case_insensitive)
        .build()
    {
        Ok(r) => r,
        Err(e) => bail!("Invalid regex query '{}': {}", query_str, e),
    };

    let dir = Path::new(target_dir.unwrap_or("."));
    let mut matches = Vec::new();
    let max_matches = 100;

    walk_and_search(dir, &regex, &mut matches, max_matches)?;

    if matches.is_empty() {
        return Ok("No matching lines found.".to_string());
    }

    let mut out = matches.join("\n");
    if matches.len() >= max_matches {
        out.push_str(&format!(
            "\n[Reached maximum limit of {} matches; refine your query]",
            max_matches
        ));
    }

    Ok(out)
}

fn walk_and_search(
    dir: &Path,
    regex: &regex::Regex,
    matches: &mut Vec<String>,
    max_matches: usize,
) -> Result<()> {
    if matches.len() >= max_matches {
        return Ok(());
    }

    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        if matches.len() >= max_matches {
            break;
        }

        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }

        if path.is_dir() {
            walk_and_search(&path, regex, matches, max_matches)?;
        } else if path.is_file() {
            if let Ok(bytes) = fs::read(&path) {
                // Skip binary files
                if bytes.iter().take(1024).any(|&b| b == 0) {
                    continue;
                }

                let text = String::from_utf8_lossy(&bytes);
                for (idx, line) in text.lines().enumerate() {
                    if regex.is_match(line) {
                        matches.push(format!("{}:{}: {}", path.display(), idx + 1, line.trim()));
                        if matches.len() >= max_matches {
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
