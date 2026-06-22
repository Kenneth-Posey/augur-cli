use augur_domain::domain::newtypes::SupportsAuto;
use augur_tui::domain::newtypes::{NumericNewtype as _, WaitSecs};
use augur_tui::domain::string_newtypes::{EndpointName, StringNewtype, ToolName};
use augur_tui::domain::tui_state::{AppScreen, AppState, LineKind};
use augur_tui::domain::types::AgentOutput;
use std::time::{Duration, Instant};

/// Verifies that `BackoffStarted` records a future backoff deadline in the status state.
#[test]
fn apply_agent_output_backoff_started_sets_backoff_deadline() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let before = Instant::now();

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::BackoffStarted(WaitSecs::of(3)),
    );

    let deadline = state
        .status
        .context_window
        .backoff_until
        .expect("BackoffStarted must set backoff_until");
    assert!(
        deadline >= before + Duration::from_secs(2),
        "backoff deadline must be in the future, got {deadline:?} vs {before:?}"
    );
}

/// Verifies that `Done` clears any active backoff deadline at end of turn.
#[test]
fn apply_agent_output_done_clears_backoff_deadline() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.status.context_window.backoff_until = Some(Instant::now() + Duration::from_secs(30));

    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);

    assert!(
        state.status.context_window.backoff_until.is_none(),
        "Done must clear backoff_until"
    );
}

/// Verifies that `TurnComplete` clears any active backoff deadline at end of turn.
#[test]
fn apply_agent_output_turn_complete_clears_backoff_deadline() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.status.context_window.backoff_until = Some(Instant::now() + Duration::from_secs(30));

    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::TurnComplete);

    assert!(
        state.status.context_window.backoff_until.is_none(),
        "TurnComplete must clear backoff_until"
    );
}

#[test]
fn apply_agent_output_models_available_is_ignored_for_non_auto_endpoint() {
    use augur_tui::domain::string_newtypes::{ModelId, ModelLabel};
    use augur_tui::domain::tui_state::EndpointModelCatalog;
    use augur_tui::domain::types::ModelOption;

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.endpoint_catalog = vec![EndpointModelCatalog::builder()
        .endpoint_name(EndpointName::new("ep"))
        .models(vec![])
        .default_display("yaml-default".into())
        .supports_auto(SupportsAuto::no())
        .build()];
    state.prompt.models.available = vec![ModelOption::builder()
        .id(ModelId::new("yaml/model"))
        .display_name(ModelLabel::new("YAML Model"))
        .build()];

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ModelsAvailable(vec![ModelOption::builder()
            .id(ModelId::new("provider/endpoint-name"))
            .display_name(ModelLabel::new("Provider Endpoint"))
            .build()]),
    );

    assert_eq!(
        state.prompt.models.available[0].id.as_str(),
        "yaml/model",
        "incoming ModelsAvailable must not override YAML-backed endpoint model list"
    );
}

#[test]
fn apply_agent_output_models_available_applies_for_auto_endpoint() {
    use augur_tui::domain::string_newtypes::{ModelId, ModelLabel};
    use augur_tui::domain::tui_state::EndpointModelCatalog;
    use augur_tui::domain::types::ModelOption;

    let mut state = AppState::new(EndpointName::new("copilot"), AppScreen::Conversation);
    state.prompt.models.endpoint_catalog = vec![EndpointModelCatalog::builder()
        .endpoint_name(EndpointName::new("copilot"))
        .models(vec![])
        .default_display("copilot".into())
        .supports_auto(SupportsAuto::yes())
        .build()];

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ModelsAvailable(vec![ModelOption::builder()
            .id(ModelId::new("gpt-5"))
            .display_name(ModelLabel::new("GPT-5"))
            .build()]),
    );

    assert_eq!(
        state.prompt.models.available[0].id.as_str(),
        "gpt-5",
        "auto-capable endpoint may update available models from ModelsAvailable events"
    );
}

