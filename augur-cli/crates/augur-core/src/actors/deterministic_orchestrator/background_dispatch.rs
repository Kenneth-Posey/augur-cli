//! Deterministic-orchestrator background dispatch adapters.

use std::fmt;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::domain::deterministic_orchestrator::{NormalizedSignal, WorkflowArtifactRef};
use crate::domain::deterministic_orchestrator_ops::{
    DispatchRequestKind, WorkflowDispatchRequest, normalize_agent_signal,
};
use augur_domain::domain::newtypes::NumericNewtype;
use augur_domain::domain::types::{FeedEntry, FeedId};
use augur_domain::domain::{
    AccumulatedText, AgentName, ModelLabel, OutputText, PassCriterion, PromptText, StringNewtype,
    ToolCallId, WorkflowSignalValue, WorkflowStepId,
};

const PASS_SIGNAL: &str = "pass";
const FAIL_SIGNAL: &str = "fail";
const NEEDS_REVISION_SIGNAL: &str = "needs-revision";

/// Opaque ticket returned by a background dispatch submission.
pub(crate) struct AgentDispatchTicket {
    /// Request kind associated with the ticket.
    pub(crate) kind: DispatchRequestKind,
    /// Workflow step associated with the ticket.
    pub(crate) step_id: WorkflowStepId,
    /// Agent targeted by the dispatch.
    pub(crate) agent: Option<AgentName>,
    runtime: BackgroundRuntimeTicket,
}

impl fmt::Debug for AgentDispatchTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentDispatchTicket")
            .field("kind", &self.kind)
            .field("step_id", &self.step_id)
            .field("agent", &self.agent)
            .finish()
    }
}

/// Errors produced by deterministic background dispatch.
#[derive(Debug)]
pub enum DispatchError {
    /// The provided request does not match the dispatch path being invoked.
    InvalidRequest(&'static str),
    /// The requested dispatch path does not define an agent name.
    MissingAgent(&'static str),
    /// The spawned background runtime exited unexpectedly.
    RuntimeFailure(String),
}

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) => write!(f, "{message}"),
            Self::MissingAgent(message) => write!(f, "{message}"),
            Self::RuntimeFailure(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for DispatchError {}

/// Launch parameters for one background agent execution.
pub struct BackgroundAgentLaunch {
    pub agent: AgentName,
    pub feed_id: FeedId,
    pub prompt: PromptText,
    pub model: Option<ModelLabel>,
}

#[derive(bon::Builder)]
/// Runtime handles for a spawned background agent session.
pub struct BackgroundRuntimeTicket {
    task: JoinHandle<()>,
    feed_rx: mpsc::Receiver<FeedEntry>,
    /// Receives the full accumulated response text when the session completes normally.
    ///
    /// `None` for test-double runtimes that do not run real SDK sessions.
    signal_rx: Option<tokio::sync::oneshot::Receiver<AccumulatedText>>,
}

impl BackgroundRuntimeTicket {
    /// Construct a runtime ticket from join and feed handles.
    pub fn new(
        task: JoinHandle<()>,
        feed_rx: mpsc::Receiver<FeedEntry>,
        signal_rx: Option<tokio::sync::oneshot::Receiver<AccumulatedText>>,
    ) -> Self {
        Self {
            task,
            feed_rx,
            signal_rx,
        }
    }
}

/// Runtime abstraction used to dispatch background agents.
pub trait BackgroundAgentRuntime: Send + Sync {
    fn dispatch(
        &self,
        launch: BackgroundAgentLaunch,
    ) -> Result<BackgroundRuntimeTicket, DispatchError>;
}

#[derive(Debug, Default)]
pub(super) struct MissingBackgroundAgentRuntime {}

impl BackgroundAgentRuntime for MissingBackgroundAgentRuntime {
    fn dispatch(
        &self,
        _launch: BackgroundAgentLaunch,
    ) -> Result<BackgroundRuntimeTicket, DispatchError> {
        Err(DispatchError::RuntimeFailure(
            "background agent runtime not configured".to_owned(),
        ))
    }
}

/// Thin adapter around the background-agent runtime.
#[derive(Clone)]
pub(crate) struct DeterministicAgentDispatcher {
    runtime: Arc<dyn BackgroundAgentRuntime>,
    /// Optional channel to tee all agent feed events to the shared feed panel.
    feed_tx: Option<mpsc::Sender<FeedEntry>>,
}

impl fmt::Debug for DeterministicAgentDispatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeterministicAgentDispatcher")
    }
}

