use crate::api::{GenerationStats, StreamChunk};
use crate::pow::solve_pow;
use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUserInfo {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

pub struct WebClient {
    client: reqwest::Client,
    pub user_token: String,
    base_url: String,
}

impl WebClient {
    pub fn new(user_token: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(90))
            .build()
            .unwrap_or_default();

        Self {
            client,
            user_token: user_token.trim().to_string(),
            base_url: "https://chat.deepseek.com".to_string(),
        }
    }

    fn build_headers(&self, pow_resp: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("accept", HeaderValue::from_static("*/*"));
        headers.insert(
            "accept-language",
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert(
            "origin",
            HeaderValue::from_static("https://chat.deepseek.com"),
        );
        headers.insert(
            "referer",
            HeaderValue::from_static("https://chat.deepseek.com/"),
        );
        headers.insert(
            "user-agent",
            HeaderValue::from_static("Dalvik/2.1.0 (Linux; U; Android 14; Pixel 7)"),
        );
        headers.insert("x-client-platform", HeaderValue::from_static("android"));
        headers.insert("x-client-version", HeaderValue::from_static("2.4.5"));
        headers.insert("x-client-locale", HeaderValue::from_static("en_US"));
        headers.insert(
            "x-client-bundle-id",
            HeaderValue::from_static("com.deepseek.chat"),
        );
        headers.insert("x-client-timezone-offset", HeaderValue::from_static("19800"));

        if !self.user_token.is_empty() {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", self.user_token)) {
                headers.insert("authorization", val);
            }
        }

        if let Some(pow) = pow_resp {
            if let Ok(val) = HeaderValue::from_str(pow) {
                headers.insert("x-ds-pow-response", val);
            }
        }

        headers
    }

    pub async fn get_current_user(&self) -> Result<WebUserInfo> {
        let url = format!("{}/api/v0/users/current", self.base_url);
        let res = self
            .client
            .get(&url)
            .headers(self.build_headers(None))
            .send()
            .await
            .context("Failed to connect to DeepSeek Web Chat")?;

        let json: Value = res.json().await.context("Failed to parse user info")?;
        let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        if code != 0 {
            let msg = json
                .get("msg")
                .and_then(|v| v.as_str())
                .unwrap_or("Unauthorized");
            bail!("DeepSeek Web Login failed: {}", msg);
        }

        let data = json.get("data").context("Missing user data")?;
        let id = data
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let name = data.get("name").and_then(|v| v.as_str()).map(String::from);
        let email = data.get("email").and_then(|v| v.as_str()).map(String::from);

        Ok(WebUserInfo { id, name, email })
    }

    pub async fn solve_pow_challenge(&self, target_path: &str) -> Result<String> {
        let url = format!("{}/api/v0/chat/create_pow_challenge", self.base_url);
        let payload = serde_json::json!({ "target_path": target_path });

        let res = self
            .client
            .post(&url)
            .headers(self.build_headers(None))
            .json(&payload)
            .send()
            .await
            .context("Failed to request PoW challenge")?;

        let json: Value = res
            .json()
            .await
            .context("Failed to parse PoW challenge response")?;

        let challenge_obj = json
            .pointer("/data/biz_data/challenge")
            .context("PoW challenge missing from response")?;

        let ch_hex = challenge_obj
            .get("challenge")
            .and_then(|v| v.as_str())
            .context("Missing challenge hex")?;
        let salt = challenge_obj
            .get("salt")
            .and_then(|v| v.as_str())
            .context("Missing salt")?;
        let expire_at = challenge_obj
            .get("expire_at")
            .and_then(|v| v.as_u64())
            .context("Missing expire_at")?;
        let difficulty = challenge_obj
            .get("difficulty")
            .and_then(|v| v.as_u64())
            .context("Missing difficulty")?;
        let signature = challenge_obj
            .get("signature")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let answer = solve_pow(ch_hex, salt, expire_at, difficulty)
            .context("Failed to solve Proof-of-Work challenge")?;

        let solution = serde_json::json!({
            "algorithm": "deepseek-v1",
            "challenge": ch_hex,
            "salt": salt,
            "answer": answer,
            "signature": signature,
            "target_path": target_path,
        });

        Ok(BASE64.encode(serde_json::to_string(&solution)?))
    }

    pub async fn create_chat_session(&self) -> Result<String> {
        let url = format!("{}/api/v0/chat_session/create", self.base_url);
        let res = self
            .client
            .post(&url)
            .headers(self.build_headers(None))
            .json(&serde_json::json!({}))
            .send()
            .await
            .context("Failed to create chat session")?;

        let json: Value = res.json().await.context("Failed to parse session create")?;
        let id = json
            .pointer("/data/biz_data/id")
            .and_then(|v| v.as_str())
            .context("Missing chat session ID")?;

        Ok(id.to_string())
    }

    pub async fn stream_chat<F>(
        &self,
        session_id: &str,
        prompt: &str,
        thinking: bool,
        mut on_chunk: F,
    ) -> Result<(String, String, GenerationStats)>
    where
        F: FnMut(StreamChunk),
    {
        let start_time = Instant::now();
        let target_path = "/api/v0/chat/completion";
        let pow_header = self.solve_pow_challenge(target_path).await?;

        let url = format!("{}{}", self.base_url, target_path);
        let payload = serde_json::json!({
            "chat_session_id": session_id,
            "parent_message_id": null,
            "model_type": null,
            "prompt": prompt,
            "ref_file_ids": [],
            "thinking_enabled": thinking,
            "search_enabled": false,
            "preempt": false,
            "action": null,
        });

        let res = self
            .client
            .post(&url)
            .headers(self.build_headers(Some(&pow_header)))
            .json(&payload)
            .send()
            .await
            .context("Failed to send message to DeepSeek Web Chat")?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            bail!("DeepSeek Web Chat error ({}): {}", status, err_body);
        }

        let mut stream = res.bytes_stream().eventsource();
        let mut full_reasoning = String::new();
        let mut full_content = String::new();

        while let Some(event) = stream.next().await {
            let event = event.context("Error reading web stream")?;
            if event.data == "[DONE]" {
                on_chunk(StreamChunk::Done);
                break;
            }

            if let Ok(json) = serde_json::from_str::<Value>(&event.data) {
                // Parse standard batch / delta chunks
                if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
                    for choice in choices {
                        if let Some(delta) = choice.get("delta") {
                            if let Some(reasoning) = delta
                                .get("reasoning_content")
                                .and_then(|v| v.as_str())
                            {
                                if !reasoning.is_empty() {
                                    full_reasoning.push_str(reasoning);
                                    on_chunk(StreamChunk::Reasoning(reasoning.to_string()));
                                }
                            }
                            if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                                if !content.is_empty() {
                                    full_content.push_str(content);
                                    on_chunk(StreamChunk::Content(content.to_string()));
                                }
                            }
                        }
                    }
                } else if let Some(v) = json.get("v").and_then(|v| v.as_str()) {
                    // Direct text chunk format
                    let p = json.get("p").and_then(|v| v.as_str()).unwrap_or("");
                    if p.contains("thinking") {
                        full_reasoning.push_str(v);
                        on_chunk(StreamChunk::Reasoning(v.to_string()));
                    } else {
                        full_content.push_str(v);
                        on_chunk(StreamChunk::Content(v.to_string()));
                    }
                }
            }
        }

        let elapsed = start_time.elapsed();
        let char_count = full_content.chars().count() + full_reasoning.chars().count();
        let estimated_tokens = (char_count / 4).max(1) as u32;

        let stats = GenerationStats {
            prompt_tokens: (prompt.chars().count() / 4).max(1) as u32,
            completion_tokens: estimated_tokens,
            total_tokens: (prompt.chars().count() / 4 + char_count / 4) as u32,
            tokens_per_sec: if elapsed.as_secs_f64() > 0.0 {
                estimated_tokens as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            },
            elapsed,
            reasoning_tokens_est: (full_reasoning.chars().count() / 4) as u32,
            ..Default::default()
        };

        Ok((full_reasoning, full_content, stats))
    }
}
