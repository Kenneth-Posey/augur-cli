/// Verifies that recv_supervisor goes dormant (never resolves) when the broadcast
/// channel is closed, rather than returning immediately and causing a spin loop.
/// A 50ms timeout is used to confirm the future stays pending indefinitely.
#[tokio::test]
async fn recv_supervisor_is_dormant_when_channel_closed() {
    use crate::domain::types::SupervisorEvent;
    use tokio::sync::broadcast;

    let (tx, mut rx) = broadcast::channel::<SupervisorEvent>(4);
    drop(tx);

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        super::recv_supervisor(Some(&mut rx)),
    )
    .await;

    assert!(
        result.is_err(),
        "recv_supervisor must not resolve when channel is closed"
    );
}

/// Verifies that recv_supervisor resolves with the sent event when the channel
/// is open and a message is available.
#[tokio::test]
async fn recv_supervisor_resolves_when_event_sent() {
    use crate::domain::plan_tree::{PlanTree, PlanTreeId};
    use crate::domain::string_newtypes::StringNewtype;
    use crate::domain::types::SupervisorEvent;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    let (tx, mut rx) = broadcast::channel::<SupervisorEvent>(4);
    let tree = Arc::new(PlanTree {
        id: PlanTreeId::new("t"),
        title: "T".into(),
        goal: "g".into(),
        root: crate::domain::plan_tree::PlanNode::new_branch("r", "Root"),
    });
    tx.send(SupervisorEvent::PlanGenerated(tree.clone()))
        .unwrap();

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        super::recv_supervisor(Some(&mut rx)),
    )
    .await;

    assert!(
        result.is_ok(),
        "recv_supervisor must resolve when an event is available"
    );
    assert!(matches!(
        result.unwrap(),
        Some(Ok(SupervisorEvent::PlanGenerated(_)))
    ));
}

/// Verifies that numeric_choice returns the matching choice text for a valid
/// 1-based integer string within the bounds of the choices slice.
#[test]
fn numeric_choice_returns_matching_choice_for_valid_index() {
    use crate::domain::string_newtypes::{ChoiceText, StringNewtype};

    let choices = vec![
        ChoiceText::new("alpha"),
        ChoiceText::new("beta"),
        ChoiceText::new("gamma"),
    ];
    let result = super::numeric_choice("2", &choices);
    assert_eq!(result, Some(ChoiceText::new("beta")));
}

/// Verifies that numeric_choice returns None when the 1-based index is out of
/// range, allowing the caller to fall back to the raw freeform string.
#[test]
fn numeric_choice_returns_none_for_out_of_range_index() {
    use crate::domain::string_newtypes::{ChoiceText, StringNewtype};

    let choices = vec![ChoiceText::new("alpha")];
    let result = super::numeric_choice("5", &choices);
    assert_eq!(result, None);
}

/// Verifies that handle_supervisor_event with DisplayOutput(IntentMessage)
/// forwards the intent text into the output pane via apply_agent_output.
#[test]
fn display_output_intent_message_appears_in_output_pane() {
    use crate::domain::string_newtypes::EndpointName;
    use crate::domain::string_newtypes::{OutputText, StringNewtype};
    use crate::domain::tui_state::{AppScreen, AppState};
    use crate::domain::types::{AgentOutput, SupervisorEvent};

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let output = AgentOutput::IntentMessage(OutputText::new("searching for config files"));
    super::handle_supervisor_event(&mut state, SupervisorEvent::DisplayOutput(output));

    let found = state
        .output
        .lines
        .iter()
        .any(|l| l.text.as_str().contains("searching for config files"));
    assert!(
        found,
        "intent message content must appear in the output pane"
    );
}

