pub mod bash;
pub mod diff;
pub mod fs;
pub mod search;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub content: String,
    pub diff: Option<String>,
    pub is_destructive: bool,
    pub summary: String,
}

pub fn get_tool_definitions(allow_subagents: bool) -> Vec<serde_json::Value> {
    let mut tools = vec![
        json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read file contents with line numbers. Use this before editing a file.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative or absolute path of the file to read"
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "Optional start line number (1-based)"
                        },
                        "end_line": {
                            "type": "integer",
                            "description": "Optional end line number (inclusive)"
                        }
                    },
                    "required": ["path"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "edit_file",
                "description": "Surgically edit a file by replacing an exact existing block of text with new text. The target string must match character-by-character.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path of the file to edit"
                        },
                        "target": {
                            "type": "string",
                            "description": "The exact character sequence to replace, including exact whitespace and indentation"
                        },
                        "replacement": {
                            "type": "string",
                            "description": "The replacement content"
                        }
                    },
                    "required": ["path", "target", "replacement"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Create a new file or completely overwrite an existing file with the provided content.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path of the file to create or overwrite"
                        },
                        "content": {
                            "type": "string",
                            "description": "Complete contents of the file"
                        },
                        "overwrite": {
                            "type": "boolean",
                            "description": "Set to true to overwrite if file already exists"
                        }
                    },
                    "required": ["path", "content"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "bash",
                "description": "Execute a shell command in the workspace (build, test, git, inspect directory).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The exact shell command to run"
                        }
                    },
                    "required": ["command"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "glob",
                "description": "Find files matching a glob pattern (e.g. 'src/**/*.rs', '*.json'). Respects ignore files.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern to match files against"
                        }
                    },
                    "required": ["pattern"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "grep",
                "description": "Search for a regex or string pattern across files in the workspace.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Regex or substring to search for"
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory or file path to search inside (default: current directory)"
                        },
                        "case_insensitive": {
                            "type": "boolean",
                            "description": "Whether search should be case-insensitive (default: false)"
                        }
                    },
                    "required": ["query"]
                }
            }
        }),
    ];

    if allow_subagents {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "spawn_subagent",
                "description": "Spawn an autonomous subagent to investigate or solve a subtask in an isolated context (e.g. codebase exploration, testing, deep analysis). Returns the subagent's final report.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "Clear, specific, and actionable task description for the subagent"
                        },
                        "role": {
                            "type": "string",
                            "description": "Short role title, e.g. 'Codebase Researcher', 'Test Runner', 'Security Auditor'"
                        }
                    },
                    "required": ["task"]
                }
            }
        }));
    }

    tools
}

pub fn execute_tool(name: &str, args_json: &str) -> Result<ToolExecutionResult> {
    let args: serde_json::Value = serde_json::from_str(args_json)
        .unwrap_or_else(|_| json!({}));

    match name {
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let start = args.get("start_line").and_then(|v| v.as_u64()).map(|n| n as usize);
            let end = args.get("end_line").and_then(|v| v.as_u64()).map(|n| n as usize);
            let content = fs::read_file(path, start, end)?;
            Ok(ToolExecutionResult {
                content,
                diff: None,
                is_destructive: false,
                summary: format!("Read {}", path),
            })
        }
        "edit_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("");
            let replacement = args.get("replacement").and_then(|v| v.as_str()).unwrap_or("");
            let (msg, diff) = fs::edit_file(path, target, replacement)?;
            Ok(ToolExecutionResult {
                content: msg,
                diff: Some(diff),
                is_destructive: true,
                summary: format!("Edit {}", path),
            })
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let overwrite = args.get("overwrite").and_then(|v| v.as_bool()).unwrap_or(false);
            let res = fs::write_file(path, content, overwrite)?;
            Ok(ToolExecutionResult {
                content: res,
                diff: None,
                is_destructive: true,
                summary: format!("Write {}", path),
            })
        }
        "bash" => {
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let res = bash::run_bash(cmd, None)?;
            let content = if res.exit_code == 0 {
                if res.stdout.is_empty() && !res.stderr.is_empty() {
                    res.stderr
                } else {
                    res.stdout
                }
            } else {
                format!(
                    "Command exited with code {}:\nSTDOUT:\n{}\nSTDERR:\n{}",
                    res.exit_code, res.stdout, res.stderr
                )
            };
            Ok(ToolExecutionResult {
                content,
                diff: None,
                is_destructive: true,
                summary: format!("Run bash: {}", cmd),
            })
        }
        "glob" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let files = search::glob_files(pattern)?;
            let content = if files.is_empty() {
                "No matching files found.".to_string()
            } else {
                files.join("\n")
            };
            Ok(ToolExecutionResult {
                content,
                diff: None,
                is_destructive: false,
                summary: format!("Glob '{}'", pattern),
            })
        }
        "grep" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let path = args.get("path").and_then(|v| v.as_str());
            let ci = args.get("case_insensitive").and_then(|v| v.as_bool()).unwrap_or(false);
            let content = search::grep_search(query, path, ci)?;
            Ok(ToolExecutionResult {
                content,
                diff: None,
                is_destructive: false,
                summary: format!("Grep '{}'", query),
            })
        }
        unknown => bail!("Unknown tool: {}", unknown),
    }
}
