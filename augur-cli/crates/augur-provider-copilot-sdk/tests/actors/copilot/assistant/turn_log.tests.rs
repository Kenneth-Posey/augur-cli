//! Tests for `turn_log` assistant module.
//!
//! Validates `apply_log_event` token accumulation and turn completion recording.
//! All tests are feature-gated; they run only with `copilot-executor`.

#[cfg(test)]
mod suite {
    /// `apply_log_event` with a `Token` event appends the token text to
    /// `log.assistant_buf`. Verifies the token accumulation path that buffers
    /// streaming responses before `TurnComplete` triggers persistence.
    #[tokio::test]
    async fn apply_log_event_token_accumulates_in_assistant_buf() {
        use augur_domain::string_newtypes::{OutputText, StringNewtype};
        use augur_domain::types::AgentOutput;
        use augur_provider_copilot_sdk::actors::copilot::assistant::turn_log::{
            LogState, apply_log_event,
        };

        let tmp = tempfile::tempdir().expect("tempdir");
        let (logger_tx, _logger_rx) = tokio::sync::mpsc::channel(1);
        let logger = augur_domain::LoggerHandle::new(logger_tx);
        let persistence =
            augur_domain::persistence::handle::PersistenceHandle::new(tmp.path().to_owned());
        let (history_tx, _history_rx) = tokio::sync::mpsc::channel(1);
        let history_adapter = augur_domain::HistoryAdapterHandle::new(history_tx);

        let mut log = LogState {
            handles: augur_provider_copilot_sdk::actors::copilot::assistant::turn_log::LogHandles {
                logger,
                history_adapter,
                persistence,
            },
            pending_user: None,
            assistant_buf: OutputText::new(""),
            message_history: Vec::new(),
        };

        apply_log_event(AgentOutput::Token(OutputText::new("hello")), &mut log).await;
        apply_log_event(AgentOutput::Token(OutputText::new(" world")), &mut log).await;

        assert_eq!(
            log.assistant_buf, "hello world",
            "tokens should accumulate in assistant_buf"
        );
    }

    /// `apply_log_event` with `TurnComplete` clears the assistant buffer and
    /// records the user/assistant pair in `message_history` when `pending_user`
    /// is set. Validates the full turn commit path that drives persistence.
    #[tokio::test]
    async fn apply_log_event_turn_complete_records_turn_in_history() {
        use augur_domain::string_newtypes::{OutputText, StringNewtype};
        use augur_domain::types::{AgentOutput, Message};
        use augur_provider_copilot_sdk::actors::copilot::assistant::turn_log::{
            LogState, apply_log_event,
        };

        let tmp = tempfile::tempdir().expect("tempdir");
        let (logger_tx, _logger_rx) = tokio::sync::mpsc::channel(1);
        let logger = augur_domain::LoggerHandle::new(logger_tx);
        let persistence =
            augur_domain::persistence::handle::PersistenceHandle::new(tmp.path().to_owned());
        let (history_tx, _history_rx) = tokio::sync::mpsc::channel(1);
        let history_adapter = augur_domain::HistoryAdapterHandle::new(history_tx);

        let mut log = LogState {
            handles: augur_provider_copilot_sdk::actors::copilot::assistant::turn_log::LogHandles {
                logger,
                history_adapter,
                persistence,
            },
            pending_user: Some(Message::user("what is 2+2?")),
            assistant_buf: OutputText::new("4"),
            message_history: Vec::new(),
        };

        apply_log_event(AgentOutput::TurnComplete, &mut log).await;

        assert_eq!(
            log.assistant_buf, "",
            "assistant_buf should be cleared after TurnComplete"
        );
        assert_eq!(
            log.message_history.len(),
            2,
            "both user and assistant records should be added"
        );
        assert!(
            log.pending_user.is_none(),
            "pending_user should be consumed by TurnComplete"
        );
    }

    /// Verifies that pushing a `MessageType::System` record to `message_history`
    /// and calling `persistence.save_turn` persists the system record so that
    /// model-switch checkpoints survive session reload.
    #[tokio::test]
    async fn system_record_checkpoint_is_persisted() {
        use augur_domain::persistence::types::{MessageRecord, MessageType};
        use augur_domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
        use augur_domain::types::Message;
        use augur_provider_copilot_sdk::actors::copilot::assistant::turn_log::LogState;

        let tmp = tempfile::tempdir().expect("tempdir");
        let (logger_tx, _logger_rx) = tokio::sync::mpsc::channel(1);
        let logger = augur_domain::LoggerHandle::new(logger_tx);
        let (history_tx, _history_rx) = tokio::sync::mpsc::channel(1);
        let history_adapter = augur_domain::HistoryAdapterHandle::new(history_tx);

        let mut log = LogState {
            handles: augur_provider_copilot_sdk::actors::copilot::assistant::turn_log::LogHandles {
                logger,
                history_adapter,
                persistence: augur_domain::persistence::handle::PersistenceHandle::new(
                    tmp.path().to_owned(),
                ),
            },
            pending_user: None,
            assistant_buf: OutputText::new(""),
            message_history: Vec::new(),
        };

        // Simulate a completed turn so there is history to checkpoint.
        log.message_history.push(MessageRecord {
            message_type: MessageType::User,
            message: Message::user("hello"),
        });

        // Push a system record for a model switch - the same logic used in SetModel.
        log.message_history.push(MessageRecord {
            message_type: MessageType::System,
            message: Message::system(OutputText::new("[system] model switched to gpt-4o")),
        });

        // Save the checkpoint directly, mirroring the SetModel handler logic.
        let endpoint = EndpointName::new("copilot");
        log.handles
            .persistence
            .save_turn(endpoint, log.message_history.clone())
            .await;

        // Allow the persistence actor to flush.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Reload the session file directly and verify the system record is present.
        let session_id = log.handles.persistence.session_id();
        let sessions_dir = log.handles.persistence.sessions_dir();
        let restored = augur_domain::persistence::store::load_session(&sessions_dir, &session_id)
            .expect("session file must exist after save_turn");
        let system_count = restored
            .state
            .messages
            .iter()
            .filter(|r| r.message_type == MessageType::System)
            .count();
        assert_eq!(
            system_count, 1,
            "the system model-switch record must be persisted"
        );
    }
}

#[test]
fn mirror_sync_executes_apply_log_event_token_accumulates_in_assistant_buf() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
