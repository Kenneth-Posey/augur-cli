//! Unit tests for `event_mapper::map_session_event`.

use augur_domain::NumericNewtype;
use augur_domain::newtypes::TokenCount;
use augur_domain::plan_tree::{NodeStatus, PlanNodeId};
use augur_domain::string_newtypes::{OutputText, StringNewtype, ToolCallId, ToolName};
use augur_domain::types::AgentOutput;
use augur_provider_copilot_sdk::actors::executor::commands::SessionEvent;
use augur_provider_copilot_sdk::actors::executor::event_mapper::map_session_event;

const EXPECTED_INPUT_TOKENS: u64 = 120;
const EXPECTED_OUTPUT_TOKENS: u64 = 45;

#[test]
fn map_session_event_delta_produces_token() {
    let event = SessionEvent::AssistantMessageDelta {
        content: OutputText::new("hello"),
    };
    let result = map_session_event(&event);
    match result {
        Some(AgentOutput::Token(text)) => assert_eq!(text.as_str(), "hello"),
        other => panic!("expected Token, got {:?}", other),
    }
}

#[test]
fn map_session_event_idle_produces_turn_complete() {
    let event = SessionEvent::SessionIdle;
    let result = map_session_event(&event);
    assert!(matches!(result, Some(AgentOutput::TurnComplete)));
}

#[test]
fn map_session_event_error_produces_error_output() {
    let event = SessionEvent::SessionError {
        message: "timeout".to_owned(),
    };
    let result = map_session_event(&event);
    match result {
        Some(AgentOutput::Error(msg)) => assert_eq!(&*msg, "timeout"),
        other => panic!("expected Error, got {:?}", other),
    }
}

#[test]
fn map_session_event_tool_start_produces_tool_call_started() {
    let event = SessionEvent::ToolExecutionStart {
        tool_name: ToolName::new("bash"),
        args: serde_json::json!({"cmd": "ls"}),
    };
    let result = map_session_event(&event);
    match result {
        Some(AgentOutput::ToolCallStarted { name, args }) => {
            assert_eq!(name.as_str(), "bash");
            assert_eq!(args, serde_json::json!({"cmd": "ls"}));
        }
        other => panic!("expected ToolCallStarted, got {:?}", other),
    }
}

#[test]
fn map_session_event_unknown_produces_none() {
    let event = SessionEvent::Unknown;
    let result = map_session_event(&event);
    assert!(result.is_none());
}

#[test]
fn map_session_event_plan_node_done_produces_update() {
    let event = SessionEvent::PlanNodeUpdated {
        node_id: PlanNodeId::new("step-1"),
        status: "done".to_owned(),
        notes: None,
    };
    let result = map_session_event(&event);
    match result {
        Some(AgentOutput::PlanNodeUpdate {
            node_id,
            status,
            notes,
        }) => {
            assert_eq!(node_id.as_str(), "step-1");
            assert_eq!(status, NodeStatus::Done);
            assert!(notes.is_none());
        }
        other => panic!("expected PlanNodeUpdate, got {:?}", other),
    }
}

#[test]
fn map_session_event_plan_node_in_progress_produces_update() {
    let event = SessionEvent::PlanNodeUpdated {
        node_id: PlanNodeId::new("step-1"),
        status: "in_progress".to_owned(),
        notes: None,
    };
    match map_session_event(&event) {
        Some(AgentOutput::PlanNodeUpdate { status, .. }) => {
            assert_eq!(status, NodeStatus::InProgress);
        }
        other => panic!("expected PlanNodeUpdate, got {:?}", other),
    }
}

#[test]
fn map_session_event_plan_node_unknown_status_falls_back_to_pending() {
    let event = SessionEvent::PlanNodeUpdated {
        node_id: PlanNodeId::new("step-1"),
        status: "mystery".to_owned(),
        notes: None,
    };
    match map_session_event(&event) {
        Some(AgentOutput::PlanNodeUpdate { status, .. }) => {
            assert_eq!(status, NodeStatus::Pending);
        }
        other => panic!("expected PlanNodeUpdate, got {:?}", other),
    }
}

