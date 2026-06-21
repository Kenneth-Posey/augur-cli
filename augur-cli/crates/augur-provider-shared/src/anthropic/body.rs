//! Anthropic request-body construction helpers.

use crate::request_context::ToolDefinition;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::domain::types::{CacheSnapshot, CachedTier, Message, Role};

/// Build Anthropic `system` content blocks with per-tier `cache_control` markers.
pub(super) fn build_system_blocks(
    system_text: &OutputText,
    snapshot: &CacheSnapshot,
) -> serde_json::Value {
    let mut blocks: Vec<serde_json::Value> =
        vec![serde_json::json!({ "type": "text", "text": system_text.as_str() })];
    for tier in &snapshot.tiers {
        let text = tier_text(tier);
        blocks.push(serde_json::json!({
            "type": "text",
            "text": text,
            "cache_control": { "type": "ephemeral" }
        }));
    }
    serde_json::Value::Array(blocks)
}

/// Render a single cache tier as a text block.
fn tier_text(tier: &CachedTier) -> String {
    tier.files
        .iter()
        .map(|f| format!("// === {} ===\n{}", f.path.display(), f.content.as_str()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Extract the system message text from a message slice.
pub(super) fn extract_system_text(messages: &[Message]) -> OutputText {
    messages
        .iter()
        .find(|m| m.role == Role::System)
        .map(|m| m.content.clone())
        .unwrap_or_else(|| OutputText::new(""))
}

/// Convert domain `Message` slice to the Anthropic `messages` array JSON shape.
pub(super) fn to_anthropic_messages(messages: &[Message]) -> serde_json::Value {
    let arr: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|msg| {
            let (role, content) = match msg.role {
                Role::User => ("user", msg.content.as_str().to_owned()),
                Role::Assistant => ("assistant", msg.content.as_str().to_owned()),
                Role::Tool => ("user", format!("[tool_result]\n{}", msg.content.as_str())),
                Role::System => unreachable!("system messages filtered above"),
            };
            serde_json::json!({ "role": role, "content": content })
        })
        .collect();
    serde_json::Value::Array(arr)
}

/// Convert `ToolDefinition` slice to the Anthropic `tools` array JSON shape.
pub(super) fn to_anthropic_tools(tools: &[ToolDefinition]) -> serde_json::Value {
    let arr: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name.as_str(),
                "description": &t.description,
                "input_schema": &t.parameters,
            })
        })
        .collect();
    serde_json::Value::Array(arr)
}
