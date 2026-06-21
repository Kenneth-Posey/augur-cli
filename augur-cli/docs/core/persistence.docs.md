# Persistence Module

The `persistence` module provides session and plan-persistence infrastructure for saving and loading application state to disk. It re-exports domain types from `augur_domain::persistence::types` and layers concrete async I/O on top of them.

## Submodules

**`store`** implements synchronous disk I/O for session data: it writes structured session files and reads them back on resume, with atomic save semantics to prevent partial-write corruption. **`handle`** provides `PersistenceHandle`, an async handle that the agent actor uses to auto-save after each completed turn, wrapping the synchronous store behind a Tokio blocking task. **`plan_persistence`** extends the persistence layer to plan-related data, including `PlanPersistenceError` and `StepArtifactRow` for saving and loading individual step artifacts.

## Architectural Role

Persistence is the bridge between runtime state and durable storage. Every actor that needs to survive a restart--the session actor for conversation history, the supervisor for plan-tree checkpoints, the agent for auto-save after each turn--relies on this module. By separating the synchronous store (raw disk I/O) from the async handle (actor-safe mailbox interface), the module keeps blocking operations off the async runtime while providing a clean API for actor consumers. The `lib.rs` comment at the module level notes that there is no direct `.tests.rs` mirror because behavior is validated through child-module tests and higher-level integration tests.