#[test]
fn apply_agent_output_models_available_ignored_when_endpoint_row_missing() {
    use augur_tui::domain::string_newtypes::{ModelId, ModelLabel};
    use augur_tui::domain::tui_state::EndpointModelCatalog;
    use augur_tui::domain::types::ModelOption;

    let mut state = AppState::new(
        EndpointName::new("unknown-endpoint"),
        AppScreen::Conversation,
    );
    state.prompt.models.endpoint_catalog = vec![EndpointModelCatalog::builder()
        .endpoint_name(EndpointName::new("known-endpoint"))
        .models(vec![])
        .default_display("known".into())
        .supports_auto(SupportsAuto::no())
        .build()];
    state.prompt.models.available = vec![ModelOption::builder()
        .id(ModelId::new("yaml/model"))
        .display_name(ModelLabel::new("YAML Model"))
        .build()];

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ModelsAvailable(vec![ModelOption::builder()
            .id(ModelId::new("provider/endpoint-name"))
            .display_name(ModelLabel::new("Provider Endpoint"))
            .build()]),
    );

    assert_eq!(
        state.prompt.models.available[0].id.as_str(),
        "yaml/model",
        "ModelsAvailable must not apply when active endpoint has no catalog row"
    );
}

/// Verifies that `ToolCallStarted` preserves tool name and args in OutputLine metadata.
#[test]
fn apply_agent_output_tool_call_started_preserves_metadata() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let initial_line_count = state.output.lines.len();

    let tool_name = ToolName::new("view");
    let tool_args = serde_json::json!({ "path": "/src/main.rs" });

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name.clone(),
            args: tool_args.clone(),
        },
    );

    // Verify a new line was added
    let new_line_count = state.output.lines.len();
    assert!(
        new_line_count > initial_line_count,
        "ToolCallStarted must add at least one line"
    );

    // Find the ToolCall line
    let tool_line = state
        .output
        .lines
        .iter()
        .find(|line| line.kind == LineKind::ToolCall)
        .expect("must have a ToolCall line after ToolCallStarted event");

    // Verify metadata is populated and correct
    let metadata = tool_line
        .metadata
        .as_ref()
        .expect("ToolCall line must have metadata from ToolCallStarted");
    assert_eq!(
        metadata.tool_name.as_str(),
        "view",
        "tool_name in metadata must match event"
    );
    assert_eq!(
        metadata.tool_args.get("path").and_then(|v| v.as_str()),
        Some("/src/main.rs"),
        "tool_args in metadata must be preserved"
    );
}

/// Verifies that tool metadata is accessible at render time without panics.
#[test]
fn apply_agent_output_tool_metadata_accessible_at_render_time() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    let tool_name = ToolName::new("grep");
    let tool_args = serde_json::json!({
        "pattern": "TODO",
        "path": "/src"
    });

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name,
            args: tool_args,
        },
    );

    // Find the ToolCall line and verify metadata is accessible
    let tool_line = state
        .output
        .lines
        .iter()
        .find(|line| line.kind == LineKind::ToolCall)
        .expect("must have a ToolCall line");

    // Verify we can access metadata fields without unwrap panicking
    if let Some(metadata) = &tool_line.metadata {
        let _tool_name_str: &str = metadata.tool_name.as_str();
        let _tool_args_obj: &serde_json::Value = &metadata.tool_args;
        // If we reach here without panic, metadata is accessible
    } else {
        panic!("ToolCall line must have metadata");
    }
}

/// Verifies that tool-call line formatting displays context and details properly.
///
/// Tests that `format_tool_call_line()` extracts tool-specific fields and
/// formats multi-line display correctly:
/// - view: shows filepath on one line, optional line range on second
/// - bash: shows description on first line, command on second
/// - glob: shows pattern on second line
/// - grep: shows pattern on second line
#[test]
fn apply_agent_output_tool_call_format_view_with_path() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    let tool_name = ToolName::new("view");
    let tool_args = serde_json::json!({ "path": "/src/main.rs" });

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name,
            args: tool_args,
        },
    );

    let tool_line = state
        .output
        .lines
        .iter()
        .find(|line| line.kind == LineKind::ToolCall)
        .expect("must have a ToolCall line");

    let text = tool_line.text.as_str();
    assert!(
        text.contains("view:"),
        "tool call should include 'view:' label, got: {}",
        text
    );
    assert!(
        text.contains("/src/main.rs"),
        "tool call should include filepath, got: {}",
        text
    );
}

