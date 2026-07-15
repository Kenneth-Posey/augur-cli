//! Dispatch runtime for the deterministic orchestrator pipeline.
//!
//! Provides a single [`BackgroundAgentRuntime`] implementation that routes to
//! the Copilot SDK or a generic LLM tool-call loop based on the active endpoint
//! provider. The orchestration logic in [`crate::actors::deterministic_orchestrator`]
//! is provider-agnostic — it only knows which agents to queue and when.

use augur_core::actors::SessionHandle;
use augur_core::actors::ToolHandle;
use augur_core::actors::deterministic_orchestrator::background_dispatch::{
    BackgroundAgentLaunch, BackgroundAgentRuntime, BackgroundRuntimeTicket, DispatchError,
};
use augur_domain::domain::string_newtypes::AgentName;
use augur_domain::domain::task_types::InstructionPrefix;
use augur_provider_copilot_sdk::actors::copilot::background_agent;
use augur_provider_openrouter::actors::LlmHandle;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Unified dispatch runtime for the deterministic orchestrator pipeline.
///
/// Routes each pipeline step to the execution strategy matching the active
/// endpoint provider. The orchestration logic is provider-agnostic — it only
/// needs to know which agents to queue and when.
///
/// ## Provider routing
///
/// | Provider | Strategy |
/// |---|---|
/// | `Copilot` | [`background_agent::run_background_agent`] — Copilot SDK session |
/// | All others | Generic LLM tool-call loop via [`run_generic_agent`] |
#[derive(Clone)]
pub struct DispatchRuntime {
    llm: LlmHandle,
    tools: ToolHandle,
    session: SessionHandle,
    instruction_prefix: Option<Arc<InstructionPrefix>>,
    /// Application config needed to resolve endpoint → provider mapping.
    app_config: augur_domain::config::types::AppConfig,
}

impl DispatchRuntime {
    /// Create a new dispatch runtime.
    ///
    /// Inputs:
    /// - `llm`: handle to the LLM actor, used to send step prompts.
    /// - `tools`: handle to the tool actor, used to execute file-tool calls.
    /// - `session`: handle to the session actor, used to resolve the active endpoint.
    /// - `app_config`: application configuration for endpoint → provider resolution.
    pub fn new(
        llm: LlmHandle,
        tools: ToolHandle,
        session: SessionHandle,
        app_config: augur_domain::config::types::AppConfig,
    ) -> Self {
        Self {
            llm,
            tools,
            session,
            instruction_prefix: None,
            app_config,
        }
    }

    /// Set the instruction prefix to prepend to every prompt.
    ///
    /// The prefix is injected before the user prompt. Use this to provide
    /// system-level instructions (e.g., seed files) that the LLM should
    /// follow during execution.
    pub fn with_instruction_prefix(mut self, prefix: Arc<InstructionPrefix>) -> Self {
        self.instruction_prefix = Some(prefix);
        self
    }

    /// Determine whether the active session is a Copilot session.
    ///
    /// Copilot is detected via `config.copilot.copilot_chat.enabled` rather
    /// than through the endpoint provider enum, because Copilot is not an
    /// endpoint provider variant — it is a separate subsystem activated by
    /// the `copilot_chat:` section in `application.yaml`.
    fn is_copilot_active(&self) -> bool {
        self.app_config.copilot.copilot_chat.enabled.0
    }
}

impl BackgroundAgentRuntime for DispatchRuntime {
    fn dispatch(
        &self,
        launch: BackgroundAgentLaunch,
    ) -> Result<BackgroundRuntimeTicket, DispatchError> {
        if self.is_copilot_active() {
            dispatch_copilot(launch)
        } else {
            dispatch_llm(
                launch,
                self.llm.clone(),
                self.tools.clone(),
                self.session.clone(),
                self.instruction_prefix.clone(),
            )
        }
    }
}

// ── Copilot SDK dispatch ────────────────────────────────────────────────────

/// Dispatch a pipeline step through the Copilot SDK.
///
/// Uses `agent: None` in the session config so the Copilot CLI does not
/// receive a pipeline agent name it does not know about. The prompt text
/// (which includes the full `.agent.md` instructions from
/// `AgentInstructionLibrary`) drives the LLM behavior.
fn dispatch_copilot(
    launch: BackgroundAgentLaunch,
) -> Result<BackgroundRuntimeTicket, DispatchError> {
    let (feed_tx, feed_rx) = mpsc::channel(1024);
    let (signal_tx, signal_rx) = tokio::sync::oneshot::channel();

    let task = tokio::spawn(async move {
        background_agent::run_background_agent(
            background_agent::BackgroundAgentArgs::builder()
                .config(
                    background_agent::BackgroundAgentConfig::builder()
                        .maybe_agent(None)
                        .feed_id(launch.feed_id)
                        .prompt(launch.prompt)
                        .maybe_model(launch.model)
                        .build(),
                )
                .feed_tx(feed_tx)
                .signal_tx(signal_tx)
                .classifier(std::sync::Arc::new(
                    augur_provider_copilot_sdk::actors::copilot::event_classifier::CopilotEventClassifier,
                ))
                .build(),
        )
        .await;
    });

    Ok(BackgroundRuntimeTicket::new(task, feed_rx, Some(signal_rx)))
}

// ── Generic LLM dispatch ────────────────────────────────────────────────────

