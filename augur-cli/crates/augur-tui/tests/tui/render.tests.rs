use augur_domain::config::types::{
    AgentConfig, AppConfig, CopilotConfig, EndpointConfig, EndpointCredentials, PersistenceConfig,
    Provider,
};
use augur_domain::domain::newtypes::IsRunning;
use augur_domain::domain::plan_tree::{PlanTree, PlanTreeId};
use augur_tui::actors::tui::assistant::status_bar::format_model_display;
use augur_tui::domain::newtypes::{
    ChoiceIndex, Count, NumericNewtype, ScrollOffset, Temperature, TimestampMs, TokenCount,
};
use augur_tui::domain::string_newtypes::{
    ChoiceText, EndpointName, EndpointUrl, FilePath, ModelLabel, ModelName, OutputText,
    StringNewtype,
};
use augur_tui::domain::tui_display_state::{DisplayConversationMode, TuiDisplayState};
use augur_tui::domain::tui_input::apply_agent_output;
use augur_tui::domain::tui_state::{
    AppScreen, AppState, LineHeader, OutputLine, OutputSelection, PlanModeState, SelectionPoint,
};
use augur_tui::domain::types::AgentOutput;
use augur_tui::tui::render::{
    RenderSlice, RenderSliceInput, ScreenPosToLineCharInput, build_inline_choice_lines,
    compute_render_slice, extract_selected_text, format_response_prefix, line_display_rows,
    rendered_line_text, screen_pos_to_line_char, scroll_marker_row, separator_line,
    split_question_lines,
};
use ratatui::layout::{Position, Rect};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;

fn model_option(
    id: impl Into<String>,
    display_name: impl Into<String>,
) -> augur_tui::domain::types::ModelOption {
    augur_tui::domain::types::ModelOption::builder()
        .id(augur_tui::domain::string_newtypes::ModelId::new(id.into()))
        .display_name(ModelLabel::new(display_name.into()))
        .build()
}