impl Default for DeterministicAgentDispatcher {
    fn default() -> Self {
        Self::new(Arc::new(MissingBackgroundAgentRuntime {}))
    }
}

impl DeterministicAgentDispatcher {
    /// Creates a new deterministic dispatcher backed by the background-agent runtime.
    pub(crate) fn new(runtime: Arc<dyn BackgroundAgentRuntime>) -> Self {
        Self {
            runtime,
            feed_tx: None,
        }
    }

    /// Creates a dispatcher that tees agent feed events to the given channel.
    ///
    /// Inputs:
    /// - `feed_tx`: sending half of the shared agent-feed mpsc channel.
    ///
    /// Side effects:
    /// - All `AgentFeedOutput` events produced by dispatched agents are forwarded
    ///   to `feed_tx` via non-blocking `try_send` (errors silently discarded).
    pub(crate) fn new_with_feed(
        runtime: Arc<dyn BackgroundAgentRuntime>,
        feed_tx: mpsc::Sender<FeedEntry>,
    ) -> Self {
        Self {
            runtime,
            feed_tx: Some(feed_tx),
        }
    }

    /// Dispatches the worker agent for a workflow step.
    pub(crate) async fn dispatch_worker_agent(
        &self,
        request: &WorkflowDispatchRequest,
    ) -> Result<AgentDispatchTicket, DispatchError> {
        self.dispatch_agent(request, DispatchRequestKind::Worker)
            .await
    }

    /// Dispatches the evaluator agent for a workflow step.
    pub(crate) async fn dispatch_evaluator_agent(
        &self,
        request: &WorkflowDispatchRequest,
    ) -> Result<AgentDispatchTicket, DispatchError> {
        self.dispatch_agent(request, DispatchRequestKind::Evaluator)
            .await
    }

    /// Waits for a background agent completion and normalizes the resulting signal.
    ///
    /// Inputs:
    /// - `ticket`: dispatch ticket returned by [`dispatch_worker_agent`] or [`dispatch_evaluator_agent`].
    ///
    /// Returns:
    /// - `Ok((NormalizedSignal::Advance, None))` when the agent emits a passing signal.
    /// - `Ok((NormalizedSignal::Hold, Some(output)))` when the agent emits a Hold signal;
    ///   `output` contains the full accumulated agent response text.
    /// - `Ok((NormalizedSignal::Hold, None))` when the agent exits without a usable signal
    ///   or uses a test-double runtime with no signal channel.
    /// - `Err(DispatchError::RuntimeFailure)` when the background task panics or joins with an error.
    pub(crate) async fn await_agent_completion(
        &self,
        ticket: AgentDispatchTicket,
    ) -> Result<(NormalizedSignal, Option<OutputText>), DispatchError> {
        await_runtime_signal(ticket.runtime, self.feed_tx.clone()).await
    }

    async fn dispatch_agent(
        &self,
        request: &WorkflowDispatchRequest,
        expected_kind: DispatchRequestKind,
    ) -> Result<AgentDispatchTicket, DispatchError> {
        let prepared_dispatch = prepare_dispatch(request, expected_kind)?;
        let runtime = self.runtime.dispatch(prepared_dispatch.launch)?;
        Ok(AgentDispatchTicket {
            kind: prepared_dispatch.kind,
            step_id: prepared_dispatch.step_id,
            agent: Some(prepared_dispatch.agent),
            runtime,
        })
    }
}