/// Dispatch a pipeline step through the generic LLM provider with tool
/// execution capability.
fn dispatch_llm(
    launch: BackgroundAgentLaunch,
    llm: LlmHandle,
    tools: ToolHandle,
    session: SessionHandle,
    instruction_prefix: Option<Arc<InstructionPrefix>>,
) -> Result<BackgroundRuntimeTicket, DispatchError> {
    let (feed_tx, feed_rx) = mpsc::channel(1024);
    let (signal_tx, signal_rx) = tokio::sync::oneshot::channel();

    let prompt_text =
        augur_domain::domain::string_newtypes::OutputText::new(launch.prompt.as_str());
    let feed_id = launch.feed_id;
    let model = launch.model;

    let task = tokio::spawn(async move {
        run_generic_agent(
            llm,
            tools,
            session,
            instruction_prefix,
            prompt_text,
            feed_id,
            feed_tx,
            signal_tx,
            model,
        )
        .await;
    });

    Ok(BackgroundRuntimeTicket::new(task, feed_rx, Some(signal_rx)))
}

// ── Generic LLM tool-call loop ──────────────────────────────────────────────

use augur_domain::domain::string_newtypes::{AccumulatedText, OutputText, StringNewtype};
use augur_domain::domain::traits::{CompletionRequest, LlmClient, ToolExecutor};
use augur_domain::domain::types::AgentFeedOutput;
use augur_domain::domain::types::{FeedEntry, FeedId, Message, StreamChunk, ToolCall};
use augur_domain::tools::definition::ToolDefinition;

/// Run one pipeline step agent through the generic LLM provider with tool
/// execution capability.
#[allow(clippy::too_many_arguments)]
async fn run_generic_agent(
    llm: LlmHandle,
    tools: ToolHandle,
    session: SessionHandle,
    instruction_prefix: Option<Arc<InstructionPrefix>>,
    prompt: OutputText,
    feed_id: FeedId,
    feed_tx: mpsc::Sender<FeedEntry>,
    signal_tx: tokio::sync::oneshot::Sender<AccumulatedText>,
    _model: Option<augur_domain::domain::string_newtypes::ModelLabel>,
) {
    let endpoint = session.active_endpoint();
    let tool_defs: Vec<ToolDefinition> = tools.definitions().to_vec();
    let mut messages = Vec::new();
    if let Some(ref prefix) = instruction_prefix {
        messages.extend(prefix.0.clone());
    }
    messages.push(Message::user(prompt.into_inner()));
    let mut full_text = String::new();

    loop {
        let request = CompletionRequest::builder()
            .endpoint(endpoint.clone())
            .messages(messages.clone())
            .tools(tool_defs.clone())
            .build();
        let mut reply_rx = llm.complete_stream(request);

        let mut text_buf = String::new();
        let mut tool_call: Option<ToolCall> = None;

        loop {
            match reply_rx.recv().await {
                Some(StreamChunk::Token(text)) => {
                    text_buf.push_str(text.as_str());
                    emit_feed_status(&feed_tx, &feed_id, text).await;
                }
                Some(StreamChunk::ToolCall {
                    id,
                    name,
                    arguments,
                }) => {
                    if tool_call.is_none() {
                        tool_call = Some(ToolCall {
                            id,
                            name,
                            arguments,
                        });
                    }
                }
                Some(StreamChunk::Done) => break,
                Some(StreamChunk::Error(error)) => {
                    tracing::warn!(%error, "generic LLM dispatch agent failed");
                    emit_feed_failure(&feed_tx, &feed_id, error).await;
                    return;
                }
                Some(StreamChunk::RateLimitRetry(_)) => continue,
                Some(_) => continue,
                None => break,
            }
        }

        if let Some(call) = tool_call {
            full_text.push_str(&text_buf);
            emit_feed_tool_event(&feed_tx, &feed_id, &call.name).await;
            match tools.execute(call.clone()).await {
                Ok(result) => {
                    messages.push(Message::assistant(OutputText::new(text_buf.clone())));
                    messages.push(Message::tool_result(
                        call.id.clone(),
                        &call.name,
                        result.output.clone(),
                    ));
                }
                Err(error) => {
                    tracing::warn!(%error, tool = %call.name, "pipeline agent tool execution failed");
                    messages.push(Message::assistant(OutputText::new(text_buf.clone())));
                    messages.push(Message::tool_result(
                        call.id.clone(),
                        &call.name,
                        OutputText::new(format!("Tool execution failed: {error}")),
                    ));
                }
            }
            text_buf.clear();
            continue;
        }

        // No tool call -- this is the final text response with the pass/fail signal.
        full_text.push_str(&text_buf);
        let _ = signal_tx.send(AccumulatedText::from(std::mem::take(&mut full_text)));
        return;
    }
}

async fn emit_feed_status(feed_tx: &mpsc::Sender<FeedEntry>, feed_id: &FeedId, text: OutputText) {
    let entry = FeedEntry {
        feed_id: feed_id.clone(),
        output: AgentFeedOutput::StatusLine(text),
    };
    let _ = feed_tx.try_send(entry);
}

async fn emit_feed_tool_event(
    feed_tx: &mpsc::Sender<FeedEntry>,
    feed_id: &FeedId,
    tool_name: &augur_domain::domain::ToolName,
) {
    let entry = FeedEntry {
        feed_id: feed_id.clone(),
        output: AgentFeedOutput::ToolEventLine(OutputText::new(format!(
            "[tool: {}]",
            tool_name.as_str()
        ))),
    };
    let _ = feed_tx.try_send(entry);
}

async fn emit_feed_failure(feed_tx: &mpsc::Sender<FeedEntry>, feed_id: &FeedId, error: OutputText) {
    let entry = FeedEntry {
        feed_id: feed_id.clone(),
        output: AgentFeedOutput::TaskFailed {
            name: AgentName::pipeline(),
            reason: error,
        },
    };
    let _ = feed_tx.try_send(entry);
}
