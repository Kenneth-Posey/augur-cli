//! TokenTrackerActor: spawns the run loop and owns all token accumulation state.

use super::handle::TokenTrackerHandle;
use super::token_tracker_actor_ops as actor_ops;
use super::token_tracker_ops::TokenTrackerCommand;
use crate::token_history::ProjectSettings;
use augur_domain::domain::channels::TOKEN_TRACKER_COMMAND_CAPACITY;
use augur_domain::domain::types::{ContextUsageStats, ProjectTokenTotals};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Spawn the token-tracker actor with default settings and no persistence path.
///
/// Uses [`ProjectSettings::default`] for initial token totals and passes
/// `None` for the settings path, so no on-disk persistence occurs.
///
/// # Returns
///
/// `(JoinHandle<()>, TokenTrackerHandle)` - the actor task handle and the
/// communication handle used to send commands.
///
/// # Panics
///
/// Panics if called outside an active Tokio runtime.
pub fn spawn() -> (JoinHandle<()>, TokenTrackerHandle) {
    spawn_with_settings(ProjectSettings::default(), None)
}

/// Spawn the token-tracker actor with caller-supplied initial settings.
///
/// Initialises token totals from `initial_settings.token_totals`. When
/// `settings_path` is `Some`, the actor persists updated totals to that file
/// after each `RecordUsage` or `ResetTotals` command via a blocking task.
///
/// # Parameters
///
/// - `initial_settings`: Provides the starting `ProjectTokenTotals`; typically
///   loaded from the project settings file at startup.
/// - `settings_path`: Filesystem path for persistence. Pass `None` to disable
///   on-disk writes (used in tests and the default [`spawn`] constructor).
///
/// # Returns
///
/// `(JoinHandle<()>, TokenTrackerHandle)` - the actor task handle and the
/// communication handle used to send commands.
///
/// # Preconditions
///
/// Must be called from within an active Tokio runtime context.
pub(crate) fn spawn_with_settings(
    initial_settings: ProjectSettings,
    settings_path: Option<std::path::PathBuf>,
) -> (JoinHandle<()>, TokenTrackerHandle) {
    let (tx, rx) = mpsc::channel(*TOKEN_TRACKER_COMMAND_CAPACITY);
    let handle = TokenTrackerHandle::new(tx);
    let state = TokenTrackerState {
        totals: initial_settings.token_totals,
        last_context: None,
        settings_path,
    };
    let join = tokio::spawn(run(state, rx));
    (join, handle)
}

/// All mutable state owned exclusively by the actor task.
///
/// `totals` grows monotonically until an explicit `ResetTotals`; `last_context`
/// is replaced on each update.
pub(super) struct TokenTrackerState {
    pub(super) totals: ProjectTokenTotals,
    pub(super) last_context: Option<ContextUsageStats>,
    pub(super) settings_path: Option<std::path::PathBuf>,
}

/// Main actor run loop: processes commands until `Shutdown` or channel close.
async fn run(mut state: TokenTrackerState, mut rx: mpsc::Receiver<TokenTrackerCommand>) {
    while let Some(cmd) = rx.recv().await {
        if bool::from(actor_ops::handle_command(cmd, &mut state).await) {
            break;
        }
    }
}
