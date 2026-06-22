use augur_domain::actors::active_model::ActiveModelHandle;
use augur_domain::actors::tool::InlineToolExecutor;
use augur_domain::config::types::{EndpointConfig, EndpointCredentials, Provider};
use augur_domain::config::{AgentConfig, AppConfig, CopilotConfig, PersistenceConfig};
use augur_domain::domain::newtypes::{Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{
    EndpointName, EndpointUrl, FilePath, ModelId, ModelName, OutputText,
};
use augur_domain::task_types::{AwaitRunResult, InstructionPrefix, RepoRoot, TaskRunId};
use augur_domain::tools::registry::ToolRegistry;
use augur_domain::{NumericNewtype, StringNewtype};
use augur_provider_openrouter::actors::llm::llm_actor;
use augur_provider_openrouter::actors::openrouter_orchestrator::openrouter_orchestrator_actor::{
    OpenRouterOrchestratorArgs, OrchestratorIoChannels, OrchestratorRuntimeHandles,
    OrchestratorTaskConfig, spawn,
};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch};

fn test_app_config() -> AppConfig {
    AppConfig {
        endpoints: vec![EndpointConfig {
            name: EndpointName::new("default"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new("http://localhost:1"),
            model: ModelName::new("openai/gpt-4o-mini"),
            credentials: EndpointCredentials::default(),
        }],
        default_endpoint: EndpointName::new("default"),
        agent: AgentConfig {
            system_prompt: OutputText::new(""),
            max_tokens: TokenCount::new(64),
            temperature: Temperature::new(0.0),
            allowed_dirs: vec![FilePath::new("./")],
        },
        copilot: CopilotConfig::default(),
        persistence: PersistenceConfig {
            log_dir: FilePath::new("./logs"),
            sessions_dir: None,
        },
        program_settings: Default::default(),
        user_settings: Default::default(),
    }
}

fn active_model_handle() -> ActiveModelHandle {
    let (cmd_tx, _cmd_rx) = mpsc::channel(4);
    let (_model_tx, model_rx) = watch::channel::<Option<ModelId>>(None);
    ActiveModelHandle::new(cmd_tx, model_rx)
}

fn test_logger() -> augur_domain::domain::actor_contracts::LoggerHandle {
    let (tx, _rx) = mpsc::channel(1);
    augur_domain::domain::actor_contracts::LoggerHandle::new(tx)
}

#[tokio::test]
async fn await_run_returns_unknown_for_never_seen_run_id() {
    let (agent_tx, _agent_rx) = broadcast::channel(8);
    let (llm_join, llm_handle) = llm_actor::spawn(
        test_app_config(),
        agent_tx,
        "test-session".to_string(),
        test_logger(),
    );
    let (feed_tx, _feed_rx) = mpsc::channel(8);
    let tool_executor = InlineToolExecutor::new(ToolRegistry::new());

    let args = OpenRouterOrchestratorArgs::builder()
        .runtime(
            OrchestratorRuntimeHandles::builder()
                .llm(llm_handle.clone())
                .active_model(active_model_handle())
                .tool_executor(tool_executor)
                .build(),
        )
        .io(OrchestratorIoChannels { feed_tx })
        .config(
            OrchestratorTaskConfig::builder()
                .allowed_dirs(vec![])
                .instruction_prefix(Arc::new(InstructionPrefix(vec![])))
                .repo_root(RepoRoot::new("."))
                .max_parallel_workers(1)
                .build(),
        )
        .build();

    let (join, handle) = spawn(args);
    let result = handle
        .await_run(TaskRunId::new("missing"))
        .expect("await_run sender")
        .await
        .expect("await_run reply");
    assert!(matches!(result, AwaitRunResult::UnknownRun { .. }));

    handle.shutdown().expect("shutdown command");
    join.await.expect("orchestrator join");
    llm_handle.shutdown();
    llm_join.await.expect("llm join");
}