#[test]
fn apply_agent_output_tool_call_format_view_with_range() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    let tool_name = ToolName::new("view");
    let tool_args = serde_json::json!({
        "path": "/src/main.rs",
        "view_range": [1, 30]
    });

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name,
            args: tool_args,
        },
    );

    let tool_lines: Vec<&str> = state
        .output
        .lines
        .iter()
        .filter(|line| line.kind == LineKind::ToolCall)
        .map(|line| line.text.as_str())
        .collect();
    assert!(
        tool_lines.len() >= 2,
        "view with range should render multi-row"
    );
    let text = tool_lines.join("\n");
    assert!(
        text.contains("view:"),
        "tool call should include 'view:' label, got: {}",
        text
    );
    assert!(
        text.contains("/src/main.rs"),
        "tool call should include filepath, got: {}",
        text
    );
    assert!(
        text.contains("1") && text.contains("30"),
        "tool call should include line range, got: {}",
        text
    );
    assert!(tool_lines
        .iter()
        .any(|line| line.contains("[lines: 1, 30]")));
}

#[test]
fn apply_agent_output_tool_call_format_bash_command() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    let tool_name = ToolName::new("bash");
    let tool_args = serde_json::json!({
        "command": "cargo test",
        "description": "Run tests"
    });

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name,
            args: tool_args,
        },
    );

    let tool_lines: Vec<&str> = state
        .output
        .lines
        .iter()
        .filter(|line| line.kind == LineKind::ToolCall)
        .map(|line| line.text.as_str())
        .collect();
    assert!(tool_lines.len() >= 2, "bash should render multi-row");
    let text = tool_lines.join("\n");
    assert!(
        text.contains("Run tests"),
        "tool call should include description, got: {}",
        text
    );
    assert!(
        text.contains("cargo test"),
        "tool call should include command, got: {}",
        text
    );
    assert!(
        tool_lines.iter().any(|line| line.contains("cargo test")),
        "command should appear on its own tool-call row, got: {}",
        text
    );
}

#[test]
fn apply_agent_output_tool_call_format_glob_pattern() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    let tool_name = ToolName::new("glob");
    let tool_args = serde_json::json!({
        "pattern": "**/*.rs"
    });

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name,
            args: tool_args,
        },
    );

    let tool_lines: Vec<&str> = state
        .output
        .lines
        .iter()
        .filter(|line| line.kind == LineKind::ToolCall)
        .map(|line| line.text.as_str())
        .collect();
    assert!(tool_lines.len() >= 2, "glob should render multi-row");
    let text = tool_lines.join("\n");
    assert!(
        text.contains("glob:"),
        "tool call should include 'glob:' label, got: {}",
        text
    );
    assert!(
        text.contains("**/*.rs"),
        "tool call should include pattern, got: {}",
        text
    );
    assert!(
        tool_lines.iter().any(|line| line.contains("**/*.rs")),
        "pattern should appear on its own tool-call row, got: {}",
        text
    );
}

#[test]
fn apply_agent_output_tool_call_format_grep_pattern() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    let tool_name = ToolName::new("grep");
    let tool_args = serde_json::json!({
        "pattern": "TODO",
        "path": "/src"
    });

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name,
            args: tool_args,
        },
    );

    let tool_lines: Vec<&str> = state
        .output
        .lines
        .iter()
        .filter(|line| line.kind == LineKind::ToolCall)
        .map(|line| line.text.as_str())
        .collect();
    assert!(tool_lines.len() >= 2, "grep should render multi-row");
    let text = tool_lines.join("\n");
    assert!(
        text.contains("grep:"),
        "tool call should include 'grep:' label, got: {}",
        text
    );
    assert!(
        text.contains("TODO"),
        "tool call should include pattern, got: {}",
        text
    );
    assert!(
        tool_lines.iter().any(|line| line.contains("TODO")),
        "pattern should appear on its own tool-call row, got: {}",
        text
    );
}

#[test]
fn apply_agent_output_tool_call_format_file_create_truncates_content_preview() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    let tool_name = ToolName::new("file_create");
    let tool_args = serde_json::json!({
        "path": "/tmp/demo.txt",
        "content": "line1\nline2\nline3\nline4\nline5"
    });

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name,
            args: tool_args,
        },
    );

    let tool_lines: Vec<&str> = state
        .output
        .lines
        .iter()
        .filter(|line| line.kind == LineKind::ToolCall)
        .map(|line| line.text.as_str())
        .collect();
    let text = tool_lines.join("\n");
    assert!(
        text.contains("file_create: /tmp/demo.txt"),
        "tool call should include file path, got: {}",
        text
    );
    assert!(
        tool_lines.iter().any(|line| line.contains("line1"))
            && tool_lines.iter().any(|line| line.contains("line2"))
            && tool_lines.iter().any(|line| line.contains("line3")),
        "file_create should include first three content lines, got: {}",
        text
    );
    assert!(
        !text.contains("line4") && !text.contains("line5"),
        "file_create preview must truncate after three lines, got: {}",
        text
    );
    assert!(
        text.contains("... (+2 more lines)"),
        "file_create preview should report omitted line count, got: {}",
        text
    );
}

