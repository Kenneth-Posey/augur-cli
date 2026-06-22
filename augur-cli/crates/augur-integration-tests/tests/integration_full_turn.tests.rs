//! Integration tests: full agent turn pipeline without TUI.
//!
//! Spawns the LLM, Tool, and Agent actors against a mockito HTTP server and
//! verifies that the complete Submit → stream → AgentOutput::Done flow works
//! end-to-end without needing a real terminal or live API endpoints.

use augur_cli::wiring::BuildRegistryArgs;
use augur_cli::wiring::OptionalToolArgs;
use augur_cli::wiring::RegistryDirectoryScope;
use augur_cli::wiring::build_registry;
use augur_core::actors::agent::agent_actor::{AgentRuntime, AgentSpawnArgs, spawn as spawn_agent};
use augur_core::actors::agent::agent_ops::AgentOutput;
use augur_core::actors::file_read::file_read_actor::spawn as spawn_file_read;
use augur_core::actors::history_adapter::history_adapter_actor::{
    HistoryAdapterConfig, spawn as spawn_history_adapter,
};
use augur_core::actors::logger::logger_actor::spawn as spawn_logger;
use augur_core::actors::token_tracker;
use augur_core::actors::tool::tool_actor::spawn as spawn_tool;
use augur_core::persistence::handle::PersistenceHandle;
use augur_domain::config::types::{
    AgentConfig, AppConfig, CopilotConfig, EndpointConfig, EndpointCredentials, PersistenceConfig,
    Provider,
};
use augur_domain::domain::feeds::HistoryFeedMessage;
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{
    EndpointName, EndpointUrl, FilePath, ModelName, OutputText, PromptText, StringNewtype,
};
use augur_domain::domain::task_types::AgentExtensions;
use augur_provider_openrouter::actors::llm::llm_actor::spawn as spawn_llm;
use std::sync::Once;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tracing::info;

static TEST_TRACING: Once = Once::new();
const RECV_TIMEOUT_SECS: u64 = 30;

fn init_test_tracing() {
    TEST_TRACING.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::INFO)
            .try_init();
    });
}

fn output_kind(output: &AgentOutput) -> &'static str {
    match output {
        AgentOutput::Token(_) => "token",
        AgentOutput::MessageBreak => "message_break",
        AgentOutput::Done => "done",
        AgentOutput::Error(_) => "error",
        AgentOutput::Interrupted => "interrupted",
        AgentOutput::ToolCallStarted { .. } => "tool_call_started",
        AgentOutput::TurnComplete => "turn_complete",
        AgentOutput::PlanNodeUpdate { .. } => "plan_node_update",
        AgentOutput::UsageUpdate { .. } => "usage_update",
        AgentOutput::ToolCallCompleted { .. } => "tool_call_completed",
        AgentOutput::SystemMessage(_) => "system_message",
        AgentOutput::CompactionComplete { .. } => "compaction_complete",
        AgentOutput::ModelsAvailable(_) => "models_available",
        AgentOutput::ActiveModelChanged(_) => "active_model_changed",
        AgentOutput::IntentMessage(_) => "intent_message",
        AgentOutput::ToolProgress { .. } => "tool_progress",
        AgentOutput::ToolPartialResult { .. } => "tool_partial_result",
        AgentOutput::BackoffStarted(_) => "backoff_started",
        AgentOutput::UsageSnapshot(_) => "usage_snapshot",
    }
}

async fn recv_output_with_timeout(
    rx: &mut broadcast::Receiver<AgentOutput>,
    test_name: &'static str,
    observed_events: usize,
) -> AgentOutput {
    match tokio::time::timeout(Duration::from_secs(RECV_TIMEOUT_SECS), rx.recv()).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            panic!("{test_name}: output channel error after {observed_events} events: {error}")
        }
        Err(_) => panic!(
            "{test_name}: timed out waiting for AgentOutput after {RECV_TIMEOUT_SECS}s (observed_events={observed_events})"
        ),
    }
}

fn fake_token_tracker() -> augur_core::actors::TokenTrackerHandle {
    let tmp = tempfile::tempdir().expect("tempdir for fake token tracker");
    let (_join, handle) = token_tracker::spawn();
    std::mem::forget(tmp);
    handle
}

fn fake_history_adapter() -> augur_core::actors::HistoryAdapterHandle {
    let (history_tx, _history_rx) = mpsc::channel::<HistoryFeedMessage>(128);
    let (_join, handle) = spawn_history_adapter(HistoryAdapterConfig {
        history_tx,
        capacity: 128,
    });
    handle
}

