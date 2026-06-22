//! Guided-plan event helpers for the TUI actor.

use super::TuiHandles;
use crate::domain::tui_state::{
    current_timestamp_ms, AppState, ConversationMode, OutputLine, PendingResponseMeta,
};
use augur_domain::domain::guided_plan::{GuidedPlanEvent, PhaseStatus};
use augur_domain::domain::newtypes::{NumericNewtype, PhaseIndex};
use augur_domain::domain::string_newtypes::{OutputText, PromptText, StringNewtype};
use augur_domain::domain::types::{AgentFeedOutput, SupervisorEvent};
use augur_domain::domain::AgentName;

/// Dispatch handle-owning side effects for guided-plan events.
pub(super) fn apply_guided_plan_actions(
    state: &mut AppState,
    event: &GuidedPlanEvent,
    handles: &TuiHandles<'_>,
) {
    match event {
        GuidedPlanEvent::CompactRequested => {
            handles.agent.compact();
        }
        GuidedPlanEvent::CommitRequested => {
            let ep = state.agent.endpoint_name.clone();
            state.agent.thinking.is_active = true.into();
            state.agent.thinking.label = "Committing...".into();
            state.agent.pending_response = Some(
                PendingResponseMeta::builder()
                    .ts(current_timestamp_ms())
                    .model(state.status.model_display.clone())
                    .build(),
            );
            handles
                .agent
                .submit(PromptText::new("create message and commit"), Some(ep));
        }
        _ => {}
    }
}

/// Apply a guided-plan event to visible TUI state.
pub(super) fn handle_guided_plan_event(state: &mut AppState, event: GuidedPlanEvent) {
    apply_phase_status_event(state, &event);
    apply_review_token_event(state, &event);
    apply_hook_output_event(state, &event);
    apply_plan_complete_event(state, &event);
    apply_plan_failed_event(state, &event);
    apply_compact_requested_event(state, &event);
    apply_commit_requested_event(state, &event);
}

fn apply_phase_status_event(state: &mut AppState, event: &GuidedPlanEvent) {
    if let GuidedPlanEvent::PhaseStatusChanged { phase_idx, status } = event {
        handle_phase_status_changed(state, *phase_idx, status.clone());
    }
}

fn apply_review_token_event(state: &mut AppState, event: &GuidedPlanEvent) {
    if let GuidedPlanEvent::ReviewToken(token) = event {
        handle_review_token_event(state, token.clone());
    }
}

fn apply_hook_output_event(state: &mut AppState, event: &GuidedPlanEvent) {
    if let GuidedPlanEvent::HookOutput { line, .. } = event {
        state.push_tool_call_line(line.clone());
    }
}

fn apply_plan_complete_event(state: &mut AppState, event: &GuidedPlanEvent) {
    if let GuidedPlanEvent::PlanComplete = event {
        if let ConversationMode::GuidedPlan(ref mut ui) = state.interaction.mode {
            ui.review_active = false.into();
        }
        state.push_system_message(OutputText::from("[system] guided plan complete."));
        state.push_output_newline();
    }
}

fn apply_plan_failed_event(state: &mut AppState, event: &GuidedPlanEvent) {
    if let GuidedPlanEvent::PlanFailed { reason, .. } = event {
        state.push_error_line(format!("[plan failed] {reason}"));
        state.push_output_newline();
    }
}

fn apply_compact_requested_event(state: &mut AppState, event: &GuidedPlanEvent) {
    if let GuidedPlanEvent::CompactRequested = event {
        state.push_system_message(OutputText::from(
            "[system] guided plan: compacting context...",
        ));
        state.set_guided_plan_compact_flag();
    }
}

fn apply_commit_requested_event(state: &mut AppState, event: &GuidedPlanEvent) {
    if let GuidedPlanEvent::CommitRequested = event {
        let ts = current_timestamp_ms();
        state.push_user_input_line(OutputText::from("> [guided plan] committing phase..."), ts);
        state.push_output_newline();
        state.push_output_newline();
    }
}

fn handle_phase_status_changed(state: &mut AppState, phase_idx: PhaseIndex, status: PhaseStatus) {
    if let ConversationMode::GuidedPlan(ref mut ui) = state.interaction.mode {
        if let Some(entry) = ui.phases.get_mut(phase_idx.inner()) {
            entry.1 = status.clone();
        }
        if matches!(status, PhaseStatus::InProgress) {
            ui.current_phase = phase_idx.inner();
        }
    }
}

fn handle_review_token_event(state: &mut AppState, token: OutputText) {
    let needs_header = matches!(&state.interaction.mode, ConversationMode::GuidedPlan(ui) if !bool::from(ui.review_active));
    if needs_header {
        if let ConversationMode::GuidedPlan(ref mut ui) = state.interaction.mode {
            ui.review_active = true.into();
        }
        state
            .output
            .lines
            .push(OutputLine::tool_call(OutputText::from("Reviewer:")));
        state
            .output
            .lines
            .push(OutputLine::plain(OutputText::from("")));
    }
    state.push_output_token(token);
}

/// Convert supervisor events into optional agent-feed updates.
pub(super) fn supervisor_event_to_feed(event: &SupervisorEvent) -> Option<AgentFeedOutput> {
    map_step_started_feed(event)
        .or_else(|| map_step_completed_feed(event))
        .or_else(|| map_step_failed_feed(event))
        .or_else(|| map_execution_complete_feed(event))
        .or_else(|| map_plan_generated_feed(event))
        .or_else(|| map_supervisor_failed_feed(event))
}

fn map_step_started_feed(event: &SupervisorEvent) -> Option<AgentFeedOutput> {
    if let SupervisorEvent::StepStarted(id) = event {
        Some(AgentFeedOutput::TaskStarted {
            name: AgentName::from(id.as_str()),
            model: None,
        })
    } else {
        None
    }
}

fn map_step_completed_feed(event: &SupervisorEvent) -> Option<AgentFeedOutput> {
    if let SupervisorEvent::StepCompleted(id) = event {
        Some(AgentFeedOutput::TaskCompleted {
            name: AgentName::from(id.as_str()),
        })
    } else {
        None
    }
}

fn map_step_failed_feed(event: &SupervisorEvent) -> Option<AgentFeedOutput> {
    if let SupervisorEvent::StepFailed { id, reason } = event {
        Some(AgentFeedOutput::TaskFailed {
            name: AgentName::from(id.as_str()),
            reason: OutputText::from(reason.as_str()),
        })
    } else {
        None
    }
}

fn map_execution_complete_feed(event: &SupervisorEvent) -> Option<AgentFeedOutput> {
    if let SupervisorEvent::ExecutionComplete = event {
        Some(AgentFeedOutput::StatusLine(OutputText::new(
            "All steps complete.",
        )))
    } else {
        None
    }
}

fn map_plan_generated_feed(event: &SupervisorEvent) -> Option<AgentFeedOutput> {
    if let SupervisorEvent::PlanGenerated(_) = event {
        Some(AgentFeedOutput::StatusLine(OutputText::new(
            "Plan generated.",
        )))
    } else {
        None
    }
}

fn map_supervisor_failed_feed(event: &SupervisorEvent) -> Option<AgentFeedOutput> {
    if let SupervisorEvent::Failed { reason } = event {
        Some(AgentFeedOutput::TaskFailed {
            name: AgentName::from("supervisor"),
            reason: OutputText::from(reason.as_str()),
        })
    } else {
        None
    }
}