#[test]
fn apply_agent_output_tool_call_rows_do_not_store_embedded_newlines() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: ToolName::new("bash"),
            args: serde_json::json!({
                "description": "Run tests",
                "command": "cargo test"
            }),
        },
    );
    let tool_lines: Vec<&str> = state
        .output
        .lines
        .iter()
        .filter(|line| line.kind == LineKind::ToolCall)
        .map(|line| line.text.as_str())
        .collect();
    assert!(
        tool_lines.len() >= 2,
        "bash formatter should render multiple rows"
    );
    assert!(
        tool_lines.iter().all(|line| !line.contains('\n')),
        "each tool-call row must be stored as a single logical line"
    );
}

#[test]
fn apply_agent_output_tool_call_format_unknown_tool() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    let tool_name = ToolName::new("custom_tool");
    let tool_args = serde_json::json!({
        "param": "value"
    });

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name,
            args: tool_args,
        },
    );

    let tool_line = state
        .output
        .lines
        .iter()
        .find(|line| line.kind == LineKind::ToolCall)
        .expect("must have a ToolCall line");

    let text = tool_line.text.as_str();
    assert!(
        text.contains("custom_tool:"),
        "tool call should include tool name, got: {}",
        text
    );
    assert!(
        text.contains("value"),
        "tool call should include extracted value, got: {}",
        text
    );
}

/// Verifies that `Done` (emitted when `AssistantMessage` arrives) resets scroll to bottom.
/// This ensures streamed responses display their final content visible on screen.
#[test]
fn apply_agent_output_done_resets_scroll_to_bottom_when_at_bottom() {
    use augur_tui::domain::string_newtypes::OutputText;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    // Add content and verify we're at the bottom (scroll_offset == 0)
    state.push_output_token(OutputText::new("Hello"));
    assert_eq!(
        *state.output.scroll_offset.get(),
        0,
        "Should start at bottom"
    );

    // Apply Done (which should add newlines and keep scroll at bottom)
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);

    // Verify scroll is still at bottom
    assert_eq!(
        *state.output.scroll_offset.get(),
        0,
        "Done should keep scroll at bottom"
    );

    // Verify closing newlines were added
    let lines = &state.output.lines;
    assert!(lines.len() >= 2, "Done should have added closing newlines");
    assert!(
        lines[lines.len() - 1].text.as_str().is_empty(),
        "Last line should be empty (closing newline)"
    );
    assert!(
        lines[lines.len() - 2].text.as_str().is_empty(),
        "Second-to-last line should be empty (closing newline)"
    );
}

/// Verifies that `TurnComplete` (emitted when `SessionIdle` arrives) also resets scroll.
#[test]
fn apply_agent_output_turn_complete_resets_scroll_to_bottom_when_at_bottom() {
    use augur_tui::domain::string_newtypes::OutputText;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    // Add content and verify we're at the bottom
    state.push_output_token(OutputText::new("Response"));
    assert_eq!(
        *state.output.scroll_offset.get(),
        0,
        "Should start at bottom"
    );

    // Apply TurnComplete
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::TurnComplete);

    // Verify scroll is still at bottom
    assert_eq!(
        *state.output.scroll_offset.get(),
        0,
        "TurnComplete should keep scroll at bottom"
    );
}

/// Verifies that `finish_turn_output` is idempotent: calling `Done` twice appends
/// exactly 2 blank lines (not 4).  Both `Done` and `TurnComplete` invoke
/// `finish_turn_output`; when both fire for the same turn the second call must
/// be a no-op.
#[test]
fn finish_turn_output_is_idempotent_second_call_adds_no_lines() {
    use augur_tui::domain::string_newtypes::OutputText;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    state.push_output_token(OutputText::new("Hello"));
    let lines_before_turn_end = state.output.lines.len();

    // First Done - should append exactly 2 blank lines.
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);
    let lines_after_first = state.output.lines.len();

    // Second TurnComplete for the same turn - must be a no-op (no extra blanks).
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::TurnComplete);
    let lines_after_second = state.output.lines.len();

    let added_by_first = lines_after_first - lines_before_turn_end;
    assert_eq!(
        added_by_first, 2,
        "first Done must append exactly 2 blank lines, got {added_by_first}"
    );
    assert_eq!(
        lines_after_second, lines_after_first,
        "second TurnComplete must not append any lines (idempotent), \
         line count changed from {lines_after_first} to {lines_after_second}"
    );
}

