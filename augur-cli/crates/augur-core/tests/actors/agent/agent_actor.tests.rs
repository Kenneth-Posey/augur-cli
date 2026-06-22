use augur_core::actors::agent::agent_actor::{AgentRuntime, AgentServices, AgentSpawnArgs, spawn};
use augur_core::actors::agent::agent_ops::{AgentOutput, DEFAULT_MAX_ITERATIONS};
use augur_core::actors::logger::logger_actor::spawn as spawn_logger;
use augur_core::helpers::fake_history_adapter::fake_history_adapter_handle;
use augur_core::helpers::fake_llm::FakeLlmClient;
use augur_core::helpers::fake_token_tracker::fake_token_tracker_handle;
use augur_core::helpers::fake_tool::FakeToolExecutor;
use augur_core::persistence::handle::PersistenceHandle;
use augur_domain::config::types::{AgentConfig, AppConfig, PersistenceConfig};
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{
    EndpointName, FilePath, OutputText, PromptText, StringNewtype, ToolCallId, ToolName,
};
use augur_domain::domain::task_types::AgentExtensions;
use augur_domain::domain::traits::LlmClient;
use augur_domain::domain::types::StreamChunk;
use augur_domain::tools::definition::ToolDefinition;
use tempfile::TempDir;

fn test_agent_config() -> AgentConfig {
    AgentConfig {
        system_prompt: OutputText::new("You are a helpful assistant."),
        max_tokens: TokenCount::new(4096),
        temperature: Temperature::new(0.7),
        allowed_dirs: vec![],
    }
}

fn temp_persistence() -> (PersistenceHandle, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let handle = PersistenceHandle::new(dir.path().to_owned());
    (handle, dir)
}

fn make_args<L: LlmClient, T: augur_core::actors::tool::handle::ToolExecutor>(
    llm: L,
    tools: T,
    persistence: PersistenceHandle,
) -> AgentSpawnArgs<L, T> {
    let tmp = tempfile::tempdir().expect("tempdir for logger");
    let (_logger_join, logger) = spawn_logger(tmp.path().to_path_buf());
    std::mem::forget(tmp);
    let config = test_agent_config();
    AgentSpawnArgs::builder()
        .llm(llm)
        .tools(tools)
        .config(config.clone())
        .services(
            AgentServices::builder()
                .persistence(persistence)
                .logger(logger)
                .token_tracker(fake_token_tracker_handle().1)
                .history_adapter(fake_history_adapter_handle())
                .build(),
        )
        .runtime(
            AgentRuntime::builder()
                .extensions(AgentExtensions {
                    cache: None,
                    instruction_prefix: None,
                    message_compactor: None,
                })
                .app_config(AppConfig {
                    endpoints: vec![],
                    default_endpoint: EndpointName::new("test"),
                    agent: config,
                    copilot: Default::default(),
                    persistence: PersistenceConfig {
                        log_dir: FilePath::new("./logs"),
                        sessions_dir: None,
                    },
                    program_settings: Default::default(),
                    user_settings: Default::default(),
                })
                .build(),
        )
        .build()
}

struct AlwaysErrToolExecutor;

#[async_trait::async_trait]
impl augur_core::actors::tool::handle::ToolExecutor for AlwaysErrToolExecutor {
    fn definitions(&self) -> &[ToolDefinition] {
        &[]
    }

    async fn execute(
        &self,
        _call: augur_core::actors::tool::tool_ops::ToolCall,
    ) -> anyhow::Result<augur_core::tools::handler::ToolCallResult> {
        Err(anyhow::anyhow!("No such file or directory (os error 2)"))
    }
}

#[tokio::test]
async fn spawn_and_shutdown() {
    let (persistence, _dir) = temp_persistence();
    let llm = FakeLlmClient::new(vec![]);
    let tools = FakeToolExecutor::always_ok("");
    let (join, handle) = spawn(make_args(llm, tools, persistence));
    handle.shutdown();
    join.await.expect("actor task panicked");
}