#[test]
fn map_session_event_plan_node_failed_carries_notes() {
    let event = SessionEvent::PlanNodeUpdated {
        node_id: PlanNodeId::new("step-2"),
        status: "failed".to_owned(),
        notes: Some("compile error".to_owned()),
    };
    let result = map_session_event(&event);
    match result {
        Some(AgentOutput::PlanNodeUpdate { status, notes, .. }) => {
            assert_eq!(status, NodeStatus::Failed("compile error".into()));
            assert_eq!(notes.as_deref(), Some("compile error"));
        }
        other => panic!("expected PlanNodeUpdate, got {:?}", other),
    }
}

#[test]
fn map_session_event_plan_node_failed_without_notes_uses_empty_reason() {
    let event = SessionEvent::PlanNodeUpdated {
        node_id: PlanNodeId::new("step-3"),
        status: "failed".to_owned(),
        notes: None,
    };
    match map_session_event(&event) {
        Some(AgentOutput::PlanNodeUpdate { status, notes, .. }) => {
            assert_eq!(status, NodeStatus::Failed("".into()));
            assert!(notes.is_none());
        }
        other => panic!("expected PlanNodeUpdate, got {:?}", other),
    }
}

#[test]
fn map_session_event_tool_complete_produces_none() {
    let event = SessionEvent::ToolExecutionComplete {
        tool_call_id: ToolCallId::new("call-1"),
    };
    let result = map_session_event(&event);
    assert!(result.is_none());
}

#[test]
fn map_session_event_message_complete_produces_done() {
    let event = SessionEvent::AssistantMessageComplete;
    let result = map_session_event(&event);
    assert!(matches!(result, Some(AgentOutput::Done)));
}

#[test]
fn map_session_event_tool_start_null_args() {
    let event = SessionEvent::ToolExecutionStart {
        tool_name: ToolName::new("file_read"),
        args: serde_json::Value::Null,
    };
    let result = map_session_event(&event);
    match result {
        Some(AgentOutput::ToolCallStarted { name, args }) => {
            assert_eq!(name.as_str(), "file_read");
            assert_eq!(args, serde_json::Value::Null);
        }
        other => panic!("expected ToolCallStarted, got {:?}", other),
    }
}

#[test]
fn map_session_event_usage_produces_usage_update() {
    let event = SessionEvent::AssistantUsage {
        input_tokens: Some(TokenCount::new(EXPECTED_INPUT_TOKENS)),
        output_tokens: Some(TokenCount::new(EXPECTED_OUTPUT_TOKENS)),
        cache_read_tokens: None,
    };
    let result = map_session_event(&event);
    assert!(
        matches!(result, Some(AgentOutput::UsageUpdate { .. })),
        "expected UsageUpdate, got {:?}",
        result
    );
}

#[test]
fn map_session_event_usage_absent_fields_produces_none_counts() {
    let event = SessionEvent::AssistantUsage {
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
    };
    let result = map_session_event(&event);
    assert!(
        matches!(result, Some(AgentOutput::UsageUpdate { .. })),
        "expected UsageUpdate, got {:?}",
        result
    );
}

#[test]
fn map_session_event_assistant_intent_produces_intent_message() {
    let event = SessionEvent::AssistantIntent {
        intent: OutputText::new("I will read the config file"),
    };
    match map_session_event(&event) {
        Some(AgentOutput::IntentMessage(text)) => {
            assert_eq!(text.as_str(), "I will read the config file");
        }
        other => panic!("expected IntentMessage, got {:?}", other),
    }
}

#[test]
fn map_session_event_tool_progress_produces_tool_progress() {
    let event = SessionEvent::ToolProgress {
        tool_call_id: ToolCallId::new("tc-42"),
        message: OutputText::new("searching 5 directories..."),
    };
    match map_session_event(&event) {
        Some(AgentOutput::ToolProgress {
            tool_call_id,
            message,
        }) => {
            assert_eq!(tool_call_id.as_str(), "tc-42");
            assert_eq!(message.as_str(), "searching 5 directories...");
        }
        other => panic!("expected ToolProgress, got {:?}", other),
    }
}

#[test]
fn map_session_event_tool_partial_result_produces_tool_partial_result() {
    let event = SessionEvent::ToolPartialResult {
        tool_call_id: ToolCallId::new("tc-55"),
        output: OutputText::new("partial output\nmore output"),
    };
    match map_session_event(&event) {
        Some(AgentOutput::ToolPartialResult {
            tool_call_id,
            output,
        }) => {
            assert_eq!(tool_call_id.as_str(), "tc-55");
            assert_eq!(output.as_str(), "partial output\nmore output");
        }
        other => panic!("expected ToolPartialResult, got {:?}", other),
    }
}