/// Verifies that `push_user_input_line` resets the idempotency guard so the
/// next call to `finish_turn_output` (next agent turn) appends blank lines again.
#[test]
fn finish_turn_output_resets_after_user_input() {
    use augur_tui::domain::newtypes::TimestampMs;
    use augur_tui::domain::string_newtypes::OutputText;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    // First turn: token → Done.
    state.push_output_token(OutputText::new("Turn one"));
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);
    let lines_after_first_turn = state.output.lines.len();

    // User sends next message - must reset the guard.
    state.push_user_input_line(OutputText::new("next prompt"), TimestampMs::new(0));

    // Second turn: token → Done must append 2 blank lines again.
    state.push_output_token(OutputText::new("Turn two"));
    let lines_before_second_end = state.output.lines.len();
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);
    let lines_after_second_turn = state.output.lines.len();

    let added_by_second = lines_after_second_turn - lines_before_second_end;
    assert_eq!(
        added_by_second, 2,
        "Done for second turn must append 2 blank lines after user input resets the guard, \
         got {added_by_second}"
    );
    // Sanity: first turn did produce some lines.
    assert!(
        lines_after_first_turn > 0,
        "first turn should have produced output lines"
    );
}

/// Verifies that a background agent turn (no preceding user-input line) still
/// appends its closing blank lines when `Done` fires.
///
/// Before the fix, `is_turn_complete` stays `true` from the previous turn because
/// only `push_user_input_line` resets it - background agents start without a user
/// message, so the guard is never re-armed and `finish_turn_output` returns early
/// (adds 0 lines instead of 2).
///
/// After the fix, the first `AgentOutput::Token` of the new turn resets
/// `is_turn_complete = false` so that the subsequent `Done` appends exactly 2
/// closing blank lines.
#[test]
fn finish_turn_output_resets_on_background_agent_token_without_user_input() {
    use augur_tui::domain::string_newtypes::OutputText;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    // Turn 1: arrive via AgentOutput::Token so `handle_token_output` is exercised.
    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::Token(OutputText::new("Turn one")),
    );
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);
    // Guard is now true; no user input follows (background-agent scenario).

    // Background agent: a Token arrives without any preceding push_user_input_line.
    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::Token(OutputText::new("Turn two")),
    );
    let lines_before_second_end = state.output.lines.len();

    // Done for the background-agent turn must still append 2 closing blank lines.
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);
    let lines_after_second_end = state.output.lines.len();

    let added = lines_after_second_end - lines_before_second_end;
    assert_eq!(
        added, 2,
        "Done for a background-agent turn must append exactly 2 blank lines even without \
         a preceding user-input line, but got {added}"
    );
}

/// Verifies that `Error` then `Done` appends blank lines only once.
///
/// `handle_error_output` must set `is_turn_complete = true` after calling
/// `push_turn_end` so that a subsequent `Done` event is a no-op and does not
/// append a second set of blank lines.
#[test]
fn error_then_done_appends_only_one_set_of_blank_lines() {
    use augur_tui::domain::string_newtypes::OutputText;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    // Arm the turn: a token arrives so thinking is active.
    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::Token(OutputText::new("partial")),
    );

    // Error fires, should push_turn_end and set is_turn_complete.
    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::Error(OutputText::new("something failed")),
    );
    let lines_after_error = state.output.lines.len();

    // Done fires for the same turn - must be a no-op (guard already set).
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);
    let lines_after_done = state.output.lines.len();

    assert_eq!(
        lines_after_done, lines_after_error,
        "Done after Error must not append any additional lines, \
         but line count changed from {lines_after_error} to {lines_after_done}"
    );
}