struct PreparedDispatch {
    kind: DispatchRequestKind,
    step_id: WorkflowStepId,
    agent: AgentName,
    launch: BackgroundAgentLaunch,
}

/// Builds a typed runtime dispatch for the requested worker or evaluator path.
fn prepare_dispatch(
    request: &WorkflowDispatchRequest,
    expected_kind: DispatchRequestKind,
) -> Result<PreparedDispatch, DispatchError> {
    let request_kind_matches = request.kind == expected_kind;
    if !request_kind_matches {
        return Err(DispatchError::InvalidRequest(
            "dispatch request kind did not match the requested dispatch path",
        ));
    }

    let agent = agent_for_kind(request, &expected_kind).ok_or(DispatchError::MissingAgent(
        "dispatch request did not define an agent for this path",
    ))?;
    let prompt = prompt_for_request(request, &expected_kind);
    let model = request
        .dispatch
        .model
        .as_ref()
        .map(|m| ModelLabel::new(m.as_str()));

    Ok(PreparedDispatch {
        kind: expected_kind,
        step_id: request.step_id.clone(),
        agent: agent.clone(),
        launch: BackgroundAgentLaunch {
            agent,
            feed_id: FeedId::Agent(ToolCallId::from(request.step_id.as_str())),
            prompt,
            model,
        },
    })
}

/// Returns the typed agent configured for the requested dispatch path.
fn agent_for_kind(
    request: &WorkflowDispatchRequest,
    dispatch_kind: &DispatchRequestKind,
) -> Option<AgentName> {
    match dispatch_kind {
        DispatchRequestKind::Worker => request.dispatch.worker_agent.clone(),
        DispatchRequestKind::Evaluator => request.dispatch.evaluator_agent.clone(),
    }
}

/// Builds the runtime prompt for the requested dispatch path.
fn prompt_for_request(
    request: &WorkflowDispatchRequest,
    dispatch_kind: &DispatchRequestKind,
) -> PromptText {
    if let Some(prompt) = request.dispatch.prompt.clone() {
        return prompt;
    }
    match dispatch_kind {
        DispatchRequestKind::Worker => build_worker_prompt(request),
        DispatchRequestKind::Evaluator => build_evaluator_prompt(request),
    }
}

/// Formats a bulleted list section, omitting it entirely when the items are empty.
fn format_artifact_section(heading: &str, items: &[WorkflowArtifactRef]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let bullets: String = items
        .iter()
        .map(|item| format!("- {}\n", item.path))
        .collect();
    format!("{heading}\n{bullets}\n")
}

/// Formats a bulleted criteria section, omitting it entirely when the items are empty.
fn format_criteria_section(heading: &str, items: &[PassCriterion]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let bullets: String = items
        .iter()
        .map(|item| format!("- {}\n", item.as_str()))
        .collect();
    format!("{heading}\n{bullets}\n")
}

/// Builds the worker prompt for a dispatch request.
fn build_worker_prompt(request: &WorkflowDispatchRequest) -> PromptText {
    let agent_name = request
        .dispatch
        .worker_agent
        .as_ref()
        .map(|a| a.to_string())
        .unwrap_or_default();

    let feature_context_section = request
        .artifacts
        .feature_context
        .as_deref()
        .map(|ctx| format!("Feature request context:\n{ctx}\n\n"))
        .unwrap_or_default();

    let inputs_section =
        format_artifact_section("Expected inputs:", &request.artifacts.expected_inputs);
    let artifacts_section = format_artifact_section(
        "Artifacts to produce or update:",
        &request.artifacts.created_artifacts,
    );
    let criteria_section =
        format_criteria_section("Pass criteria:", &request.artifacts.pass_criteria);

    PromptText::from(format!(
        "{feature_context_section}You are the worker agent for workflow step `{step_id}`.\nAgent: {agent_name}\n\n\
         {inputs_section}\
         {artifacts_section}\
         {criteria_section}\
         Complete your work then emit exactly \"pass\" or \"fail\" as your final signal.",
        step_id = request.step_id,
    ))
}

