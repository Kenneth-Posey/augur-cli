//! Tests for TUI layout module: snapshot collection and render correctness.

use crate::actors::tui::tui_actor::runtime::layout::{
    collect_render_snapshot, render_layout, TuiOverlayHandles, TuiSubActorHandles,
};
use crate::actors::tui_chat_menu::tui_chat_menu_ops::ChatMenuState;
use crate::actors::tui_dynamic_controls::tui_dynamic_controls_ops::DynamicControlsState;
use crate::actors::tui_spinner::tui_spinner_ops::{SpinnerState, SpinnerTarget};
use crate::domain::string_newtypes::{EndpointName, OutputText, StatusLabel, StringNewtype};
use crate::domain::tui_display_state::TuiDisplayState;
use crate::domain::tui_render::AppRenderer;
use crate::domain::tui_state::{AppScreen, AppState};

// ── helpers ─────────────────────────────────────────────────────────────────

fn noop_renderer_for_display(_: &mut ratatui::Frame<'_>, _: &TuiDisplayState) {}

fn make_sub_actor_handles() -> TuiSubActorHandles {
    use crate::actors::tui_agent_panel::tui_agent_panel_actor::{
        spawn as spawn_agent_panel, TuiAgentPanelConfig,
    };
    use crate::actors::tui_ask_panel::tui_ask_panel_actor::spawn as spawn_ask_panel;
    use crate::actors::tui_chat_menu::tui_chat_menu_actor::spawn as spawn_chat_menu;
    use crate::actors::tui_dynamic_controls::tui_dynamic_controls_actor::spawn as spawn_controls;
    use crate::actors::tui_main_feed_panel::tui_main_feed_panel_actor::{
        spawn as spawn_main_feed, TuiMainFeedConfig,
    };
    use crate::actors::tui_main_feed_panel::tui_main_feed_panel_ops::MainFeedItem;
    use crate::actors::tui_spinner::tui_spinner_actor::spawn as spawn_spinner;
    use crate::domain::newtypes::Count;
    use crate::domain::types::AgentFeedOutput;

    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel::<AgentFeedOutput>(8);
    let (main_feed_tx, _main_feed_rx) = tokio::sync::mpsc::channel::<MainFeedItem>(8);

    let (_, agent_panel_handle) = spawn_agent_panel(TuiAgentPanelConfig {
        unified_tx: agent_feed_tx,
        capacity: 8,
    });
    let (_, main_feed_handle) = spawn_main_feed(TuiMainFeedConfig {
        unified_tx: main_feed_tx,
        capacity: 8,
    });
    let (_, ask_panel_handle) = spawn_ask_panel(Count::of(8));
    let (_, chat_menu_handle) = spawn_chat_menu(Count::of(8));
    let (_, spinner_handle) = spawn_spinner(Count::of(8));
    let (_, controls_handle) = spawn_controls(Count::of(8));

    TuiSubActorHandles::builder()
        .main_feed(main_feed_handle)
        .agent_panel(agent_panel_handle)
        .ask_panel(ask_panel_handle)
        .overlays(
            TuiOverlayHandles::builder()
                .chat_menu(chat_menu_handle)
                .spinner(spinner_handle)
                .controls(controls_handle)
                .build(),
        )
        .build()
}

fn empty_snapshot() -> crate::actors::tui::tui_actor::runtime::layout::TuiRenderSnapshot {
    use crate::actors::tui::tui_actor::runtime::layout::TuiRenderSnapshot;
    TuiRenderSnapshot::builder()
        .chat_menu(ChatMenuState::default())
        .spinner(
            SpinnerState::builder()
                .target(SpinnerTarget::MainConversation)
                .build(),
        )
        .controls(DynamicControlsState::default())
        .renderer(noop_renderer_for_display as AppRenderer)
        .build()
}

fn conversation_app_state() -> AppState {
    AppState::new(EndpointName::new("ep"), AppScreen::Conversation)
}

// ── tests ────────────────────────────────────────────────────────────────────

/// `collect_render_snapshot` copies the current chat-menu state from the handle.
#[tokio::test]
async fn test_collect_render_snapshot_copies_chat_menu_state() {
    let handles = make_sub_actor_handles();

    // Set the chat menu to visible with known items.
    handles
        .overlays
        .chat_menu
        .show(vec![OutputText::from("alpha"), OutputText::from("beta")]);
    // Give the actor a tick to process the command.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let snapshot = collect_render_snapshot(&handles, noop_renderer_for_display as AppRenderer);

    assert!(
        snapshot.chat_menu.visible,
        "chat_menu snapshot should be visible"
    );
    assert_eq!(snapshot.chat_menu.items, vec!["alpha", "beta"]);
}

/// `collect_render_snapshot` copies the current spinner state from the handle.
#[tokio::test]
async fn test_collect_render_snapshot_copies_spinner_state() {
    let handles = make_sub_actor_handles();

    // Activate the spinner.
    handles.overlays.spinner.start(
        SpinnerTarget::MainConversation,
        StatusLabel::from("thinking…"),
    );
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let snapshot = collect_render_snapshot(&handles, noop_renderer_for_display as AppRenderer);

    assert!(snapshot.spinner.active, "spinner snapshot should be active");
    assert_eq!(snapshot.spinner.label, "thinking…");
}

/// `render_layout` must not panic when given a minimal empty snapshot and a
/// default `AppState`. Uses ratatui `TestBackend` to produce a real `Frame`.
#[tokio::test]
async fn test_render_layout_does_not_panic_on_empty_snapshot() {
    use ratatui::{backend::TestBackend, Terminal};

    let snapshot = empty_snapshot();
    let app_state = conversation_app_state();
    let display = crate::domain::tui_display_state::TuiDisplayState::project_from(&app_state);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_layout(frame, &snapshot, &display))
        .unwrap();
}
