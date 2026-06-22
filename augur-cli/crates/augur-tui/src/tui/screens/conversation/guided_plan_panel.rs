//! Guided-plan panel rendering helpers for the conversation screen.

use crate::domain::tui_state::GuidedPlanUiState;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::{Frame, layout::Rect};

/// Render the right-hand guided plan phase panel.
pub(super) fn render_guided_plan_panel(frame: &mut Frame, state: &GuidedPlanUiState, area: Rect) {
    let lines = guided_plan_panel_lines(state, area);
    let paragraph = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .block(Block::default());
    frame.render_widget(paragraph, area);
}

fn phase_status_icon(status: &augur_domain::domain::guided_plan::PhaseStatus) -> &'static str {
    phase_status_icon_in_progress(status).unwrap_or_else(|| phase_status_icon_terminal(status))
}

fn phase_status_icon_in_progress(
    status: &augur_domain::domain::guided_plan::PhaseStatus,
) -> Option<&'static str> {
    use augur_domain::domain::guided_plan::PhaseStatus;
    match status {
        PhaseStatus::Pending => Some("[ ]"),
        PhaseStatus::InProgress => Some("[~]"),
        PhaseStatus::AwaitingHooks => Some("[?]"),
        _ => None,
    }
}

fn phase_status_icon_terminal(
    status: &augur_domain::domain::guided_plan::PhaseStatus,
) -> &'static str {
    use augur_domain::domain::guided_plan::PhaseStatus;
    match status {
        PhaseStatus::NeedsRework(_) => "[!]",
        PhaseStatus::Complete => "[✓]",
        PhaseStatus::Failed(_) => "[✗]",
        PhaseStatus::Pending | PhaseStatus::InProgress | PhaseStatus::AwaitingHooks => {
            unreachable!("covered by phase_status_icon_in_progress")
        }
    }
}

fn guided_plan_panel_lines(state: &GuidedPlanUiState, area: Rect) -> Vec<Line<'static>> {
    let mut lines = header_lines(state, area.width as usize);
    for (idx, phase) in state.phases.iter().enumerate() {
        lines.extend(phase_lines(idx, phase, state.current_phase));
    }
    append_review_status(&mut lines, area.height as usize, state.review_active.into());
    lines
}

fn header_lines(state: &GuidedPlanUiState, width: usize) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![Span::styled(
            format!(" {} ", state.plan_name),
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("─".repeat(width)),
    ]
}

fn phase_lines(
    idx: usize,
    (phase_name, status): &(
        augur_domain::domain::string_newtypes::PhaseName,
        augur_domain::domain::guided_plan::PhaseStatus,
    ),
    current_phase: usize,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        format!(" {} {}", phase_status_icon(status), phase_name),
        phase_title_style(idx == current_phase),
    ))];
    if let Some(reason_line) = phase_reason_line(status) {
        lines.push(reason_line);
    }
    lines
}

fn phase_title_style(is_current: bool) -> Style {
    if is_current {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

fn phase_reason_line(
    status: &augur_domain::domain::guided_plan::PhaseStatus,
) -> Option<Line<'static>> {
    use augur_domain::domain::guided_plan::PhaseStatus;
    match status {
        PhaseStatus::NeedsRework(reason) => Some(status_reason_line(reason, Color::Yellow)),
        PhaseStatus::Failed(reason) => Some(status_reason_line(reason, Color::Red)),
        _ => None,
    }
}

fn status_reason_line(reason: &str, color: Color) -> Line<'static> {
    Line::from(Span::styled(
        format!("     ↳ {reason}"),
        Style::default().fg(color).add_modifier(Modifier::DIM),
    ))
}

fn append_review_status(lines: &mut Vec<Line<'static>>, area_height: usize, review_active: bool) {
    if !review_active {
        return;
    }
    while lines.len() < area_height.saturating_sub(1) {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        " Reviewer active… ",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
    )));
}
