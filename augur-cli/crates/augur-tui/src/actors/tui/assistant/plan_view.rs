//! Plan view helpers: node status mutation, supervisor receive, numeric choice,
//! supervisor event handling, and query lifecycle functions.

use super::clipboard::paste_from_clipboard;
use crate::domain::tui_input::{MOUSE_SCROLL_LINES, MouseAction, classify_mouse};
use crate::domain::tui_state::{
    AppState, ConversationMode, OutputLine, PlanModeState, QueryState, current_timestamp_ms,
};
use augur_domain::domain::newtypes::{Count, ScrollOffset};
use augur_domain::domain::plan_tree::{NodeStatus, PlanNodeId};
use augur_domain::domain::string_newtypes::{
    ChoiceText, FailureReason, OutputText, PromptText, StringNewtype,
};
use augur_domain::domain::types::SupervisorEvent;
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use tokio::sync::broadcast;

/// Status label text shown while the agent is processing a response.
const THINKING_LABEL: &str = "Thinking...";

/// Mutate the plan tree node with `id` to `status` when in `ConversationMode::Plan`.
///
/// No-op when not in plan mode or when the node id is not found in the tree.
///
/// Consumers: `handle_supervisor_event` in `actor.rs` for step lifecycle events.
pub(crate) fn update_plan_node_status(state: &mut AppState, id: &PlanNodeId, status: NodeStatus) {
    if let ConversationMode::Plan(ref mut ps) = state.interaction.mode {
        ps.tree.update_node_status(id, status);
    }
}

/// Map a trimmed freeform string to a choice text when it looks like a 1-based index.
///
/// Returns the corresponding choice text when `s` parses as a 1-based integer within
/// the bounds of `choices`. Returns `None` for non-numeric input or out-of-range
/// values, allowing the caller to fall back to the raw freeform string.
///
/// Consumers: `resolve_query_answer` in `actor.rs`.
fn numeric_choice(s: &str, choices: &[ChoiceText]) -> Option<ChoiceText> {
    let n: usize = s.parse().ok()?;
    if n >= 1 && n <= choices.len() {
        choices.get(n - 1).cloned()
    } else {
        None
    }
}

/// Receive from an optional supervisor broadcast channel without blocking.
///
/// Returns `std::future::pending()` when the receiver is absent, which keeps
/// the `select!` branch dormant without removing it from the select at compile
/// time. This allows the supervisor branch to exist unconditionally in the
/// select while still being a no-op when no supervisor is wired.
///
/// Also returns `std::future::pending()` when the channel has been closed
/// (all senders dropped), preventing the select! arm from spinning in a tight
/// loop when the supervisor actor has exited.
///
/// Consumers: `select_next_event` supervisor arm in `actor.rs`.
pub(crate) async fn recv_supervisor(
    rx: Option<&mut broadcast::Receiver<SupervisorEvent>>,
) -> Option<Result<SupervisorEvent, broadcast::error::RecvError>> {
    match rx {
        None => std::future::pending().await,
        Some(rx) => match rx.recv().await {
            Err(broadcast::error::RecvError::Closed) => std::future::pending().await,
            result => Some(result),
        },
    }
}

/// Handle a mouse event when in `ConversationMode::Plan`.
///
/// Routes right-clicks to paste, plan-panel scrolls to the plan tree,
/// and output-panel scrolls to the output area.
///
/// Consumers: `handle_mouse_event` in `actor.rs`.
pub(crate) fn handle_plan_mouse_scroll(state: &mut AppState, event: crossterm::event::MouseEvent) {
    use crossterm::event::{MouseButton, MouseEventKind};
    if let MouseEventKind::Down(MouseButton::Right) = event.kind {
        paste_from_clipboard(state);
        return;
    }
    if is_in_plan_panel(state, event.column) {
        handle_plan_panel_scroll(state, event.kind);
        return;
    }
    handle_output_panel_scroll(state, event);
}

fn is_in_plan_panel(state: &AppState, column: u16) -> bool {
    let plan_panel_area = state.output.panel_areas.plan_panel_area.get();
    plan_panel_area.width > 0
        && column >= plan_panel_area.x
        && column < plan_panel_area.x + plan_panel_area.width
}

