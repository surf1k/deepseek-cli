use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default = "default_true")]
    pub show_stats: bool,
    #[serde(default = "default_true")]
    pub auto_compact: bool,
    #[serde(default = "default_compact_threshold")]
    pub compact_threshold_chars: usize,
}

fn default_base_url() -> String {
    "https://api.deepseek.com".to_string()
}

fn default_model() -> String {
    "deepseek-chat".to_string()
}

fn default_temperature() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

fn default_compact_threshold() -> usize {
    40_000
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: default_base_url(),
            default_model: default_model(),
            temperature: default_temperature(),
            max_tokens: None,
            system_prompt: None,
            show_stats: true,
            auto_compact: true,
            compact_threshold_chars: default_compact_threshold(),
        }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "deepseek", "deepseek-cli")
            .context("Failed to determine configuration directory")?;
        let config_dir = proj_dirs.config_dir();
        Ok(config_dir.join("config.toml"))
    }

    pub fn history_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "deepseek", "deepseek-cli")
            .context("Failed to determine configuration directory")?;
        let data_dir = proj_dirs.data_dir();
        fs::create_dir_all(data_dir)?;
        Ok(data_dir.join("history.txt"))
    }

    pub fn load() -> Self {
        let path = match Self::config_path() {
            Ok(p) => p,
            Err(_) => return Self::default(),
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn resolve_api_key(&self) -> Option<String> {
        if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
            if !key.trim().is_empty() {
                return Some(key.trim().to_string());
            }
        }
        self.api_key.as_ref().map(|k| k.trim().to_string())
    }

    pub fn canonicalize_model(input: &str) -> String {
        match input.to_lowercase().as_str() {
            "v3" | "chat" | "deepseek-v3" | "deepseek-chat" => "deepseek-chat".to_string(),
            "r1" | "reasoner" | "deepseek-r1" | "deepseek-reasoner" | "think" => {
                "deepseek-reasoner".to_string()
            }
            other => other.to_string(),
        }
    }
}
