use std::any::Any;
use std::sync::Arc;

use augur_domain::background_events::{
    BackgroundEventClassifier, BackgroundEventPriority, BackgroundPanelMode, FlushIntervalMs,
    QueueCapacity,
};
use augur_domain::newtypes::NumericNewtype;
use augur_domain::types::AgentFeedOutput;
use augur_domain::{StringNewtype, TokenTrackerCommand, TokenTrackerHandle};
use augur_provider_copilot_sdk::actors::copilot::background_feed_dispatcher::{
    StreamFeedConfig, stream_to_feed,
};
use copilot_sdk::SessionEventData;
use copilot_sdk::events::UserMessageData;
use futures_util::StreamExt;
use tokio::sync::mpsc;

struct AlwaysInformationalClassifier;

impl BackgroundEventClassifier for AlwaysInformationalClassifier {
    fn classify(&self, _raw_event: &dyn Any) -> Option<BackgroundEventPriority> {
        Some(BackgroundEventPriority::Informational)
    }
}

fn build_user_message_event(content: &str) -> SessionEventData {
    SessionEventData::UserMessage(UserMessageData {
        content: content.to_string(),
        transformed_content: None,
        attachments: None,
        source: None,
    })
}

#[tokio::test]
async fn stream_to_feed_emits_mapped_status_line_for_classified_event() {
    let (usage_tx, _usage_rx) = mpsc::channel::<TokenTrackerCommand>(8);
    let token_tracker = TokenTrackerHandle::new(usage_tx);
    let (tx, rx) = mpsc::channel(8);
    let config = StreamFeedConfig {
        mode: BackgroundPanelMode::Normal,
        max_queued_events: QueueCapacity::new(8),
        flush_interval_ms: FlushIntervalMs::new(10),
        token_tracker,
        classifier: Arc::new(AlwaysInformationalClassifier),
    };
    let mut stream = stream_to_feed(config, rx);

    tx.send(build_user_message_event("background ping"))
        .await
        .expect("event must send");
    drop(tx);

    match stream.next().await {
        Some(AgentFeedOutput::StatusLine(text)) => assert_eq!(text.as_str(), "→ background ping"),
        other => panic!("expected first mapped status line, got {other:?}"),
    }
}
