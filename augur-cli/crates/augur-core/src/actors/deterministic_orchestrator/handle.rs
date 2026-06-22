//! Handle for the deterministic orchestrator actor.

use super::commands::DeterministicOrchestratorCmd;
use crate::domain::deterministic_orchestrator::DeterministicOrchestratorEvent;
use augur_domain::domain::string_newtypes::{FeatureContext, FeatureSlug, StringNewtype};
use augur_domain::domain::types::AutomatedUserMessage;
use tokio::sync::{broadcast, mpsc};

/// Pipeline resume behavior for deterministic orchestrator startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineResumeMode {
    ResumeExisting,
    StartFresh,
}

impl PipelineResumeMode {
    fn as_bool(self) -> bool {
        matches!(self, Self::ResumeExisting)
    }
}

/// Public handle for sending commands to and subscribing to deterministic
/// orchestrator runtime events.
#[derive(Clone)]
pub struct DeterministicOrchestratorHandle {
    /// Sending half of the runtime command channel.
    pub(crate) cmd_tx: mpsc::Sender<DeterministicOrchestratorCmd>,
    /// Broadcast sender shared by all event subscribers.
    pub(crate) event_tx: broadcast::Sender<DeterministicOrchestratorEvent>,
    /// Broadcast sender for automated user messages fed back to the LLM.
    pub(crate) auto_msg_tx: broadcast::Sender<AutomatedUserMessage>,
}

impl DeterministicOrchestratorHandle {
    /// Creates a handle from the actor's shared channel endpoints.
    ///
    /// Inputs:
    /// - `cmd_tx`: sending half of the runtime command mpsc channel.
    /// - `event_tx`: broadcast sender for orchestrator events.
    /// - `auto_msg_tx`: broadcast sender for automated user messages.
    pub(crate) fn new(
        cmd_tx: mpsc::Sender<DeterministicOrchestratorCmd>,
        event_tx: broadcast::Sender<DeterministicOrchestratorEvent>,
        auto_msg_tx: broadcast::Sender<AutomatedUserMessage>,
    ) -> Self {
        Self {
            cmd_tx,
            event_tx,
            auto_msg_tx,
        }
    }

    /// Begins runtime execution from the actor-owned repository root.
    ///
    /// Inputs:
    /// - `feature_context`: combined user message and attachment content, if any.
    ///   When `None`, the pipeline relies on conversation history as context.
    /// - `feature_slug`: user-supplied slug override, if provided via `--slug`.
    ///   When `None`, the slug is derived from `feature_context` at runtime.
    /// - `resume`: when `true`, steps whose output artifacts already exist on disk
    ///   are skipped; the pipeline starts from the first incomplete step.
    ///
    /// Side effects:
    /// - Sends `DeterministicOrchestratorCmd::Start` to the actor.
    ///
    /// Outputs:
    /// - Returns `()` after enqueueing a start request attempt.
    ///
    /// Invariants:
    /// - The resume contract stays semantic and never exposes primitive runtime
    ///   flags in the public API surface.
    pub fn start(
        &self,
        feature_context: Option<FeatureContext>,
        feature_slug: Option<FeatureSlug>,
        resume: PipelineResumeMode,
    ) {
        let _ = self.cmd_tx.try_send(DeterministicOrchestratorCmd::Start {
            feature_context: feature_context.map(StringNewtype::into_inner),
            feature_slug: feature_slug.map(StringNewtype::into_inner),
            resume: resume.as_bool(),
        });
    }

    /// Returns a fresh broadcast receiver for deterministic runtime events.
    pub fn subscribe(&self) -> broadcast::Receiver<DeterministicOrchestratorEvent> {
        self.event_tx.subscribe()
    }

    /// Returns a fresh broadcast receiver for automated user messages.
    ///
    /// Receivers created here receive messages the orchestrator emits to be
    /// fed back to the LLM actor as if the user had typed them.
    pub fn subscribe_automated_messages(&self) -> broadcast::Receiver<AutomatedUserMessage> {
        self.auto_msg_tx.subscribe()
    }

    /// Requests a graceful actor shutdown.
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.try_send(DeterministicOrchestratorCmd::Shutdown);
    }
}
