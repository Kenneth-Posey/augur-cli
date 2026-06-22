//! TUI layout: sub-actor handle aggregation, per-frame snapshot collection,
//! and the top-level render entry point.
//!
//! [`TuiOverlayHandles`] bundles the three overlay sub-actor handles (chat
//! menu, spinner, dynamic controls). [`TuiSubActorHandles`] bundles all four
//! panel handle groups for the TUI runtime. [`collect_render_snapshot`] reads
//! each watch channel once per frame into a [`TuiRenderSnapshot`], eliminating
//! borrow conflicts between the watch-channel borrows and the `AppState` borrow
//! needed for rendering. [`render_layout`] is the single render entry point.

use crate::actors::tui_agent_panel::TuiAgentPanelHandle;
use crate::actors::tui_ask_panel::TuiAskPanelHandle;
use crate::actors::tui_chat_menu::TuiChatMenuHandle;
use crate::actors::tui_chat_menu::tui_chat_menu_ops::ChatMenuState;
use crate::actors::tui_dynamic_controls::TuiDynamicControlsHandle;
use crate::actors::tui_dynamic_controls::tui_dynamic_controls_ops::DynamicControlsState;
use crate::actors::tui_main_feed_panel::TuiMainFeedPanelHandle;
use crate::actors::tui_spinner::TuiSpinnerHandle;
use crate::actors::tui_spinner::tui_spinner_ops::SpinnerState;
use crate::domain::tui_display_state::TuiDisplayState;
use crate::domain::tui_render::AppRenderer;
use ratatui::Frame;

/// Aggregates the three overlay TUI sub-actor handles.
///
/// Groups the chat-menu, spinner, and dynamic-controls handles so that
/// [`TuiSubActorHandles`] stays within the five-field limit while
/// accommodating the new panel handles added in Phase 2.
///
/// Consumers: `collect_render_snapshot`, [`TuiSubActorHandles`].
#[derive(bon::Builder)]
pub struct TuiOverlayHandles {
    /// Handle to the TUI chat-menu sub-actor.
    pub chat_menu: TuiChatMenuHandle,
    /// Handle to the TUI spinner sub-actor.
    pub spinner: TuiSpinnerHandle,
    /// Handle to the TUI dynamic-controls sub-actor.
    pub controls: TuiDynamicControlsHandle,
}

/// Aggregates the four TUI sub-actor handle groups needed for per-frame snapshot
/// collection.
///
/// Constructed once by the TUI actor runtime after all sub-actors are spawned
/// and stored on [`super::super::TuiSpawnArgs`]. Passed to
/// `collect_render_snapshot` each frame to read watch-channel state without
/// holding live borrows across the render call.
///
/// Consumers: `collect_render_snapshot`, `wiring.rs`, integration tests.
#[derive(bon::Builder)]
pub struct TuiSubActorHandles {
    /// Handle to the TUI main feed panel sub-actor.
    pub main_feed: TuiMainFeedPanelHandle,
    /// Handle to the TUI agent panel sub-actor.
    pub agent_panel: TuiAgentPanelHandle,
    /// Handle to the TUI ask panel sub-actor.
    pub ask_panel: TuiAskPanelHandle,
    /// Bundled overlay handles: chat menu, spinner, and dynamic controls.
    pub overlays: TuiOverlayHandles,
}

/// Per-frame snapshot of watch-channel state from the three stateful sub-actors.
///
/// Collected once at the start of each render pass by [`collect_render_snapshot`]
/// so that no watch-channel borrows remain live when the render functions run.
/// All three fields are cheap clones from watch-channel cells.
/// The `renderer` field carries the injected render function pointer so that
/// [`render_layout`] can call it without importing from `crate::tui` (L10).
///
/// Consumers: [`render_layout`], layout tests.
// `chat_menu`, `spinner`, and `controls` are only read in `#[cfg(test)]`
// (layout.tests.rs); the allow suppresses the resulting false-positive warning.
#[allow(dead_code)]
#[derive(bon::Builder)]
pub struct TuiRenderSnapshot {
    /// Current chat-menu state (visible, items, selected action).
    pub chat_menu: ChatMenuState,
    /// Current spinner state (active, label, target).
    pub spinner: SpinnerState,
    /// Current dynamic controls state (controls list, visibility flag).
    pub controls: DynamicControlsState,
    /// Injected render function; called once per frame by [`render_layout`].
    pub renderer: AppRenderer,
}

/// Collect a per-frame render snapshot from the sub-actor watch channels.
///
/// Reads `current_state()` from the chat-menu, spinner, and dynamic-controls
/// handles via `handles.overlays`. Each read is a momentary borrow of the watch
/// channel's internal cell - no shared mutable state. The returned snapshot is
/// owned and can be passed freely to `render_layout` without borrow conflicts.
///
/// Inputs:
/// - `handles`   - reference to the TUI sub-actor handles.
/// - `renderer`  - the injected render function to bundle into the snapshot.
///
/// Returns: an owned [`TuiRenderSnapshot`] reflecting the latest published state.
pub fn collect_render_snapshot(
    handles: &TuiSubActorHandles,
    renderer: AppRenderer,
) -> TuiRenderSnapshot {
    TuiRenderSnapshot::builder()
        .chat_menu(handles.overlays.chat_menu.current_state())
        .spinner(handles.overlays.spinner.current_state())
        .controls(handles.overlays.controls.current_state())
        .renderer(renderer)
        .build()
}

/// Render the full TUI layout for one frame.
///
/// Delegates to the injected render function stored in `snapshot.renderer`.
/// This keeps the actor layer (`L8`) free from direct imports of the render
/// layer (`L10`) - the function pointer is the only coupling.
///
/// Inputs:
/// - `frame`     - mutable ratatui frame for the current draw pass.
/// - `snapshot`  - per-frame snapshot collected by [`collect_render_snapshot`];
///   carries the injected renderer.
/// - `display`   - display-state snapshot for this frame.
///
/// Side effects: writes widgets into `frame`; no I/O or channel operations.
pub fn render_layout(frame: &mut Frame, snapshot: &TuiRenderSnapshot, display: &TuiDisplayState) {
    (snapshot.renderer)(frame, display);
}

#[cfg(test)]
#[path = "../../../../../tests/actors/tui/tui_actor/runtime/layout.tests.rs"]
mod tests;
