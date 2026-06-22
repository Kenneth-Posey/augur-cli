use augur_domain::string_newtypes::OutputText;
use augur_domain::task_types::{AgentSpecName, InstructionPrefix, RepoRoot, TaskDepth, TaskSignal};
use augur_domain::tools::handler::ToolCallResult;
use augur_domain::traits::{CompletionRequest, LlmClient, ToolExecutor};
use augur_domain::types::{StreamChunk, ToolCall};
use augur_domain::{PromptText, StringNewtype, ToolDefinition};
use augur_provider_openrouter::actors::openrouter_task::openrouter_task_actor::{
    OpenRouterTaskArgs, TaskConfig, TaskCorrelation, TaskRequestSpec, TaskRuntimeOptions,
    TaskServices, spawn,
};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

struct FakeLlm;

impl LlmClient for FakeLlm {
    fn complete_stream(&self, _request: CompletionRequest) -> mpsc::Receiver<StreamChunk> {
        let (tx, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            let _ = tx.send(StreamChunk::Token(OutputText::new("done"))).await;
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
        unreachable!("no tool calls expected")
    }
}

#[tokio::test]
async fn spawn_returns_cloneable_task_handle() {
    let temp = tempfile::tempdir().expect("tempdir");
    let spec_path = temp.path().join("planner.agent.md");
    std::fs::write(&spec_path, "You are planner").expect("write spec");
    let (signal_tx, signal_rx) = oneshot::channel::<TaskSignal>();
    let (feed_tx, _feed_rx) = mpsc::channel(16);

    let args = OpenRouterTaskArgs::builder()
        .llm(FakeLlm)
        .tools(FakeTools { defs: vec![] })
        .task_config(
            TaskConfig::builder()
                .request(
                    TaskRequestSpec::builder()
                        .agent_name(AgentSpecName::new("planner"))
                        .prompt(PromptText::new("ping"))
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

    let (join, handle) = spawn(args);
    let _cloned = handle.clone();

    let signal = signal_rx.await.expect("task signal");
    assert!(matches!(signal, TaskSignal::Completed { .. }));
    join.await.expect("task join");
}
