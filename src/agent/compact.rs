use crate::api::{AgentMessage, DeepSeekClient};
use anyhow::{Context, Result};
use colored::*;

pub async fn compact_history(
    client: &DeepSeekClient,
    model: &str,
    messages: &mut Vec<AgentMessage>,
) -> Result<String> {
    if messages.len() <= 3 {
        return Ok("Conversation is too short to compact.".to_string());
    }

    println!("{} Compacting conversation context...", "🗜️".yellow());

    // Separate system prompt and recent messages
    let mut history_to_summarize = String::new();
    let mut start_idx = 0;

    if let Some(first) = messages.first() {
        if first.role == "system" {
            start_idx = 1;
        }
    }

    // Keep the last 2 messages intact
    let end_idx = messages.len().saturating_sub(2);
    if end_idx <= start_idx {
        return Ok("Context already minimal.".to_string());
    }

    for msg in &messages[start_idx..end_idx] {
        let content = msg.content.as_deref().unwrap_or("[tool call / output]");
        history_to_summarize.push_str(&format!("{}: {}\n\n", msg.role.to_uppercase(), content));
    }

    let compact_prompt = vec![
        AgentMessage::system(
            "You are a technical context compression engine. \
Summarize the provided conversation history into a dense, concise Markdown summary. \
Highlight: \
1. The user's primary objectives. \
2. Key technical discoveries, decisions, and bugs resolved. \
3. Files read, edited, created, or tested. \
4. Current state of the work. \
Be factual, direct, and keep it under 400 words."
        ),
        AgentMessage::user(format!(
            "Please summarize this conversation trajectory:\n\n{}",
            history_to_summarize
        )),
    ];

    let (summary_msg, _) = client
        .chat_with_tools(model, &compact_prompt, None, Some(0.2))
        .await
        .context("Failed to generate compaction summary")?;

    let summary = summary_msg
        .content
        .unwrap_or_else(|| "No summary generated.".to_string());

    // Construct new compacted message list
    let mut new_messages = Vec::new();
    if start_idx == 1 {
        new_messages.push(messages[0].clone());
    }

    new_messages.push(AgentMessage::system(format!(
        "--- PREVIOUS CONTEXT (COMPACTED) ---\n{}\n--- END COMPACTED CONTEXT ---",
        summary
    )));

    // Append recent messages
    for msg in &messages[end_idx..] {
        new_messages.push(msg.clone());
    }

    let before_count = messages.len();
    let after_count = new_messages.len();
    *messages = new_messages;

    let result_msg = format!(
        "Context compacted: reduced {} messages to {} messages.",
        before_count, after_count
    );
    println!("{} {}", "✓".green(), result_msg);

    Ok(summary)
}
