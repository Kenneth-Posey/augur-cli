//! Tests for `copilot::agent_feed_ops`.
//!
//! Tests for `classify_sdk_event` were removed in Phase 3 when that function
//! was deleted and its logic absorbed into `FeedRouter`. This module covers the
//! remaining stateless helpers directly: `map_sub_agent_delta_output`,
//! `map_tool_start_output`, `map_tool_progress_output`,
//! `map_tool_complete_output`, `ActiveToolCallMap`, and `ToolInfo`.

mod suite {
    use copilot_sdk::{
        AssistantMessageDeltaData, ToolExecutionCompleteData, ToolExecutionError,
        ToolExecutionProgressData, ToolExecutionStartData,
    };

    use augur_domain::types::AgentFeedOutput;
    use augur_domain::{StringNewtype, ToolCallId, ToolName};
    use augur_provider_copilot_sdk::actors::copilot::agent_feed_ops::{
        map_sub_agent_delta_output, map_tool_complete_output, map_tool_progress_output,
        map_tool_start_output, ActiveToolCallMap, ToolInfo,
    };

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Unwrap an `AgentFeedOutput::StatusLine` from a `Some`, panicking with a
    /// helpful message on any other shape.
    fn unwrap_status_line(output: Option<AgentFeedOutput>) -> String {
        match output {
            Some(AgentFeedOutput::StatusLine(text)) => text.to_string(),
            other => panic!("expected Some(StatusLine(_)), got {:?}", other),
        }
    }

    /// Unwrap an `AgentFeedOutput::ToolEventLine` from a `Some`, panicking with a
    /// helpful message on any other shape.
    fn unwrap_tool_event_line(output: Option<AgentFeedOutput>) -> String {
        match output {
            Some(AgentFeedOutput::ToolEventLine(text)) => text.to_string(),
            other => panic!("expected Some(ToolEventLine(_)), got {:?}", other),
        }
    }

    // ── map_sub_agent_delta_output ────────────────────────────────────────────

    /// Non-empty `delta_content` must produce `Some(StatusLine)` whose text
    /// matches the content verbatim.
    #[test]
    fn delta_output_non_empty_returns_status_line() {
        let data = AssistantMessageDeltaData {
            message_id: "m1".to_owned(),
            delta_content: "hello".to_owned(),
            total_response_size_bytes: None,
            parent_tool_call_id: None,
        };
        let result = map_sub_agent_delta_output(&data);
        assert_eq!(unwrap_status_line(result), "hello");
    }

    /// Empty `delta_content` must produce `None` - no output for blank deltas.
    #[test]
    fn delta_output_empty_content_returns_none() {
        let data = AssistantMessageDeltaData {
            message_id: "m2".to_owned(),
            delta_content: "".to_owned(),
            total_response_size_bytes: None,
            parent_tool_call_id: None,
        };
        let result = map_sub_agent_delta_output(&data);
        assert!(
            result.is_none(),
            "empty delta_content must yield None, got {:?}",
            result
        );
    }

    // ── map_tool_start_output ─────────────────────────────────────────────────

    /// Tool start formatting matches the main feed: bash uses description + command rows.
    #[test]
    fn tool_start_output_bash_matches_main_feed_format() {
        let data = ToolExecutionStartData {
            tool_name: "bash".to_owned(),
            tool_call_id: "tc1".to_owned(),
            arguments: Some(
                serde_json::json!({"description": "Run tests", "command": "cargo test --lib"}),
            ),
            parent_tool_call_id: None,
        };
        let result = map_tool_start_output(&data);
        assert_eq!(
            unwrap_tool_event_line(result),
            "  → Run tests\n    cargo test --lib"
        );
    }

    /// Absent args follow the same fallback as main-feed tool formatting.
    #[test]
    fn tool_start_output_no_arguments_uses_main_fallback() {
        let data = ToolExecutionStartData {
            tool_name: "read_file".to_owned(),
            tool_call_id: "tc2".to_owned(),
            arguments: None,
            parent_tool_call_id: None,
        };
        let result = map_tool_start_output(&data);
        assert_eq!(unwrap_tool_event_line(result), "  → read_file: null");
    }

    /// View formatting preserves the path and optional line range metadata row.
    #[test]
    fn tool_start_output_view_with_range_matches_main_feed_format() {
        let data = ToolExecutionStartData {
            tool_name: "view".to_owned(),
            tool_call_id: "tc-view".to_owned(),
            arguments: Some(serde_json::json!({"path": "src/lib.rs", "view_range": [1, 30]})),
            parent_tool_call_id: None,
        };
        let result = map_tool_start_output(&data);
        assert_eq!(
            unwrap_tool_event_line(result),
            "  → view: src/lib.rs\n    [lines: 1, 30]"
        );
    }