#[tokio::test]
async fn submit_prompt_yields_tokens_then_done() {
    let (persistence, _dir) = temp_persistence();
    let chunks = vec![
        StreamChunk::Token(OutputText::new("hello")),
        StreamChunk::Done,
    ];
    let llm = FakeLlmClient::new(vec![chunks]);
    let tools = FakeToolExecutor::always_ok("");
    let (_, handle) = spawn(make_args(llm, tools, persistence));
    let mut rx = handle.subscribe_output();
    handle.submit(PromptText::new("test"), EndpointName::new("ep"));

    let mut tokens: Vec<OutputText> = vec![];
    let mut got_done = false;
    for _ in 0..40 {
        let next = tokio::time::timeout(std::time::Duration::from_millis(250), rx.recv()).await;
        match next {
            Ok(Ok(AgentOutput::Token(t))) => tokens.push(t),
            Ok(Ok(AgentOutput::Done)) => {
                got_done = true;
                break;
            }
            Ok(Ok(AgentOutput::Error(e))) => panic!("unexpected error: {e}"),
            Ok(Ok(_)) => {}
            _ => break,
        }
    }
    assert!(!tokens.is_empty(), "expected at least one token");
    assert_eq!(tokens[0], OutputText::new("hello"));
    assert!(
        got_done,
        "expected AgentOutput::Done after token stream completion"
    );
}

#[tokio::test]
async fn restore_session_replaces_history() {
    use augur_core::persistence::{MessageRecord, MessageType};
    let (persistence, _dir) = temp_persistence();
    let response = vec![StreamChunk::Token(OutputText::new("ok")), StreamChunk::Done];
    let llm = FakeLlmClient::new(vec![response]);
    let llm_spy = llm.clone();
    let tools = FakeToolExecutor::always_ok("");
    let (_, handle) = spawn(make_args(llm, tools, persistence));

    let records = vec![MessageRecord {
        message_type: MessageType::User,
        message: augur_domain::domain::types::Message::user(PromptText::new("previous question")),
    }];
    handle.restore(records);

    let mut rx = handle.subscribe_output();
    handle.submit(PromptText::new("new question"), EndpointName::new("ep"));
    for _ in 0..40 {
        let next = tokio::time::timeout(std::time::Duration::from_millis(250), rx.recv()).await;
        match next {
            Ok(Ok(AgentOutput::Done)) => break,
            Ok(Ok(AgentOutput::Error(e))) => panic!("unexpected error: {e}"),
            Ok(Ok(_)) => {}
            _ => break,
        }
    }

    let received = llm_spy.received.lock().expect("received lock");
    assert!(received[0].len() >= 2, "expected system + submitted prompt");
}