fn minimal_config() -> AppConfig {
    let ep = EndpointConfig {
        name: EndpointName::new("claude"),
        provider: Provider::Anthropic,
        base_url: EndpointUrl::new("https://api.anthropic.com"),
        model: ModelName::new("claude-sonnet-4-6"),
        credentials: EndpointCredentials::default(),
    };
    AppConfig {
        endpoints: vec![ep],
        default_endpoint: EndpointName::new("claude"),
        agent: AgentConfig {
            system_prompt: OutputText::new(""),
            max_tokens: TokenCount::new(1024),
            temperature: Temperature::new(1.0),
            allowed_dirs: vec![],
        },
        copilot: CopilotConfig::default(),
        persistence: PersistenceConfig {
            log_dir: FilePath::new("./logs"),
            sessions_dir: None,
        },
        program_settings: Default::default(),
        user_settings: Default::default(),
    }
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {:?} failed: stdout={:?} stderr={:?}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn init_git_repo(branch: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-b", branch]);
    git(dir.path(), &["config", "user.name", "Test User"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    std::fs::write(dir.path().join("tracked.txt"), "tracked\n").expect("seed tracked file");
    git(dir.path(), &["add", "tracked.txt"]);
    git(dir.path(), &["commit", "-m", "initial"]);
    dir
}

fn cwd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct CurrentDirGuard {
    _lock: MutexGuard<'static, ()>,
    previous: PathBuf,
}

impl CurrentDirGuard {
    fn enter(path: &Path) -> Self {
        let lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::current_dir().expect("current dir");
        std::env::set_current_dir(path).expect("set current dir");
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).expect("restore current dir");
    }
}

fn status_state_for_repo(repo: &Path, displayed_branch: &str) -> AppState {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.status.cwd = repo.display().to_string().into();
    state.status.git_branch = Some(displayed_branch.into());
    state
}

fn make_plan_mode_state() -> PlanModeState {
    PlanModeState {
        tree: PlanTree::new(
            PlanTreeId::new("render-test-plan"),
            "Render Test Plan",
            "test goal",
        ),
        running: IsRunning::no(),
        tree_scroll: ScrollOffset::of(0),
    }
}

///
/// temperature=1.0 → EffortLevel::High → label "high"; model "claude-sonnet-4-6".
#[test]
fn format_model_display_with_known_endpoint() {
    let config = minimal_config();
    let ep = EndpointName::new("claude");
    let display = format_model_display(&config, &ep);
    assert_eq!(display, "claude-sonnet-4-6 (high)");
}

/// Verifies that separator_line produces exactly `width` horizontal rule characters.
///
/// Each character must be the box-drawing '─' (U+2500). The count is by char, not by byte.
#[test]
fn render_separator_fills_width() {
    let line = separator_line(augur_tui::domain::newtypes::Count::of(10));
    let char_count = line.to_string().chars().count();
    assert_eq!(char_count, 10);
    assert!(line.to_string().chars().all(|c| c == '─'));
}

/// Verifies that separator_line with width 0 returns an empty string.
#[test]
fn render_separator_zero_width_is_empty() {
    let line = separator_line(augur_tui::domain::newtypes::Count::of(0));
    assert!(line.to_string().is_empty());
}

/// Verifies that scroll_marker_row places the marker at the bottom (height-1) when
/// scroll_offset is 0, meaning the user is viewing the most recent content.
#[test]
fn scroll_marker_at_bottom_when_offset_zero() {
    let marker = scroll_marker_row(
        augur_tui::tui::components::primary_feed_utils::ScrollRenderContext::builder()
            .total_lines(100)
            .visible_lines(20)
            .scroll_offset(0)
            .indicator_height(20)
            .build(),
    );
    assert!(marker.visible);
    assert_eq!(marker.row, augur_tui::domain::newtypes::Count::of(19));
}

/// Verifies that scroll_marker_row places the marker at row 0 when scrolled to the
/// maximum offset, meaning the user is viewing the oldest content.
#[test]
fn scroll_marker_at_top_when_fully_scrolled() {
    // total=100, visible=20 → max_offset=80
    let marker = scroll_marker_row(
        augur_tui::tui::components::primary_feed_utils::ScrollRenderContext::builder()
            .total_lines(100)
            .visible_lines(20)
            .scroll_offset(80)
            .indicator_height(20)
            .build(),
    );
    assert!(marker.visible);
    assert_eq!(marker.row, augur_tui::domain::newtypes::Count::of(0));
}

/// Verifies that scroll_marker_row hides the marker when all content fits within
/// the visible area (no scrolling is possible).
#[test]
fn scroll_marker_hidden_when_content_fits_in_view() {
    let marker = scroll_marker_row(
        augur_tui::tui::components::primary_feed_utils::ScrollRenderContext::builder()
            .total_lines(10)
            .visible_lines(20)
            .scroll_offset(0)
            .indicator_height(20)
            .build(),
    );
    assert!(!marker.visible);
}

/// Verifies that scroll_marker_row returns no marker when indicator_height is zero.
#[test]
fn scroll_marker_hidden_when_indicator_height_zero() {
    let marker = scroll_marker_row(
        augur_tui::tui::components::primary_feed_utils::ScrollRenderContext::builder()
            .total_lines(100)
            .visible_lines(20)
            .scroll_offset(0)
            .indicator_height(0)
            .build(),
    );
    assert!(!marker.visible);
}

/// Verifies that build_inline_choice_lines prefixes each line with its 1-based number.
///
/// Lines must follow the format "  N. {text}" for unselected and "> N. {text}" for the
/// currently selected item, matching the inline query input area rendering contract.
#[test]
fn build_inline_choice_lines_formats_with_numbers() {
    let choices = vec![
        ChoiceText::new("Alpha"),
        ChoiceText::new("Beta"),
        ChoiceText::new("Gamma"),
    ];
    let lines = build_inline_choice_lines(&choices, None);
    assert_eq!(lines[0], "  1. Alpha");
    assert_eq!(lines[1], "  2. Beta");
    assert_eq!(lines[2], "  3. Gamma");
}

/// Verifies that build_inline_choice_lines marks the selected item with "> " prefix.
///
/// Only the matching item (0-based index) receives the "> " prefix; all others use
/// two spaces so the selection is visually distinct.
#[test]
fn build_inline_choice_lines_marks_selected_with_arrow() {
    let choices = vec![ChoiceText::new("A"), ChoiceText::new("B")];
    let lines = build_inline_choice_lines(&choices, Some(ChoiceIndex::new(1)));
    assert_eq!(lines[0], "  1. A");
    assert_eq!(lines[1], "> 2. B");
}

// --- line_display_rows tests ---

/// Verifies that an empty line always occupies exactly one display row, since
/// a blank separator still takes a row in the paragraph widget.
#[test]
fn line_display_rows_empty_text_returns_one() {
    assert_eq!(
        line_display_rows(&OutputText::new(""), Count::new(80)),
        Count::new(1)
    );
}

/// Verifies that text shorter than the content width fits in a single display row.
#[test]
fn line_display_rows_short_text_returns_one() {
    assert_eq!(
        line_display_rows(&OutputText::new("hello"), Count::new(80)),
        Count::new(1)
    );
}

/// Verifies that text whose character count exactly equals the content width
/// occupies exactly one display row without wrapping.
#[test]
fn line_display_rows_text_fills_exactly_one_row() {
    let text = "x".repeat(80);
    assert_eq!(
        line_display_rows(&OutputText::new(text), Count::new(80)),
        Count::new(1)
    );
}

/// Verifies that a single character over the content width triggers wrapping
/// to exactly two display rows.
#[test]
fn line_display_rows_one_char_over_width_returns_two() {
    let text = "x".repeat(81);
    assert_eq!(
        line_display_rows(&OutputText::new(text), Count::new(80)),
        Count::new(2)
    );
}

/// Verifies that text exactly double the content width occupies two rows.
#[test]
fn line_display_rows_double_width_returns_two() {
    let text = "x".repeat(160);
    assert_eq!(
        line_display_rows(&OutputText::new(text), Count::new(80)),
        Count::new(2)
    );
}

/// Verifies that short space-separated words produce more display rows than a
/// pure character-count estimate would predict. This is the core word-wrap
/// correctness property: "ab cd ef" at width 4 occupies 3 rows (each word
/// wraps because the previous word + space leaves no room), not 2.
#[test]
fn line_display_rows_word_wrap_exceeds_char_count_estimate() {
    // "ab cd ef" = 8 chars, ceil(8/4)=2, but word-wrap gives 3
    let text = OutputText::new("ab cd ef");
    assert_eq!(
        line_display_rows(&text, Count::new(4)),
        Count::new(3),
        "word-wrap should produce 3 rows for 'ab cd ef' at width 4"
    );
}

/// Verifies that a word longer than the row width is character-broken
/// across as many rows as needed.
#[test]
fn line_display_rows_long_word_character_breaks() {
    // "abcdefg" (7 chars) at width 3 → "abc"|"def"|"g" = 3 rows
    let text = OutputText::new("abcdefg");
    assert_eq!(
        line_display_rows(&text, Count::new(3)),
        Count::new(3),
        "long word must be character-broken across rows"
    );
}

/// Verifies that a single wide (2-column) character fills 2 display columns,
/// so 4 wide chars at width 4 occupies exactly 1 row (not 2).
#[test]
fn line_display_rows_wide_chars_count_display_columns() {
    // "中中中中" - 4 CJK chars, each 2 display cols = 8 cols → wraps at width 4
    // Each char alone fills a row: "中" = 2 cols at width 4 → 2 wide chars per row
    // 4 wide chars / 2 per row = 2 rows
    let text = OutputText::new("中中中中");
    assert_eq!(
        line_display_rows(&text, Count::new(4)),
        Count::new(2),
        "4 wide chars (2 cols each) at width 4 should occupy 2 rows"
    );
}

/// Verifies that two wide chars exactly fill one row at width 4 (2+2=4).
#[test]
fn line_display_rows_wide_chars_exact_fit() {
    let text = OutputText::new("中中");
    assert_eq!(
        line_display_rows(&text, Count::new(4)),
        Count::new(1),
        "2 wide chars (2 cols each) at width 4 should fit in 1 row"
    );
}

/// Verifies that zero-width combining characters do not increase the row count.
#[test]
fn line_display_rows_combining_chars_zero_width() {
    // 'a' + combining grave accent U+0300 = 1 display column
    let text = OutputText::new("a\u{0300}b\u{0300}");
    assert_eq!(
        line_display_rows(&text, Count::new(2)),
        Count::new(1),
        "combining chars must not inflate the display column count"
    );
}

// --- compute_render_slice tests ---

/// Verifies that an empty line list produces a (0, 0, 0) slice with no scroll.
#[test]
fn compute_render_slice_empty_lines_returns_zero_slice() {
    let lines: Vec<OutputLine> = vec![];
    let render_slice = render_slice_for(&lines, (10, 0, 80));
    assert_eq!(render_slice.start, 0);
    assert_eq!(render_slice.end, 0);
    assert_eq!(render_slice.para_scroll, 0);
}

/// Verifies that when there are fewer lines than the visible height, the slice
/// starts at index 0 with no paragraph scroll - all content fits in the view.
#[test]
fn compute_render_slice_fewer_lines_than_visible_shows_all() {
    let lines: Vec<OutputLine> = (0..3)
        .map(|i| OutputLine::plain(format!("line{i}")))
        .collect();
    let render_slice = render_slice_for(&lines, (10, 0, 80));
    assert_eq!(render_slice.start, 0);
    assert_eq!(render_slice.end, 3);
    assert_eq!(render_slice.para_scroll, 0);
}

/// Verifies that with no wrapping and scroll_offset=0, the slice selects the
/// last `visible` logical lines with no paragraph scroll offset.
#[test]
fn compute_render_slice_no_wrapping_auto_scroll_selects_last_n_lines() {
    let lines: Vec<OutputLine> = (0..20)
        .map(|i| OutputLine::plain(format!("line{i}")))
        .collect();
    let render_slice = render_slice_for(&lines, (10, 0, 80));
    assert_eq!(render_slice.start, 10);
    assert_eq!(render_slice.end, 20);
    assert_eq!(render_slice.para_scroll, 0);
}

/// Regression: trailing blank separator rows should not become the bottom anchor,
/// even when earlier lines wrap.
#[test]
fn compute_render_slice_wrapping_line_excludes_trailing_separators() {
    let lines = vec![
        OutputLine::plain("x".repeat(15)), // 2 display rows
        OutputLine::plain("text"),         // 1 row
        OutputLine::plain("more"),         // 1 row
        OutputLine::plain(""),             // blank separator 1
        OutputLine::plain(""),             // blank separator 2
    ];
    let visible = 5;
    let content_width = 10;
    let render_slice = render_slice_for(&lines, (visible, 0, content_width));

    assert_eq!(render_slice.start, 0);
    assert_eq!(render_slice.end, 3);
    assert_eq!(render_slice.para_scroll, 0);
}

/// Verifies that scroll_offset skips the bottom N display rows and the
/// slice shows `visible` rows ending just before the skipped boundary.
#[test]
fn compute_render_slice_scroll_offset_excludes_bottom_lines() {
    let lines: Vec<OutputLine> = (0..10)
        .map(|i| OutputLine::plain(format!("line{i}")))
        .collect();
    // scroll_offset=2: skip 2 display rows. Each line is 1 row, so lines[8] and
    // lines[9] are scrolled past (not shown). visible=4 → show lines[4..8].
    let render_slice = render_slice_for(&lines, (4, 2, 80));
    assert_eq!(render_slice.start, 4);
    assert_eq!(render_slice.end, 8);
    assert_eq!(render_slice.para_scroll, 0);
}

/// Verifies that scroll_offset combined with wrapping still presents exactly
/// `visible` display rows, with the bottom-cutoff excluding scrolled-past rows.
#[test]
fn compute_render_slice_scroll_offset_with_wrapping_adjusts_start() {
    // Line 0 wraps to 2 rows; lines 1-5 are single-row.
    // scroll_offset=1 skips 1 display row (line5). visible=4.
    let lines = vec![
        OutputLine::plain("x".repeat(15)), // 2 display rows (width 10)
        OutputLine::plain("line1"),
        OutputLine::plain("line2"),
        OutputLine::plain("line3"),
        OutputLine::plain("line4"),
        OutputLine::plain("line5"), // scrolled past (1 display row)
    ];
    let render_slice = render_slice_for(&lines, (4, 1, 10));
    // bottom_cutoff = 5 (line5's 1 row skipped). Need 4 display rows from lines[..5]:
    // walk back: line4(1), line3(1), line2(1), line1(1) → need fulfilled, start=1.
    assert_eq!(render_slice.start, 1);
    assert_eq!(render_slice.end, 5);
    assert_eq!(render_slice.para_scroll, 0);
}

/// Verifies that scroll_offset counts display rows, not logical lines.
///
/// A two-row line at the bottom requires scroll_offset=2 to be fully excluded.
/// With scroll_offset=1, the line cannot be partially skipped (display-row
/// granularity is whole lines), so the boundary line stays visible and
/// `fill_from_bottom` handles the partial-row case via `para_scroll`.
#[test]
fn compute_render_slice_scroll_offset_skips_display_rows_not_logical_lines() {
    // Lines: 3 single-row lines, then 1 two-row line at the tail.
    let lines = vec![
        OutputLine::plain("line0"),        // 1 display row
        OutputLine::plain("line1"),        // 1 display row
        OutputLine::plain("line2"),        // 1 display row
        OutputLine::plain("x".repeat(15)), // 2 display rows at width=10
    ];
    // scroll_offset=1: attempt to skip 1 display row from the bottom.
    // The last line has 2 rows and cannot be split - it stays visible (end=4).
    let slice1 = render_slice_for(&lines, (4, 1, 10));
    assert_eq!(
        slice1.end, 4,
        "scroll_offset=1 must keep 2-row tail line visible (cannot split its rows)"
    );

    // scroll_offset=2: the 2-row tail line is exactly 2 display rows, so it is
    // fully excluded. The visible region is lines[..3].
    let slice2 = render_slice_for(&lines, (4, 2, 10));
    assert_eq!(
        slice2.end, 3,
        "scroll_offset=2 must exclude the 2-row wrapped line"
    );
    assert_eq!(
        slice2.start, 0,
        "all 3 remaining lines fit in the 4-row viewport"
    );
    assert_eq!(slice2.para_scroll, 0);
}
/// Verifies that bottom-follow shows the newest rows even when older tool/event
/// lines are present in history.
#[test]
fn test_bottom_follow_shows_newest_rows_with_older_tool_events() {
    use augur_tui::domain::tui_state::OutputLine;

    let mut lines = vec![
        OutputLine::plain("Starting analysis..."),
        OutputLine::tool_call("→ view: /src/main.rs"),
    ];
    lines.extend((0..12).map(|i| OutputLine::plain(format!("status line {i}"))));

    let visible_rows = 5;
    let content_width = 80;
    let render_slice = render_slice_for(&lines, (visible_rows, 0, content_width));

    assert!(
        render_slice.end == lines.len(),
        "bottom-follow must include the newest line"
    );
    assert!(
        render_slice.start > 1,
        "older tool lines must not pin the viewport start; got start={}",
        render_slice.start
    );
}

/// Verifies that increasing scroll offset moves the viewport to older content
/// even when earlier important lines exist.
#[test]
fn test_scroll_offset_moves_slice_with_earlier_error_lines() {
    let mut lines: Vec<OutputLine> = (0..14)
        .map(|i| OutputLine::plain(format!("status line {i}")))
        .collect();
    lines.insert(2, OutputLine::error("early error marker"));
    let at_bottom = render_slice_for(&lines, (6, 0, 80));
    let scrolled = render_slice_for(&lines, (6, 4, 80));

    assert!(
        scrolled.end < at_bottom.end,
        "scrolling up must move the viewport away from newest rows: bottom_end={}, scrolled_end={}",
        at_bottom.end,
        scrolled.end
    );
    assert!(
        scrolled.start <= at_bottom.start,
        "scrolling up must not lock the start to a fixed line"
    );
}

/// Regression: a wrapped latest line should still compute a non-zero
/// paragraph scroll when only part of that line fits.
#[test]
fn fill_from_bottom_preserves_para_scroll_for_partial_wrapped_line() {
    let lines = vec![
        OutputLine::error("error occurred"),
        OutputLine::plain("x".repeat(25)),
        OutputLine::plain("plain a"),
        OutputLine::plain("plain b"),
    ];
    let render_slice = render_slice_for(&lines, (3, 0, 10));

    assert_eq!(
        render_slice.start, 1,
        "partial wrapped line should be the slice start"
    );
    assert_eq!(
        render_slice.para_scroll, 2,
        "partial wrapped line should keep paragraph scroll for hidden leading rows"
    );
}

/// Regression: with scroll_offset=0, the last logical line must always appear
/// in the rendered slice.
#[test]
fn fill_from_bottom_shows_last_line_when_scroll_offset_zero() {
    // Mix of plain and important lines; last line is plain output.
    let lines = vec![
        OutputLine::error("error here"),  // important, line 0
        OutputLine::plain("after error"), // line 1
        OutputLine::plain("more output"), // line 2
        OutputLine::plain("last line"),   // line 3 - must always be visible
    ];
    let render_slice = render_slice_for(&lines, (4, 0, 80));

    assert!(
        render_slice.end > 3,
        "last line (index 3) must be within end={} of the render slice",
        render_slice.end
    );
    assert!(
        render_slice.start <= 3,
        "start={} must include last line at index 3",
        render_slice.start
    );
    assert_eq!(
        render_slice.para_scroll, 0,
        "no para_scroll expected when all lines fit; got {}",
        render_slice.para_scroll
    );
}

/// Regression: bottom-follow should anchor to the newest timestamped/content
/// row, not trailing blank separator rows.
#[test]
fn fill_from_bottom_ignores_trailing_blank_padding_rows() {
    let lines = vec![
        OutputLine::builder()
            .text(OutputText::new("older message"))
            .kind(augur_tui::domain::tui_state::LineKind::Plain)
            .header(LineHeader {
                timestamp: Some(TimestampMs::new(1)),
                model_prefix: None,
            })
            .build(),
        OutputLine::builder()
            .text(OutputText::new("latest message"))
            .kind(augur_tui::domain::tui_state::LineKind::Plain)
            .header(LineHeader {
                timestamp: Some(TimestampMs::new(2)),
                model_prefix: None,
            })
            .build(),
        OutputLine::plain(""),
        OutputLine::plain(""),
    ];
    let render_slice = render_slice_for(&lines, (1, 0, 80));

    assert_eq!(
        render_slice.start, 1,
        "latest timestamped line must anchor the bottom viewport"
    );
    assert_eq!(
        render_slice.end, 2,
        "trailing blank separator lines must be excluded from bottom-follow"
    );
}

// ---------------------------------------------------------------------------
// rendered_line_text tests
// ---------------------------------------------------------------------------

/// Verifies that a plain line (no timestamp) returns the raw text unchanged.
#[test]
fn rendered_line_text_plain_has_no_prefix() {
    let line = OutputLine::plain("hello world");
    assert_eq!(rendered_line_text(&line), "hello world");
}

/// Verifies that a line with a timestamp prepends the formatted prefix to the text.
#[test]
fn rendered_line_text_with_timestamp_has_prefix() {
    let mut line = OutputLine::plain("hello");
    line.header = LineHeader {
        timestamp: Some(TimestampMs::new(0)),
        model_prefix: None,
    };
    let rendered = rendered_line_text(&line);
    // The prefix format is "[HH:MM:SS] " - just verify the text is at the end.
    assert!(
        rendered.ends_with("hello"),
        "text must follow timestamp prefix, got: {rendered}"
    );
    assert!(
        rendered.len() > "hello".len(),
        "timestamp prefix must be present"
    );
}

/// Verifies that format_response_prefix with timestamp and model produces the full prefix.
///
/// `[HH:MM:SS] model-name > ` format is expected for agent response lines.
/// Exact time values reflect local timezone; shape is checked, not specific hours.
#[test]
fn format_response_prefix_with_timestamp_and_model() {
    let header = LineHeader {
        timestamp: Some(TimestampMs::new(0)),
        model_prefix: Some("claude-4".into()),
    };
    let result = format_response_prefix(&header);
    assert_eq!(&result[0..1], "[", "must start with '['");
    assert_eq!(&result[3..4], ":");
    assert_eq!(&result[6..7], ":");
    assert_eq!(&result[9..10], "]");
    assert!(
        result.contains("claude-4"),
        "must include model name, got: {result}"
    );
    assert!(
        result.ends_with(" > "),
        "must end with ' > ', got: {result}"
    );
}

/// Verifies that format_response_prefix with timestamp only produces a bare timestamp prefix.
///
/// No model suffix expected; result must be `[HH:MM:SS] ` shaped (local timezone).
#[test]
fn format_response_prefix_timestamp_only() {
    let header = LineHeader {
        timestamp: Some(TimestampMs::new(0)),
        model_prefix: None,
    };
    let result = format_response_prefix(&header);
    assert_eq!(
        result.len(),
        11,
        "timestamp-only prefix must be 11 chars, got: {result:?}"
    );
    assert_eq!(&result[0..1], "[");
    assert_eq!(&result[3..4], ":");
    assert_eq!(&result[6..7], ":");
    assert_eq!(&result[9..10], "]");
    assert_eq!(&result[10..], " ");
}

// ---------------------------------------------------------------------------
// screen_pos_to_line_char tests
// ---------------------------------------------------------------------------

fn single_row_area() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 10,
    }
}