/// Formats the prior worker signal as a human-readable label.
fn format_prior_signal(signal: &NormalizedSignal) -> &'static str {
    match signal {
        NormalizedSignal::Advance => "pass",
        NormalizedSignal::NeedsRevision | NormalizedSignal::Hold => "fail",
    }
}

/// Builds the evaluator prompt for a dispatch request.
fn build_evaluator_prompt(request: &WorkflowDispatchRequest) -> PromptText {
    let agent_name = request
        .dispatch
        .evaluator_agent
        .as_ref()
        .map(|a| a.to_string())
        .unwrap_or_default();

    let prior_result_line = request
        .prior_execution
        .as_ref()
        .map(|exec| {
            format!(
                "Prior worker result: {}\n\n",
                format_prior_signal(&exec.worker_signal)
            )
        })
        .unwrap_or_default();

    let artifacts_section =
        format_artifact_section("Artifacts to review:", &request.artifacts.created_artifacts);
    let criteria_section =
        format_criteria_section("Pass criteria:", &request.artifacts.pass_criteria);

    PromptText::from(format!(
        "You are the evaluator (gate) agent for workflow step `{step_id}`.\nAgent: {agent_name}\n\n\
         {prior_result_line}\
         {artifacts_section}\
         {criteria_section}\
         Review the artifacts against the pass criteria then emit exactly \"pass\" or \"fail\".",
        step_id = request.step_id,
    ))
}

async fn await_runtime_signal(
    runtime: BackgroundRuntimeTicket,
    tee_tx: Option<mpsc::Sender<FeedEntry>>,
) -> Result<(NormalizedSignal, Option<OutputText>), DispatchError> {
    let mut task = runtime.task;
    let mut feed_rx = runtime.feed_rx;
    let signal_rx = runtime.signal_rx;
    let mut feed_open = true;

    loop {
        if !feed_open {
            task.await.map_err(|error| {
                DispatchError::RuntimeFailure(format!(
                    "background-agent runtime task failed: {error}"
                ))
            })?;

            return Ok(resolve_signal(signal_rx).await);
        }

        tokio::select! {
            task_result = &mut task => {
                task_result.map_err(|error| {
                    DispatchError::RuntimeFailure(format!(
                        "background-agent runtime task failed: {error}"
                    ))
                })?;
                drain_events(&mut feed_rx, tee_tx.as_ref()).await;
                return Ok(resolve_signal(signal_rx).await);
            }
            maybe_output = feed_rx.recv() => {
                match maybe_output {
                    Some(ev) => {
                        if let Some(tx) = &tee_tx {
                            let _ = tx.try_send(ev.clone());
                        }
                    }
                    None => {
                        feed_open = false;
                    }
                }
            }
        }
    }
}

/// Short timeout to avoid blocking when an agent exits cleanly without signalling.
const SIGNAL_RECEIVE_TIMEOUT_MS: u64 = 100;

/// Resolves the final workflow signal, preferring the agent's text response over feed-event heuristics.
///
/// Inputs:
/// - `signal_rx`: Optional oneshot receiver carrying the full accumulated agent response text.
///
/// Returns a `(NormalizedSignal, Option<OutputText>)` pair:
/// 1. Reading the accumulated text from `signal_rx` (with a short timeout to avoid blocking).
/// 2. Scanning for the last whole-word occurrence of "pass" or "fail" (case-insensitive, strips punctuation).
/// 3. When the signal resolves to Hold and text is available, the full text is captured as `OutputText`.
/// 4. When the signal resolves to Advance, `None` is returned for the output text.
/// 5. When `signal_rx` is `Some` but no usable signal is found, returning fail-closed with no output text.
///    Agents are required to emit "pass" or "fail"; absent signal implies a silent crash or empty exit.
/// 6. When `signal_rx` is `None`, fail-closed as Hold with no output text.
async fn resolve_signal(
    signal_rx: Option<tokio::sync::oneshot::Receiver<AccumulatedText>>,
) -> (NormalizedSignal, Option<OutputText>) {
    match signal_rx {
        Some(rx) => resolve_signal_from_receiver(rx).await,
        None => (NormalizedSignal::Hold, None),
    }
}

