//! Tests for `copilot::event_mapper::map_sdk_event`.
//!
//! These tests require the `copilot-executor` feature because they use
//! `copilot_sdk::SessionEventData` directly.
//!
//! Each test verifies a single SDK event → `AgentOutput` mapping.

mod suite {
    use augur_domain::types::AgentOutput;
    use augur_provider_copilot_sdk::actors::copilot::event_mapper::map_sdk_event;
    use copilot_sdk::SessionEventData;

    /// An `AssistantMessageDelta` event with non-empty content maps to `Token`.
    #[test]
    fn delta_event_maps_to_token() {
        use copilot_sdk::AssistantMessageDeltaData;
        let data = SessionEventData::AssistantMessageDelta(AssistantMessageDeltaData {
            message_id: "m1".to_owned(),
            delta_content: "hello".to_owned(),
            total_response_size_bytes: None,
            parent_tool_call_id: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::Token(t)) => assert_eq!(&*t, "hello"),
            other => panic!("expected Token, got {:?}", other),
        }
    }

    /// An `AssistantMessage` signals the end of the assistant's message content.
    /// It maps to `Done` to trigger turn completion logic (newlines, scroll reset, thinking clear).
    /// If `SessionIdle` also arrives later, both will call the same completion handler,
    /// which is idempotent and harmless.
    #[test]
    fn assistant_message_maps_to_done() {
        use copilot_sdk::AssistantMessageData;
        let data = SessionEventData::AssistantMessage(AssistantMessageData {
            message_id: "m1".to_owned(),
            content: "done content".to_owned(),
            chunk_content: None,
            total_response_size_bytes: None,
            tool_requests: None,
            parent_tool_call_id: None,
        });
        let result = map_sdk_event(&data);
        assert!(
            matches!(result, Some(AgentOutput::Done)),
            "AssistantMessage should map to Done, got {:?}",
            result
        );
    }

    /// Regression: when the assistant message carries tool requests, the event must
    /// remain in-turn (`MessageBreak`) instead of ending the turn (`Done`).
    #[test]
    fn assistant_message_with_tool_requests_maps_to_message_break() {
        use copilot_sdk::AssistantMessageData;
        let data = SessionEventData::AssistantMessage(AssistantMessageData {
            message_id: "m-tool".to_owned(),
            content: "running tool".to_owned(),
            chunk_content: None,
            total_response_size_bytes: None,
            tool_requests: Some(vec![]),
            parent_tool_call_id: None,
        });
        let result = map_sdk_event(&data);
        assert!(
            matches!(result, Some(AgentOutput::MessageBreak)),
            "AssistantMessage with tool requests must map to MessageBreak, got {:?}",
            result
        );
    }

    /// A `SessionIdle` event maps to `TurnComplete`.
    #[test]
    fn session_idle_maps_to_turn_complete() {
        use copilot_sdk::SessionIdleData;
        let data = SessionEventData::SessionIdle(SessionIdleData {});
        let result = map_sdk_event(&data);
        assert!(matches!(result, Some(AgentOutput::TurnComplete)));
    }

