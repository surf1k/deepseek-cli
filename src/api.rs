use anyhow::{bail, Context, Result};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
    #[serde(default)]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
}

pub struct DeepSeekClient {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

pub enum StreamChunk {
    Reasoning(String),
    Content(String),
    Done,
}

impl DeepSeekClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    pub async fn stream_chat<F>(
        &self,
        model: &str,
        messages: &[Message],
        temperature: Option<f32>,
        mut on_chunk: F,
    ) -> Result<(String, String)>
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
            temperature,
        };

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .context("Failed to send request to DeepSeek API")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            bail!("DeepSeek API returned error [{}]: {}", status, error_text);
        }

        let mut stream = response.bytes_stream().eventsource();
        let mut full_reasoning = String::new();
        let mut full_content = String::new();

        while let Some(event_res) = stream.next().await {
            let event = match event_res {
                Ok(e) => e,
                Err(err) => bail!("Error parsing SSE stream: {}", err),
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

        Ok((full_reasoning, full_content))
    }
}
