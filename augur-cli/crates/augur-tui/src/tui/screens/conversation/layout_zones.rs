//! Layout zone helpers for the conversation screen.

use augur_domain::domain::newtypes::{Count, NumericNewtype};
use ratatui::layout::{Constraint, Layout, Rect};

/// Areas for the bottom section of the chat layout.
#[derive(bon::Builder)]
pub(super) struct BottomZones {
    pub(super) hints: Rect,
    pub(super) input: Rect,
    pub(super) sep_below_input: Rect,
    pub(super) status: Rect,
}

/// Areas for each rendered zone in chat mode.
pub(super) struct ChatZones {
    /// Separator above the input area. Used to derive the conversation area.
    pub(super) top_sep_above_input: Rect,
    pub(super) bottom: BottomZones,
}

/// Compute the conversation container area from the full base area and separator rect.
pub(super) fn conv_area_above(base: Rect, sep_above_input: Rect) -> Rect {
    let conv_height = sep_above_input.y.saturating_sub(base.y);
    Rect {
        height: conv_height,
        ..base
    }
}

/// Split `area` into the nine chat zones used by the conversation layout.
pub(super) fn split_layout(area: Rect, input_rows: Count, hint_rows: Count) -> ChatZones {
    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(hint_rows.inner() as u16),
        Constraint::Length(input_rows.inner() as u16),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    ChatZones {
        top_sep_above_input: chunks[3],
        bottom: BottomZones::builder()
            .hints(chunks[4])
            .input(chunks[5])
            .sep_below_input(chunks[6])
            .status(chunks[7])
            .build(),
    }
}