#[tokio::test]
async fn max_iterations_exceeded_sends_error() {
    let (persistence, _dir) = temp_persistence();
    let repeated: Vec<Vec<StreamChunk>> = (0..DEFAULT_MAX_ITERATIONS.inner())
        .map(|_| {
            vec![
                StreamChunk::ToolCall {
                    id: ToolCallId::new("call_loop"),
                    name: ToolName::new("shell_exec"),
                    arguments: serde_json::json!({}),
                },
                StreamChunk::Done,
            ]
        })
        .collect();
    let llm = FakeLlmClient::new(repeated);
    let tools = FakeToolExecutor::always_ok("ok");
    let (_, handle) = spawn(make_args(llm, tools, persistence));

    let mut rx = handle.subscribe_output();
    handle.submit(PromptText::new("loop forever"), EndpointName::new("ep"));

    let mut got_error = false;
    for _ in 0..(DEFAULT_MAX_ITERATIONS.inner() * 3) {
        match rx.recv().await {
            Ok(AgentOutput::Error(_)) => {
                got_error = true;
                break;
            }
            Ok(AgentOutput::Done) => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    assert!(got_error, "expected AgentOutput::Error");
}

#[tokio::test]
async fn tool_execution_error_still_continues_turn() {
    let (persistence, _dir) = temp_persistence();
    let llm = FakeLlmClient::new(vec![
        vec![
            StreamChunk::ToolCall {
                id: ToolCallId::new("call_err"),
                name: ToolName::new("shell_exec"),
                arguments: serde_json::json!({"command":"cd /workspace && git log -1 --stat"}),
            },
            StreamChunk::Done,
        ],
        vec![
            StreamChunk::Token(OutputText::new("retry recovered")),
            StreamChunk::Done,
        ],
    ]);
    let (_, handle) = spawn(make_args(llm, AlwaysErrToolExecutor, persistence));

    let mut rx = handle.subscribe_output();
    handle.submit(
        PromptText::new("summarize last commit"),
        EndpointName::new("ep"),
    );

    let mut saw_recovery_token = false;
    let mut saw_tool_started = false;
    let mut saw_tool_completed = false;
    let mut saw_tool_failed = false;
    let mut saw_done = false;
    for _ in 0..80 {
        let next = tokio::time::timeout(std::time::Duration::from_millis(250), rx.recv()).await;
        match next {
            Ok(Ok(AgentOutput::Token(t))) if t.as_str().contains("retry recovered") => {
                saw_recovery_token = true;
            }
            Ok(Ok(AgentOutput::ToolCallStarted { .. })) => {
                saw_tool_started = true;
            }
            Ok(Ok(AgentOutput::ToolCallCompleted { success, .. })) => {
                saw_tool_completed = true;
                saw_tool_failed = !success.0;
            }
            Ok(Ok(AgentOutput::Done)) => {
                saw_done = true;
                break;
            }
            Ok(Ok(AgentOutput::Error(e))) => panic!("unexpected error: {e}"),
            Ok(Ok(_)) => {}
            _ => break,
        }
    }

    assert!(
        saw_recovery_token,
        "expected follow-up LLM response token after tool execution error"
    );
    assert!(saw_tool_started, "expected tool-start lifecycle signal");
    assert!(
        saw_tool_completed,
        "expected tool-completed lifecycle signal"
    );
    assert!(
        saw_tool_failed,
        "expected failed status for tool execution error"
    );
    assert!(saw_done, "expected turn completion signal");
}

#[tokio::test]
async fn empty_post_tool_turn_retries_after_error_tool_result() {
    let (persistence, _dir) = temp_persistence();
    let llm = FakeLlmClient::new(vec![
        vec![
            StreamChunk::ToolCall {
                id: ToolCallId::new("call_err"),
                name: ToolName::new("shell_exec"),
                arguments: serde_json::json!({"command":"cd /workspace && git log -1 --stat"}),
            },
            StreamChunk::Done,
        ],
        vec![StreamChunk::Done],
        vec![
            StreamChunk::Token(OutputText::new("error follow-up recovered")),
            StreamChunk::Done,
        ],
    ]);
    let llm_probe = llm.clone();
    let (_, handle) = spawn(make_args(llm, AlwaysErrToolExecutor, persistence));

    let mut rx = handle.subscribe_output();
    handle.submit(
        PromptText::new("summarize last commit"),
        EndpointName::new("openrouter"),
    );

    let mut saw_done = false;
    let mut saw_recovery_token = false;
    for _ in 0..80 {
        let next = tokio::time::timeout(std::time::Duration::from_millis(250), rx.recv()).await;
        match next {
            Ok(Ok(AgentOutput::Token(t))) if t.as_str().contains("error follow-up recovered") => {
                saw_recovery_token = true;
            }
            Ok(Ok(AgentOutput::Done)) => {
                saw_done = true;
                break;
            }
            Ok(Ok(AgentOutput::Error(e))) => panic!("unexpected error: {e}"),
            Ok(Ok(_)) => {}
            _ => break,
        }
    }

    assert!(
        saw_recovery_token,
        "expected retry token after empty error follow-up"
    );
    assert!(
        saw_done,
        "expected completion after retrying empty post-tool follow-up"
    );
    let request_count = llm_probe.received.lock().unwrap().len();
    assert_eq!(
        request_count, 3,
        "expected initial + empty follow-up + retry"
    );
}

#[tokio::test]
async fn error_tool_result_allows_two_empty_follow_up_retries() {
    let (persistence, _dir) = temp_persistence();
    let llm = FakeLlmClient::new(vec![
        vec![
            StreamChunk::ToolCall {
                id: ToolCallId::new("call_err"),
                name: ToolName::new("shell_exec"),
                arguments: serde_json::json!({"command":"cd /workspace && git log -1 --stat"}),
            },
            StreamChunk::Done,
        ],
        vec![StreamChunk::Done],
        vec![StreamChunk::Done],
        vec![
            StreamChunk::Token(OutputText::new("second retry recovered")),
            StreamChunk::Done,
        ],
    ]);
    let llm_probe = llm.clone();
    let (_, handle) = spawn(make_args(llm, AlwaysErrToolExecutor, persistence));

    let mut rx = handle.subscribe_output();
    handle.submit(
        PromptText::new("summarize last commit"),
        EndpointName::new("openrouter"),
    );

    let mut saw_recovery_token = false;
    let mut saw_done = false;
    for _ in 0..120 {
        let next = tokio::time::timeout(std::time::Duration::from_millis(250), rx.recv()).await;
        match next {
            Ok(Ok(AgentOutput::Token(t))) if t.as_str().contains("second retry recovered") => {
                saw_recovery_token = true;
            }
            Ok(Ok(AgentOutput::Done)) => {
                saw_done = true;
                break;
            }
            Ok(Ok(AgentOutput::Error(e))) => panic!("unexpected error: {e}"),
            Ok(Ok(_)) => {}
            _ => break,
        }
    }

    assert!(
        saw_recovery_token,
        "expected recovery token after two empty follow-ups"
    );
    assert!(saw_done, "expected completion after recovery token");
    let request_count = llm_probe.received.lock().unwrap().len();
    assert_eq!(
        request_count, 4,
        "expected initial + two empty follow-ups + final recovery request"
    );
}

#[tokio::test]
async fn empty_post_tool_turn_retries_once_after_successful_tool_result() {
    let (persistence, _dir) = temp_persistence();
    let llm = FakeLlmClient::new(vec![
        vec![
            StreamChunk::ToolCall {
                id: ToolCallId::new("call_ok"),
                name: ToolName::new("shell_exec"),
                arguments: serde_json::json!({"command":"echo hi"}),
            },
            StreamChunk::Done,
        ],
        vec![StreamChunk::Done],
        vec![
            StreamChunk::Token(OutputText::new("retried follow-up")),
            StreamChunk::Done,
        ],
    ]);
    let llm_probe = llm.clone();
    let (_, handle) = spawn(make_args(
        llm,
        FakeToolExecutor::always_ok("ok"),
        persistence,
    ));

    let mut rx = handle.subscribe_output();
    handle.submit(
        PromptText::new("run command"),
        EndpointName::new("openrouter"),
    );

    let mut saw_retry_token = false;
    let mut saw_done = false;
    let mut saw_error = false;
    for _ in 0..120 {
        let next = tokio::time::timeout(std::time::Duration::from_millis(250), rx.recv()).await;
        match next {
            Ok(Ok(AgentOutput::Token(t))) if t.as_str().contains("retried follow-up") => {
                saw_retry_token = true;
            }
            Ok(Ok(AgentOutput::Done)) => {
                saw_done = true;
                break;
            }
            Ok(Ok(AgentOutput::Error(_))) => {
                saw_error = true;
                break;
            }
            Ok(Ok(_)) => {}
            _ => break,
        }
    }

    assert!(
        !saw_error,
        "turn should recover from a single empty follow-up"
    );
    assert!(
        saw_retry_token,
        "expected token from one-time retry follow-up"
    );
    assert!(saw_done, "expected turn completion signal");
    let request_count = llm_probe.received.lock().unwrap().len();
    assert_eq!(
        request_count, 3,
        "expected initial + empty follow-up + one retry"
    );
}

// Budget is 8 for successful tool results.  Provide 6 empty follow-ups then a
// real recovery response - verifies the retry budget absorbs a realistic burst.
#[tokio::test]
async fn burst_of_empty_responses_recovers_when_model_eventually_replies() {
    let (persistence, _dir) = temp_persistence();
    let llm = FakeLlmClient::new(vec![
        vec![
            StreamChunk::ToolCall {
                id: ToolCallId::new("call_ok"),
                name: ToolName::new("shell_exec"),
                arguments: serde_json::json!({"command":"echo hi"}),
            },
            StreamChunk::Done,
        ],
        vec![StreamChunk::Done],
        vec![StreamChunk::Done],
        vec![StreamChunk::Done],
        vec![StreamChunk::Done],
        vec![StreamChunk::Done],
        vec![StreamChunk::Done],
        vec![
            StreamChunk::Token(OutputText::new("burst-recovered")),
            StreamChunk::Done,
        ],
    ]);
    let llm_probe = llm.clone();
    let (_, handle) = spawn(make_args(
        llm,
        FakeToolExecutor::always_ok("ok"),
        persistence,
    ));

    let mut rx = handle.subscribe_output();
    handle.submit(
        PromptText::new("run command"),
        EndpointName::new("openrouter"),
    );

    let mut saw_recovery = false;
    let mut saw_done = false;
    let mut saw_error = false;
    for _ in 0..120 {
        let next = tokio::time::timeout(std::time::Duration::from_millis(250), rx.recv()).await;
        match next {
            Ok(Ok(AgentOutput::Token(t))) if t.as_str().contains("burst-recovered") => {
                saw_recovery = true;
            }
            Ok(Ok(AgentOutput::Done)) => {
                saw_done = true;
                break;
            }
            Ok(Ok(AgentOutput::Error(_))) => {
                saw_error = true;
                break;
            }
            Ok(Ok(_)) => {}
            _ => break,
        }
    }

    assert!(
        !saw_error,
        "retry budget should absorb a burst of 6 empty responses"
    );
    assert!(saw_recovery, "expected recovery token after burst");
    assert!(saw_done, "turn should complete after recovery");
    let request_count = llm_probe.received.lock().unwrap().len();
    assert_eq!(request_count, 8, "initial + 6 empty retries + 1 recovery");
}

// Budget is 8 for successful tool results.  Provide 9 empty follow-ups so the
// budget is fully exhausted - verifies the give-up path emits a visible error.
#[tokio::test]
async fn budget_exhausted_emits_error_instead_of_silent_complete() {
    let (persistence, _dir) = temp_persistence();
    let mut batches = vec![vec![
        StreamChunk::ToolCall {
            id: ToolCallId::new("call_ok"),
            name: ToolName::new("shell_exec"),
            arguments: serde_json::json!({"command":"echo hi"}),
        },
        StreamChunk::Done,
    ]];
    // 9 empty responses exhaust budget=8 and trigger give-up on the 9th.
    for _ in 0..9 {
        batches.push(vec![StreamChunk::Done]);
    }
    let (_, handle) = spawn(make_args(
        FakeLlmClient::new(batches),
        FakeToolExecutor::always_ok("ok"),
        persistence,
    ));

    let mut rx = handle.subscribe_output();
    handle.submit(
        PromptText::new("run command"),
        EndpointName::new("openrouter"),
    );

    let mut saw_error = false;
    let mut saw_done = false;
    for _ in 0..120 {
        let next = tokio::time::timeout(std::time::Duration::from_millis(250), rx.recv()).await;
        match next {
            Ok(Ok(AgentOutput::Error(_))) => {
                saw_error = true;
                break;
            }
            Ok(Ok(AgentOutput::Done)) => {
                saw_done = true;
                break;
            }
            Ok(Ok(_)) => {}
            _ => break,
        }
    }

    assert!(saw_error, "budget exhaustion should emit a visible error");
    assert!(
        !saw_done,
        "turn should not emit Done after budget exhaustion"
    );
}

#[tokio::test]
async fn stream_disconnect_without_done_emits_error() {
    let (persistence, _dir) = temp_persistence();
    // Empty batch means channel closes without emitting StreamChunk::Done.
    let llm = FakeLlmClient::new(vec![vec![]]);
    let (_, handle) = spawn(make_args(llm, FakeToolExecutor::always_ok(""), persistence));

    let mut rx = handle.subscribe_output();
    handle.submit(PromptText::new("hello"), EndpointName::new("openrouter"));

    let mut saw_error = false;
    let mut saw_done = false;
    for _ in 0..40 {
        let next = tokio::time::timeout(std::time::Duration::from_millis(250), rx.recv()).await;
        match next {
            Ok(Ok(AgentOutput::Error(e))) => {
                saw_error = e.as_str().contains("no response received");
                break;
            }
            Ok(Ok(AgentOutput::Done)) => {
                saw_done = true;
                break;
            }
            Ok(Ok(_)) => {}
            _ => break,
        }
    }

    assert!(
        !saw_done,
        "turn should not complete silently on disconnected stream"
    );
    assert!(
        saw_error,
        "expected explicit disconnected-stream error when no Done/text/tool-call arrives"
    );
}

#[tokio::test]
async fn tool_call_follow_up_request_preserves_assistant_text() {
    let (persistence, _dir) = temp_persistence();
    let llm = FakeLlmClient::new(vec![
        vec![
            StreamChunk::Token(OutputText::new("prelude context")),
            StreamChunk::ToolCall {
                id: ToolCallId::new("call_ok"),
                name: ToolName::new("shell_exec"),
                arguments: serde_json::json!({"command":"echo hi"}),
            },
            StreamChunk::Done,
        ],
        vec![
            StreamChunk::Token(OutputText::new("done")),
            StreamChunk::Done,
        ],
    ]);
    let llm_probe = llm.clone();
    let (_, handle) = spawn(make_args(
        llm,
        FakeToolExecutor::always_ok("ok"),
        persistence,
    ));

    let mut rx = handle.subscribe_output();
    handle.submit(
        PromptText::new("run command"),
        EndpointName::new("openrouter"),
    );

    for _ in 0..80 {
        let next = tokio::time::timeout(std::time::Duration::from_millis(250), rx.recv()).await;
        match next {
            Ok(Ok(AgentOutput::Done)) => break,
            Ok(Ok(AgentOutput::Error(e))) => panic!("unexpected error: {e}"),
            Ok(Ok(_)) => {}
            _ => break,
        }
    }

    let requests = llm_probe.received.lock().unwrap();
    assert!(
        requests.len() >= 2,
        "expected follow-up request after tool call"
    );
    let second_request = &requests[1];
    let saw_tool_assistant_with_text = second_request.iter().any(|message| {
        message.tool_calls.is_some() && message.content.as_str().contains("prelude context")
    });
    assert!(
        saw_tool_assistant_with_text,
        "follow-up request must preserve assistant text alongside tool call context"
    );
}