fn handle_plan_panel_scroll(state: &mut AppState, kind: crossterm::event::MouseEventKind) {
    use crossterm::event::MouseEventKind;
    match kind {
        MouseEventKind::ScrollUp => state.plan_scroll_up(Count::of(MOUSE_SCROLL_LINES)),
        MouseEventKind::ScrollDown => state.plan_scroll_down(Count::of(MOUSE_SCROLL_LINES)),
        _ => {}
    }
}

fn handle_output_panel_scroll(state: &mut AppState, event: crossterm::event::MouseEvent) {
    let output_area = state.output.panel_areas.output_area.get();
    match classify_mouse(event, output_area) {
        MouseAction::ScrollUp(n) => state.scroll_up(Count::of(n)),
        MouseAction::ScrollDown(n) => state.scroll_down(Count::of(n)),
        _ => {}
    }
}

/// Transition `AppState` into `ConversationMode::Query` for the given request.
///
/// Builds a `QueryState` from the incoming request fields and sets
/// `state.interaction.mode`. No-op when `req` is `None`.
///
/// Consumers: `select_next_event` query arm in `actor.rs`.
pub(crate) fn handle_query_request(state: &mut AppState, req: Option<QueryUserRequest>) {
    let Some(r) = req else { return };
    let qs = QueryState::builder()
        .question(r.question)
        .choices(r.choices)
        .freeform(PromptText::new(""))
        .reply_tx(r.reply_tx)
        .build();
    state.interaction.mode = ConversationMode::Query(qs);
}

/// Apply a `SupervisorEvent` to `AppState`, updating the plan tree or output.
///
/// Called from `select_next_event` on every supervisor broadcast message.
/// Transitions:
/// - `PlanGenerated` → enter `ConversationMode::Plan` with the received tree snapshot.
/// - `StepStarted(id)` → mark node `InProgress` in the active tree.
/// - `StepCompleted(id)` → mark node `Done` in the active tree.
/// - `StepFailed { id, reason }` → mark node `Failed(reason)` in the active tree.
/// - `ExecutionComplete` → set `running = false` on the plan state.
/// - `Failed { reason }` → append an error line to the chat output.
/// - `CheckpointTriggered(_)` → no-op; supervisor handles commit/compact itself.
/// - `DisplayOutput(output)` → forward to `apply_agent_output` so intent,
///   progress, and partial-result lines appear in the output pane during execution.
///
/// Consumers: `select_next_event` supervisor arm in `actor.rs`.
pub(crate) fn handle_supervisor_event(state: &mut AppState, event: SupervisorEvent) {
    apply_supervisor_event(state, event);
}

fn apply_supervisor_event(state: &mut AppState, event: SupervisorEvent) {
    if let Some(step_event) = to_plan_step_event(&event) {
        apply_plan_step_event(state, step_event);
        return;
    }
    if let Some(runtime_event) = to_plan_runtime_event(&event) {
        apply_plan_runtime_event(state, runtime_event);
        return;
    }
    apply_supervisor_passthrough(state, event);
}

enum PlanStepEvent {
    Started(PlanNodeId),
    Completed(PlanNodeId),
    Failed { id: PlanNodeId, reason: OutputText },
}

fn apply_plan_step_event(state: &mut AppState, event: PlanStepEvent) {
    match event {
        PlanStepEvent::Started(id) => update_plan_node_status(state, &id, NodeStatus::InProgress),
        PlanStepEvent::Completed(id) => update_plan_node_status(state, &id, NodeStatus::Done),
        PlanStepEvent::Failed { id, reason } => {
            update_plan_node_status(state, &id, failed_status(reason.as_str()))
        }
    }
}

enum PlanRuntimeEvent {
    Done,
    Failed(OutputText),
}

fn to_plan_step_event(event: &SupervisorEvent) -> Option<PlanStepEvent> {
    match event {
        SupervisorEvent::StepStarted(id) => Some(PlanStepEvent::Started(id.clone())),
        SupervisorEvent::StepCompleted(id) => Some(PlanStepEvent::Completed(id.clone())),
        SupervisorEvent::StepFailed { id, reason } => Some(PlanStepEvent::Failed {
            id: id.clone(),
            reason: reason.clone(),
        }),
        _ => None,
    }
}

