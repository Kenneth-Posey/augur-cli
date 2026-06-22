//! `CopilotChatActor` - thin orchestration actor for the GitHub Copilot SDK.
//!
//! Spawned by `wiring.rs` when `config.copilot_chat.enabled` is true.
//! Without the `copilot-executor` feature, the actor exits immediately with
//! a warning so the rest of the system still compiles and tests pass.
//!
//! Startup sequence (feature-enabled):
//! 1. Build `copilot_sdk::Client` using `CopilotChatConfig`.
//! 2. Start the client subprocess.
//! 3. Check auth status - emit error and exit if not authenticated.
//! 4. Emit available models and seed the status bar (client-level, no session needed).
//! 5. Wait for the TUI picker to signal which session to start (`wait_for_session_signal`).
//!    The TUI sends `ReplaceSession(None)` for a new session or `ReplaceSession(Some(id))`
//!    to restore an existing one (preceded by a `Restore` command with message history).
//! 6. Create or resume the chat session based on the picker signal.
//! 7. Record the SDK session ID in the persistence handle for future restores.
//! 8. Spawn the event dispatch loop.
//! 9. Enter the command loop, dispatching `CopilotChatCmd` to the session.

mod command_handlers;
mod command_loop;
mod runtime_types;
mod session_activation;
mod session_lifecycle;
mod startup;

use super::handle::{CopilotChatHandle, make_output_channel};
use augur_domain::channels::COPILOT_COMMAND_CAPACITY;
use augur_domain::config::types::CopilotChatConfig;
use augur_domain::persistence::handle::PersistenceHandle;
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use augur_domain::types::{AgentOutput, FeedEntry};
use augur_domain::{HistoryAdapterHandle, LoggerHandle, TokenTrackerHandle};
use runtime_types::{RunArgs, RunHandles};
use tokio::sync::mpsc;

/// Outbound channel bundle for the Copilot actor.
///
/// Groups the query-user sender, the agent-feed sender, and the token-tracker
/// handle so `CopilotSpawnArgs` stays within the 5-field limit.
pub struct CopilotChannels {
    /// Sender for `query_user` tool requests. The TUI actor holds the receiver
    /// and displays an interactive prompt when a request arrives.
    pub query_tx: mpsc::Sender<QueryUserRequest>,
    /// Sender for agent-feed output events. Background agent tasks push
    /// progress events here; the TUI actor holds the receiver.
    pub agent_feed_tx: mpsc::Sender<FeedEntry>,
    /// Token-tracker handle for recording usage from background agent sessions.
    pub token_tracker: TokenTrackerHandle,
}

/// Arguments for spawning the `CopilotChatActor`.
///
/// Bundles config, logger, persistence, history adapter, and the channel bundle
/// so `spawn` takes a single parameter rather than growing beyond the 3-param limit.
/// Callers: `wiring::wire_chat_provider`.
#[derive(bon::Builder)]
pub struct CopilotSpawnArgs {
    /// Runtime configuration for the Copilot SDK session.
    pub config: CopilotChatConfig,
    /// Logger handle for turn-level message logging.
    pub logger: LoggerHandle,
    /// Persistence handle for saving conversation turns to disk.
    pub persistence: PersistenceHandle,
    /// History adapter handle for fire-and-forget conversation message recording.
    pub history_adapter: HistoryAdapterHandle,
    /// Outbound channel bundle (query sender + agent-feed sender + token tracker).
    pub channels: CopilotChannels,
}

/// Spawn the `CopilotChatActor` and return its handle.
///
/// Creates the command channel, output broadcast channel, and handle, then
/// spawns the actor task. When `config.enabled` is false the task exits
/// immediately without emitting any output.
///
/// Callers: `wiring::wire_chat_provider` (feature-gated) when
/// `config.copilot_chat.enabled` is true.
#[tracing::instrument(skip_all, level = "info")]
pub async fn spawn(args: CopilotSpawnArgs) -> (tokio::task::JoinHandle<()>, CopilotChatHandle) {
    let (cmd_tx, cmd_rx) = mpsc::channel(*COPILOT_COMMAND_CAPACITY);
    let output_tx = make_output_channel();
    let handle = CopilotChatHandle::new(cmd_tx, output_tx.clone());
    let run_args = RunArgs::builder()
        .cmd_rx(cmd_rx)
        .output_tx(output_tx)
        .handles(
            RunHandles::builder()
                .logger(args.logger)
                .persistence(args.persistence)
                .history_adapter(args.history_adapter)
                .build(),
        )
        .channels(args.channels)
        .build();
    let join = tokio::spawn(run(args.config, run_args));
    (join, handle)
}

/// Actor run loop. Routes to the SDK path or exits when feature is absent.
async fn run(config: CopilotChatConfig, args: RunArgs) {
    run_with_sdk(config, args).await;
}

/// Emit an `AgentOutput` on the broadcast channel.
///
/// Logs a debug message when all subscribers have dropped.
/// Called from the event dispatch loop and the command loop error paths.
fn emit(output: AgentOutput, tx: &tokio::sync::broadcast::Sender<AgentOutput>) {
    if tx.send(output).is_err() {
        tracing::debug!("CopilotChatActor: no output subscribers, event dropped");
    }
}

/// Full SDK startup: build client, authenticate, create or resume session, command loop.
async fn run_with_sdk(config: CopilotChatConfig, args: RunArgs) {
    if !config.enabled {
        tracing::info!("CopilotChatActor: disabled in config, exiting");
        return;
    }

    let Some(client) = startup::start_sdk_client(&config, &args.output_tx).await else {
        return;
    };
    startup::run_active_session(&client, &config, args).await;
    let _ = client.stop().await;
    tracing::info!("CopilotChatActor: stopped cleanly");
}
