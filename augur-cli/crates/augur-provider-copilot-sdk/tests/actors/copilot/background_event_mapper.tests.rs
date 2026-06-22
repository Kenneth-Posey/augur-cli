use augur_domain::background_events::{BackgroundEventPriority, BackgroundPanelMode};
use augur_domain::types::AgentFeedOutput;
use augur_domain::StringNewtype;
use augur_provider_copilot_sdk::actors::copilot::background_event_mapper::map_background_event;
use copilot_sdk::events::{SessionResumeData, SessionStartData, UserMessageData};
use copilot_sdk::SessionEventData;

fn session_start_event() -> SessionEventData {
    SessionEventData::SessionStart(SessionStartData {
        session_id: "s1".to_string(),
        version: 1.0,
        producer: "test".to_string(),
        copilot_version: "1.0".to_string(),
        start_time: "2024-01-01".to_string(),
        selected_model: None,
    })
}

fn user_message_event(content: &str) -> SessionEventData {
    SessionEventData::UserMessage(UserMessageData {
        content: content.to_string(),
        transformed_content: None,
        attachments: None,
        source: None,
    })
}

fn session_resume_event() -> SessionEventData {
    SessionEventData::SessionResume(SessionResumeData {
        resume_time: "1000".to_string(),
        event_count: 5.0,
    })
}

#[test]
fn maps_critical_session_start_to_status_line() {
    let mapped = map_background_event(
        &session_start_event(),
        BackgroundEventPriority::Critical,
        BackgroundPanelMode::Normal,
    );
    match mapped {
        Some(AgentFeedOutput::StatusLine(text)) => assert_eq!(text.as_str(), "Session started"),
        other => panic!("expected Some(StatusLine), got {other:?}"),
    }
}

#[test]
fn maps_informational_user_message_with_arrow_prefix() {
    let mapped = map_background_event(
        &user_message_event("hello"),
        BackgroundEventPriority::Informational,
        BackgroundPanelMode::Normal,
    );
    match mapped {
        Some(AgentFeedOutput::StatusLine(text)) => assert_eq!(text.as_str(), "→ hello"),
        other => panic!("expected Some(StatusLine), got {other:?}"),
    }
}

#[test]
fn filters_debug_event_in_normal_mode() {
    let mapped = map_background_event(
        &session_resume_event(),
        BackgroundEventPriority::Debug,
        BackgroundPanelMode::Normal,
    );
    assert!(
        mapped.is_none(),
        "debug events must be filtered in normal mode"
    );
}