fn render_slice_for(lines: &[OutputLine], viewport: (usize, usize, usize)) -> RenderSlice {
    let (visible_rows, scroll_offset, content_width) = viewport;
    compute_render_slice(
        RenderSliceInput::builder()
            .lines(lines)
            .visible_rows(augur_tui::domain::newtypes::Count::new(visible_rows))
            .scroll_offset(augur_tui::domain::newtypes::ScrollOffset::of(scroll_offset))
            .content_width(augur_tui::domain::newtypes::Count::new(content_width))
            .build(),
    )
}

fn screen_pos_input<'a>(
    screen_pos: Position,
    lines: &'a [OutputLine],
    frame: (Rect, RenderSlice),
) -> ScreenPosToLineCharInput<'a> {
    let (content_area, render_slice) = frame;
    ScreenPosToLineCharInput::builder()
        .screen_pos(screen_pos)
        .lines(lines)
        .content_area(content_area)
        .render_slice(render_slice)
        .build()
}

fn selection_state(lines: Vec<OutputLine>, area: Rect, scroll_offset: usize) -> AppState {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.output.lines = lines;
    state
        .output
        .scroll_offset
        .set(ScrollOffset::of(scroll_offset));
    state.output.panel_areas.output_area.set(area);
    state
}

fn select_range(state: &mut AppState, anchor: (u16, u16), cursor: (u16, u16)) {
    state.output.selection = Some(OutputSelection {
        anchor: SelectionPoint {
            row: anchor.0,
            col: anchor.1,
        },
        cursor: SelectionPoint {
            row: cursor.0,
            col: cursor.1,
        },
    });
}

