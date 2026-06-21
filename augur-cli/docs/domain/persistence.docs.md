# Persistence

The `persistence` module defines the on-disk session storage model, the async persistence handle, and all filesystem I/O operations for saving, loading, listing, and deleting session records. It contains three submodules: `types` (the session data model), `handle` (the async wrapper used by actors), and `store` (filesystem I/O functions).

## Key Components

- **`types`**: Defines the full persistence schema. `SessionRecord` pairs a `SessionMeta` (id, creation/update timestamps, endpoint name, `SessionMetaFlags`) with a `SessionState` (message records in chronological order, optional OpenRouter context history snapshot, optional strategy tree). `SessionMetaFlags` tracks the Copilot SDK session id and whether the session was spawned from the ask panel. `SessionSummary` provides a compact projection for session listing, built by the `summarize` function. The module also defines the `StrategyTree` hierarchy (`StrategyNode`, `StrategyNodeKind::Branch/Leaf`, `NodeMeta`) for persisting guided strategy trees alongside conversation state.

- **`handle`**: Provides `PersistenceHandle`, a `Clone`-able async-safe handle that wraps an `Arc<Mutex<PersistenceInner>>`. It manages session identity (UUID, creation timestamp, SDK session id), maintains a queue of uncommitted user commands that are merged into the next save, and exposes methods for session lifecycle: `save_turn` (asynchronously writes a complete session record via `spawn_blocking`), `reset_to_new_session` (generates a fresh UUID), `restore_from` (loads state from an existing record), and OpenRouter context history management. The `SessionIdentity` struct is built via the `bon::Builder` derive macro.

- **`store`**: Provides all filesystem I/O functions. `save_session` writes atomically via a `.tmp` rename pattern. `load_session` and `delete_session` read or remove individual session JSON files. `list_sessions` returns up to 20 recent session summaries, sorted by most recent update. The module also includes Git repository detection utilities (`detect_git_repo_name`, `apply_repo_subdir`, `extract_repo_name_from_git_config`) that organize session files into per-repository subdirectories, and `resolve_sessions_dir` for handling `~`-prefixed session directory paths.

## Role in the Ecosystem

This module defines the contract between in-memory conversation state and durable storage. Every actor that persists sessions - the agent actor, the Copilot chat actor, and the executor actor - depends on `PersistenceHandle` for async-safe writes and on the `SessionRecord`/`SessionSummary` types for the on-disk format. The session listing functions are consumed by the TUI session picker, and the strategy tree types support the guided plan persistence path used by the supervisor actor.