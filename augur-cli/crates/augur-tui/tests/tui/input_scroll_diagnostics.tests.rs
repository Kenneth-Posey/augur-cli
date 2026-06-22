//! Diagnostic tests to understand why main panel scrolling isn't working in real UI.
//!
//! These tests reveal the gap between unit tests (which explicitly initialize output_area)
//! and the real UI (where output_area remains at Rect::default() until first render).

use augur_tui::domain::newtypes::{Count, NumericNewtype};
use augur_tui::domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
use augur_tui::domain::tui_input::{classify_mouse, MouseAction, MOUSE_SCROLL_LINES};
use augur_tui::domain::tui_state::{AppScreen, AppState, OutputLine};
use crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

fn key(
    code: crossterm::event::KeyCode,
    mods: crossterm::event::KeyModifiers,
) -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent {
        code,
        modifiers: mods,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    }
}

#[allow(dead_code)]
fn key_unused(
    code: crossterm::event::KeyCode,
    mods: crossterm::event::KeyModifiers,
) -> crossterm::event::KeyEvent {
    key(code, mods)
}

fn default_state() -> AppState {
    AppState::new(EndpointName::new("ep"), AppScreen::Conversation)
}

fn mouse_event(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::NONE,
    }
}

// ── Diagnostic Test 1: Scroll event with uninitialized output_area ───────────────────

/// **DIAGNOSTIC TEST**: Reveals the core bug - scrolling with uninitialized output_area.
///
/// This test demonstrates that when `output_area` is at its default (zero dimensions),
/// a scroll event at screen coordinates (40, 12) is classified as `Ignored` instead of
/// `ScrollUp`. This explains why scrolling doesn't work in the real UI before the first
/// render.
///
/// **Expected behavior**: The scroll event should be classified as a scroll action.
/// **Actual behavior**: With zero-sized output_area, the event is ignored.
///
/// Run this test with output to see the dimensions of the zero-initialized Rect:
/// ```
/// cargo test --lib diagnostic -- --nocapture
/// ```
#[test]
fn diagnostic_main_panel_scroll_without_output_area_initialized() {
    let state = default_state();

    // Verify that output_area is at its default (zero dimensions)
    let uninitialized_output_area = state.output.panel_areas.output_area.get();
    eprintln!("\n=== DIAGNOSTIC: Uninitialized output_area ===");
    eprintln!(
        "  x={}, y={}, width={}, height={}",
        uninitialized_output_area.x,
        uninitialized_output_area.y,
        uninitialized_output_area.width,
        uninitialized_output_area.height
    );

    // Verify it matches Rect::default()
    assert_eq!(
        uninitialized_output_area,
        Rect::default(),
        "output_area should start at Rect::default() (zero dimensions)"
    );

    // Simulate a scroll event at typical main panel coordinates (40, 12)
    let event = mouse_event(MouseEventKind::ScrollUp, 40, 12);

    // Classify the event against the zero-sized output_area
    let action = classify_mouse(event, uninitialized_output_area);

    eprintln!(
        "  Mouse scroll at (col=40, row=12) classified as: {:?}",
        action
    );
    eprintln!("  → This is the BUG: scroll should work, but it's Ignored because");
    eprintln!("    the output_area has zero height and zero width.");

    // This assertion will PASS, confirming the bug exists
    assert!(
        matches!(action, MouseAction::Ignored),
        "With zero-sized output_area, scroll events are Ignored (this is the bug!)"
    );
}

// ── Diagnostic Test 2: First-frame behavior (render hasn't run yet) ─────────────────

/// **DIAGNOSTIC TEST**: Simulates the first frame before render is called.
///
/// In the real UI, events arrive very quickly after the TUI starts. The main render loop
/// may not have executed yet, meaning `output_area` is still at zero dimensions.
/// This test verifies that scroll events on the first frame are indeed ignored.
#[test]
fn diagnostic_main_panel_scroll_first_frame_behavior() {
    let mut state = default_state();

    // Add some content to the output
    state
        .output
        .lines
        .push(OutputLine::plain(OutputText::new("Hello")));
    state
        .output
        .lines
        .push(OutputLine::plain(OutputText::new("World")));
    state
        .output
        .lines
        .push(OutputLine::plain(OutputText::new("Test")));

    eprintln!("\n=== DIAGNOSTIC: First-frame scroll behavior ===");
    eprintln!("  Output has {} lines", state.output.lines.len());

    // Before first render: output_area is uninitialized
    let pre_render_area = state.output.panel_areas.output_area.get();
    eprintln!(
        "  Pre-render output_area: Rect{{x={}, y={}, width={}, height={}}}",
        pre_render_area.x, pre_render_area.y, pre_render_area.width, pre_render_area.height
    );

    // Check scroll state before any events
    let scroll_before = state.output.scroll_offset.get();
    eprintln!("  Scroll offset before: {}", scroll_before);

    // Try to scroll (this will be ignored because output_area is zero-sized)
    let event = mouse_event(MouseEventKind::ScrollUp, 40, 12);
    let action = classify_mouse(event, pre_render_area);

    eprintln!("  Scroll event classified as: {:?}", action);
    eprintln!("  → On first frame, scroll events arrive BEFORE render updates output_area");
    eprintln!("    so they are Ignored even though the user intended to scroll.");

    // The scroll action won't execute because it's Ignored
    match action {
        MouseAction::ScrollUp(n) => {
            state.scroll_up(Count::new(n));
        }
        _ => {
            eprintln!("  Scroll action was not executed (event ignored)");
        }
    }

    let scroll_after = state.output.scroll_offset.get();
    eprintln!("  Scroll offset after: {}", scroll_after);
    assert_eq!(
        scroll_before, scroll_after,
        "Scroll state should not change when event is ignored"
    );
}

