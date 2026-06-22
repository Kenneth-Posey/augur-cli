# augur-tui

This crate provides the terminal UI layer including the Ratatui-based rendering, actor-backed event loop, key dispatch, layout engines, and assistant panels for ask, agent, chat menu, dynamic controls, main feed, and spinner interactions. It also owns TUI state management and input domain models.

## Documents

- [Crate Overview](crate-overview.docs.md) -- Architecture, major subsystems, and design decisions for the augur-tui crate.
- [Actors](actors.docs.md) -- TUI actor implementations: main TUI actor and specialized panel actors (agent feed, ask panel, chat menu, dynamic controls, main feed, spinner).
- [Domain](domain.docs.md) -- TUI domain models: state machine (AppState, TuiDisplayState), input classifiers (key/mouse/query actions), render utilities, and status-bar helpers.
- [TUI Rendering](tui.docs.md) -- Rendering components, screen implementations, layout engines, and widget primitives.