/// Verifies that mapping the top-left corner of the content area to the first
/// line returns line 0 with char offset 0.
#[test]
fn screen_pos_to_line_char_first_line_start() {
    let lines = vec![
        OutputLine::plain("abcdefghij"),
        OutputLine::plain("klmnopqrst"),
    ];
    let area = single_row_area();
    let pos = screen_pos_to_line_char(screen_pos_input(
        Position::new(0, 0),
        &lines,
        (area, render_slice_for(&lines, (10, 0, area.width as usize))),
    ));
    assert_eq!(pos.line_index, 0);
    assert_eq!(pos.char_offset, 0);
}

/// Verifies that a column offset within the first line maps to the correct char offset.
#[test]
fn screen_pos_to_line_char_first_line_mid_col() {
    let lines = vec![
        OutputLine::plain("abcdefghij"),
        OutputLine::plain("klmnopqrst"),
    ];
    let area = single_row_area();
    // row 0, col 5 → char 5 within line 0
    let pos = screen_pos_to_line_char(screen_pos_input(
        Position::new(5, 0),
        &lines,
        (area, render_slice_for(&lines, (10, 0, area.width as usize))),
    ));
    assert_eq!(pos.line_index, 0);
    assert_eq!(pos.char_offset, 5);
}

/// Verifies that row 1 with no wrapping maps to line index 1 within the lines slice.
#[test]
fn screen_pos_to_line_char_second_line() {
    let lines = vec![OutputLine::plain("line one"), OutputLine::plain("line two")];
    let area = single_row_area();
    // Each line fits in one display row (width=20, text<20 chars).
    // Screen row 1 → lines[1], char offset = col.
    let pos = screen_pos_to_line_char(screen_pos_input(
        Position::new(3, 1),
        &lines,
        (area, render_slice_for(&lines, (10, 0, area.width as usize))),
    ));
    assert_eq!(pos.line_index, 1);
    assert_eq!(pos.char_offset, 3);
}

