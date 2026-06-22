//! Private helper operations for the token-tracker actor.

use super::token_tracker_actor::TokenTrackerState;
use super::token_tracker_ops::{TokenTrackerCommand, accumulate};
use crate::token_history;
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::types::ProjectTokenTotals;

/// Persist token totals to the optional settings file path.
pub(super) async fn persist_totals(path: Option<&std::path::Path>, totals: &ProjectTokenTotals) {
    let Some(path) = path else {
        return;
    };
    let path = path.to_path_buf();
    let totals = totals.clone();
    let save_result = tokio::task::spawn_blocking(move || {
        let mut settings = token_history::load_or_create(path.as_path())?;
        settings.token_totals = totals;
        token_history::save(&settings, path.as_path())
    })
    .await;
    match save_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!(error = %e, "failed to persist token totals"),
        Err(e) => tracing::warn!(error = %e, "token totals persistence task failed"),
    }
}

/// Dispatch one token-tracker command and return `true` when the actor should stop.
pub(super) async fn handle_command(
    cmd: TokenTrackerCommand,
    state: &mut TokenTrackerState,
) -> IsPredicate {
    match cmd {
        TokenTrackerCommand::Shutdown => IsPredicate::yes(),
        TokenTrackerCommand::Snapshot(tx) => {
            let _ = tx.send(state.totals.clone());
            IsPredicate::no()
        }
        TokenTrackerCommand::ContextSnapshot(tx) => {
            let _ = tx.send(state.last_context.clone());
            IsPredicate::no()
        }
        command => {
            handle_mutating_command(command, state).await;
            IsPredicate::no()
        }
    }
}

async fn handle_mutating_command(cmd: TokenTrackerCommand, state: &mut TokenTrackerState) {
    match cmd {
        TokenTrackerCommand::RecordUsage(usage) => {
            accumulate(&mut state.totals, &usage);
            persist_totals(state.settings_path.as_deref(), &state.totals).await;
        }
        TokenTrackerCommand::RecordContext(stats) => {
            state.last_context = Some(stats);
        }
        TokenTrackerCommand::ResetTotals => {
            state.totals = ProjectTokenTotals::default();
            persist_totals(state.settings_path.as_deref(), &state.totals).await;
        }
        TokenTrackerCommand::Snapshot(_)
        | TokenTrackerCommand::ContextSnapshot(_)
        | TokenTrackerCommand::Shutdown => {}
    }
}
