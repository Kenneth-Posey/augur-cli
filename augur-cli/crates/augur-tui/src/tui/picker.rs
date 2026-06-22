//! Session picker rendering: displays a list of saved sessions at startup.

use crate::domain::tui_state::PickerState;
use augur_domain::domain::newtypes::NumericNewtype;
use augur_domain::domain::string_newtypes::StringNewtype;
use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

/// Milliseconds per second used to convert epoch deltas to elapsed seconds.
const MILLIS_PER_SECOND: u64 = 1_000;
/// Seconds per minute for elapsed-time formatting.
const SECS_PER_MINUTE: u64 = 60;
/// Seconds per hour for elapsed-time formatting.
const SECS_PER_HOUR: u64 = 60 * SECS_PER_MINUTE;
/// Seconds per day for elapsed-time formatting.
const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;
/// Largest elapsed value still shown in seconds.
const MAX_SECONDS_LABEL: u64 = SECS_PER_MINUTE - 1;
/// Largest elapsed value still shown in minutes.
const MAX_MINUTES_LABEL: u64 = SECS_PER_HOUR - 1;
/// Largest elapsed value still shown in hours.
const MAX_HOURS_LABEL: u64 = SECS_PER_DAY - 1;

/// Render the session picker screen into the full terminal frame.
///
/// When sessions are available, renders a navigable list with endpoint name,
/// timestamp, message count, and preview text. When the list is empty, shows
/// a centered prompt to start a new session. Called by `render` when the TUI
/// is in `AppScreen::SessionSelector`.
pub fn render_picker(f: &mut Frame, state: &PickerState, area: Rect) {
    if state.sessions.is_empty() {
        render_empty_picker(f, area);
    } else {
        render_session_list(f, state, area);
    }
}

/// Render the empty-state picker: a centered message prompting the user to start fresh.
fn render_empty_picker(f: &mut Frame, area: Rect) {
    let msg = "No sessions found. Press N or Enter to start a new session.";
    let para = Paragraph::new(msg).alignment(Alignment::Center);
    f.render_widget(para, area);
}

/// Render the session list with the currently selected item highlighted.
fn render_session_list(f: &mut Frame, state: &PickerState, area: Rect) {
    let items: Vec<ListItem> = state.sessions.iter().map(session_list_item).collect();
    let title = "Restore a session (\u{2191}\u{2193} navigate, Enter restore, D delete, N new)";
    let block = Block::default().title(title).borders(Borders::ALL);
    let list = List::new(items).block(block).highlight_symbol("> ");
    let mut list_state = ListState::default().with_selected(Some(state.selected.inner()));
    f.render_stateful_widget(list, area, &mut list_state);
}

/// Build a display `ListItem` for one session summary.
fn session_list_item(s: &crate::domain::tui_state::PickerSessionSummary) -> ListItem<'static> {
    let line = format!(
        "[{}] {} | {} msgs | {}",
        s.identity.endpoint_name.as_str(),
        format_elapsed(s.identity.created_at.inner()),
        s.message_count,
        s.preview.as_str(),
    );
    ListItem::new(line)
}

/// Format a millisecond timestamp as a human-readable elapsed time string.
///
/// Returns strings like "5s ago", "3m ago", "2h ago", "4d ago".
/// Used in the session picker list to show when each session was last active.
fn format_elapsed(created_ms: u64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(created_ms);
    let diff_secs = now_ms.saturating_sub(created_ms) / MILLIS_PER_SECOND;
    elapsed_label(diff_secs)
}

/// Convert a seconds-since-creation value into a short elapsed label.
fn elapsed_label(secs: u64) -> String {
    match secs {
        0..=MAX_SECONDS_LABEL => format!("{secs}s ago"),
        SECS_PER_MINUTE..=MAX_MINUTES_LABEL => format!("{}m ago", secs / SECS_PER_MINUTE),
        SECS_PER_HOUR..=MAX_HOURS_LABEL => format!("{}h ago", secs / SECS_PER_HOUR),
        _ => format!("{}d ago", secs / SECS_PER_DAY),
    }
}
