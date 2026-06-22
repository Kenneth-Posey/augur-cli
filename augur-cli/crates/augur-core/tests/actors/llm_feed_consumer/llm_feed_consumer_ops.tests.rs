//! Unit tests for LLM feed consumer ops: chunk classification and routing.

use augur_core::actors::llm_feed_consumer::llm_feed_consumer_ops::{
    LlmFeedOutputChannels, classify_chunk, route_chunk,
};
use augur_domain::domain::feeds::LlmFeedTag;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolCallId, ToolName};
use augur_domain::domain::types::StreamChunk;
use tokio::sync::mpsc;

/// Verifies that a Token chunk is classified as UserChunk.
#[test]
fn test_classify_chunk_user_chunk() {
    let chunk = StreamChunk::Token(OutputText::new("hello".to_owned()));
    assert_eq!(classify_chunk(&chunk), LlmFeedTag::UserChunk);
}

/// Verifies that a ToolCall chunk is classified as ToolRequest.
#[test]
fn test_classify_chunk_tool_request() {
    let chunk = StreamChunk::ToolCall {
        id: ToolCallId::new("call_classify"),
        name: ToolName::new("my_tool".to_owned()),
        arguments: serde_json::json!({"key": "value"}),
    };
    assert_eq!(classify_chunk(&chunk), LlmFeedTag::ToolRequest);
}

/// Verifies that an Error chunk is classified as Error.
#[test]
fn test_classify_chunk_error() {
    let chunk = StreamChunk::Error(OutputText::new("something went wrong".to_owned()));
    assert_eq!(classify_chunk(&chunk), LlmFeedTag::Error);
}

/// Verifies that a Done chunk (control signal) passes through as UserChunk.
#[test]
fn test_classify_chunk_done_passes_through_as_user_chunk() {
    let chunk = StreamChunk::Done;
    assert_eq!(classify_chunk(&chunk), LlmFeedTag::UserChunk);
}

/// Verifies that a Token chunk is routed to the user_chunk channel with UserChunk tag.
#[test]
fn test_route_chunk_sends_to_user_channel() {
    let (bg_tx, _bg_rx) = mpsc::channel(8);
    let (thinking_tx, _thinking_rx) = mpsc::channel(8);
    let (user_tx, mut user_rx) = mpsc::channel(8);
    let (tool_tx, _tool_rx) = mpsc::channel(8);

    let outputs = LlmFeedOutputChannels::builder()
        .bg_agent_tx(bg_tx)
        .thinking_tx(thinking_tx)
        .user_chunk_tx(user_tx)
        .tool_request_tx(tool_tx)
        .build();

    let chunk = StreamChunk::Token(OutputText::new("hello".to_owned()));
    route_chunk(chunk, &outputs);

    let msg = user_rx
        .try_recv()
        .expect("user channel must have a message");
    assert_eq!(msg.tag, LlmFeedTag::UserChunk);
}

/// Verifies that a ToolCall chunk is routed to the tool_request channel with ToolRequest tag.
#[test]
fn test_route_chunk_sends_to_tool_channel() {
    let (bg_tx, _bg_rx) = mpsc::channel(8);
    let (thinking_tx, _thinking_rx) = mpsc::channel(8);
    let (user_tx, _user_rx) = mpsc::channel(8);
    let (tool_tx, mut tool_rx) = mpsc::channel(8);

    let outputs = LlmFeedOutputChannels::builder()
        .bg_agent_tx(bg_tx)
        .thinking_tx(thinking_tx)
        .user_chunk_tx(user_tx)
        .tool_request_tx(tool_tx)
        .build();

    let chunk = StreamChunk::ToolCall {
        id: ToolCallId::new("call_route"),
        name: ToolName::new("shell_exec".to_owned()),
        arguments: serde_json::json!({"cmd": "ls"}),
    };
    route_chunk(chunk, &outputs);

    let msg = tool_rx
        .try_recv()
        .expect("tool channel must have a message");
    assert_eq!(msg.tag, LlmFeedTag::ToolRequest);
}

/// Verifies that an Error chunk is routed to the user_chunk channel with Error tag.
#[test]
fn test_route_chunk_error_sent_to_user_channel() {
    let (bg_tx, _bg_rx) = mpsc::channel(8);
    let (thinking_tx, _thinking_rx) = mpsc::channel(8);
    let (user_tx, mut user_rx) = mpsc::channel(8);
    let (tool_tx, _tool_rx) = mpsc::channel(8);

    let outputs = LlmFeedOutputChannels::builder()
        .bg_agent_tx(bg_tx)
        .thinking_tx(thinking_tx)
        .user_chunk_tx(user_tx)
        .tool_request_tx(tool_tx)
        .build();

    let chunk = StreamChunk::Error(OutputText::new("stream error".to_owned()));
    route_chunk(chunk, &outputs);

    let msg = user_rx
        .try_recv()
        .expect("user channel must have error message");
    assert_eq!(msg.tag, LlmFeedTag::Error);
}
