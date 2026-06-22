//! Query overlay rendering: displays a question, optional choices, and free-form input.

use crate::domain::tui_state::QueryState;
use augur_domain::domain::string_newtypes::ChoiceText;
use augur_domain::domain::string_newtypes::{OutputText, PromptText, StringNewtype};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

struct ChoicesBlockParams<'a> {
    choices: &'a [ChoiceText],
    selected: Option<usize>,
}

struct FreeformBlockParams<'a> {
    freeform: &'a str,
    has_choices: bool,
}

/// Render the query overlay for the full terminal frame.
///
/// Dispatches to a two-zone layout (question + freeform) when there are no choices,
/// or a three-zone layout (question + choices + freeform) when choices are present.
/// Kept as a standalone renderer for direct test use; the TUI now renders queries
/// inline via `render_query_inline` in render.rs so the output pane stays visible.
pub fn render_query(f: &mut Frame, state: &QueryState) {
    if state.choices.is_empty() {
        render_no_choices(f, state);
    } else {
        render_with_choices(f, state);
    }
}

/// Render the two-zone layout when no choices are present.
fn render_no_choices(f: &mut Frame, state: &QueryState) {
    let area = f.area();
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);
    render_question_block(f, chunks[0], &state.question);
    render_freeform_block(
        f,
        chunks[1],
        FreeformBlockParams {
            freeform: &state.freeform,
            has_choices: false,
        },
    );
}

/// Render the three-zone layout when choices are present.
fn render_with_choices(f: &mut Frame, state: &QueryState) {
    let area = f.area();
    let choices_height = compute_choices_height(state.choices.len(), area.height);
    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(choices_height),
        Constraint::Length(3),
    ])
    .split(area);
    render_question_block(f, chunks[0], &state.question);
    render_choices_block(
        f,
        chunks[1],
        ChoicesBlockParams {
            choices: &state.choices,
            selected: state.selected,
        },
    );
    render_freeform_block(
        f,
        chunks[2],
        FreeformBlockParams {
            freeform: &state.freeform,
            has_choices: true,
        },
    );
}

/// Calculate a bounded height for the choices block.
///
/// Adds 2 rows for the block borders. Clamped so it does not exceed half the
/// terminal height and is at least 3 rows (1 item + 2 borders).
fn compute_choices_height(choice_count: usize, terminal_height: u16) -> u16 {
    let raw = (choice_count as u16).saturating_add(2);
    raw.min(terminal_height / 2).max(3)
}

fn render_question_block(f: &mut Frame, area: Rect, question: &str) {
    let para = Paragraph::new(question)
        .block(Block::default().borders(Borders::ALL).title("Question"))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn render_choices_block(f: &mut Frame, area: Rect, params: ChoicesBlockParams<'_>) {
    let items: Vec<ListItem> = params
        .choices
        .iter()
        .map(|c| ListItem::new(c.to_string()))
        .collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Choices (\u{2191}\u{2193} to navigate)");
    let list = List::new(items)
        .block(block)
        .highlight_symbol("> ")
        .highlight_style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED));
    let mut list_state = ListState::default().with_selected(params.selected);
    f.render_stateful_widget(list, area, &mut list_state);
}

fn render_freeform_block(f: &mut Frame, area: Rect, params: FreeformBlockParams<'_>) {
    let label = freeform_label(params.has_choices);
    let content = format!("{}{}", label, params.freeform);
    let para = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Response (Enter to submit)"),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

/// Build display lines for the choices list with "> " prefix for the selected item.
///
/// Returns one formatted string per choice in input order. The selected item (matching
/// `selected`) is prefixed with `"> "`; all others with `"  "`. Used by `query_content`
/// and tests to verify logical selection state without requiring a live terminal frame.
fn build_choice_lines(choices: &[ChoiceText], selected: Option<usize>) -> Vec<OutputText> {
    choices
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let prefix = if selected == Some(i) { "> " } else { "  " };
            OutputText::new(format!("{prefix}{c}"))
        })
        .collect()
}

/// Return the label for the free-form input field.
///
/// Returns `"Free-form: "` when choices are present (the user may also select a choice),
/// or `"Your response: "` when no choices exist (only free-form input is available).
/// Used by `query_content` and `render_freeform_block`.
fn freeform_label(has_choices: bool) -> &'static str {
    if has_choices {
        "Free-form: "
    } else {
        "Your response: "
    }
}

/// Produce a testable summary of the query overlay content without a live terminal.
///
/// Returns `(question, choice_lines, freeform_line)` combining `build_choice_lines`
/// and `freeform_label`. Used by `tests/tui/query.tests.rs` to verify rendering logic
/// independently of ratatui frame construction.
fn query_content(state: &QueryState) -> (PromptText, Vec<OutputText>, PromptText) {
    let question = state.question.clone();
    let choices = build_choice_lines(&state.choices, state.selected);
    let label = freeform_label(!state.choices.is_empty());
    let freeform = PromptText::new(format!("{}{}", label, state.freeform));
    (question, choices, freeform)
}