// ── Diagnostic Test 3: Scroll state mutation (verify state changes work) ────────────

/// **DIAGNOSTIC TEST**: Verifies that scroll state DOES change when we manually call scroll methods.
///
/// This test confirms that once a scroll action is recognized, the state mutation works.
/// The issue is not with the scroll logic itself, but with event classification when
/// `output_area` is zero-sized.
#[test]
fn diagnostic_scroll_state_mutation() {
    let mut state = default_state();

    // Add enough content for scrolling to matter
    for i in 0..30 {
        state
            .output
            .lines
            .push(OutputLine::plain(OutputText::new(format!("Line {}", i))));
    }

    eprintln!("\n=== DIAGNOSTIC: Scroll state mutation ===");

    let initial_offset = state.output.scroll_offset.get();
    eprintln!("  Initial scroll_offset: {}", initial_offset);

    // Manually call scroll_up (simulating what would happen if classify_mouse returned ScrollUp)
    state.scroll_up(Count::new(MOUSE_SCROLL_LINES));

    let after_scroll_up = state.output.scroll_offset.get();
    eprintln!(
        "  After scroll_up({}): {}",
        MOUSE_SCROLL_LINES, after_scroll_up
    );

    assert!(
        after_scroll_up > initial_offset,
        "scroll_up should increase scroll_offset"
    );

    // Now scroll back down
    state.scroll_down(Count::new(MOUSE_SCROLL_LINES));

    let after_scroll_down = state.output.scroll_offset.get();
    eprintln!(
        "  After scroll_down({}): {}",
        MOUSE_SCROLL_LINES, after_scroll_down
    );

    assert_eq!(
        after_scroll_down, initial_offset,
        "scroll_down should return to original offset"
    );

    eprintln!("  → State mutation works correctly. The bug is in event classification,");
    eprintln!("    not in the scroll logic itself.");
}

// ── Diagnostic Test 4: Scroll works when output_area is properly initialized ───────

/// **COMPARISON TEST**: Shows that scrolling DOES work when output_area is initialized.
///
/// This is what the existing unit tests do: they explicitly set a valid output_area.
/// This test verifies that the scroll classification works correctly with proper setup.
#[test]
fn diagnostic_main_panel_scroll_with_initialized_output_area() {
    let state = default_state();

    // Initialize output_area to a typical terminal size (80x24)
    let valid_output_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };
    state.output.panel_areas.output_area.set(valid_output_area);

    eprintln!("\n=== DIAGNOSTIC: Scroll WITH initialized output_area ===");
    eprintln!(
        "  output_area: Rect{{x={}, y={}, width={}, height={}}}",
        valid_output_area.x, valid_output_area.y, valid_output_area.width, valid_output_area.height
    );

    // Now the same scroll event at (40, 12) should work
    let event = mouse_event(MouseEventKind::ScrollUp, 40, 12);
    let action = classify_mouse(event, valid_output_area);

    eprintln!(
        "  Mouse scroll at (col=40, row=12) classified as: {:?}",
        action
    );

    assert!(
        matches!(action, MouseAction::ScrollUp(n) if n == MOUSE_SCROLL_LINES),
        "With initialized output_area, scroll events are correctly classified"
    );

    eprintln!("  → Scrolling WORKS when output_area is initialized.");
    eprintln!("    This is why unit tests pass but the real UI doesn't scroll.");
}

// ── Diagnostic Test 5: Event timing race condition ────────────────────────────────

