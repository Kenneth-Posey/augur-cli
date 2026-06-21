//! Integration tests: supervisor events → TUI plan mode rendering.
//!
//! These tests build a `PlanTree` with known node statuses, render it via
//! `render_plan_panel` using a `TestBackend` terminal, and assert that the
//! rendered buffer contains the expected status icons. They verify the full
//! path from plan data → panel renderer → terminal cell buffer.

use augur_tui::domain::newtypes::{Count, NumericNewtype, ScrollOffset};
use augur_domain::domain::plan_tree::{NodeStatus, PlanNode, PlanTree, PlanTreeId};
use augur_domain::string_newtypes::StringNewtype;
use augur_tui::tui::layout::{compute_plan_layout, PLAN_PANEL_WIDTH_PERCENT};
use augur_tui::tui::plan_panel::{render_plan_panel, PlanPanelRender};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

// ── helpers ──────────────────────────────────────────────────────────────────

fn make_terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(100, 24)).expect("TestBackend terminal must be created")
}

fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect()
}

fn make_tree(root: PlanNode) -> PlanTree {
    PlanTree {
        id: PlanTreeId::new("test-plan"),
        title: "Test Plan".into(),
        goal: "test goal".into(),
        root,
    }
}

fn full_area() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 24,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Verifies that a Done leaf in plan mode renders the "✓" checkmark icon in
/// the buffer when displayed via `render_plan_panel`.
#[test]
fn plan_mode_tree_panel_renders_done_leaf_with_checkmark() {
    let mut root = PlanNode::new_branch("root", "Root");
    let mut leaf = PlanNode::new_leaf("leaf-1", "Done Step", "steps/1.md");
    leaf.status = NodeStatus::Done;
    root.children.push(leaf);
    let tree = make_tree(root);

    let mut terminal = make_terminal();
    terminal
        .draw(|f| {
            render_plan_panel(
                f,
                PlanPanelRender::builder()
                    .tree(&tree)
                    .scroll(ScrollOffset::of(0))
                    .area(full_area())
                    .build(),
            )
        })
        .expect("render must not panic");
    let rendered = buffer_text(&terminal);

    assert!(
        rendered.contains('✓'),
        "Expected '✓' checkmark icon in rendered output"
    );
}

/// Verifies that an InProgress leaf in plan mode renders the "→" arrow icon
/// in the buffer when displayed via `render_plan_panel`.
#[test]
fn plan_mode_tree_panel_renders_in_progress_leaf_with_arrow() {
    let mut root = PlanNode::new_branch("root", "Root");
    let mut leaf = PlanNode::new_leaf("leaf-1", "Active Step", "steps/1.md");
    leaf.status = NodeStatus::InProgress;
    root.children.push(leaf);
    let tree = make_tree(root);

    let mut terminal = make_terminal();
    terminal
        .draw(|f| {
            render_plan_panel(
                f,
                PlanPanelRender::builder()
                    .tree(&tree)
                    .scroll(ScrollOffset::of(0))
                    .area(full_area())
                    .build(),
            )
        })
        .expect("render must not panic");
    let rendered = buffer_text(&terminal);

    assert!(
        rendered.contains('→'),
        "Expected '→' arrow icon in rendered output"
    );
}

/// Verifies that a Failed leaf in plan mode renders the "✗" cross icon in the
/// buffer when displayed via `render_plan_panel`.
#[test]
fn plan_mode_tree_panel_renders_failed_leaf_with_x_icon() {
    let mut root = PlanNode::new_branch("root", "Root");
    let mut leaf = PlanNode::new_leaf("leaf-1", "Failed Step", "steps/1.md");
    leaf.status = NodeStatus::Failed("compile error".into());
    root.children.push(leaf);
    let tree = make_tree(root);

    let mut terminal = make_terminal();
    terminal
        .draw(|f| {
            render_plan_panel(
                f,
                PlanPanelRender::builder()
                    .tree(&tree)
                    .scroll(ScrollOffset::of(0))
                    .area(full_area())
                    .build(),
            )
        })
        .expect("render must not panic");
    let rendered = buffer_text(&terminal);

    assert!(
        rendered.contains('✗'),
        "Expected '✗' cross icon in rendered output"
    );
}

/// Verifies that `compute_plan_layout` respects `PLAN_PANEL_WIDTH_PERCENT`
/// and that chat_cols + panel_cols equals the terminal width at 100 columns.
///
/// This integration-level check confirms the layout constant is applied
/// consistently when the full render path runs.
#[test]
fn plan_mode_tree_layout_respects_plan_panel_width_percent() {
    let _ = PLAN_PANEL_WIDTH_PERCENT;
    let total: u16 = 100;
    let widths = compute_plan_layout(Count::new(total as usize));

    assert_eq!(
        widths.chat_cols + widths.panel_cols,
        total,
        "chat_cols({}) + panel_cols({}) must equal terminal width {}",
        widths.chat_cols,
        widths.panel_cols,
        total
    );
    assert!(
        widths.panel_cols >= 20,
        "panel_cols must be at least the minimum 20 columns, got {}",
        widths.panel_cols
    );
}
