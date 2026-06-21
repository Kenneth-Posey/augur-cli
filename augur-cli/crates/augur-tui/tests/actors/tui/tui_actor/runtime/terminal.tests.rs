use super::*;
use crate::domain::newtypes::ScrollOffset;
use crate::domain::plan_tree::PlanTree;
use crate::domain::string_newtypes::{
    ChoiceText, EndpointName, OutputText, PromptText, StringNewtype,
};
use crate::domain::traits::ChatProvider;
use crate::domain::tui_state::{
    AppScreen, AppState, ConversationMode, OutputSelection, PlanModeState, QueryState,
    SelectionPoint,
};
use crate::domain::types::AgentOutput;
use crate::persistence::types::MessageRecord;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::layout::Rect;
use std::sync::{Arc, Mutex};

fn conversation_state() -> AppState {
    AppState::new(EndpointName::new("ep"), AppScreen::Conversation)
}

fn plan_state() -> AppState {
    let mut state = conversation_state();
    state.interaction.mode = ConversationMode::Plan(PlanModeState {
        tree: PlanTree::new("p1", "Test Plan", "goal"),
        running: false,
        tree_scroll: ScrollOffset::of(0),
    });
    state.output.panel_areas.output_area.set(Rect {
        x: 0,
        y: 0,
        width: 60,
        height: 24,
    });
    state.output.panel_areas.plan_panel_area.set(Rect {
        x: 60,
        y: 0,
        width: 20,
        height: 24,
    });
    state
}

fn query_state() -> AppState {
    let mut state = conversation_state();
    let (reply_tx, _reply_rx) = tokio::sync::oneshot::channel::<OutputText>();
    state.interaction.mode = ConversationMode::Query(QueryState {
        question: PromptText::new("Pick one"),
        choices: vec![ChoiceText::new("yes"), ChoiceText::new("no")],
        selected: None,
        freeform: PromptText::new(""),
        reply_tx,
    });
    state
}

fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn set_output_area(state: &mut AppState) {
    state.output.panel_areas.output_area.set(Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    });
}

fn set_secondary_panel_area(state: &mut AppState, area: Rect) {
    state.output.panel_areas.secondary_panel_area.set(area);
}

struct NullChat {
    compact_calls: Arc<Mutex<usize>>,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
}

impl NullChat {
    fn new() -> Self {
        let (output_tx, _) = tokio::sync::broadcast::channel(1);
        Self {
            compact_calls: Arc::new(Mutex::new(0)),
            output_tx,
        }
    }
}

impl ChatProvider for NullChat {
    fn submit(&self, _: PromptText, _: Option<EndpointName>) {}

    fn interrupt(&self) {}

    fn shutdown(&self) {}

    fn restore(&self, _: Vec<MessageRecord>) {}

    fn subscribe_output(&self) -> tokio::sync::broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }

    fn compact(&self) {
        *self.compact_calls.lock().expect("compact lock") += 1;
    }
}

struct TestRigCoreHandles {
    command: crate::actors::command::handle::CommandHandle,
    session: crate::actors::SessionHandle,
    persistence: crate::persistence::handle::PersistenceHandle,
}

struct TestRigToolHandles {
    scanner: crate::actors::file_scanner::FileScannerHandle,
    guided_plan: crate::actors::guided_plan::GuidedPlanHandle,
    ask: crate::actors::ask::AskHandle,
    logger: crate::actors::LoggerHandle,
}

struct TestRigResources {
    _persistence_dir: tempfile::TempDir,
    _scanner_join: tokio::task::JoinHandle<()>,
    _ask_dir: tempfile::TempDir,
    _logger_join: tokio::task::JoinHandle<()>,
}

struct TestRig {
    provider: NullChat,
    core: TestRigCoreHandles,
    tools: TestRigToolHandles,
    _resources: TestRigResources,
}

