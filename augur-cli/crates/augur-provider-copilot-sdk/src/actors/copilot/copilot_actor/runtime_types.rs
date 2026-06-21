use super::super::commands::CopilotChatCmd;
use augur_domain::persistence::handle::PersistenceHandle;
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use augur_domain::types::{AgentOutput, FeedEntry};
use augur_domain::{HistoryAdapterHandle, LoggerHandle, TokenTrackerHandle};
use tokio::sync::{broadcast, mpsc};

use super::CopilotChannels;

/// Non-channel dependencies threaded through the actor run path.
///
/// Groups logger, persistence, and history adapter handle so `RunArgs` stays within
/// the 5-field limit.
/// Consumers: `RunArgs`, `run_with_sdk`.
#[derive(bon::Builder)]
pub(super) struct RunHandles {
    pub(super) logger: LoggerHandle,
    pub(super) persistence: PersistenceHandle,
    pub(super) history_adapter: HistoryAdapterHandle,
}

/// Channels, handles, and query sender threaded through the run path.
///
/// Bundles owned receivers, sender, handles, and channels so `run`
/// and `run_with_sdk` each take two parameters (config + args) within the 3-param limit.
#[derive(bon::Builder)]
pub(super) struct RunArgs {
    pub(super) cmd_rx: mpsc::Receiver<CopilotChatCmd>,
    pub(super) output_tx: broadcast::Sender<AgentOutput>,
    pub(super) handles: RunHandles,
    pub(super) channels: CopilotChannels,
}

/// Dispatch channel and handle bundle for `CopilotCmdContext`.
///
/// Bundles query, agent-feed, and token-tracker so `CopilotCmdContext` stays
/// within the 5-field limit when all three are needed.
/// Consumers: `CopilotCmdContext`, `spawn_background_agent`, `activate_session`.
pub(super) struct CopilotDispatchHandles {
    pub(super) query_tx: mpsc::Sender<QueryUserRequest>,
    pub(super) agent_feed_tx: mpsc::Sender<FeedEntry>,
    pub(super) token_tracker: TokenTrackerHandle,
}

/// Runtime context for the command loop and session restart helper.
///
/// Bundles the mutable command receiver, output broadcast sender, logging
/// state, and dispatch handles so `run_command_loop` and
/// `attempt_session_restart` each take two parameters (session/client + context)
/// within the 3-param limit.
/// Consumers: `run_with_sdk`, `run_command_loop`, `attempt_session_restart`.
#[derive(bon::Builder)]
pub(super) struct CopilotCmdContext<'a> {
    pub(super) cmd_rx: &'a mut mpsc::Receiver<CopilotChatCmd>,
    pub(super) output_tx: &'a broadcast::Sender<AgentOutput>,
    pub(super) log: super::super::assistant::LogState,
    /// Dispatch channel bundle: query, agent-feed, and token-tracker.
    pub(super) dispatch: CopilotDispatchHandles,
}

/// Exit reason returned by `run_command_loop`.
///
/// Allows `run_with_sdk` to decide what action to take after the loop returns
/// without relying on a boolean. `Clean` means the loop exited by design;
/// `FatalError` means a session send error occurred and a restart should be
/// attempted; `ReplaceSession` means the TUI requested a new or resumed SDK
/// session and `run_with_sdk` must create/resume it before re-entering the loop.
pub(super) enum LoopExit {
    /// The loop exited cleanly via `Shutdown` or channel close.
    Clean,
    /// A fatal SDK send error occurred; the caller should attempt one restart.
    FatalError,
    /// The TUI requested a new or resumed SDK session.
    ///
    /// `run_with_sdk` must call `create_or_resume_session` with the given ID
    /// (or create a fresh session when `None`) and re-enter the command loop.
    ReplaceSession(Option<augur_domain::string_newtypes::SdkSessionId>),
}

#[derive(bon::Builder)]
pub(super) struct InitialSessionState {
    pub(super) session: std::sync::Arc<copilot_sdk::Session>,
    pub(super) log: super::super::assistant::LogState,
    pub(super) pending_restore: Vec<augur_domain::persistence::types::MessageRecord>,
}

#[derive(bon::Builder)]
pub(super) struct InitialSessionInputs<'a> {
    pub(super) client: &'a copilot_sdk::Client,
    pub(super) config: &'a augur_domain::config::types::CopilotChatConfig,
    pub(super) output_tx: &'a broadcast::Sender<AgentOutput>,
    pub(super) cmd_rx: &'a mut mpsc::Receiver<CopilotChatCmd>,
}

