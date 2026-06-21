//! Command types for the TUI ask panel actor.

use crate::domain::tui_state::OutputLine;

/// Commands accepted by the TUI ask panel actor.
///
/// `Open` and `Close` toggle the panel's visible state. `SeedHistory`,
/// `AppendLine`, `Scroll`, and `SetThinking` mutate the inner
/// `AskPanelState` when the panel is open. `Shutdown` stops the actor.
pub enum AskPanelCmd {
    /// Open the ask panel, initialising its state to the default if not already open.
    Open,
    /// Close the ask panel, clearing its state.
    Close,
    /// Inject a snapshot of main-conversation history lines into the ask panel output.
    SeedHistory(Vec<OutputLine>),
    /// Append a single display line to the ask panel output.
    AppendLine(OutputLine),
    /// Scroll the ask panel by `delta` lines (positive = down, negative = up).
    ///
    /// Clamps at zero; no maximum limit is enforced by the actor.
    Scroll(i64),
    /// Set the thinking indicator. `true` while the ask actor is processing a turn.
    SetThinking(bool),
    /// Stop the actor task.
    Shutdown,
}
