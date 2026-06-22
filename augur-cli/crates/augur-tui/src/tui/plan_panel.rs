//! Plan panel rendering: tree display for the right-side plan panel in plan mode.

use augur_domain::domain::newtypes::{Count, NumericNewtype, ScrollOffset};
use augur_domain::domain::plan_tree::{NodeStatus, PlanNode, PlanTree};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

/// Input bundle for rendering the right-side plan panel.
#[derive(Clone, Copy, bon::Builder)]
pub struct PlanPanelRender<'a> {
    /// Tree to display in the panel.
    pub tree: &'a PlanTree,
    /// Logical line offset applied to the flattened tree text.
    pub scroll: ScrollOffset,
    /// Terminal rect assigned to the panel.
    pub area: Rect,
}

/// Display icon for a pending plan node.
const ICON_PENDING: &str = "·";
/// Display icon for an in-progress plan node.
const ICON_IN_PROGRESS: &str = "→";
/// Display icon for a completed plan node.
const ICON_DONE: &str = "✓";
/// Display icon for a failed plan node.
const ICON_FAILED: &str = "✗";

/// Build a flat list of display strings for a plan node and all its descendants.
///
/// Format per node: `{indent}{icon} {title}{checkpoint_marker}`.
/// - `indent` is `"  "` repeated `depth` times.
/// - `icon` is `"✓"` Done, `"→"` InProgress, `"·"` Pending, `"✗"` Failed.
/// - `checkpoint_marker` is `" ⊙"` when a checkpoint is configured, else `""`.
///     - Branch nodes emit their own line first, then children are emitted recursively
///       at `depth + 1`. Used by `render_plan_panel` to populate the panel paragraph.
fn build_tree_lines(node: &PlanNode, depth: Count) -> Vec<OutputText> {
    let indent = "  ".repeat(depth.inner());
    let icon = status_icon(&node.status);
    let checkpoint_marker = checkpoint_suffix(node);
    let line = OutputText::from(format!(
        "{}{} {}{}",
        indent, icon, node.title, checkpoint_marker
    ));

    let mut lines = vec![line];
    for child in &node.children {
        lines.extend(build_tree_lines(child, Count::new(depth.inner() + 1)));
    }
    lines
}

/// Render the plan tree into the given `area` inside a bordered block.
///
/// Calls `build_tree_lines` from the tree root, applies the `scroll` offset,
/// and renders each visible line with status-appropriate styling: failed lines
/// are red, in-progress lines are bold, all others use the default style.
/// The panel block title shows the tree's title string.
pub fn render_plan_panel(frame: &mut Frame, render: PlanPanelRender<'_>) {
    let all_lines = build_tree_lines(&render.tree.root, Count::new(0));
    let visible: Vec<Line> = all_lines
        .iter()
        .skip(render.scroll.inner())
        .map(|s| styled_tree_line(s.as_str()))
        .collect();

    let block = Block::bordered().title(render.tree.title.as_str());
    let paragraph = Paragraph::new(Text::from(visible)).block(block);
    frame.render_widget(paragraph, render.area);
}

/// Return the single-character icon for a node status.
fn status_icon(status: &NodeStatus) -> &'static str {
    match status {
        NodeStatus::Pending => ICON_PENDING,
        NodeStatus::InProgress => ICON_IN_PROGRESS,
        NodeStatus::Done => ICON_DONE,
        NodeStatus::Failed(_) => ICON_FAILED,
    }
}

/// Return the checkpoint suffix string for a node.
///
/// Returns `" ⊙"` when a checkpoint is configured on the node, else `""`.
fn checkpoint_suffix(node: &PlanNode) -> &'static str {
    match node.config.checkpoint.is_some() {
        true => " ⊙",
        false => "",
    }
}

/// Apply status-appropriate styling to a pre-built tree display line.
///
/// Detects the icon character at the start of the non-whitespace content:
/// - `✗` → red foreground (failure).
/// - `→` → bold (in-progress).
/// - All others → default style.
fn styled_tree_line(line: &str) -> Line<'_> {
    let trimmed = line.trim_start();
    let style = if trimmed.starts_with(ICON_FAILED) {
        Style::default().fg(Color::Red)
    } else if trimmed.starts_with(ICON_IN_PROGRESS) {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::from(Span::styled(line.to_owned(), style))
}