/// Verifies that when the content is below the visible area (pos past all lines),
/// the function clamps to the last line and last char.
#[test]
fn screen_pos_to_line_char_clamps_past_end() {
    let lines = vec![OutputLine::plain("abc")];
    let area = single_row_area();
    // row 99 is far past any content - should return last line, last char.
    let pos = screen_pos_to_line_char(screen_pos_input(
        Position::new(0, 99),
        &lines,
        (area, render_slice_for(&lines, (10, 0, area.width as usize))),
    ));
    assert_eq!(pos.line_index, 0);
    assert_eq!(pos.char_offset, 3); // "abc" has 3 chars
}

/// Verifies that an empty lines slice returns (0, 0) without panicking.
#[test]
fn screen_pos_to_line_char_empty_lines_returns_origin() {
    let lines: Vec<OutputLine> = vec![];
    let area = single_row_area();
    let pos = screen_pos_to_line_char(screen_pos_input(
        Position::new(0, 0),
        &lines,
        (area, render_slice_for(&lines, (10, 0, area.width as usize))),
    ));
    assert_eq!(pos.line_index, 0);
    assert_eq!(pos.char_offset, 0);
}

// ---------------------------------------------------------------------------
// extract_selected_text tests
// ---------------------------------------------------------------------------

#[test]
fn extract_selected_text_single_line_returns_selected_segment() {
    let mut state = selection_state(
        vec![OutputLine::plain("abcdef")],
        Rect {
            x: 0,
            y: 0,
            width: 21,
            height: 4,
        },
        0,
    );
    select_range(&mut state, (0, 1), (0, 4));

    let selected = extract_selected_text(&state).expect("selection");
    assert_eq!(selected.as_str(), "bcd");
}