fn make_config(base_url: &str) -> AppConfig {
    AppConfig {
        endpoints: vec![EndpointConfig {
            name: EndpointName::new("test"),
            provider: Provider::Ollama,
            base_url: EndpointUrl::new(base_url),
            model: ModelName::new("test-model"),
            credentials: EndpointCredentials::default(),
        }],
        default_endpoint: EndpointName::new("test"),
        agent: AgentConfig {
            system_prompt: OutputText::new("You are a test assistant."),
            max_tokens: TokenCount::new(256),
            temperature: Temperature::new(0.0),
            allowed_dirs: vec![],
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

/// Tests that two token chunks followed by [DONE] arrive correctly through
/// all actors and are collected in the right order with no errors.
#[tokio::test]
async fn full_turn_no_tools() {
    init_test_tracing();
    info!("full_turn_no_tools: starting");
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n",
            "data: [DONE]\n",
        ))
        .create();

    let base_url = server.url();
    info!(%base_url, "full_turn_no_tools: mock server ready");
    let config = make_config(&base_url);
    let (agent_tx, _) = broadcast::channel(256);
    let tmp_log = tempfile::tempdir().expect("log tmp dir");
    let (_logger_join, logger) = spawn_logger(tmp_log.path().to_path_buf());
    let (llm_join, llm_handle) = spawn_llm(
        config.clone(),
        agent_tx,
        "test-session".to_string(),
        logger.clone(),
    );
    let (query_tx, _query_rx) = mpsc::channel(1);
    let (_fr_join, file_read) = spawn_file_read(vec![]);
    let (tool_join, tool_handle) = spawn_tool(build_registry(BuildRegistryArgs {
        query_tx,
        file_read,
        cache: None,
        dirs: RegistryDirectoryScope {
            allowed_dirs: vec![],
            excluded_dirs: vec![],
        },
        optional: OptionalToolArgs {
            spawn_agent: None,
            lsp: None,
        },
    }));
    let tmp_no_tools = tempfile::tempdir().expect("tmp dir");
    let persistence = PersistenceHandle::new(tmp_no_tools.path().to_path_buf());
    let (agent_join, agent_handle) = spawn_agent(
        AgentSpawnArgs::builder()
            .llm(llm_handle.clone())
            .tools(tool_handle.clone())
            .config(config.agent.clone())
            .services(
                augur_core::actors::agent::agent_actor::AgentServices::builder()
                    .persistence(persistence)
                    .logger(logger)
                    .token_tracker(fake_token_tracker())
                    .history_adapter(fake_history_adapter())
                    .build(),
            )
            .runtime(
                AgentRuntime::builder()
                    .extensions(AgentExtensions {
                        cache: None,
                        instruction_prefix: None,
                        message_compactor: None,
                    })
                    .app_config(config.clone())
                    .build(),
            )
            .build(),
    );

    let mut rx = agent_handle.subscribe_output();
    info!("full_turn_no_tools: submitting prompt");
    agent_handle.submit(PromptText::new("test prompt"), EndpointName::new("test"));

    let mut tokens = vec![];
    let mut observed_events = 0usize;
    loop {
        let output = recv_output_with_timeout(&mut rx, "full_turn_no_tools", observed_events).await;
        observed_events += 1;
        info!(
            event = output_kind(&output),
            observed_events, "full_turn_no_tools: received output"
        );
        match output {
            AgentOutput::Token(t) => tokens.push(t.into_inner()),
            AgentOutput::MessageBreak => {}
            AgentOutput::Done => break,
            AgentOutput::Error(e) => panic!("unexpected error: {e}"),
            AgentOutput::Interrupted => panic!("unexpected Interrupted"),
            AgentOutput::ToolCallStarted { .. } => {}
            AgentOutput::TurnComplete
            | AgentOutput::PlanNodeUpdate { .. }
            | AgentOutput::UsageUpdate { .. }
            | AgentOutput::ToolCallCompleted { .. }
            | AgentOutput::SystemMessage(_)
            | AgentOutput::CompactionComplete { .. }
            | AgentOutput::ModelsAvailable(_)
            | AgentOutput::ActiveModelChanged(_)
            | AgentOutput::IntentMessage(_)
            | AgentOutput::ToolProgress { .. }
            | AgentOutput::ToolPartialResult { .. }
            | AgentOutput::BackoffStarted(_)
            | AgentOutput::UsageSnapshot(_) => {}
        }
    }

    info!(token_count = tokens.len(), "full_turn_no_tools: completed");
    assert_eq!(tokens.join(""), "hello world");

    agent_handle.shutdown();
    tool_handle.shutdown();
    llm_handle.shutdown();
    let _ = tokio::join!(agent_join, tool_join, llm_join);
}

/// Tests that a tool call response causes the agent to execute the tool and
/// submit a second LLM request, collecting the final "done" token.
///
/// Mockito 1.x matches mocks in registration order, preferring the first mock
/// that still has "missing hits". Mock1 (tool call) is registered first with
/// `.expect(1)` so it is preferred until it is satisfied. Mock2 (fallback
/// "done") is registered second and takes over for all subsequent requests.
#[tokio::test]
async fn full_turn_one_tool_call() {
    init_test_tracing();
    info!("full_turn_one_tool_call: starting");
    let mut server = mockito::Server::new_async().await;

    // Register tool-call mock first with .expect(1): matches first request only.
    // Delta 1 carries the tool name; delta 2 carries arguments; delta 3 signals
    // completion via finish_reason so the stateful accumulator emits the ToolCall.
    let _mock1 = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"function\":{",
            "\"name\":\"shell_exec\",",
            "\"arguments\":\"{\\\"command\\\":\\\"echo hi\\\"}\"",
            "}}]}}]}\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n",
            "data: [DONE]\n",
        ))
        .expect(1)
        .create();

    // Register fallback mock second: matches all subsequent requests.
    let _mock2 = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"done\"}}]}\n",
            "data: [DONE]\n",
        ))
        .create();

    let base_url = server.url();
    info!(%base_url, "full_turn_one_tool_call: mock server ready");
    let config = make_config(&base_url);
    let (agent_tx2, _) = broadcast::channel(256);
    let tmp_log2 = tempfile::tempdir().expect("log tmp dir 2");
    let (_logger_join2, logger2) = spawn_logger(tmp_log2.path().to_path_buf());
    let (llm_join, llm_handle) = spawn_llm(
        config.clone(),
        agent_tx2,
        "test-session".to_string(),
        logger2.clone(),
    );
    let (query_tx, _query_rx) = mpsc::channel(1);
    let (_fr_join2, file_read2) = spawn_file_read(vec![]);
    let (tool_join, tool_handle) = spawn_tool(build_registry(BuildRegistryArgs {
        query_tx,
        file_read: file_read2,
        cache: None,
        dirs: RegistryDirectoryScope {
            allowed_dirs: vec![],
            excluded_dirs: vec![],
        },
        optional: OptionalToolArgs {
            spawn_agent: None,
            lsp: None,
        },
    }));
    let tmp_tool_call = tempfile::tempdir().expect("tmp dir");
    let persistence = PersistenceHandle::new(tmp_tool_call.path().to_path_buf());
    let (agent_join, agent_handle) = spawn_agent(
        AgentSpawnArgs::builder()
            .llm(llm_handle.clone())
            .tools(tool_handle.clone())
            .config(config.agent.clone())
            .services(
                augur_core::actors::agent::agent_actor::AgentServices::builder()
                    .persistence(persistence)
                    .logger(logger2)
                    .token_tracker(fake_token_tracker())
                    .history_adapter(fake_history_adapter())
                    .build(),
            )
            .runtime(
                AgentRuntime::builder()
                    .extensions(AgentExtensions {
                        cache: None,
                        instruction_prefix: None,
                        message_compactor: None,
                    })
                    .app_config(config.clone())
                    .build(),
            )
            .build(),
    );

    let mut rx = agent_handle.subscribe_output();
    info!("full_turn_one_tool_call: submitting prompt");
    agent_handle.submit(PromptText::new("run a command"), EndpointName::new("test"));

    let mut last_token = String::new();
    let mut observed_events = 0usize;
    loop {
        let output =
            recv_output_with_timeout(&mut rx, "full_turn_one_tool_call", observed_events).await;
        observed_events += 1;
        info!(
            event = output_kind(&output),
            observed_events, "full_turn_one_tool_call: received output"
        );
        match output {
            AgentOutput::Token(t) => last_token = t.into_inner(),
            AgentOutput::MessageBreak => {}
            AgentOutput::Done => break,
            AgentOutput::Error(e) => panic!("unexpected error: {e}"),
            AgentOutput::Interrupted => panic!("unexpected Interrupted"),
            AgentOutput::ToolCallStarted { .. } => {}
            AgentOutput::TurnComplete
            | AgentOutput::PlanNodeUpdate { .. }
            | AgentOutput::UsageUpdate { .. }
            | AgentOutput::ToolCallCompleted { .. }
            | AgentOutput::SystemMessage(_)
            | AgentOutput::CompactionComplete { .. }
            | AgentOutput::ModelsAvailable(_)
            | AgentOutput::ActiveModelChanged(_)
            | AgentOutput::IntentMessage(_)
            | AgentOutput::ToolProgress { .. }
            | AgentOutput::ToolPartialResult { .. }
            | AgentOutput::BackoffStarted(_)
            | AgentOutput::UsageSnapshot(_) => {}
        }
    }

    info!(%last_token, "full_turn_one_tool_call: completed");
    assert_eq!(last_token, "done");

    agent_handle.shutdown();
    tool_handle.shutdown();
    llm_handle.shutdown();
    let _ = tokio::join!(agent_join, tool_join, llm_join);
}
