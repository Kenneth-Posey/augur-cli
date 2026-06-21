# augur-core

The largest crate containing the majority of runtime actor implementations. It provides actors for the agent turn loop, LLM communication, tool execution, session management, file access, caching, command dispatch, file scanning, guided plan execution, supervision, orchestration, history adaptation, token tracking, and more. It also owns configuration loading, persistence, plan storage, macros, and token history.

## Documents

- [Crate Overview](crate-overview.docs.md) -- Architecture, major subsystems, and design decisions for the augur-core crate.
- [Actors](actors.docs.md) -- The actor runtime: 20 concurrent service actors for dispatch, filesystem access, observability, and plan orchestration.
- [Config](config.docs.md) -- Configuration loading, saving, and runtime access to program and user settings.
- [Domain](domain.docs.md) -- Core-owned domain contracts for the deterministic orchestrator: workflow documents, step execution modes, and failure routing.
- [Helpers](helpers.docs.md) -- Fake actor implementations for deterministic testing of agent, LLM, tool, and other actor interactions.
- [Macros](macros.docs.md) -- Utility macros for trait aliasing and poisoned-lock recovery.
- [Persistence](persistence.docs.md) -- Session and plan-persistence infrastructure: synchronous store, async handle, and plan artifact rows.
- [Plan Store](plan_store.docs.md) -- Async disk I/O for plan-tree documents: save, load, read/write step files on disk.
- [Token History](token_history.docs.md) -- Project-level token usage persistence with atomic save semantics.
- [Tools](tools.docs.md) -- Tool abstraction layer: built-in tool implementations, handler dispatch, registry, and execution helpers.