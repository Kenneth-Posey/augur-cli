use augur_domain::ToolDefinition;
use augur_domain::string_newtypes::OutputText;
use augur_domain::task_types::{
    AgentSpecName, InstructionPrefix, RepoRoot, TaskDepth, TaskRunId, TaskSignal,
};
use augur_domain::tools::handler::ToolCallResult;
use augur_domain::traits::{CompletionRequest, LlmClient, ToolExecutor};
use augur_domain::types::{AgentFeedOutput, FeedEntry, StreamChunk, ToolCall};
use augur_domain::{ModelId, PromptText, StringNewtype};
use augur_provider_openrouter::actors::openrouter_task::openrouter_task_actor::{
    OpenRouterTaskArgs, TaskConfig, TaskCorrelation, TaskRequestSpec, TaskRuntimeOptions,
    TaskServices, spawn,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{mpsc, oneshot};

struct FakeLlm;

impl LlmClient for FakeLlm {
    fn complete_stream(&self, _request: CompletionRequest) -> mpsc::Receiver<StreamChunk> {
        let (tx, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            let _ = tx.send(StreamChunk::Token(OutputText::new("hello"))).await;
            let _ = tx.send(StreamChunk::Done).await;
        });
        rx
    }
}

struct FakeTools {
    defs: Vec<ToolDefinition>,
}

#[async_trait::async_trait]
impl ToolExecutor for FakeTools {
    fn definitions(&self) -> &[ToolDefinition] {
        &self.defs
    }

    async fn execute(&self, _call: ToolCall) -> anyhow::Result<ToolCallResult> {
        unreachable!("no tool calls are emitted by FakeLlm in this test")
    }
}

struct ToolThenRecoveryLlm {
    calls: Arc<AtomicUsize>,
}

impl LlmClient for ToolThenRecoveryLlm {
    fn complete_stream(&self, _request: CompletionRequest) -> mpsc::Receiver<StreamChunk> {
        let (tx, rx) = mpsc::channel(8);
        let call_index = self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(async move {
            if call_index == 0 {
                let _ = tx
                    .send(StreamChunk::ToolCall {
                        id: augur_domain::ToolCallId::new("call-1"),
                        name: augur_domain::ToolName::new("shell_exec"),
                        arguments: serde_json::json!({ "command": "pwd -l" }),
                    })
                    .await;
                let _ = tx.send(StreamChunk::Done).await;
            } else {
                let _ = tx
                    .send(StreamChunk::Token(OutputText::new("recovered")))
                    .await;
                let _ = tx.send(StreamChunk::Done).await;
            }
        });
        rx
    }
}

struct FailingTools {
    defs: Vec<ToolDefinition>,
}

#[async_trait::async_trait]
impl ToolExecutor for FailingTools {
    fn definitions(&self) -> &[ToolDefinition] {
        &self.defs
    }

    async fn execute(&self, _call: ToolCall) -> anyhow::Result<ToolCallResult> {
        anyhow::bail!("No such file or directory (os error 2)");
    }
}

#[tokio::test]
async fn task_actor_emits_completed_signal_without_network() {
    let temp = tempfile::tempdir().expect("tempdir");
    let spec_path = temp.path().join("planner.agent.md");
    std::fs::write(&spec_path, "You are planner").expect("write spec");

    let (signal_tx, signal_rx) = oneshot::channel::<TaskSignal>();
    let (feed_tx, mut feed_rx) = mpsc::channel::<FeedEntry>(16);

    let args = OpenRouterTaskArgs::builder()
        .llm(FakeLlm)
        .tools(FakeTools { defs: vec![] })
        .task_config(
            TaskConfig::builder()
                .request(
                    TaskRequestSpec::builder()
                        .agent_name(AgentSpecName::new("planner"))
                        .prompt(PromptText::new("say hi"))
                        .depth(TaskDepth::root())
                        .build(),
                )
                .runtime(
                    TaskRuntimeOptions::builder()
                        .maybe_model_override(Some(ModelId::new("openai/gpt-4o-mini")))
                        .build(),
                )
                .correlation(
                    TaskCorrelation::builder()
                        .signal_tx(signal_tx)
                        .maybe_run_id(Some(TaskRunId::new("run-1")))
                        .build(),
                )
                .build(),
        )
        .task_services(
            TaskServices::builder()
                .feed_tx(feed_tx)
                .instruction_prefix(Arc::new(InstructionPrefix(vec![])))
                .spec_base_path(RepoRoot::new(temp.path().display().to_string()))
                .maybe_token_tracker(None)
                .maybe_orchestrator(None)
                .build(),
        )
        .build();

    let (join, _handle) = spawn(args);
    let signal = signal_rx.await.expect("task signal");
    assert!(matches!(signal, TaskSignal::Completed { .. }));

    let mut saw_started = false;
    let mut saw_completed = false;
    while let Ok(entry) =
        tokio::time::timeout(std::time::Duration::from_millis(25), feed_rx.recv()).await
    {
        let Some(entry) = entry else { break };
        match entry.output {
            AgentFeedOutput::TaskStarted { .. } => saw_started = true,
            AgentFeedOutput::TaskCompleted { .. } => {
                saw_completed = true;
                break;
            }
            _ => {}
        }
    }
    assert!(saw_started, "task must emit TaskStarted");
    assert!(saw_completed, "task must emit TaskCompleted");

    join.await.expect("task join");
}

#[tokio::test]
async fn task_loop_continues_after_tool_transport_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let spec_path = temp.path().join("planner.agent.md");
    std::fs::write(&spec_path, "You are planner").expect("write spec");
    let (signal_tx, signal_rx) = oneshot::channel::<TaskSignal>();
    let (feed_tx, _feed_rx) = mpsc::channel::<FeedEntry>(16);
    let llm = ToolThenRecoveryLlm {
        calls: Arc::new(AtomicUsize::new(0)),
    };

    let args = OpenRouterTaskArgs::builder()
        .llm(llm)
        .tools(FailingTools {
            defs: vec![ToolDefinition::new(
                "shell_exec",
                "Run a shell command",
                serde_json::json!({"type":"object"}),
            )],
        })
        .task_config(
            TaskConfig::builder()
                .request(
                    TaskRequestSpec::builder()
                        .agent_name(AgentSpecName::new("planner"))
                        .prompt(PromptText::new("run command"))
                        .depth(TaskDepth::root())
                        .build(),
                )
                .runtime(
                    TaskRuntimeOptions::builder()
                        .maybe_model_override(None)
                        .build(),
                )
                .correlation(
                    TaskCorrelation::builder()
                        .signal_tx(signal_tx)
                        .maybe_run_id(None)
                        .build(),
                )
                .build(),
        )
        .task_services(
            TaskServices::builder()
                .feed_tx(feed_tx)
                .instruction_prefix(Arc::new(InstructionPrefix(vec![])))
                .spec_base_path(RepoRoot::new(temp.path().display().to_string()))
                .maybe_token_tracker(None)
                .maybe_orchestrator(None)
                .build(),
        )
        .build();

    let (join, _handle) = spawn(args);
    let signal = signal_rx.await.expect("task signal");
    assert!(
        matches!(signal, TaskSignal::Completed { .. }),
        "task must keep looping after tool transport errors"
    );
    join.await.expect("task join");
}
