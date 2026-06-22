use augur_cli::wiring::{
    domain_runtime_config_ref, spawn_domain_actors, spawn_infrastructure, take_openrouter_feed_rx,
};
use augur_core::config::load_config;
use augur_domain::config::types::{AppConfig, ProgramSettings};
use augur_domain::domain::StringNewtype;
use augur_domain::domain::newtypes::{NumericNewtype, TimestampSecs};
use augur_domain::domain::string_newtypes::{EndpointName, PromptText};
use augur_domain::domain::types::{AgentFeedOutput, AgentOutput};
use std::sync::Once;
use tokio::sync::broadcast;

const LIVE_GATE_ENV: &str = "DCMK_RUN_LIVE_OPENROUTER";
const OPENROUTER_KEY_ENV: &str = "OPENROUTER_API_KEY";
static TRACING_INIT: Once = Once::new();

#[derive(Default)]
struct LiveCycleStats {
    tokens: usize,
    saw_done: bool,
    saw_error: bool,
    error_text: Option<String>,
    tool_calls_started: usize,
    tool_calls_completed: usize,
    saw_task_started: bool,
    saw_task_completed: bool,
    saw_task_failed: bool,
    task_failed_reason: Option<String>,
    status_lines: usize,
}

fn gate_enabled() -> bool {
    std::env::var(LIVE_GATE_ENV)
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn init_live_tracing() {
    TRACING_INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("debug,llm_raw=info")
            .with_test_writer()
            .try_init();
    });
}

fn live_openrouter_config() -> AppConfig {
    let mut config = load_config(None).expect("load default config with secrets overlay");
    config.default_endpoint = EndpointName::new("openrouter");
    config
}

async fn wait_for_eventful_turn(
    output_rx: &mut broadcast::Receiver<AgentOutput>,
    feed_rx: &mut tokio::sync::mpsc::Receiver<augur_domain::domain::types::FeedEntry>,
    timeout: std::time::Duration,
) -> LiveCycleStats {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut stats = LiveCycleStats::default();
    loop {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::select! {
            out = output_rx.recv() => {
                match out {
                    Ok(AgentOutput::Token(_)) => stats.tokens += 1,
                    Ok(AgentOutput::Done) => stats.saw_done = true,
                    Ok(AgentOutput::Error(err)) => {
                        stats.saw_error = true;
                        stats.error_text = Some(err.as_str().to_owned());
                    }
                    Ok(AgentOutput::ToolCallStarted { .. }) => stats.tool_calls_started += 1,
                    Ok(AgentOutput::ToolCallCompleted { .. }) => stats.tool_calls_completed += 1,
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            feed = feed_rx.recv() => {
                match feed {
                    Some(entry) => match entry.output {
                        AgentFeedOutput::TaskStarted { .. } => stats.saw_task_started = true,
                        AgentFeedOutput::TaskCompleted { .. } => stats.saw_task_completed = true,
                        AgentFeedOutput::TaskFailed { reason, .. } => {
                            stats.saw_task_failed = true;
                            stats.task_failed_reason = Some(reason.as_str().to_owned());
                        }
                        AgentFeedOutput::StatusLine(_) => stats.status_lines += 1,
                        _ => {}
                    },
                    None => break,
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(25)) => {}
        }
        if stats.saw_done && stats.saw_task_completed {
            break;
        }
    }
    stats
}

#[tokio::test]
#[ignore = "live OpenRouter diagnostic; requires DCMK_RUN_LIVE_OPENROUTER=1 and OPENROUTER_API_KEY"]
async fn live_openrouter_background_agent_cycle_reaches_task_completion_and_turn_done() {
    init_live_tracing();
    if !gate_enabled() {
        return;
    }
    if std::env::var(OPENROUTER_KEY_ENV).is_err() {
        return;
    }

    let config = live_openrouter_config();
    let program_settings = ProgramSettings::default();
    let mut core = spawn_infrastructure(&config, &program_settings, TimestampSecs::new(1));
    let mut openrouter_feed_rx = take_openrouter_feed_rx(&mut core);
    let (agent_feed_tx, _agent_feed_rx) =
        tokio::sync::mpsc::channel(*augur_domain::domain::channels::AGENT_FEED_CAPACITY);
    let domain = spawn_domain_actors(
        domain_runtime_config_ref(&config, &program_settings),
        &core,
        agent_feed_tx,
    )
    .await;

    let mut output_rx = domain.agent.handle.subscribe_output();

    domain
        .agent
        .handle
        .submit(PromptText::new("hello"), EndpointName::new("openrouter"));
    let hello_stats = wait_for_eventful_turn(
        &mut output_rx,
        &mut openrouter_feed_rx,
        std::time::Duration::from_secs(60),
    )
    .await;
    assert!(
        hello_stats.saw_done,
        "hello turn must complete; stats={:?}",
        (
            hello_stats.tokens,
            hello_stats.saw_error,
            hello_stats.error_text.as_deref().unwrap_or(""),
            hello_stats.saw_task_started,
            hello_stats.saw_task_completed,
            hello_stats.saw_task_failed,
            hello_stats.task_failed_reason.as_deref().unwrap_or(""),
            hello_stats.status_lines
        )
    );
    assert!(
        hello_stats.tokens > 0 && !hello_stats.saw_error,
        "hello turn must stream output without terminal error"
    );

    domain.agent.handle.submit(
        PromptText::new(
            "Run shell_exec with command `git log -1 --stat`, then summarize the last commit.",
        ),
        EndpointName::new("openrouter"),
    );
    let cycle_stats = wait_for_eventful_turn(
        &mut output_rx,
        &mut openrouter_feed_rx,
        std::time::Duration::from_secs(180),
    )
    .await;

    assert!(
        cycle_stats.tool_calls_started > 0,
        "second turn must start at least one tool call; stats={:?}",
        (
            cycle_stats.tokens,
            cycle_stats.saw_done,
            cycle_stats.saw_error,
            cycle_stats.error_text.as_deref().unwrap_or(""),
            cycle_stats.tool_calls_started,
            cycle_stats.tool_calls_completed,
            cycle_stats.saw_task_started,
            cycle_stats.saw_task_completed,
            cycle_stats.saw_task_failed,
            cycle_stats.task_failed_reason.as_deref().unwrap_or(""),
            cycle_stats.status_lines
        )
    );
    assert!(
        cycle_stats.tool_calls_completed > 0,
        "second turn must complete at least one tool call"
    );
    assert!(cycle_stats.saw_done, "outer turn must emit Done");
    assert!(
        cycle_stats.tokens > 0 && !cycle_stats.saw_error,
        "outer turn must stream response tokens and finish without terminal error"
    );
}
