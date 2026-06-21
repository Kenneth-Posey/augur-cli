use augur_domain::background_events::{BackgroundEventClassifier, BackgroundEventPriority};
use copilot_sdk::SessionEventData;
use std::any::Any;

/// Copilot-specific classifier that maps SDK events into core background priorities.
pub struct CopilotEventClassifier;

impl BackgroundEventClassifier for CopilotEventClassifier {
    fn classify(&self, raw_event: &dyn Any) -> Option<BackgroundEventPriority> {
        // `Any` downcast requires `'static`; `SessionEventData` is expected to remain `'static`.
        let event = raw_event.downcast_ref::<SessionEventData>()?;
        use SessionEventData as E;

        match event {
            E::SessionStart(_)
            | E::SessionError(_)
            | E::SessionShutdown(_)
            | E::Abort(_)
            | E::CustomAgentFailed(_)
            | E::PermissionRequested(_) => Some(BackgroundEventPriority::Critical),

            E::UserMessage(_)
            | E::AssistantTurnStart(_)
            | E::AssistantIntent(_)
            | E::AssistantMessage(_)
            | E::AssistantMessageDelta(_)
            | E::AssistantTurnEnd(_)
            | E::ToolUserRequested(_)
            | E::ToolExecutionStart(_)
            | E::ToolExecutionComplete(_)
            | E::ToolExecutionProgress(_)
            | E::CustomAgentStarted(_)
            | E::CustomAgentCompleted(_)
            | E::CustomAgentSelected(_)
            | E::HookStart(_)
            | E::HookEnd(_)
            | E::SkillInvoked(_)
            | E::ExternalToolRequested(_)
            | E::SessionHandoff(_) => Some(BackgroundEventPriority::Informational),

            E::SessionResume(_)
            | E::SessionIdle(_)
            | E::SessionInfo(_)
            | E::SessionModelChange(_)
            | E::SessionTruncation(_)
            | E::PendingMessagesModified(_)
            | E::AssistantReasoning(_)
            | E::AssistantReasoningDelta(_)
            | E::AssistantUsage(_)
            | E::ToolExecutionPartialResult(_)
            | E::SystemMessage(_)
            | E::SessionCompactionStart(_)
            | E::SessionCompactionComplete(_)
            | E::SessionSnapshotRewind(_) => Some(BackgroundEventPriority::Debug),

            E::SessionUsageInfo(_) | E::Unknown(_) => None,
        }
    }
}