/// **DIAGNOSTIC TEST**: Examines the race condition between event handling and rendering.
///
/// In the real UI, there's a potential race:
/// 1. User moves mouse over main panel and scrolls
/// 2. Event arrives at handle_mouse_event()
/// 3. classify_mouse() is called with state.output.panel_areas.output_area.get()
/// 4. If render hasn't updated output_area yet, it's still Rect::default()
/// 5. Event is ignored
///
/// This test documents this timing issue.
#[test]
fn diagnostic_event_timing_race_condition() {
    let mut state = default_state();

    // Add content
    for i in 0..10 {
        state
            .output
            .lines
            .push(OutputLine::plain(OutputText::new(format!(
                "Content line {}",
                i
            ))));
    }

    eprintln!("\n=== DIAGNOSTIC: Event timing race condition ===");

    // Scenario: Events arrive before first render
    let uninitialized_area = state.output.panel_areas.output_area.get();
    eprintln!("  T=0: UI starts, output_area = Rect::default()");
    eprintln!(
        "       (width={}, height={})",
        uninitialized_area.width, uninitialized_area.height
    );

    // User scrolls immediately
    let scroll_event = mouse_event(MouseEventKind::ScrollUp, 40, 12);
    let action = classify_mouse(scroll_event, uninitialized_area);
    eprintln!("  T=1: User scrolls → classified as {:?}", action);

    if matches!(action, MouseAction::Ignored) {
        eprintln!("       → Event is IGNORED (bug manifests here)");
        eprintln!("         User's scroll is lost because output_area hasn't been set yet.");
    }

    // Later, render runs and sets output_area
    let valid_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };
    state.output.panel_areas.output_area.set(valid_area);
    eprintln!("  T=2: First render runs, output_area updated to (width=80, height=24)");

    // Now subsequent scrolls work
    let scroll_event2 = mouse_event(MouseEventKind::ScrollUp, 40, 12);
    let action2 = classify_mouse(scroll_event2, valid_area);
    eprintln!("  T=3: User scrolls again → classified as {:?}", action2);

    if matches!(action2, MouseAction::ScrollUp(_)) {
        eprintln!("       → Event is ACCEPTED (scrolling now works)");
    }

    eprintln!("\n  Summary of the bug:");
    eprintln!("  - Early scroll events (before first render) are ignored");
    eprintln!("  - Later scroll events (after first render) work correctly");
    eprintln!("  - This creates the perception that scrolling is 'broken'");
}

// ── Diagnostic Test 6: Secondary panel interaction ──────────────────────────────────

/// **DIAGNOSTIC TEST**: Check if the issue also affects secondary panels.
///
/// The bug could also exist in secondary panel scrolling if their output_area
/// fields are also uninitialized.
#[test]
fn diagnostic_secondary_panel_scroll_uninitialized() {
    let state = default_state();

    eprintln!("\n=== DIAGNOSTIC: Secondary panel output_area ===");

    // Check agent feed panel area
    let agent_feed_area = state.output.panel_areas.secondary_panel_area.get();
    eprintln!(
        "  Agent feed output_area: Rect{{x={}, y={}, width={}, height={}}}",
        agent_feed_area.x, agent_feed_area.y, agent_feed_area.width, agent_feed_area.height
    );

    assert_eq!(
        agent_feed_area,
        Rect::default(),
        "Secondary panel output_area also starts uninitialized"
    );

    eprintln!("  → Secondary panels have the same issue as main panel");
    eprintln!("    All scroll events in uninitialized panels are Ignored");
}

// ── Documentation: How to fix this bug ──────────────────────────────────────────────
//
// ROOT CAUSE:
// `handle_mouse_event()` in `src/actors/tui/actor/runtime/terminal.rs:66` calls:
//   `classify_mouse(event, state.output.panel_areas.output_area.get())`
//
// But `output_area` is only set during rendering (in `render_output()`), and isn't
// set until the first frame. Mouse events can arrive before the first render completes,
// causing them to be classified against a zero-sized Rect, which always returns `Ignored`.
//
// POTENTIAL FIXES:
//
// 1. Initialize output_area with a sensible default (terminal size)
//    - Call `terminal.size()` and initialize output_area in AppState::new()
//    - Would require changing the constructor signature
//
// 2. Set output_area as soon as the terminal is created (before event loop)
//    - In the TUI actor setup, after creating the Terminal, set output_area to the
//      actual terminal dimensions
//    - This ensures output_area is valid before any events arrive
//
// 3. Defer scrolling until after first render
//    - Track whether render has been called
//    - Return EventOutcome::NoOp for scroll events until output_area is initialized
//    - User experience: scrolling "turns on" after first frame
//
// 4. Use terminal dimensions as fallback
//    - In classify_mouse or handle_mouse_event, if output_area is zero-sized,
//      use the known terminal dimensions as a fallback
//    - Requires having access to terminal size in the event handler
//
// Fix #2 seems best: initialize output_area with terminal dimensions as soon as
// the terminal is created, before the event loop begins.