impl TestRig {
    async fn new() -> Self {
        let command = crate::actors::command::command_actor::build(&[]);
        let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
        let persistence_dir = tempfile::tempdir().expect("tempdir");
        let persistence =
            crate::persistence::handle::PersistenceHandle::new(persistence_dir.path().to_owned());
        let (scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
        let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
        let (ask, ask_dir) = crate::tests::helpers::fake_ask::make_ask_handle().await;
        let (logger_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
        Self {
            provider: NullChat::new(),
            core: TestRigCoreHandles {
                command,
                session,
                persistence,
            },
            tools: TestRigToolHandles {
                scanner,
                guided_plan,
                ask,
                logger,
            },
            _resources: TestRigResources {
                _persistence_dir: persistence_dir,
                _scanner_join: scanner_join,
                _ask_dir: ask_dir,
                _logger_join: logger_join,
            },
        }
    }

    fn handles(&self) -> crate::actors::tui::tui_actor::TuiHandles<'_> {
        let (_catalog_manager_join, catalog_manager) =
            crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
        crate::actors::tui::tui_actor::TuiHandles {
            agent: &self.provider,
            session: &self.core.session,
            persistence: &self.core.persistence,
            tools: crate::actors::tui::tui_actor::TuiToolHandles {
                command: &self.core.command,
                file_scanner: &self.tools.scanner,
                guided_plan: &self.tools.guided_plan,
                ask: &self.tools.ask,
                logger: &self.tools.logger,
            },
            work: crate::actors::tui::tui_actor::TuiWorkHandles {
                orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
                catalog_manager,
            },
        }
    }
}

/// Verifies that a right-click is handled as a paste action and always requests a redraw.
#[test]
fn handle_mouse_event_right_click_returns_redraw_and_pastes_when_clipboard_available() {
    let mut state = conversation_state();
    state.prompt.buffer = "prefix".to_owned();
    state.prompt.cursor = state.prompt.buffer.len();

    let expected = " pasted";
    let mut clipboard = arboard::Clipboard::new().ok();
    let clipboard_available = clipboard
        .as_mut()
        .and_then(|clipboard| clipboard.set_text(expected).ok().map(|_| clipboard))
        .and_then(|clipboard| clipboard.get_text().ok())
        .is_some_and(|text| text == expected);

    let outcome = handle_mouse_event(
        &mut state,
        mouse_event(MouseEventKind::Down(MouseButton::Right), 12, 4),
    );

    assert!(matches!(outcome, EventOutcome::Redraw));
    if clipboard_available {
        assert_eq!(state.prompt.buffer, "prefix pasted");
        assert_eq!(state.prompt.cursor, "prefix pasted".len());
    } else {
        assert_eq!(state.prompt.buffer, "prefix");
        assert_eq!(state.prompt.cursor, "prefix".len());
    }
}

/// Verifies that SelectionStart creates a new anchored selection at the clicked point.
#[test]
fn handle_mouse_event_selection_start_sets_anchor_and_cursor() {
    let mut state = conversation_state();
    set_output_area(&mut state);

    let outcome = handle_mouse_event(
        &mut state,
        mouse_event(MouseEventKind::Down(MouseButton::Left), 10, 5),
    );

    assert!(matches!(outcome, EventOutcome::Redraw));
    assert_eq!(
        state.output.selection,
        Some(OutputSelection {
            anchor: SelectionPoint { row: 5, col: 10 },
            cursor: SelectionPoint { row: 5, col: 10 },
        })
    );
}

/// Verifies that SelectionExtend updates only the cursor endpoint of the active selection.
#[test]
fn handle_mouse_event_selection_extend_updates_cursor() {
    let mut state = conversation_state();
    set_output_area(&mut state);
    state.output.selection = Some(OutputSelection {
        anchor: SelectionPoint { row: 3, col: 4 },
        cursor: SelectionPoint { row: 3, col: 4 },
    });

    let outcome = handle_mouse_event(
        &mut state,
        mouse_event(MouseEventKind::Drag(MouseButton::Left), 15, 8),
    );

    assert!(matches!(outcome, EventOutcome::Redraw));
    assert_eq!(
        state.output.selection,
        Some(OutputSelection {
            anchor: SelectionPoint { row: 3, col: 4 },
            cursor: SelectionPoint { row: 8, col: 15 },
        })
    );
}

/// Verifies that ClearSelection removes the active selection when clicked outside the output area.
#[test]
fn handle_mouse_event_clear_selection_clears_active_selection() {
    let mut state = conversation_state();
    set_output_area(&mut state);
    state.output.selection = Some(OutputSelection {
        anchor: SelectionPoint { row: 2, col: 2 },
        cursor: SelectionPoint { row: 6, col: 12 },
    });

    let outcome = handle_mouse_event(
        &mut state,
        mouse_event(MouseEventKind::Down(MouseButton::Left), 120, 40),
    );

    assert!(matches!(outcome, EventOutcome::Redraw));
    assert_eq!(state.output.selection, None);
}

/// Verifies that `handle_mouse_event` routes plan-mode scrolls through `handle_plan_mouse_scroll` and requests a redraw.
#[test]
fn handle_mouse_event_routes_plan_mode_scrolls_to_plan_panel() {
    let mut state = plan_state();

    let outcome = handle_mouse_event(&mut state, mouse_event(MouseEventKind::ScrollUp, 65, 5));

    assert!(matches!(outcome, EventOutcome::Redraw));
    let ConversationMode::Plan(plan) = &state.interaction.mode else {
        panic!("expected plan mode");
    };
    assert!(
        plan.tree_scroll > ScrollOffset::of(0),
        "plan-panel scrolls must be delegated to handle_plan_mouse_scroll"
    );
    assert_eq!(
        state.output.scroll_offset.get(),
        ScrollOffset::of(0),
        "plan-panel scrolling must not mutate the chat scroll offset"
    );
}

/// Verifies that main panel mouse scrolling works correctly after a secondary panel is closed.
/// This is a regression test for the bug where stale secondary_panel_area coordinates
/// would intercept mouse events that should scroll the main panel.
#[test]
fn handle_mouse_event_main_panel_scroll_after_closing_secondary_panel() {
    let mut state = conversation_state();
    set_output_area(&mut state);

    // Add some output lines so we can scroll
    for i in 0..100 {
        state
            .output
            .lines
            .push(crate::domain::tui_state::OutputLine::plain(
                OutputText::new(format!("Line {}", i)),
            ));
    }

    // Simulate secondary panel being open and occupying right side
    let secondary_area = Rect {
        x: 40,
        y: 0,
        width: 40,
        height: 24,
    };
    set_secondary_panel_area(&mut state, secondary_area);

    // Now simulate what render_secondary_container does when secondary_view is None:
    // it should clear the secondary_panel_area to Rect::default()
    // This is what the fix does - it prevents stale coordinates from intercepting events
    set_secondary_panel_area(&mut state, Rect::default());

    // Initial scroll offset
    let initial_scroll = state.output.scroll_offset.get();
    // Using ScrollUp since that's what increases scroll_offset
    let outcome = handle_mouse_event(&mut state, mouse_event(MouseEventKind::ScrollUp, 20, 12));

    // Should successfully scroll the main panel
    assert!(matches!(outcome, EventOutcome::Redraw));
    assert!(
        state.output.scroll_offset.get() > initial_scroll,
        "main panel should scroll up"
    );
}

/// Verifies that when secondary_panel_area has non-zero dimensions, it intercepts mouse events
/// and prevents main panel scrolling (expected behavior when secondary panel is open).
#[test]
fn handle_mouse_event_secondary_panel_intercepts_scrolls_when_active() {
    let mut state = conversation_state();
    set_output_area(&mut state);

    // Add some output lines so we can scroll
    for i in 0..100 {
        state
            .output
            .lines
            .push(crate::domain::tui_state::OutputLine::plain(
                OutputText::new(format!("Line {}", i)),
            ));
    }

    // Simulate secondary panel being open and occupying right side with non-zero area
    let secondary_area = Rect {
        x: 40,
        y: 0,
        width: 40,
        height: 24,
    };
    set_secondary_panel_area(&mut state, secondary_area);

    // Initial scroll offset
    let initial_scroll = state.output.scroll_offset.get();
    let outcome = handle_mouse_event(&mut state, mouse_event(MouseEventKind::ScrollUp, 60, 12));

    // Should handle scroll as agent feed scroll, not main panel scroll
    assert!(matches!(outcome, EventOutcome::Redraw));
    // The scroll should NOT have changed the main panel's scroll offset
    // (it would have scrolled the agent feed instead)
    assert_eq!(
        state.output.scroll_offset.get(),
        initial_scroll,
        "main panel scroll offset should not change when secondary panel area is active"
    );
}

/// Verifies that `handle_terminal_event` returns `Quit` when the event stream ends or yields an I/O error.
#[tokio::test]
async fn handle_terminal_event_none_or_error_returns_quit() {
    let rig = TestRig::new().await;
    let mut none_state = conversation_state();
    let mut error_state = conversation_state();

    let none_outcome = handle_terminal_event(&mut none_state, None, &rig.handles()).await;
    let error_outcome = handle_terminal_event(
        &mut error_state,
        Some(Err(std::io::Error::other("read failed"))),
        &rig.handles(),
    )
    .await;

    assert!(matches!(none_outcome, EventOutcome::Quit));
    assert!(matches!(error_outcome, EventOutcome::Quit));
}

/// Verifies that `handle_terminal_event` normalizes pasted text into the prompt buffer and requests a redraw.
#[tokio::test]
async fn handle_terminal_event_paste_returns_redraw() {
    let rig = TestRig::new().await;
    let mut state = conversation_state();
    state.prompt.buffer = "prefix".to_owned();
    state.prompt.cursor = state.prompt.buffer.len();

    let outcome = handle_terminal_event(
        &mut state,
        Some(Ok(Event::Paste("line1\nline2".to_owned()))),
        &rig.handles(),
    )
    .await;

    assert!(matches!(outcome, EventOutcome::Redraw));
    assert_eq!(state.prompt.buffer, "prefixline1 line2");
    assert_eq!(state.prompt.cursor, "prefixline1 line2".len());
}

/// Verifies that `handle_terminal_event` returns `Redraw` for terminal resize events.
#[tokio::test]
async fn handle_terminal_event_resize_returns_redraw() {
    let rig = TestRig::new().await;
    let mut state = conversation_state();

    let outcome =
        handle_terminal_event(&mut state, Some(Ok(Event::Resize(120, 40))), &rig.handles()).await;

    assert!(matches!(outcome, EventOutcome::Redraw));
}

/// Verifies that `handle_terminal_event` returns `Redraw` when a key event continues in query mode.
#[tokio::test]
async fn handle_terminal_event_key_continue_returns_redraw() {
    let rig = TestRig::new().await;
    let mut state = query_state();

    let outcome = handle_terminal_event(
        &mut state,
        Some(Ok(Event::Key(key_event(KeyCode::Down, KeyModifiers::NONE)))),
        &rig.handles(),
    )
    .await;

    assert!(matches!(outcome, EventOutcome::Redraw));
    let ConversationMode::Query(query) = &state.interaction.mode else {
        panic!("expected query mode");
    };
    assert_eq!(
        query.selected,
        Some(0),
        "continuing query-mode keys must still be applied before redraw"
    );
}

/// Verifies that `handle_terminal_event` returns `Quit` when key dispatch breaks in query mode.
#[tokio::test]
async fn handle_terminal_event_key_break_returns_quit() {
    let rig = TestRig::new().await;
    let mut state = query_state();

    let outcome = handle_terminal_event(
        &mut state,
        Some(Ok(Event::Key(key_event(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )))),
        &rig.handles(),
    )
    .await;

    assert!(matches!(outcome, EventOutcome::Quit));
}

/// Verifies that `handle_terminal_event` returns `NoOp` for unrelated terminal events.
#[tokio::test]
async fn handle_terminal_event_unrelated_event_returns_noop() {
    let rig = TestRig::new().await;
    let mut state = conversation_state();

    let outcome =
        handle_terminal_event(&mut state, Some(Ok(Event::FocusGained)), &rig.handles()).await;

    assert!(matches!(outcome, EventOutcome::NoOp));
}