#[test]
fn extract_selected_text_multi_line_joins_lines_with_newline() {
    let mut state = selection_state(
        vec![OutputLine::plain("abc"), OutputLine::plain("def")],
        Rect {
            x: 0,
            y: 0,
            width: 21,
            height: 4,
        },
        0,
    );
    select_range(&mut state, (0, 1), (1, 2));

    let selected = extract_selected_text(&state).expect("selection");
    assert_eq!(selected.as_str(), "bc\nde");
}

#[test]
fn extract_selected_text_narrow_output_area_returns_none() {
    let mut state = selection_state(
        vec![OutputLine::plain("abc")],
        Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 4,
        },
        0,
    );
    select_range(&mut state, (0, 0), (0, 1));

    assert!(extract_selected_text(&state).is_none());
}

#[test]
fn extract_selected_text_clamps_blank_space_to_last_rendered_line() {
    let mut state = selection_state(
        vec![
            OutputLine::plain("old0"),
            OutputLine::plain("old1"),
            OutputLine::plain("new2"),
            OutputLine::plain("new3"),
        ],
        Rect {
            x: 0,
            y: 0,
            width: 21,
            height: 4,
        },
        2,
    );
    select_range(&mut state, (1, 0), (3, 0));

    let selected = extract_selected_text(&state).expect("selection");
    assert_eq!(selected.as_str(), "old1");
}

// ---------------------------------------------------------------------------
// End of tests
// ---------------------------------------------------------------------------

/// Verifies that a turn-complete refresh renders the current checked-out branch
/// from live git state instead of preserving a stale displayed branch name.
#[test]
fn status_bar_git_branch_renders_current_repo_branch_after_turn_complete() {
    let repo = init_git_repo("feature/current-display");
    let _cwd = CurrentDirGuard::enter(repo.path());
    let mut state = status_state_for_repo(repo.path(), "stale-branch");

    apply_agent_output(&mut state, AgentOutput::TurnComplete);

    let rendered = augur_tui::tui::render::status_left(&state.status, None);
    assert_eq!(
        rendered,
        format!("{} [feature/current-display]", repo.path().display()),
        "branch display must be refreshed from current git state after TurnComplete",
    );
}

/// Verifies that the branch display updates to a newly checked-out branch after
/// a turn completes, matching the current repository state.
#[test]
fn status_bar_git_branch_updates_after_branch_change() {
    let repo = init_git_repo("main");
    let _cwd = CurrentDirGuard::enter(repo.path());
    let mut state = status_state_for_repo(repo.path(), "main");
    git(repo.path(), &["checkout", "-b", "feature/updated-branch"]);

    apply_agent_output(&mut state, AgentOutput::Done);

    let rendered = augur_tui::tui::render::status_left(&state.status, None);
    assert_eq!(
        rendered,
        format!("{} [feature/updated-branch]", repo.path().display()),
        "branch display must follow branch changes after Done",
    );
}

/// Verifies that a dirty working tree renders an asterisk on the branch display
/// after a turn-complete status refresh.
#[test]
fn status_bar_git_branch_shows_asterisk_when_repo_is_dirty() {
    let repo = init_git_repo("main");
    let _cwd = CurrentDirGuard::enter(repo.path());
    let mut state = status_state_for_repo(repo.path(), "main");
    std::fs::write(repo.path().join("dirty.txt"), "pending change\n").expect("write dirty file");

    apply_agent_output(&mut state, AgentOutput::TurnComplete);

    let rendered = augur_tui::tui::render::status_left(&state.status, None);
    assert!(
        rendered.contains("[main*]"),
        "dirty branch display must include an asterisk, got: {rendered}",
    );
}

// ---------------------------------------------------------------------------
// split_question_lines tests
// ---------------------------------------------------------------------------

/// Verifies that a single-line question produces exactly one Line with the question text.
#[test]
fn split_question_lines_single_line_returns_one_line() {
    let lines = split_question_lines(&augur_tui::domain::string_newtypes::PromptText::new(
        "hello world",
    ));
    assert_eq!(lines.len(), 1);
}

/// Verifies that a question containing a newline produces two separate Lines.
///
/// Each segment separated by `\n` must map to a distinct Line so ratatui renders
/// them on separate rows without relying on the Wrap widget for explicit breaks.
#[test]
fn split_question_lines_splits_on_newline() {
    let lines = split_question_lines(&augur_tui::domain::string_newtypes::PromptText::new(
        "first\nsecond",
    ));
    assert_eq!(lines.len(), 2);
}

