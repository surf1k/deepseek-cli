use crate::api::{AgentMessage, GenerationStats};
use anyhow::{bail, Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub id: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub model: String,
    pub messages: Vec<AgentMessage>,
    pub stats: GenerationStats,
}

impl SessionData {
    pub fn new(model: String, messages: Vec<AgentMessage>) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let id = format!("{:x}", now);

        Self {
            id,
            created_at: now,
            updated_at: now,
            model,
            messages,
            stats: GenerationStats::default(),
        }
    }
}

pub fn sessions_dir() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "deepseek", "deepseek-cli")
        .context("Failed to determine project directories")?;
    let dir = proj_dirs.config_dir().join("sessions");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn save_session(session: &SessionData) -> Result<()> {
    let dir = sessions_dir()?;
    let path = dir.join(format!("{}.json", session.id));
    let latest_path = dir.join("latest.json");

    let json = serde_json::to_string_pretty(session)?;
    fs::write(&path, &json)?;
    fs::write(&latest_path, &json)?;
    Ok(())
}

pub fn load_latest_session() -> Result<SessionData> {
    let dir = sessions_dir()?;
    let latest_path = dir.join("latest.json");
    if !latest_path.exists() {
        bail!("No active previous session found to resume.");
    }
    let content = fs::read_to_string(latest_path)?;
    let session: SessionData = serde_json::from_str(&content)?;
    Ok(session)
}

pub fn list_sessions() -> Result<Vec<SessionData>> {
    let dir = sessions_dir()?;
    let mut sessions = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if name == "latest" {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(session) = serde_json::from_str::<SessionData>(&content) {
                    sessions.push(session);
                }
            }
        }
    }

    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sessions)
}
