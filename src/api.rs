use crate::tools::ToolCall;
use anyhow::{bail, Context, Result};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl AgentMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: Some(name.into()),
        }
    }
}

// Keep backwards-compatible Message alias
pub type Message = AgentMessage;

#[derive(Debug, Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: &'a [AgentMessage],
    stream: bool,
    stream_options: StreamOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct AgentToolCallRequest<'a> {
    model: &'a str,
    messages: &'a [AgentMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Deserialize, Default)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    #[serde(default)]
    delta: ChunkDelta,
    #[serde(default)]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
    #[serde(default)]
    pub prompt_cache_hit_tokens: Option<u32>,
    #[serde(default)]
    pub prompt_cache_miss_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatResponseChoice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponseChoice {
    pub message: AgentMessage,
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerationStats {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cache_hit_tokens: u32,
    pub cache_miss_tokens: u32,
    pub reasoning_tokens_est: u32,
    pub elapsed: Duration,
    pub tokens_per_sec: f64,
}

pub struct DeepSeekClient {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

pub enum StreamChunk {
    Reasoning(String),
    Content(String),
    #[allow(dead_code)]
    Usage(Usage),
    Done,
}

impl DeepSeekClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .connect_timeout(Duration::from_secs(15))
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    pub async fn chat_with_tools(
        &self,
        model: &str,
        messages: &[AgentMessage],
        tools: Option<Vec<serde_json::Value>>,
        temperature: Option<f32>,
    ) -> Result<(AgentMessage, GenerationStats)> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .context("Invalid characters in API key")?,
        );

        let body = AgentToolCallRequest {
            model,
            messages,
            tools,
            temperature,
        };

        let start_time = Instant::now();

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .context("Failed to connect to DeepSeek API endpoint")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            bail!("DeepSeek API error [{}]: {}", status, error_text);
        }

        let resp_body: ChatCompletionResponse = response
            .json()
            .await
            .context("Failed to parse DeepSeek response")?;

        let elapsed = start_time.elapsed();
        let mut stats = GenerationStats {
            elapsed,
            ..Default::default()
        };

        if let Some(u) = resp_body.usage {
            stats.prompt_tokens = u.prompt_tokens;
            stats.completion_tokens = u.completion_tokens;
            stats.total_tokens = u.total_tokens;
            stats.cache_hit_tokens = u.prompt_cache_hit_tokens.unwrap_or(0);
            stats.cache_miss_tokens = u.prompt_cache_miss_tokens.unwrap_or(0);
            if elapsed.as_secs_f64() > 0.0 && stats.completion_tokens > 0 {
                stats.tokens_per_sec = stats.completion_tokens as f64 / elapsed.as_secs_f64();
            }
        }

        let choice = resp_body
            .choices
            .into_iter()
            .next()
            .context("No completion choices returned by DeepSeek API")?;

        Ok((choice.message, stats))
    }

    pub async fn stream_chat<F>(
        &self,
        model: &str,
        messages: &[AgentMessage],
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        mut on_chunk: F,
    ) -> Result<(String, String, GenerationStats)>
    where
        F: FnMut(StreamChunk),
    {
        let url = format!("{}/chat/completions", self.base_url);

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .context("Invalid characters in API key")?,
        );

        let body = ChatCompletionRequest {
            model,
            messages,
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
            temperature,
            max_tokens,
        };

        let start_time = Instant::now();

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .context("Failed to connect to DeepSeek API endpoint")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            if status.as_u16() == 401 {
                bail!("Invalid API key. Please check your DEEPSEEK_API_KEY (401 Unauthorized)");
            } else if status.as_u16() == 402 {
                bail!("Insufficient balance on your DeepSeek account (402 Payment Required)");
            } else if status.as_u16() == 429 {
                bail!("DeepSeek API rate limit reached. Please wait a moment and try again (429 Rate Limit)");
            } else {
                bail!("DeepSeek API error [{}]: {}", status, error_text);
            }
        }

        let mut stream = response.bytes_stream().eventsource();
        let mut full_reasoning = String::new();
        let mut full_content = String::new();
        let mut final_usage: Option<Usage> = None;

        while let Some(event_res) = stream.next().await {
            let event = match event_res {
                Ok(e) => e,
                Err(err) => bail!("Stream disconnected: {}", err),
            };

            let data = event.data.trim();
            if data == "[DONE]" {
                on_chunk(StreamChunk::Done);
                break;
            }

            if data.is_empty() {
                continue;
            }

            let chunk: ChatCompletionChunk = match serde_json::from_str(data) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if let Some(usage) = chunk.usage {
                on_chunk(StreamChunk::Usage(usage.clone()));
                final_usage = Some(usage);
            }

            for choice in chunk.choices {
                if let Some(reasoning) = choice.delta.reasoning_content {
                    if !reasoning.is_empty() {
                        full_reasoning.push_str(&reasoning);
                        on_chunk(StreamChunk::Reasoning(reasoning));
                    }
                }

                if let Some(content) = choice.delta.content {
                    if !content.is_empty() {
                        full_content.push_str(&content);
                        on_chunk(StreamChunk::Content(content));
                    }
                }
            }
        }

        let elapsed = start_time.elapsed();
        let mut stats = GenerationStats {
            elapsed,
            ..Default::default()
        };

        if let Some(u) = final_usage {
            stats.prompt_tokens = u.prompt_tokens;
            stats.completion_tokens = u.completion_tokens;
            stats.total_tokens = u.total_tokens;
            stats.cache_hit_tokens = u.prompt_cache_hit_tokens.unwrap_or(0);
            stats.cache_miss_tokens = u.prompt_cache_miss_tokens.unwrap_or(0);
            if elapsed.as_secs_f64() > 0.0 && stats.completion_tokens > 0 {
                stats.tokens_per_sec = stats.completion_tokens as f64 / elapsed.as_secs_f64();
            }
        } else {
            let char_count = full_content.chars().count() + full_reasoning.chars().count();
            let estimated_tokens = (char_count / 4).max(1) as u32;
            stats.completion_tokens = estimated_tokens;
            if elapsed.as_secs_f64() > 0.0 {
                stats.tokens_per_sec = estimated_tokens as f64 / elapsed.as_secs_f64();
            }
        }

        stats.reasoning_tokens_est = (full_reasoning.chars().count() / 4) as u32;

        Ok((full_reasoning, full_content, stats))
    }
}