/// Verifies that a question with three segments produces three Lines.
#[test]
fn split_question_lines_multiple_newlines_produce_multiple_lines() {
    let lines = split_question_lines(&augur_tui::domain::string_newtypes::PromptText::new(
        "a\nb\nc",
    ));
    assert_eq!(lines.len(), 3);
}

/// Verifies that an empty question returns exactly one empty Line.
///
/// An empty question must not collapse to zero lines - at least one Line
/// is required so the question row is always visible in the layout.
#[test]
fn split_question_lines_empty_returns_one_empty_line() {
    let lines = split_question_lines(&augur_tui::domain::string_newtypes::PromptText::new(""));
    assert_eq!(lines.len(), 1);
}

// ---------------------------------------------------------------------------
// Phase 5: controls_row_hint and ask panel render tests
// ---------------------------------------------------------------------------

/// Verifies that controls_row_hint returns ctrl+w/close-ask when the ask panel is open.
#[test]
fn controls_row_hint_ask_open_shows_esc_close_ask() {
    use augur_tui::domain::tui_state::SecondaryView;
    use augur_tui::tui::render::controls_row_hint;
    let hint = controls_row_hint(Some(&SecondaryView::Ask), &DisplayConversationMode::Chat);
    assert_eq!(hint.key, "ctrl+w");
    assert_eq!(hint.description, "close ask");
}

/// Verifies that controls_row_hint returns esc/close-plan when in plan mode and ask is closed.
#[test]
fn controls_row_hint_plan_mode_shows_esc_close_plan() {
    use augur_tui::tui::render::controls_row_hint;
    let hint = controls_row_hint(None, &DisplayConversationMode::Plan(make_plan_mode_state()));
    assert_eq!(hint.key, "esc");
    assert_eq!(hint.description, "close plan");
}

/// Verifies that controls_row_hint returns shift+tab/open-ask by default.
#[test]
fn controls_row_hint_default_shows_shift_tab_open_ask() {
    use augur_tui::tui::render::controls_row_hint;
    let hint = controls_row_hint(None, &DisplayConversationMode::Chat);
    assert_eq!(hint.key, "shift+tab");
    assert_eq!(hint.description, "open ask");
}

/// Verifies that ask-panel-open takes priority over plan-mode in controls_row_hint.
#[test]
fn controls_row_hint_ask_takes_priority_over_plan() {
    use augur_tui::domain::tui_state::SecondaryView;
    use augur_tui::tui::render::controls_row_hint;
    let hint = controls_row_hint(
        Some(&SecondaryView::Ask),
        &DisplayConversationMode::Plan(make_plan_mode_state()),
    );
    assert_eq!(hint.key, "ctrl+w");
    assert_eq!(hint.description, "close ask");
}