async fn resolve_signal_from_receiver(
    rx: tokio::sync::oneshot::Receiver<AccumulatedText>,
) -> (NormalizedSignal, Option<OutputText>) {
    let received: Result<Result<AccumulatedText, _>, _> = tokio::time::timeout(
        std::time::Duration::from_millis(SIGNAL_RECEIVE_TIMEOUT_MS),
        rx,
    )
    .await;
    let Ok(Ok(text)) = received else {
        return (NormalizedSignal::Hold, None);
    };
    signal_from_text(text).unwrap_or((NormalizedSignal::Hold, None))
}

fn signal_from_text(text: AccumulatedText) -> Option<(NormalizedSignal, Option<OutputText>)> {
    let signal_word = extract_signal_from_text(text.as_str())?;
    let raw = WorkflowSignalValue::from(signal_word);
    let normalized = normalize_agent_signal(&raw);
    let output = (normalized == NormalizedSignal::Hold).then(|| OutputText::from(text.as_str()));
    Some((normalized, output))
}

/// Extracts the last recognized `pass`, `fail`, or `needs-revision` signal from
/// accumulated agent response text.
///
/// Inputs:
/// - `text`: Full accumulated response text from the agent session.
///
/// Returns:
/// - `Some("pass")` if the last recognized signal is `pass`.
/// - `Some("fail")` if the last recognized signal is `fail`.
/// - `Some("needs-revision")` if the last recognized signal is `needs-revision`.
/// - `None` if no recognized signal appears in the text.
///
/// The text is split on every non-alphabetic character boundary when searching for whole-word
/// `pass` and `fail`, so signal words attached directly to punctuation (e.g. `it.pass`,
/// `**fail**`, `[pass]`) are correctly identified. The hyphenated `needs-revision` signal is
/// detected via case-insensitive substring search before the token scan. The recognized signal
/// with the highest byte position wins, so later signals override earlier ones.
fn extract_signal_from_text(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    let needs_revision = lower
        .rfind(NEEDS_REVISION_SIGNAL)
        .map(|position| (position, NEEDS_REVISION_SIGNAL));
    let pass =
        last_signal_word_position(&lower, PASS_SIGNAL).map(|position| (position, PASS_SIGNAL));
    let fail =
        last_signal_word_position(&lower, FAIL_SIGNAL).map(|position| (position, FAIL_SIGNAL));

    [needs_revision, pass, fail]
        .into_iter()
        .flatten()
        .max_by_key(|(position, _)| *position)
        .map(|(_, signal)| signal)
}

fn last_signal_word_position(text: &str, target: &str) -> Option<usize> {
    text.match_indices(target)
        .filter_map(|(start, _)| is_signal_word_at(text, start, target).then_some(start))
        .last()
}

fn is_signal_word_at(text: &str, start: usize, target: &str) -> bool {
    let end = start + target.len();
    is_signal_boundary(text[..start].chars().next_back())
        && is_signal_boundary(text[end..].chars().next())
}

fn is_signal_boundary(character: Option<char>) -> bool {
    character.is_none_or(|value| !value.is_alphabetic())
}

async fn drain_events(
    feed_rx: &mut mpsc::Receiver<FeedEntry>,
    tee_tx: Option<&mpsc::Sender<FeedEntry>>,
) {
    while let Ok(output) = feed_rx.try_recv() {
        if let Some(tx) = tee_tx {
            let _ = tx.try_send(output.clone());
        }
    }
}
