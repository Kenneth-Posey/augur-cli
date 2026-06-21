use super::*;
use crate::domain::newtypes::{Count, ScrollOffset};
use crate::domain::string_newtypes::{EndpointName, StringNewtype};

const EXCESSIVE_SCROLL_OFFSET: Count = Count::of(10);
const LARGE_SCROLL_AMOUNT: Count = Count::of(100);

/// Verifies that clamp_output_scroll_offset prevents scrolling past the top
/// by clamping an excessive offset to the calculated maximum safe value.
/// `last_render_width` is set to 80 (simulating a rendered terminal) so the
/// real display-row path is exercised instead of the pre-render skip path.
#[test]
fn clamp_output_scroll_offset_prevents_scrolling_past_top() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    state.output.lines = vec![
        OutputLine::plain("message 1"),
        OutputLine::plain("message 2"),
        OutputLine::plain("message 3"),
    ];

    // Simulate a rendered terminal at width 80 so clamping is active.
    state.output.last_render_width.set(80);
    state
        .output
        .scroll_offset
        .set(ScrollOffset::of(EXCESSIVE_SCROLL_OFFSET.inner()));
    state.clamp_output_scroll_offset();

    assert_eq!(
        state.output.scroll_offset.get(),
        ScrollOffset::of(2),
        "scroll_offset should be clamped to max of {}, got {}",
        2,
        state.output.scroll_offset.get()
    );
}

/// Verifies that clamp_output_scroll_offset allows valid offsets within bounds.
/// `last_render_width` is set to 80 so the real display-row path is exercised.
#[test]
fn clamp_output_scroll_offset_allows_valid_offsets() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    state.output.lines = vec![
        OutputLine::plain("message 1"),
        OutputLine::plain("message 2"),
        OutputLine::plain("message 3"),
    ];

    // Simulate a rendered terminal at width 80 so clamping is active.
    state.output.last_render_width.set(80);
    state.output.scroll_offset.set(ScrollOffset::of(1));
    state.clamp_output_scroll_offset();

    assert_eq!(
        state.output.scroll_offset.get(),
        ScrollOffset::of(1),
        "valid scroll_offset should not be clamped"
    );
}

/// Verifies that scroll_up applies bounds checking and clamps to the max offset.
/// `last_render_width` is set to 80 so the real display-row path is exercised.
#[test]
fn scroll_up_applies_bounds_checking() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    state.output.lines = vec![
        OutputLine::plain("message 1"),
        OutputLine::plain("message 2"),
        OutputLine::plain("message 3"),
    ];

    // Simulate a rendered terminal at width 80 so clamping is active.
    state.output.last_render_width.set(80);
    state.output.scroll_offset.set(ScrollOffset::of(0));
    state.scroll_up(LARGE_SCROLL_AMOUNT);

    assert_eq!(
        state.output.scroll_offset.get(),
        ScrollOffset::of(2),
        "scroll_up should clamp to max_offset of 2"
    );
}

/// Verifies that scroll_down clamps to zero and prevents negative offsets.
#[test]
fn scroll_down_clamps_to_zero() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    state.output.lines = vec![
        OutputLine::plain("message 1"),
        OutputLine::plain("message 2"),
    ];

    state.output.scroll_offset.set(ScrollOffset::of(3));
    state.scroll_down(LARGE_SCROLL_AMOUNT);

    assert_eq!(
        state.output.scroll_offset.get(),
        ScrollOffset::of(0),
        "scroll_down should clamp to 0"
    );
}

/// Verifies that clamp_agent_feed_scroll_offset prevents scrolling past the top
/// by clamping an excessive offset to the calculated maximum safe value.
#[test]
fn clamp_agent_feed_scroll_offset_prevents_scrolling_past_top() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    state.interaction.panel.agent_feed.output =
        vec![OutputLine::plain("line 1"), OutputLine::plain("line 2")];

    state.interaction.panel.agent_feed.scroll = ScrollOffset::of(EXCESSIVE_SCROLL_OFFSET.inner());
    state.clamp_agent_feed_scroll_offset();

    assert_eq!(
        state.interaction.panel.agent_feed.scroll,
        ScrollOffset::of(1),
        "agent_feed scroll should be clamped to max of 1"
    );
}

