use augur_domain::newtypes::Count;
use augur_domain::string_newtypes::{AccumulatedText, OutputText, StringNewtype};
use augur_domain::task_types::{AgentInstructions, AgentSpecName, TaskSignal};
use augur_domain::tool_types::ToolDefinition;
use augur_domain::types::{AgentFeedOutput, Message};
use augur_provider_openrouter::actors::openrouter_task::openrouter_task_actor_ops::{
    build_task_system_prompt, is_at_iteration_limit, prepend_prefix, signal_to_feed_event,
};

#[test]
fn build_task_system_prompt_returns_instructions_when_no_tools_are_registered() {
    let instructions = AgentInstructions::new("keep this prompt");
    let prompt = build_task_system_prompt(&instructions, &[]);
    assert_eq!(prompt.as_str(), "keep this prompt");
}

#[test]
fn build_task_system_prompt_includes_tool_list_and_size_check_guidance() {
    let tools = vec![
        ToolDefinition::new("shell_exec", "Run shell commands", serde_json::json!({})),
        ToolDefinition::new("size_check", "Estimate output size", serde_json::json!({})),
    ];
    let prompt = build_task_system_prompt(&AgentInstructions::new("base"), &tools);
    assert!(prompt.as_str().contains("## Available tools"));
    assert!(prompt
        .as_str()
        .contains("**shell_exec**: Run shell commands"));
    assert!(prompt
        .as_str()
        .contains("call `size_check` before heavy reads/searches"));
}

#[test]
fn prepend_prefix_places_prefix_messages_before_existing_messages() {
    let prefix = augur_domain::task_types::InstructionPrefix(vec![
        Message::user("prefix-1"),
        Message::assistant("prefix-2"),
    ]);
    let combined = prepend_prefix(&prefix, &[Message::user("live-message")]);
    let contents = combined
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>();
    assert_eq!(contents, vec!["prefix-1", "prefix-2", "live-message"]);
}

#[test]
fn signal_to_feed_event_maps_all_task_signal_variants() {
    let name = AgentSpecName::new("planner");
    let completed = signal_to_feed_event(
        &name,
        &TaskSignal::Completed {
            output: AccumulatedText::new("ok"),
        },
    );
    assert!(matches!(completed, AgentFeedOutput::TaskCompleted { .. }));

    let failed = signal_to_feed_event(
        &name,
        &TaskSignal::Failed {
            reason: OutputText::new("boom"),
        },
    );
    assert!(matches!(failed, AgentFeedOutput::TaskFailed { .. }));

    let cancelled = signal_to_feed_event(&name, &TaskSignal::Cancelled);
    match cancelled {
        AgentFeedOutput::TaskFailed { reason, .. } => assert_eq!(reason.as_str(), "cancelled"),
        _ => panic!("cancelled should map to TaskFailed"),
    }
}

#[test]
fn is_at_iteration_limit_returns_true_at_or_above_max() {
    assert!(bool::from(is_at_iteration_limit(
        Count::of(2),
        Count::of(2)
    )));
    assert!(bool::from(is_at_iteration_limit(
        Count::of(3),
        Count::of(2)
    )));
    assert!(!bool::from(is_at_iteration_limit(
        Count::of(1),
        Count::of(2)
    )));
}
