//! Private helper operations for the TUI spinner actor.

use super::tui_spinner_ops::{SpinnerState, SpinnerTarget};
use crate::domain::newtypes::IsActive;
use crate::domain::string_newtypes::SpinnerLabel;
use augur_domain::domain::string_newtypes::{StatusLabel, StringNewtype};
use tokio::sync::watch;

/// Start the spinner for `target` with the supplied status label.
pub(super) fn apply_start(
    state_tx: &watch::Sender<SpinnerState>,
    target: SpinnerTarget,
    label: StatusLabel,
) {
    state_tx.send_replace(
        SpinnerState::builder()
            .active(IsActive::yes())
            .label(SpinnerLabel::new(label.as_str()))
            .target(target)
            .build(),
    );
}

/// Stop the spinner for `target` and clear its status label.
pub(super) fn apply_stop(state_tx: &watch::Sender<SpinnerState>, target: SpinnerTarget) {
    state_tx.send_replace(
        SpinnerState::builder()
            .active(IsActive::no())
            .label(SpinnerLabel::new(""))
            .target(target)
            .build(),
    );
}