/// Verifies that agent_feed_scroll_up applies bounds checking and clamps appropriately.
#[test]
fn agent_feed_scroll_up_applies_bounds_checking() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    state.interaction.panel.agent_feed.output = vec![
        OutputLine::plain("line 1"),
        OutputLine::plain("line 2"),
        OutputLine::plain("line 3"),
    ];

    state.interaction.panel.agent_feed.scroll = ScrollOffset::of(0);
    state.agent_feed_scroll_up(LARGE_SCROLL_AMOUNT);

    assert_eq!(
        state.interaction.panel.agent_feed.scroll,
        ScrollOffset::of(2),
        "agent_feed_scroll_up should clamp to max_offset of 2"
    );
}

/// Verifies that agent_feed_scroll_down clamps to zero and prevents negative offsets.
#[test]
fn agent_feed_scroll_down_clamps_to_zero() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    state.interaction.panel.agent_feed.output =
        vec![OutputLine::plain("line 1"), OutputLine::plain("line 2")];

    state.interaction.panel.agent_feed.scroll = ScrollOffset::of(5);
    state.agent_feed_scroll_down(LARGE_SCROLL_AMOUNT);

    assert_eq!(
        state.interaction.panel.agent_feed.scroll,
        ScrollOffset::of(0),
        "agent_feed_scroll_down should clamp to 0"
    );
}

/// Verifies feed selection clamps a selected transcript scroll offset to its own
/// output length so scrollbar math matches the newly selected feed.
#[test]
fn select_next_agent_feed_clamps_selected_feed_scroll_to_feed_length() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.agent_feed.feeds = vec![
        crate::domain::tui_state::AgentFeedTranscript {
            feed_id: crate::domain::types::FeedId::Agent(
                crate::domain::string_newtypes::ToolCallId::from("agent-1"),
            ),
            panel: crate::domain::tui_state::AgentFeedPanel {
                output: vec![
                    OutputLine::plain("a"),
                    OutputLine::plain("b"),
                    OutputLine::plain("c"),
                ],
                scroll: ScrollOffset::of(1),
                buffers: Default::default(),
            },
            ..Default::default()
        },
        crate::domain::tui_state::AgentFeedTranscript {
            feed_id: crate::domain::types::FeedId::Agent(
                crate::domain::string_newtypes::ToolCallId::from("agent-2"),
            ),
            panel: crate::domain::tui_state::AgentFeedPanel {
                output: vec![OutputLine::plain("x"), OutputLine::plain("y")],
                scroll: ScrollOffset::of(50),
                buffers: Default::default(),
            },
            ..Default::default()
        },
    ];
    state.interaction.panel.agent_feed.selected_feed = Some(0);
    state.sync_selected_agent_feed();

    let changed = state.select_next_agent_feed();
    assert!(bool::from(changed), "next feed selection should succeed");
    assert_eq!(state.interaction.panel.agent_feed.selected_feed, Some(1));
    assert_eq!(
        state.interaction.panel.agent_feed.scroll,
        ScrollOffset::of(1),
        "selected feed scroll mirror must clamp to selected feed max offset"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.feeds[1].scroll,
        ScrollOffset::of(1),
        "selected transcript scroll should be clamped in-place"
    );
}

/// Verifies that `clamp_output_scroll_offset` skips clamping when `last_render_width`
/// is 0 (before the first render).  At width 0 we have no reliable display-row count,
/// so the clamp would mis-use logical line count as display rows and incorrectly cut
/// the user's offset for wrapped content.  The first real render will correct the
/// offset via `recalculate_scroll_for_width_change`.
#[test]
fn clamp_output_scroll_offset_skips_clamp_when_render_width_is_zero() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    // Two logical lines, but they could each wrap to many display rows.
    state.output.lines = vec![OutputLine::plain("line 1"), OutputLine::plain("line 2")];

    // last_render_width stays at its default (0 - not yet rendered).
    assert_eq!(
        state.output.last_render_width.get(),
        0,
        "pre-condition: width must be 0"
    );

    // Set an offset that would be incorrectly clamped to `lines.len()-1 = 1`
    // if the fallback branch used `lines.len().saturating_sub(1)`.
    state.output.scroll_offset.set(ScrollOffset::of(5));
    state.clamp_output_scroll_offset();

    assert_eq!(
        state.output.scroll_offset.get(),
        ScrollOffset::of(5),
        "clamp must be skipped when render width is 0; offset should remain 5, \
         got {}",
        state.output.scroll_offset.get()
    );
}