#[derive(bon::Builder)]
pub(super) struct InitialSessionServices<'a> {
    pub(super) logger: LoggerHandle,
    pub(super) persistence: PersistenceHandle,
    pub(super) history_adapter: HistoryAdapterHandle,
    pub(super) token_tracker: &'a TokenTrackerHandle,
}

#[derive(bon::Builder)]
pub(super) struct CommandContextArgs<'a> {
    pub(super) cmd_rx: &'a mut mpsc::Receiver<CopilotChatCmd>,
    pub(super) output_tx: &'a broadcast::Sender<AgentOutput>,
    pub(super) dispatch: CopilotDispatchHandles,
    pub(super) initial_state: InitialSessionState,
}

#[derive(bon::Builder)]
pub(super) struct ActivateSessionArgs<'a, 'b> {
    pub(super) session: &'a std::sync::Arc<copilot_sdk::Session>,
    pub(super) ctx: &'a CopilotCmdContext<'b>,
    pub(super) reason: &'static str,
}

#[derive(bon::Builder)]
pub(super) struct SessionLifecycleArgs<'a, 'b> {
    pub(super) client: &'a copilot_sdk::Client,
    pub(super) config: &'a augur_domain::config::types::CopilotChatConfig,
    pub(super) initial_session: std::sync::Arc<copilot_sdk::Session>,
    pub(super) ctx: &'a mut CopilotCmdContext<'b>,
}

#[derive(bon::Builder)]
pub(super) struct ResolveLoopExitArgs<'a, 'b> {
    pub(super) client: &'a copilot_sdk::Client,
    pub(super) config: &'a augur_domain::config::types::CopilotChatConfig,
    pub(super) exit: LoopExit,
    pub(super) restart_tried: &'a mut bool,
    pub(super) ctx: &'a mut CopilotCmdContext<'b>,
}

#[derive(bon::Builder)]
pub(super) struct RestartSessionArgs<'a, 'b> {
    pub(super) client: &'a copilot_sdk::Client,
    pub(super) config: &'a augur_domain::config::types::CopilotChatConfig,
    pub(super) restart_tried: &'a mut bool,
    pub(super) ctx: &'a mut CopilotCmdContext<'b>,
}

#[derive(bon::Builder)]
pub(super) struct ReplaceSessionArgs<'a, 'b> {
    pub(super) client: &'a copilot_sdk::Client,
    pub(super) config: &'a augur_domain::config::types::CopilotChatConfig,
    pub(super) sdk_id: Option<augur_domain::string_newtypes::SdkSessionId>,
    pub(super) ctx: &'a mut CopilotCmdContext<'b>,
}

#[derive(bon::Builder)]
pub(super) struct CommandLoopState<'a, 'b> {
    pub(super) session: &'a copilot_sdk::Session,
    pub(super) ctx: &'a mut CopilotCmdContext<'b>,
    pub(super) log_rx: &'a mut tokio::sync::broadcast::Receiver<AgentOutput>,
}

pub(super) struct RunInitialStateInputs<'a> {
    pub(super) client: &'a copilot_sdk::Client,
    pub(super) config: &'a augur_domain::config::types::CopilotChatConfig,
    pub(super) services: RunInitialServices<'a>,
}

pub(super) struct RunInitialServices<'a> {
    pub(super) logger: LoggerHandle,
    pub(super) persistence: PersistenceHandle,
    pub(super) history_adapter: HistoryAdapterHandle,
    pub(super) token_tracker: &'a TokenTrackerHandle,
}

pub(super) struct ActiveSessionCommandContextArgs<'a> {
    pub(super) cmd_rx: &'a mut mpsc::Receiver<CopilotChatCmd>,
    pub(super) output_tx: &'a broadcast::Sender<AgentOutput>,
    pub(super) dispatch: CopilotDispatchHandles,
    pub(super) initial_state: InitialSessionState,
}

pub(super) struct StartActiveSessionLifecycleArgs<'a, 'b> {
    pub(super) client: &'a copilot_sdk::Client,
    pub(super) config: &'a augur_domain::config::types::CopilotChatConfig,
    pub(super) initial_session: std::sync::Arc<copilot_sdk::Session>,
    pub(super) ctx: &'a mut CopilotCmdContext<'b>,
}
