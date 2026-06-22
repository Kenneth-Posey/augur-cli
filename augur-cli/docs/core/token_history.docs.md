# Token History Module

The `token_history` module (`token_history.rs`) manages project-level token usage state that persists across all sessions. It reads and writes a `ProjectSettings` struct to `state/token-history.json` in the working directory, tracking cumulative token totals for chat and review flows.

## Public API

**`ProjectSettings`** is the root struct, containing a `token_totals` field of type `ProjectTokenTotals` (imported from `augur-domain`). It derives `Serialize` and `Deserialize` with `#[serde(default)]` on addable fields so that future extensions remain backward-compatible. The module provides five free functions: **`token_history_path`** returns the canonical file path (always `./state/token-history.json`), **`load_or_create`** reads settings from disk or returns defaults when the file is absent, **`ensure_initialized`** creates a default file if one does not exist, **`save`** writes settings using an atomic temp-file rename to prevent partial-write corruption, and a private **`create_parent_dirs`** helper ensures the `state/` directory exists before writing.

## Architectural Role

Token history is the single source of truth for cumulative token usage across the application's lifetime. Unlike per-session token tracking (which the `token_tracker` actor handles in memory), this module persists totals to disk so that budget-aware agents and supervisors can make decisions based on long-term consumption. The atomic save pattern (`write to .tmp, then rename`) guarantees that a crash during save never corrupts the history file--consumers always see either the previous complete state or the new complete state, never a partial write.