    /// A `SessionError` event maps to `Error` with the message text.
    #[test]
    fn session_error_maps_to_error() {
        use copilot_sdk::SessionErrorData;
        let data = SessionEventData::SessionError(SessionErrorData {
            error_type: "timeout".to_owned(),
            message: "timeout".to_owned(),
            stack: None,
            code: None,
            provider_call_id: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::Error(msg)) => assert_eq!(&*msg, "timeout"),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    /// A `ToolExecutionStart` event maps to `ToolCallStarted` with the tool name.
    #[test]
    fn tool_execution_start_maps_to_tool_call_started() {
        use copilot_sdk::ToolExecutionStartData;
        let data = SessionEventData::ToolExecutionStart(ToolExecutionStartData {
            tool_name: "shell_exec".to_owned(),
            tool_call_id: "tc1".to_owned(),
            arguments: None,
            parent_tool_call_id: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::ToolCallStarted { name, .. }) => {
                assert_eq!(&*name, "shell_exec");
            }
            other => panic!("expected ToolCallStarted, got {:?}", other),
        }
    }

    /// An `Abort` event maps to `AgentOutput::Error` carrying the abort reason string.
    #[test]
    fn abort_maps_to_error() {
        use copilot_sdk::AbortData;
        let data = SessionEventData::Abort(AbortData {
            reason: "user cancelled".to_owned(),
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::Error(msg)) => assert_eq!(&*msg, "user cancelled"),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    /// An `AssistantUsage` event maps to `AgentOutput::UsageUpdate`.
    #[test]
    fn assistant_usage_maps_to_usage_update() {
        use copilot_sdk::AssistantUsageData;
        let data = SessionEventData::AssistantUsage(AssistantUsageData {
            model: None,
            input_tokens: Some(120.0),
            output_tokens: Some(45.0),
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost: None,
            duration: None,
            initiator: None,
            api_call_id: None,
            provider_call_id: None,
            quota_snapshots: None,
        });
        let result = map_sdk_event(&data);
        assert!(
            matches!(result, Some(AgentOutput::UsageUpdate { .. })),
            "expected UsageUpdate, got {:?}",
            result
        );
    }

    /// An `AssistantUsage` event with cache_read_tokens still maps to `UsageUpdate`.
    #[test]
    fn assistant_usage_maps_cache_read_tokens() {
        use copilot_sdk::AssistantUsageData;
        let data = SessionEventData::AssistantUsage(AssistantUsageData {
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: Some(500.0),
            cache_write_tokens: None,
            cost: None,
            duration: None,
            initiator: None,
            api_call_id: None,
            provider_call_id: None,
            quota_snapshots: None,
        });
        let result = map_sdk_event(&data);
        assert!(
            matches!(result, Some(AgentOutput::UsageUpdate { .. })),
            "expected UsageUpdate, got {:?}",
            result
        );
    }

    /// An `AssistantUsage` event with a model string maps the value to
    /// `AgentOutput::UsageUpdate::model` as a `ModelId`.
    ///
    /// The model name from the SDK usage event is the canonical source for the
    /// status bar model display; this verifies the field is preserved end-to-end.
    #[test]
    fn assistant_usage_maps_model_to_usage_update() {
        use copilot_sdk::AssistantUsageData;
        let data = SessionEventData::AssistantUsage(AssistantUsageData {
            model: Some("gpt-4o".to_owned()),
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost: None,
            duration: None,
            initiator: None,
            api_call_id: None,
            provider_call_id: None,
            quota_snapshots: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::UsageUpdate { model, .. }) => {
                assert_eq!(model.as_deref(), Some("gpt-4o"));
            }
            other => panic!("expected UsageUpdate, got {:?}", other),
        }
    }

    /// A `ToolExecutionComplete` event maps to `AgentOutput::ToolCallCompleted`
    /// carrying the tool call id, success flag, and optional result text.
    #[test]
    fn tool_execution_complete_maps_to_tool_call_completed() {
        use copilot_sdk::{ToolExecutionCompleteData, ToolResultContent};
        let data = SessionEventData::ToolExecutionComplete(ToolExecutionCompleteData {
            tool_call_id: "tc-42".to_owned(),
            success: true,
            is_user_requested: None,
            result: Some(ToolResultContent {
                content: "output text".to_owned(),
            }),
            error: None,
            tool_telemetry: None,
            parent_tool_call_id: None,
            mcp_server_name: None,
            mcp_tool_name: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::ToolCallCompleted {
                name,
                success,
                result,
                ..
            }) => {
                assert_eq!(&*name, "tc-42");
                assert!(success);
                assert_eq!(result.as_deref(), Some("output text"));
            }
            other => panic!("expected ToolCallCompleted, got {:?}", other),
        }
    }

    /// A failed `ToolExecutionComplete` event (success=false, result=None, error=Some)
    /// maps to `ToolCallCompleted` with `success=false` and `result` containing the
    /// error message. This ensures error details appear in the JSONL log instead of
    /// showing an empty string.
    #[test]
    fn tool_execution_complete_error_uses_error_message() {
        use copilot_sdk::{ToolExecutionCompleteData, ToolExecutionError};
        let data = SessionEventData::ToolExecutionComplete(ToolExecutionCompleteData {
            tool_call_id: "tc-err".to_owned(),
            success: false,
            is_user_requested: None,
            result: None,
            error: Some(ToolExecutionError {
                message: "permission denied".to_owned(),
                code: Some("PERMISSION_DENIED".to_owned()),
            }),
            tool_telemetry: None,
            parent_tool_call_id: None,
            mcp_server_name: None,
            mcp_tool_name: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::ToolCallCompleted {
                name,
                success,
                result,
                ..
            }) => {
                assert_eq!(&*name, "tc-err");
                assert!(!success);
                assert_eq!(result.as_deref(), Some("permission denied"));
            }
            other => panic!("expected ToolCallCompleted, got {:?}", other),
        }
    }

    /// An unknown event variant produces `None` (silently dropped).
    #[test]
    fn unknown_event_produces_none() {
        let data = SessionEventData::Unknown(serde_json::json!({"type": "future_event"}));
        let result = map_sdk_event(&data);
        assert!(result.is_none());
    }

    /// Informational lifecycle events (SessionStart, SessionResume) produce `None`.
    #[test]
    fn lifecycle_events_produce_none() {
        use copilot_sdk::SessionStartData;
        let start = SessionEventData::SessionStart(SessionStartData {
            session_id: "s1".to_owned(),
            version: 0.0,
            producer: String::new(),
            copilot_version: String::new(),
            start_time: String::new(),
            selected_model: None,
        });
        let resume = SessionEventData::SessionResume(copilot_sdk::SessionResumeData {
            resume_time: String::new(),
            event_count: 0.0,
        });
        assert!(map_sdk_event(&start).is_none());
        assert!(map_sdk_event(&resume).is_none());
    }

    /// A `SessionCompactionStart` event maps to `SystemMessage`.
    ///
    /// `SessionCompactionStart` emits a timestamped "[system] compacting context..."
    /// message so the user sees a timestamped indicator when compaction fires -
    /// whether triggered by `/compact` or the automatic background threshold.
    #[test]
    fn session_compaction_start_maps_to_system_message() {
        use copilot_sdk::SessionCompactionStartData;
        let data = SessionEventData::SessionCompactionStart(SessionCompactionStartData {});
        let output = map_sdk_event(&data);
        assert!(matches!(output, Some(AgentOutput::SystemMessage(_))));
        if let Some(AgentOutput::SystemMessage(t)) = output {
            assert!(t.to_string().contains("compacting"));
        }
    }

    /// A successful `SessionCompactionComplete` with token stats maps to
    /// `CompactionComplete` carrying a summary message with the token counts.
    #[test]
    fn session_compaction_complete_success_maps_to_compaction_complete_with_stats() {
        use copilot_sdk::SessionCompactionCompleteData;
        let data = SessionEventData::SessionCompactionComplete(SessionCompactionCompleteData {
            success: true,
            error: None,
            pre_compaction_tokens: Some(50_000.0),
            post_compaction_tokens: Some(12_500.0),
            pre_compaction_messages_length: None,
            post_compaction_messages_length: None,
            compaction_tokens_used: None,
            messages_removed: None,
            tokens_removed: None,
            summary_content: None,
            checkpoint_number: None,
            checkpoint_path: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::CompactionComplete { text }) => {
                let s: &str = &text;
                assert!(
                    s.contains("50000"),
                    "expected pre-token count in output, got: {s}"
                );
                assert!(
                    s.contains("12500"),
                    "expected post-token count in output, got: {s}"
                );
            }
            other => panic!("expected CompactionComplete with stats, got {:?}", other),
        }
    }

    /// A successful `SessionCompactionComplete` with no token stats maps to `CompactionComplete`.
    #[test]
    fn session_compaction_complete_success_no_stats_maps_to_compaction_complete() {
        use copilot_sdk::SessionCompactionCompleteData;
        let data = SessionEventData::SessionCompactionComplete(SessionCompactionCompleteData {
            success: true,
            error: None,
            pre_compaction_tokens: None,
            post_compaction_tokens: None,
            pre_compaction_messages_length: None,
            post_compaction_messages_length: None,
            compaction_tokens_used: None,
            messages_removed: None,
            tokens_removed: None,
            summary_content: None,
            checkpoint_number: None,
            checkpoint_path: None,
        });
        let result = map_sdk_event(&data);
        assert!(
            matches!(result, Some(AgentOutput::CompactionComplete { .. })),
            "expected CompactionComplete for success with no stats, got {:?}",
            result
        );
    }

    /// A failed `SessionCompactionComplete` maps to `AgentOutput::Error` with
    /// the error string so the failure is visible in the conversation pane.
    #[test]
    fn session_compaction_complete_failure_maps_to_error() {
        use copilot_sdk::SessionCompactionCompleteData;
        let data = SessionEventData::SessionCompactionComplete(SessionCompactionCompleteData {
            success: false,
            error: Some("out of memory".to_owned()),
            pre_compaction_tokens: None,
            post_compaction_tokens: None,
            pre_compaction_messages_length: None,
            post_compaction_messages_length: None,
            compaction_tokens_used: None,
            messages_removed: None,
            tokens_removed: None,
            summary_content: None,
            checkpoint_number: None,
            checkpoint_path: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::Error(msg)) => {
                assert!(
                    msg.contains("out of memory"),
                    "expected error text in output, got: {msg}"
                );
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }

    /// A `SessionError` containing the old wrong-method-name message is now forwarded.
    ///
    /// With the SDK bug fixed (`session.history.compact` is now the correct method),
    /// the `-32601` error for `session.compaction.compact` should not occur at runtime.
    /// The suppression that existed for this case has been removed; all `SessionError`
    /// events are now forwarded as `AgentOutput::Error` without exception.
    #[test]
    fn session_error_is_forwarded_not_suppressed() {
        use copilot_sdk::SessionErrorData;
        let data = SessionEventData::SessionError(SessionErrorData {
            error_type: "JsonRpcError".to_owned(),
            message: "json rpc error -32601 unhandled method session.compaction.compact".to_owned(),
            stack: None,
            code: Some(-32601.0),
            provider_call_id: None,
        });
        let result = map_sdk_event(&data);
        assert!(
            matches!(result, Some(AgentOutput::Error(_))),
            "session errors must be forwarded, not suppressed"
        );
    }

    /// A non-compact `SessionError` still maps to `AgentOutput::Error`.
    ///
    /// Only the specific compact-method-not-found error is suppressed; all other
    /// session errors must still surface to the user via `AgentOutput::Error`.
    #[test]
    fn session_error_non_compact_is_forwarded() {
        use copilot_sdk::SessionErrorData;
        let data = SessionEventData::SessionError(SessionErrorData {
            error_type: "timeout".to_owned(),
            message: "stream timed out".to_owned(),
            stack: None,
            code: Some(-32000.0),
            provider_call_id: None,
        });
        match map_sdk_event(&data) {
            Some(AgentOutput::Error(msg)) => assert_eq!(&*msg, "stream timed out"),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    /// A `SessionUsageInfo` event should not map to any output (it's been removed from tracking).
    #[test]
    fn session_usage_info_maps_to_none() {
        use copilot_sdk::SessionUsageInfoData;
        let data = SessionEventData::SessionUsageInfo(SessionUsageInfoData {
            token_limit: 128_000.0,
            current_tokens: 45_000.0,
            messages_length: 12.0,
        });
        let result = map_sdk_event(&data);
        assert!(
            result.is_none(),
            "expected None for SessionUsageInfo (token tracking removed), got {:?}",
            result
        );
    }

    /// An `AssistantIntent` event maps to `AgentOutput::IntentMessage` carrying the intent text.
    ///
    /// The intent text is preserved verbatim so the TUI can display it as a plain line
    /// immediately above the tool-call lines that follow.
    #[test]
    fn assistant_intent_maps_to_intent_message() {
        use copilot_sdk::AssistantIntentData;
        let data = SessionEventData::AssistantIntent(AssistantIntentData {
            intent: "I will search for relevant files".to_owned(),
        });
        match map_sdk_event(&data) {
            Some(AgentOutput::IntentMessage(text)) => {
                assert_eq!(&*text, "I will search for relevant files");
            }
            other => panic!("expected IntentMessage, got {:?}", other),
        }
    }

    /// A `ToolExecutionProgress` event maps to `AgentOutput::ToolProgress` carrying
    /// the `tool_call_id` and progress message verbatim.
    #[test]
    fn tool_execution_progress_maps_to_tool_progress() {
        use copilot_sdk::ToolExecutionProgressData;
        let data = SessionEventData::ToolExecutionProgress(ToolExecutionProgressData {
            tool_call_id: "tc-77".to_owned(),
            progress_message: "reading 3 files...".to_owned(),
        });
        match map_sdk_event(&data) {
            Some(AgentOutput::ToolProgress {
                tool_call_id,
                message,
            }) => {
                assert_eq!(tool_call_id.to_string(), "tc-77");
                assert_eq!(&*message, "reading 3 files...");
            }
            other => panic!("expected ToolProgress, got {:?}", other),
        }
    }

    /// A `ToolExecutionPartialResult` event maps to `AgentOutput::ToolPartialResult`
    /// carrying the `tool_call_id` and the partial output chunk verbatim.
    #[test]
    fn tool_execution_partial_result_maps_to_tool_partial_result() {
        use copilot_sdk::ToolExecutionPartialResultData;
        let data = SessionEventData::ToolExecutionPartialResult(ToolExecutionPartialResultData {
            tool_call_id: "tc-99".to_owned(),
            partial_output: "line one\nline two".to_owned(),
        });
        match map_sdk_event(&data) {
            Some(AgentOutput::ToolPartialResult {
                tool_call_id,
                output,
            }) => {
                assert_eq!(tool_call_id.to_string(), "tc-99");
                assert_eq!(&*output, "line one\nline two");
            }
            other => panic!("expected ToolPartialResult, got {:?}", other),
        }
    }

    /// An `AssistantMessageDelta` with non-empty content always maps to `Token`
    /// from the stateless mapper; suppression of sub-agent deltas is now
    /// the router's responsibility, not the mapper's.
    #[test]
    fn delta_during_subagent_maps_to_none() {
        use copilot_sdk::AssistantMessageDeltaData;
        let data = SessionEventData::AssistantMessageDelta(AssistantMessageDeltaData {
            message_id: "m1".to_owned(),
            delta_content: "hello".to_owned(),
            total_response_size_bytes: None,
            parent_tool_call_id: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::Token(t)) => assert_eq!(&*t, "hello"),
            other => panic!(
                "stateless mapper must always produce Token for non-empty delta, got {:?}",
                other
            ),
        }
    }

    /// A `ToolExecutionStart` for the "task" tool always maps to `ToolCallStarted`
    /// from the stateless mapper; suppression of the task tool launch is now
    /// the router's responsibility, not the mapper's.
    #[test]
    fn tool_execution_start_during_task_maps_to_none() {
        use copilot_sdk::ToolExecutionStartData;
        let data = SessionEventData::ToolExecutionStart(ToolExecutionStartData {
            tool_name: "task".to_owned(),
            tool_call_id: "id".to_owned(),
            arguments: None,
            parent_tool_call_id: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::ToolCallStarted { name, .. }) => {
                assert_eq!(&*name, "task");
            }
            other => panic!(
                "stateless mapper must always produce ToolCallStarted for task start, got {:?}",
                other
            ),
        }
    }

    /// A `ToolExecutionStart` for a regular (non-task) tool while state is `Idle` maps to
    /// `ToolCallStarted`.
    ///
    /// Only the outer "task" tool is suppressed; all other tool launches must appear in
    /// the main conversation feed normally.
    #[test]
    fn tool_execution_start_regular_tool_maps_to_started() {
        use copilot_sdk::ToolExecutionStartData;
        let data = SessionEventData::ToolExecutionStart(ToolExecutionStartData {
            tool_name: "shell_exec".to_owned(),
            tool_call_id: "tc-reg".to_owned(),
            arguments: None,
            parent_tool_call_id: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::ToolCallStarted { name, .. }) => {
                assert_eq!(&*name, "shell_exec");
            }
            other => panic!("expected ToolCallStarted for regular tool, got {:?}", other),
        }
    }

    /// A `ToolExecutionComplete` always maps to `ToolCallCompleted` from the
    /// stateless mapper; suppression of the task tool completion is now
    /// the router's responsibility, not the mapper's.
    #[test]
    fn tool_execution_complete_during_await_maps_to_none() {
        use copilot_sdk::{ToolExecutionCompleteData, ToolResultContent};
        let data = SessionEventData::ToolExecutionComplete(ToolExecutionCompleteData {
            tool_call_id: "id".to_owned(),
            success: true,
            is_user_requested: None,
            result: Some(ToolResultContent {
                content: "done".to_owned(),
            }),
            error: None,
            tool_telemetry: None,
            parent_tool_call_id: None,
            mcp_server_name: None,
            mcp_tool_name: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::ToolCallCompleted { name, success, .. }) => {
                assert_eq!(&*name, "id");
                assert!(success);
            }
            other => panic!(
                "stateless mapper must always produce ToolCallCompleted, got {:?}",
                other
            ),
        }
    }

    /// A `ToolExecutionComplete` while state is `Idle` maps to `ToolCallCompleted`.
    ///
    /// Non-task tool completions must surface normally in the main conversation feed.
    #[test]
    fn tool_execution_complete_idle_maps_to_completed() {
        use copilot_sdk::{ToolExecutionCompleteData, ToolResultContent};
        let data = SessionEventData::ToolExecutionComplete(ToolExecutionCompleteData {
            tool_call_id: "tc-idle".to_owned(),
            success: true,
            is_user_requested: None,
            result: Some(ToolResultContent {
                content: "result text".to_owned(),
            }),
            error: None,
            tool_telemetry: None,
            parent_tool_call_id: None,
            mcp_server_name: None,
            mcp_tool_name: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::ToolCallCompleted { name, success, .. }) => {
                assert_eq!(&*name, "tc-idle");
                assert!(success);
            }
            other => panic!("expected ToolCallCompleted for idle state, got {:?}", other),
        }
    }

    /// A `ToolExecutionPartialResult` always maps to `ToolPartialResult` from the
    /// stateless mapper; suppression of background-agent partial results is now
    /// the router's responsibility, not the mapper's.
    #[test]
    fn tool_partial_result_during_agent_active_maps_to_none() {
        use copilot_sdk::ToolExecutionPartialResultData;
        let data = SessionEventData::ToolExecutionPartialResult(ToolExecutionPartialResultData {
            tool_call_id: "tc-partial".to_owned(),
            partial_output: "partial output...".to_owned(),
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::ToolPartialResult {
                tool_call_id,
                output,
            }) => {
                assert_eq!(tool_call_id.to_string(), "tc-partial");
                assert_eq!(&*output, "partial output...");
            }
            other => panic!(
                "stateless mapper must always produce ToolPartialResult, got {:?}",
                other
            ),
        }
    }

    /// A `ToolExecutionProgress` always maps to `ToolProgress` from the
    /// stateless mapper; suppression of background-agent progress is now
    /// the router's responsibility, not the mapper's.
    #[test]
    fn tool_progress_during_agent_active_maps_to_none() {
        use copilot_sdk::ToolExecutionProgressData;
        let data = SessionEventData::ToolExecutionProgress(ToolExecutionProgressData {
            tool_call_id: "tc-prog".to_owned(),
            progress_message: "scanning...".to_owned(),
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::ToolProgress {
                tool_call_id,
                message,
            }) => {
                assert_eq!(tool_call_id.to_string(), "tc-prog");
                assert_eq!(&*message, "scanning...");
            }
            other => panic!(
                "stateless mapper must always produce ToolProgress, got {:?}",
                other
            ),
        }
    }

    /// A `ToolExecutionProgress` while state is `Idle` maps to `ToolProgress`.
    ///
    /// Non-agent tool progress must appear in the main conversation feed normally.
    #[test]
    fn tool_progress_idle_maps_to_progress() {
        use copilot_sdk::ToolExecutionProgressData;
        let data = SessionEventData::ToolExecutionProgress(ToolExecutionProgressData {
            tool_call_id: "tc-prog-idle".to_owned(),
            progress_message: "reading files...".to_owned(),
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::ToolProgress {
                tool_call_id,
                message,
            }) => {
                assert_eq!(tool_call_id.to_string(), "tc-prog-idle");
                assert_eq!(&*message, "reading files...");
            }
            other => panic!("expected ToolProgress for idle state, got {:?}", other),
        }
    }

    /// Compile-time check that `map_sdk_event` accepts exactly one argument.
    ///
    /// With the new stateless signature this is a zero-state call that must compile.
    /// Fails to compile until `map_sdk_event`'s `state` parameter is removed in Step 2.
    #[test]
    fn map_sdk_event_has_no_state_param() {
        use copilot_sdk::SessionIdleData;
        let data = SessionEventData::SessionIdle(SessionIdleData {});
        let result = map_sdk_event(&data);
        assert!(
            matches!(result, Some(AgentOutput::TurnComplete)),
            "map_sdk_event(&data) with no state arg must produce TurnComplete for SessionIdle"
        );
    }

    /// With the stateless signature an `AssistantMessageDelta` with non-empty content
    /// must always produce `Some(Token(...))` - suppression is now the router's job.
    ///
    /// Fails to compile until `map_sdk_event`'s `state` parameter is removed in Step 2.
    #[test]
    fn map_sdk_event_delta_agent_active_no_suppression() {
        use copilot_sdk::AssistantMessageDeltaData;
        let data = SessionEventData::AssistantMessageDelta(AssistantMessageDeltaData {
            message_id: "m2".to_owned(),
            delta_content: "hi".to_owned(),
            total_response_size_bytes: None,
            parent_tool_call_id: None,
        });
        let result = map_sdk_event(&data);
        match result {
            Some(AgentOutput::Token(t)) => assert_eq!(&*t, "hi"),
            other => panic!(
                "stateless map_sdk_event must always produce Token for non-empty delta, got {:?}",
                other
            ),
        }
    }
}