/// Verifies that `Interrupted` then `Done` appends blank lines only once.
///
/// `handle_interrupted_output` must set `is_turn_complete = true` after calling
/// `push_turn_end` so that a subsequent `Done` event is a no-op.
#[test]
fn interrupted_then_done_appends_only_one_set_of_blank_lines() {
    use augur_tui::domain::string_newtypes::OutputText;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    // Simulate a turn in progress: push a token and manually activate thinking.
    // (thinking.is_active is set by the TUI actor's submit handler, not by Token.)
    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::Token(OutputText::new("partial")),
    );
    state.agent.thinking.is_active = true.into();

    // Interrupted fires - push_turn_end branch executes because is_active = true.
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Interrupted);
    let lines_after_interrupted = state.output.lines.len();

    // Done fires for the same turn - must be a no-op.
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);
    let lines_after_done = state.output.lines.len();

    assert_eq!(
        lines_after_done, lines_after_interrupted,
        "Done after Interrupted must not append any additional lines, \
         but line count changed from {lines_after_interrupted} to {lines_after_done}"
    );
}

/// Verifies that `reset_for_new_session` re-arms the guard so the next `Done`
/// appends its closing blank lines as expected.
///
/// After `Done` sets `is_turn_complete = true`, calling `reset_for_new_session`
/// must clear it so that the next `Done` in the new session fires normally.
#[test]
fn reset_for_new_session_allows_subsequent_finish() {
    use augur_tui::domain::string_newtypes::OutputText;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    // Turn 1: token → Done sets is_turn_complete = true.
    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::Token(OutputText::new("turn one")),
    );
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);
    assert!(
        state.agent.is_turn_complete,
        "Done must set is_turn_complete"
    );

    // Reset clears the guard and all output.
    state.reset_for_new_session();
    assert!(
        !state.agent.is_turn_complete,
        "reset_for_new_session must clear is_turn_complete"
    );

    // New session: token → Done should append 2 blank lines.
    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::Token(OutputText::new("turn two")),
    );
    let lines_before = state.output.lines.len();
    augur_tui::domain::tui_input::apply_agent_output(&mut state, AgentOutput::Done);
    let lines_after = state.output.lines.len();

    let added = lines_after - lines_before;
    assert_eq!(
        added, 2,
        "Done after reset_for_new_session must append exactly 2 blank lines, got {added}"
    );
}

// ── Token Tracker: UsageSnapshot TUI event ────────────────────────────────────

/// Verifies AgentOutput::UsageSnapshot variant can be pattern-matched (compile-time check).
#[test]
fn test_usage_snapshot_variant_defined() {
    use augur_tui::domain::types::ProjectTokenTotals;
    let output = AgentOutput::UsageSnapshot(ProjectTokenTotals::default());
    assert!(matches!(output, AgentOutput::UsageSnapshot(_)));
}

/// Verifies apply_agent_output with UsageSnapshot updates state.status.token_totals.
#[test]
fn test_apply_agent_output_usage_snapshot_updates_status() {
    use augur_tui::domain::newtypes::TokenCount;
    use augur_tui::domain::types::ProjectTokenTotals;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    assert_eq!(state.status.token_totals.tokens_in, TokenCount::ZERO);

    let totals = ProjectTokenTotals {
        tokens_in: TokenCount::new(800),
        ..Default::default()
    };
    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::UsageSnapshot(totals),
    );
    assert_eq!(state.status.token_totals.tokens_in, TokenCount::new(800));
}

/// Verifies apply_agent_output with UsageSnapshot only changes token_totals, not other fields.
#[test]
fn test_apply_agent_output_usage_snapshot_does_not_mutate_other_fields() {
    use augur_tui::domain::string_newtypes::ModelLabel;
    use augur_tui::domain::types::ProjectTokenTotals;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.status.model_display = ModelLabel::new("claude-3");
    state.status.git_branch = Some("main".into());

    let lines_before = state.output.lines.len();
    let prompt_before = state.prompt.buffer.to_string();

    augur_tui::domain::tui_input::apply_agent_output(
        &mut state,
        AgentOutput::UsageSnapshot(ProjectTokenTotals::default()),
    );

    // Other fields must be unchanged
    assert_eq!(
        state.status.model_display,
        ModelLabel::new("claude-3"),
        "model_display must not change"
    );
    assert!(
        matches!(&state.status.git_branch, Some(b) if b.as_str() == "main"),
        "git_branch must not change"
    );
    assert_eq!(
        state.output.lines.len(),
        lines_before,
        "output lines must not change"
    );
    assert_eq!(
        state.prompt.buffer.to_string(),
        prompt_before,
        "prompt buffer must not change"
    );
}