/// Verifies that render does not panic when the ask panel is open alongside chat mode.
#[test]
fn render_with_ask_panel_open_does_not_panic() {
    use augur_tui::domain::string_newtypes::EndpointName;
    use augur_tui::domain::tui_state::{AppScreen, AppState, AskPanelState};
    use augur_tui::tui::render::render_with_overlays;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal must be created");
    let ep = EndpointName::new("test");
    let mut state = AppState::new(ep, AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    terminal
        .draw(|frame| render_with_overlays(frame, &TuiDisplayState::project_from(&state)))
        .expect("draw must succeed");
}

/// Verifies that render does not panic in Chat mode with ask panel closed (controls row visible).
#[test]
fn render_controls_row_visible_when_no_ask_panel() {
    use augur_tui::domain::string_newtypes::EndpointName;
    use augur_tui::domain::tui_state::{AppScreen, AppState};
    use augur_tui::tui::render::render_with_overlays;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal must be created");
    let ep = EndpointName::new("test");
    let state = AppState::new(ep, AppScreen::Conversation);
    terminal
        .draw(|frame| render_with_overlays(frame, &TuiDisplayState::project_from(&state)))
        .expect("draw must succeed");
}

/// Verifies that render_ask_panel renders the ask panel title when ask is focused.
#[test]
fn render_ask_panel_with_focused_state_does_not_panic() {
    use augur_tui::domain::string_newtypes::EndpointName;
    use augur_tui::domain::tui_state::{AppScreen, AppState, AskPanelState, InputFocus};
    use augur_tui::tui::render::render_with_overlays;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal must be created");
    let ep = EndpointName::new("test");
    let mut state = AppState::new(ep, AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.input_focus = InputFocus::Ask;
    terminal
        .draw(|frame| render_with_overlays(frame, &TuiDisplayState::project_from(&state)))
        .expect("draw must succeed");
}

/// Verifies that when InputFocus::Ask is active the input row shows the "[ask]" prefix
/// next to the caret instead of in the status bar.
///
/// After rendering with ask focus, some row within the main content area must contain
/// "[ask]" (the input-row prefix), and the status bar row must not contain it.
#[test]
fn render_input_shows_ask_prefix_when_ask_focused() {
    use augur_tui::domain::string_newtypes::EndpointName;
    use augur_tui::domain::tui_state::{AppScreen, AppState, AskPanelState, InputFocus};
    use augur_tui::tui::render::render_with_overlays;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal must be created");
    let ep = EndpointName::new("test");
    let mut state = AppState::new(ep, AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.input_focus = InputFocus::Ask;
    terminal
        .draw(|frame| render_with_overlays(frame, &TuiDisplayState::project_from(&state)))
        .expect("draw must succeed");
    let buf = terminal.backend().buffer();
    let row_texts: Vec<String> = (0..24u16)
        .map(|y| {
            (0..80u16)
                .map(|x| {
                    buf.cell((x, y))
                        .map(|c| c.symbol().to_owned())
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect();
    // [ask] must appear somewhere in the non-controls rows (0..23)
    let ask_in_content = row_texts[..23].iter().any(|row| row.contains("[ask]"));
    // Status bar is at y=21 in an 80x24 chat layout (0 hints, 1 input row)
    let ask_in_status = row_texts
        .get(21)
        .map(|r| r.contains("[ask]"))
        .unwrap_or(false);
    assert!(
        ask_in_content,
        "[ask] must appear in a content row; rows: {row_texts:?}"
    );
    assert!(
        !ask_in_status,
        "[ask] must not appear in the status bar row; status: {:?}",
        row_texts.get(21)
    );
}

/// Verifies that the status bar does not show an [ask] prefix even when ask panel is focused.
///
/// Moving [ask] to the input-row caret means the status bar must always show only
/// the file-path and token-count content regardless of input focus.
#[test]
fn render_status_bar_omits_ask_prefix_when_ask_focused() {
    use augur_tui::domain::string_newtypes::EndpointName;
    use augur_tui::domain::tui_state::{AppScreen, AppState, AskPanelState, InputFocus};
    use augur_tui::tui::render::render_with_overlays;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal must be created");
    let ep = EndpointName::new("test");
    let mut state = AppState::new(ep, AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.input_focus = InputFocus::Ask;
    terminal
        .draw(|frame| render_with_overlays(frame, &TuiDisplayState::project_from(&state)))
        .expect("draw must succeed");
    let buf = terminal.backend().buffer();
    // Status row is y=21 (input=y19, sep=y20, status=y21 in 80x24 layout with 0 hints, 1 input row)
    let status_row: String = (0..80u16)
        .map(|x| {
            buf.cell((x, 21))
                .map(|c| c.symbol().to_owned())
                .unwrap_or_default()
        })
        .collect();
    assert!(
        !status_row.contains("[ask]"),
        "status bar must not show [ask]; got: {status_row:?}"
    );
}

/// Verifies that the `/model` picker scrolls to keep the selected model visible
/// after navigation moves beyond the initially visible hint window, and scrolls
/// back up when selection returns near the top.
#[test]
fn render_model_picker_scrolls_to_keep_selected_item_visible() {
    use augur_tui::actors::tui::assistant::key_dispatch::refresh_model_hints;
    use augur_tui::domain::string_newtypes::EndpointName;
    use augur_tui::domain::tui_input::{KeyAction, apply_key};
    use augur_tui::domain::tui_state::{AppScreen, AppState};
    use augur_tui::tui::render::render_with_overlays;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut state = AppState::new(EndpointName::new("test"), AppScreen::Conversation);
    state.prompt.models.available = (0..12)
        .map(|idx| model_option(format!("model-{idx:02}"), format!("Model {idx:02}")))
        .collect();
    state.prompt.buffer = "/model ".to_owned().into();
    refresh_model_hints(&mut state);

    for _ in 0..11 {
        let _ = apply_key(&mut state, KeyAction::CompletionDown);
    }
    assert_eq!(state.prompt.completions.model_picker.selected, Some(11));

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal must be created");
    terminal
        .draw(|frame| render_with_overlays(frame, &TuiDisplayState::project_from(&state)))
        .expect("draw must succeed");
    let buf = terminal.backend().buffer();
    let down_rows: Vec<String> = (0..24u16)
        .map(|y| {
            (0..80u16)
                .map(|x| {
                    buf.cell((x, y))
                        .map(|c| c.symbol().to_owned())
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect();
    let down_rendered = down_rows.join("\n");

    assert!(
        down_rendered.contains("Model 10"),
        "scrolling down must keep the selected model visible; rows: {down_rows:?}"
    );
    assert!(
        !down_rendered.contains("Auto"),
        "scrolling down past the first window must move the top rows out of view; rows: {down_rows:?}"
    );

    for _ in 0..10 {
        let _ = apply_key(&mut state, KeyAction::CompletionUp);
    }
    assert_eq!(state.prompt.completions.model_picker.selected, Some(1));

    terminal
        .draw(|frame| render_with_overlays(frame, &TuiDisplayState::project_from(&state)))
        .expect("draw must succeed");
    let buf = terminal.backend().buffer();
    let up_rows: Vec<String> = (0..24u16)
        .map(|y| {
            (0..80u16)
                .map(|x| {
                    buf.cell((x, y))
                        .map(|c| c.symbol().to_owned())
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect();
    let up_rendered = up_rows.join("\n");

    assert!(
        up_rendered.contains("Model 00"),
        "scrolling back up must bring the newly selected upper item back into view; rows: {up_rows:?}"
    );
    assert!(
        !up_rendered.contains("Model 10"),
        "scrolling back up must move lower-window items out of view again; rows: {up_rows:?}"
    );
}

// ---------------------------------------------------------------------------
// Phase 4: shell dispatch tests
// ---------------------------------------------------------------------------

/// Verifies that render dispatches to the session selector screen without panicking.
///
/// When the AppScreen is SessionSelector the shell must route to
/// render_session_selector. The draw must succeed without a panic.
#[test]
fn render_shell_dispatches_to_session_selector() {
    use augur_tui::domain::string_newtypes::EndpointName;
    use augur_tui::domain::tui_state::{AppScreen, AppState, PickerState};
    use augur_tui::tui::render::render_with_overlays;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal must be created");
    let mut state = AppState::new(EndpointName::new("test"), AppScreen::Conversation);
    state.interaction.screen = AppScreen::SessionSelector(PickerState {
        sessions: vec![],
        selected: Count::new(0),
    });
    terminal
        .draw(|frame| render_with_overlays(frame, &TuiDisplayState::project_from(&state)))
        .expect("draw must succeed for session selector");
}

/// Verifies that render dispatches to the conversation screen without panicking.
///
/// When the AppScreen is Conversation the shell must route to render_conversation.
/// The draw must succeed without a panic.
#[test]
fn render_shell_dispatches_to_conversation() {
    use augur_tui::domain::string_newtypes::EndpointName;
    use augur_tui::domain::tui_state::{AppScreen, AppState};
    use augur_tui::tui::render::render_with_overlays;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal must be created");
    let state = AppState::new(EndpointName::new("test"), AppScreen::Conversation);
    terminal
        .draw(|frame| render_with_overlays(frame, &TuiDisplayState::project_from(&state)))
        .expect("draw must succeed for conversation");
}