fn to_plan_runtime_event(event: &SupervisorEvent) -> Option<PlanRuntimeEvent> {
    match event {
        SupervisorEvent::ExecutionComplete => Some(PlanRuntimeEvent::Done),
        SupervisorEvent::Failed { reason } => Some(PlanRuntimeEvent::Failed(reason.clone())),
        _ => None,
    }
}

fn apply_supervisor_passthrough(state: &mut AppState, event: SupervisorEvent) {
    match event {
        SupervisorEvent::PlanGenerated(tree) => enter_plan_mode(state, tree),
        SupervisorEvent::DisplayOutput(output) => {
            crate::domain::tui_input::apply_agent_output(state, output);
        }
        SupervisorEvent::CheckpointTriggered(_)
        | SupervisorEvent::StepStarted(_)
        | SupervisorEvent::StepCompleted(_)
        | SupervisorEvent::StepFailed { .. }
        | SupervisorEvent::ExecutionComplete
        | SupervisorEvent::Failed { .. } => {}
    }
}

fn apply_plan_runtime_event(state: &mut AppState, event: PlanRuntimeEvent) {
    match event {
        PlanRuntimeEvent::Done => mark_plan_not_running(state),
        PlanRuntimeEvent::Failed(reason) => push_supervisor_error(state, reason.as_str()),
    }
}

fn enter_plan_mode(
    state: &mut AppState,
    tree: std::sync::Arc<augur_domain::domain::plan_tree::PlanTree>,
) {
    let plan_state = PlanModeState::builder()
        .tree((*tree).clone())
        .running(false.into())
        .tree_scroll(ScrollOffset::of(0))
        .build();
    state.interaction.mode = ConversationMode::Plan(plan_state);
}

fn failed_status(reason: &str) -> NodeStatus {
    NodeStatus::Failed(FailureReason::new(reason))
}

fn mark_plan_not_running(state: &mut AppState) {
    if let ConversationMode::Plan(ref mut ps) = state.interaction.mode {
        ps.running = false.into();
    }
}

fn push_supervisor_error(state: &mut AppState, reason: &str) {
    state
        .output
        .lines
        .push(OutputLine::plain(format!("Supervisor error: {}", reason)));
}

/// Resolve the user's answer from query state and send it on the oneshot channel.
///
/// Takes the `QueryState` out of `state.interaction.mode` (setting mode back to `Chat`).
/// Pushes the answer as a user-input line to the output area. Sets
/// `thinking_label` to `"Thinking..."` to indicate resumed agent processing.
/// When no answer can be determined, the query is dismissed silently.
///
/// Consumers: `dispatch_query_key` in `actor.rs`.
pub(crate) fn handle_query_submit(state: &mut AppState) {
    let Some(qs) = state.take_query_state() else {
        return;
    };
    let Some(answer) = resolve_query_answer(&qs) else {
        return;
    };
    let ts = current_timestamp_ms();
    state.push_user_input_line(OutputText::new(format!("> {}", answer)), ts);
    state.push_output_newline();
    state.push_output_newline();
    state.agent.thinking.label = THINKING_LABEL.into();
    let _ = qs.reply_tx.send(answer);
}

/// Derive the user's answer from the query state.
///
/// Resolution order:
/// 1. If `freeform` is non-empty and parses as a 1-based integer within `choices`
///    bounds, return the matching choice text (numeric shortcut).
/// 2. If `freeform` is non-empty but not a valid choice index, return it as-is.
/// 3. Otherwise return the selected choice by index.
///     - Returns `None` when both freeform and selected are absent.
///
/// Consumers: `handle_query_submit` in this module.
pub(crate) fn resolve_query_answer(qs: &QueryState) -> Option<OutputText> {
    let trimmed = qs.freeform.trim();
    if !trimmed.is_empty() {
        return numeric_choice(trimmed, &qs.choices)
            .map(|choice| OutputText::new(choice.as_str()))
            .or_else(|| Some(OutputText::new(trimmed)));
    }
    qs.selected
        .and_then(|i| qs.choices.get(i))
        .map(|choice| OutputText::new(choice.as_str()))
}