/// BUG 1 regression: dispatch_plan_esc must return true when it handles the transition,
/// indicating that dispatch_plan_key should not fall through to dispatch_chat_key.
#[test]
fn dispatch_plan_esc_returns_true_when_transitioning_to_chat() {
    use crate::actors::tui::assistant::key_dispatch::dispatch_plan_esc;
    use crate::domain::plan_tree::{PlanTree, PlanTreeId};
    use crate::domain::string_newtypes::{EndpointName, StringNewtype};
    use crate::domain::tui_state::{
        AppScreen, AppState, ConversationMode, PlanModeState, SecondaryView,
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let tree = PlanTree {
        id: PlanTreeId::new("t"),
        title: "T".into(),
        goal: "g".into(),
        root: crate::domain::plan_tree::PlanNode::new_branch("r", "Root"),
    };
    state.interaction.mode = ConversationMode::Plan(PlanModeState {
        tree,
        running: false,
        tree_scroll: crate::domain::newtypes::ScrollOffset::of(0),
    });
    state.interaction.panel.secondary_view = Some(SecondaryView::Ask);

    let handled = dispatch_plan_esc(&mut state);

    assert!(
        handled.is_some(),
        "dispatch_plan_esc must return true when it transitions mode to Chat"
    );
    assert!(matches!(state.interaction.mode, ConversationMode::Chat));
    // secondary_view must be untouched - only mode transitions
    assert_eq!(
        state.interaction.panel.secondary_view,
        Some(SecondaryView::Ask)
    );
}

/// BUG 3 regression: scroll in secondary region must not route to plan panel.
#[test]
fn plan_mode_scroll_in_secondary_region_does_not_route_to_plan_panel() {
    use crate::domain::plan_tree::{PlanTree, PlanTreeId};
    use crate::domain::string_newtypes::{EndpointName, StringNewtype};
    use crate::domain::tui_state::{AppScreen, AppState, ConversationMode, PlanModeState};
    use crossterm::event::{MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let tree = PlanTree {
        id: PlanTreeId::new("t"),
        title: "T".into(),
        goal: "g".into(),
        root: crate::domain::plan_tree::PlanNode::new_branch("r", "Root"),
    };
    state.interaction.mode = ConversationMode::Plan(PlanModeState {
        tree,
        running: false,
        tree_scroll: crate::domain::newtypes::ScrollOffset::of(0),
    });

    // Simulate three-pane layout: output_area = primary feed (cols 0..90),
    // plan_panel_area = cols 150..200
    state.output.panel_areas.output_area.set(Rect {
        x: 0,
        y: 0,
        width: 90,
        height: 40,
    });
    state.output.panel_areas.plan_panel_area.set(Rect {
        x: 150,
        y: 0,
        width: 50,
        height: 40,
    });

    // A scroll at column 100 is in the secondary container (cols 90..149), NOT plan panel
    let scroll_event = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 100,
        row: 10,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };

    let initial_scroll = if let ConversationMode::Plan(ref ps) = state.interaction.mode {
        ps.tree_scroll
    } else {
        panic!("not plan mode")
    };

    super::handle_plan_mouse_scroll(&mut state, scroll_event);

    let final_scroll = if let ConversationMode::Plan(ref ps) = state.interaction.mode {
        ps.tree_scroll
    } else {
        panic!("not plan mode")
    };

    assert_eq!(
        initial_scroll, final_scroll,
        "scroll in secondary region must not change tree_scroll"
    );
}

/// Verifies that handle_supervisor_event with DisplayOutput(ToolProgress)
/// forwards the progress text as a tool-call line in the output pane.
#[test]
fn display_output_tool_progress_appears_in_output_pane() {
    use crate::domain::string_newtypes::EndpointName;
    use crate::domain::string_newtypes::{OutputText, StringNewtype};
    use crate::domain::tui_state::{AppScreen, AppState};
    use crate::domain::types::{AgentOutput, SupervisorEvent};

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let output = AgentOutput::ToolProgress {
        tool_call_id: "tc-1".into(),
        message: OutputText::new("reading 3 files"),
    };
    super::handle_supervisor_event(&mut state, SupervisorEvent::DisplayOutput(output));

    let found = state
        .output
        .lines
        .iter()
        .any(|l| l.text.as_str().contains("reading 3 files"));
    assert!(
        found,
        "tool progress message must appear in the output pane"
    );
}

fn make_plan_tree_with_leaf() -> crate::domain::plan_tree::PlanTree {
    use crate::domain::plan_tree::{PlanNode, PlanTree, PlanTreeId};
    use crate::domain::string_newtypes::StringNewtype;

    PlanTree {
        id: PlanTreeId::new("plan-1"),
        title: "Coverage Plan".into(),
        goal: "close the gap".into(),
        root: PlanNode::new_branch("root", "Root").add_child(PlanNode::new_leaf(
            "step-1",
            "Implement coverage",
            "steps/step-1.md",
        )),
    }
}

/// Verifies that `PlanGenerated` enters plan mode with the received tree snapshot.
#[test]
fn handle_supervisor_event_plan_generated_enters_plan_mode() {
    use crate::domain::string_newtypes::{EndpointName, StringNewtype};
    use crate::domain::tui_state::{AppScreen, AppState, ConversationMode};
    use crate::domain::types::SupervisorEvent;
    use std::sync::Arc;

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    super::handle_supervisor_event(
        &mut state,
        SupervisorEvent::PlanGenerated(Arc::new(make_plan_tree_with_leaf())),
    );

    match &state.interaction.mode {
        ConversationMode::Plan(plan_state) => {
            assert_eq!(plan_state.tree.title, "Coverage Plan");
            assert_eq!(plan_state.tree.root.children.len(), 1);
            assert!(
                !plan_state.running,
                "new plan snapshot starts in preview mode"
            );
        }
        _ => panic!("PlanGenerated must enter ConversationMode::Plan"),
    }
}

/// Verifies that step lifecycle events mutate the active plan node status through started, completed, and failed states.
#[test]
fn handle_supervisor_event_step_lifecycle_mutates_node_status() {
    use crate::domain::plan_tree::{NodeStatus, PlanNodeId};
    use crate::domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
    use crate::domain::tui_state::{AppScreen, AppState, ConversationMode, PlanModeState};
    use crate::domain::types::SupervisorEvent;

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::Plan(PlanModeState {
        tree: make_plan_tree_with_leaf(),
        running: true,
        tree_scroll: crate::domain::newtypes::ScrollOffset::of(0),
    });

    super::handle_supervisor_event(
        &mut state,
        SupervisorEvent::StepStarted(PlanNodeId::new("step-1")),
    );
    match &state.interaction.mode {
        ConversationMode::Plan(plan_state) => assert_eq!(
            plan_state.tree.root.children[0].status,
            NodeStatus::InProgress
        ),
        _ => panic!("expected plan mode"),
    }

    super::handle_supervisor_event(
        &mut state,
        SupervisorEvent::StepCompleted(PlanNodeId::new("step-1")),
    );
    match &state.interaction.mode {
        ConversationMode::Plan(plan_state) => {
            assert_eq!(plan_state.tree.root.children[0].status, NodeStatus::Done)
        }
        _ => panic!("expected plan mode"),
    }

    super::handle_supervisor_event(
        &mut state,
        SupervisorEvent::StepFailed {
            id: PlanNodeId::new("step-1"),
            reason: OutputText::new("cargo test failed"),
        },
    );
    match &state.interaction.mode {
        ConversationMode::Plan(plan_state) => assert_eq!(
            plan_state.tree.root.children[0].status,
            NodeStatus::Failed("cargo test failed".into())
        ),
        _ => panic!("expected plan mode"),
    }
}

/// Verifies that `ExecutionComplete` clears the plan state's running flag without leaving plan mode.
#[test]
fn handle_supervisor_event_execution_complete_stops_running_plan() {
    use crate::domain::string_newtypes::{EndpointName, StringNewtype};
    use crate::domain::tui_state::{AppScreen, AppState, ConversationMode, PlanModeState};
    use crate::domain::types::SupervisorEvent;

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::Plan(PlanModeState {
        tree: make_plan_tree_with_leaf(),
        running: true,
        tree_scroll: crate::domain::newtypes::ScrollOffset::of(0),
    });

    super::handle_supervisor_event(&mut state, SupervisorEvent::ExecutionComplete);

    match &state.interaction.mode {
        ConversationMode::Plan(plan_state) => {
            assert!(!plan_state.running, "ExecutionComplete must clear running")
        }
        _ => panic!("ExecutionComplete must keep the UI in plan mode"),
    }
}

/// Verifies that supervisor failure events append a visible error line to the output pane.
#[test]
fn handle_supervisor_event_failure_appends_output_line() {
    use crate::domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
    use crate::domain::tui_state::{AppScreen, AppState};
    use crate::domain::types::SupervisorEvent;

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    super::handle_supervisor_event(
        &mut state,
        SupervisorEvent::Failed {
            reason: OutputText::new("planner crashed"),
        },
    );

    let output = state
        .output
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        output.contains("Supervisor error: planner crashed"),
        "failure reason must be visible in the output pane"
    );
}
