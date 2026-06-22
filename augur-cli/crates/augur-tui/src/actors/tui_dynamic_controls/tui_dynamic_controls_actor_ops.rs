//! Private helper operations for the TUI dynamic-controls actor.

use super::handle::ControlsVisibility;
use super::tui_dynamic_controls_ops::{ControlItem, DynamicControlsState};
use crate::domain::newtypes::IsVisible;
use tokio::sync::watch;

/// Replace the current dynamic-controls item list.
pub(super) fn apply_set_controls(
    state_tx: &watch::Sender<DynamicControlsState>,
    items: Vec<ControlItem>,
) {
    let mut next = state_tx.borrow().clone();
    next.controls = items;
    state_tx.send_replace(next);
}

/// Set dynamic-controls panel visibility.
pub(super) fn apply_set_visible(
    state_tx: &watch::Sender<DynamicControlsState>,
    visible: ControlsVisibility,
) {
    let mut next = state_tx.borrow().clone();
    next.visible = IsVisible::from(bool::from(visible));
    state_tx.send_replace(next);
}