    /// Unknown tools follow the same default formatter and use first string arg.
    #[test]
    fn tool_start_output_unknown_tool_uses_main_default_field_extraction() {
        let data = ToolExecutionStartData {
            tool_name: "custom_tool".to_owned(),
            tool_call_id: "tc-unk".to_owned(),
            arguments: Some(serde_json::json!({"some_field": "some_value"})),
            parent_tool_call_id: None,
        };
        let result = map_tool_start_output(&data);
        assert_eq!(
            unwrap_tool_event_line(result),
            "  → custom_tool: some_value"
        );
    }

    /// File write formatting truncates content preview lines the same way as main feed.
    #[test]
    fn tool_start_output_file_create_truncates_preview() {
        let data = ToolExecutionStartData {
            tool_name: "file_create".to_owned(),
            tool_call_id: "tc-fw".to_owned(),
            arguments: Some(serde_json::json!({
                "path": "/tmp/demo.txt",
                "content": "line1\nline2\nline3\nline4\nline5"
            })),
            parent_tool_call_id: None,
        };
        let result = map_tool_start_output(&data);
        let line = unwrap_tool_event_line(result);
        assert!(
            line.contains("  → file_create: /tmp/demo.txt"),
            "must include file path"
        );
        assert!(
            line.contains("\n    line1")
                && line.contains("\n    line2")
                && line.contains("\n    line3"),
            "must include only first three preview lines"
        );
        assert!(
            !line.contains("line4") && !line.contains("line5"),
            "must omit extra lines from preview"
        );
        assert!(line.contains("... (+2 more lines)"));
    }

    // ── map_tool_progress_output ──────────────────────────────────────────────

    /// Every progress event must emit `Some(ToolEventLine(progress_message))`.
    /// The function is unconditional; callers apply state gating.
    #[test]
    fn tool_progress_output_always_emits() {
        let data = ToolExecutionProgressData {
            tool_call_id: "tc3".to_owned(),
            progress_message: "doing work".to_owned(),
        };
        let result = map_tool_progress_output(&data);
        assert_eq!(unwrap_tool_event_line(result), "doing work");
    }

    // ── map_tool_complete_output ──────────────────────────────────────────────

    /// A successful completion with a registry entry must emit
    /// `"✓ {tool_name}"` when no description was stored.
    #[test]
    fn tool_complete_output_success_shows_name_from_registry() {
        let mut registry = ActiveToolCallMap::new();
        registry.insert(
            ToolCallId::from("tc1"),
            ToolInfo {
                tool_name: ToolName::new("bash"),
                description: None,
            },
        );
        let data = ToolExecutionCompleteData {
            tool_call_id: "tc1".to_owned(),
            success: true,
            is_user_requested: None,
            result: None,
            error: None,
            tool_telemetry: None,
            parent_tool_call_id: None,
            mcp_server_name: None,
            mcp_tool_name: None,
        };
        let label = unwrap_tool_event_line(map_tool_complete_output(&data, &registry));
        assert_eq!(label, "✓ bash");
    }

    /// A failed completion with an error message must emit
    /// `"✗ {tool_name}: {error.message}"`.
    #[test]
    fn tool_complete_output_failure_shows_error() {
        let mut registry = ActiveToolCallMap::new();
        registry.insert(
            ToolCallId::from("tc2"),
            ToolInfo {
                tool_name: ToolName::new("bash"),
                description: None,
            },
        );
        let data = ToolExecutionCompleteData {
            tool_call_id: "tc2".to_owned(),
            success: false,
            is_user_requested: None,
            result: None,
            error: Some(ToolExecutionError {
                message: "exit 1".to_owned(),
                code: None,
            }),
            tool_telemetry: None,
            parent_tool_call_id: None,
            mcp_server_name: None,
            mcp_tool_name: None,
        };
        let label = unwrap_tool_event_line(map_tool_complete_output(&data, &registry));
        assert_eq!(label, "✗ bash: exit 1");
    }

    /// A `tool_call_id` not present in the registry must fall back to the raw
    /// `tool_call_id` string rather than panicking or producing an empty label.
    #[test]
    fn tool_complete_output_not_in_registry_falls_back_to_id() {
        let registry = ActiveToolCallMap::new();
        let data = ToolExecutionCompleteData {
            tool_call_id: "unknown-tc".to_owned(),
            success: true,
            is_user_requested: None,
            result: None,
            error: None,
            tool_telemetry: None,
            parent_tool_call_id: None,
            mcp_server_name: None,
            mcp_tool_name: None,
        };
        let label = unwrap_tool_event_line(map_tool_complete_output(&data, &registry));
        assert!(
            label.contains("unknown-tc"),
            "registry-miss must fall back to tool_call_id, got: {label:?}"
        );
    }
}
