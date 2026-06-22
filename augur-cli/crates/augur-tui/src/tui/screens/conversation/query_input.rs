//! Query-input rendering helpers for the conversation screen.

use crate::domain::tui_display_state::QueryDisplayState;
use augur_domain::domain::newtypes::{ChoiceIndex, NumericNewtype};
use augur_domain::domain::string_newtypes::{ChoiceText, OutputText, PromptText};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::{Frame, layout::Rect};

/// Render the inline query choice list and free-form input line into `area`.
pub(super) fn render_query_input(frame: &mut Frame, qs: &QueryDisplayState, area: Rect) {
    let mut lines: Vec<Line> = split_question_lines(&qs.question);
    let choice_lines = build_inline_choice_lines(&qs.choices, qs.selected.map(ChoiceIndex::new));
    lines.extend(choice_lines.into_iter().enumerate().map(|(i, text)| {
        let is_selected = qs.selected == Some(i);
        if is_selected {
            Line::from(Span::styled(
                text.to_string(),
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
            ))
        } else {
            Line::from(text.to_string())
        }
    }));
    let freeform_line = Line::from(vec![
        Span::raw(format!("❯ {}", qs.freeform)),
        Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED)),
    ]);
    lines.push(freeform_line);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        area,
    );
}

/// Split question text into ratatui lines for rendering.
pub fn split_question_lines(question: &PromptText) -> Vec<Line<'static>> {
    if question.is_empty() {
        return vec![Line::from("")];
    }
    question
        .lines()
        .map(|seg| Line::from(seg.to_owned()))
        .collect()
}

/// Build formatted choice lines for the inline query input area.
pub fn build_inline_choice_lines(
    choices: &[ChoiceText],
    selected: Option<ChoiceIndex>,
) -> Vec<OutputText> {
    choices
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let prefix = if selected == Some(ChoiceIndex::new(i)) {
                "> "
            } else {
                "  "
            };
            OutputText::from(format!("{}{}. {}", prefix, i + 1, text))
        })
        .collect()
}
