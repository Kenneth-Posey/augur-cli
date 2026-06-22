# Source Tree and Module Placement

This document describes the source tree layout for the augur-cli workspace.
For crate-level responsibilities, see [`README.md`](README.md).

## Workspace Root

```
Cargo.toml                  Workspace manifest, member crate list
crates/                     All member crates (including augur-integration-tests/)
docs/                       Module-level documentation subdirectories
changelogs/                 Changelog entries per change
plans/                      Feature planning artifacts
state/                      Runtime state artifacts and schemas
```

## Per-Crate Source Layout

Each crate follows standard Cargo conventions with `src/` as the source root,
`src/lib.rs` as the library entrypoint, and (for `augur-app`) `src/main.rs` as
the binary entrypoint.

### augur-app

Entrypoint and composition. Contains the CLI argument parser, logging setup,
actor wiring, and the `tokio::main` async entrypoint.

### augur-core

The largest crate. Source is organized by actor domain:

- `src/actors/` -- Actor implementations for each runtime responsibility
  (agent, LLM, tool execution, session, file scanning, caching, logging,
  guided planning, supervision, orchestration, etc.)
- `src/config/` -- Configuration schema, loading, program settings
- `src/domain/` -- (when present) Crate-local domain helpers
- `src/persistence/` -- Persistence handles and store implementations
- `src/plan_store/` -- Plan storage logic
- `src/tools/` -- Tool definitions, ports, handlers, registry
- `src/token_history.rs` -- Token history loading

### augur-domain

Shared domain types consumed by all other crates. Contains no runtime actors.
Key areas:

- Domain types, newtypes, and traits
- Events and event protocols
- Plan tree and state types
- Tool definitions, execution contracts, and registry
- Context management and agent spec parsing
- DAG validation, effort levels, stream state
- Channels, feeds, data structures, background event types

### augur-tui

Terminal UI crate:

- Actor-based TUI event loop
- Ratatui rendering and layout engines
- Assistant panels (ask, agent, chat menu, dynamic controls, main feed,
  spinner)
- TUI state management and input domain models
- Key dispatch

### Provider Crates

Each provider crate (`augur-provider-*`) has a consistent internal structure:

- `src/` -- Provider-specific actor(s), API client, and wire-protocol types
- `src/lib.rs` -- Crate exports and top-level re-exports

## Test Layout

- `crates/augur-integration-tests/tests/` holds cross-crate integration tests.
- Per-crate test modules are co-located in `src/` as `#[cfg(test)] mod tests`
  blocks or in `tests/` subdirectories mirroring the source structure.
- Integration-level harness files (e.g., `crates/augur-integration-tests/tests/integration_full_turn.tests.rs`) live
  in the integration test crate alongside the per-crate test trees.

## Documentation Layout

Each workspace crate has a corresponding subdirectory under `docs/`:

- `docs/app/` -- augur-app documentation
- `docs/core/` -- augur-core documentation
- `docs/domain/` -- augur-domain documentation
- `docs/tui/` -- augur-tui documentation
- `docs/provider-anthropic/` -- Anthropic provider documentation
- `docs/provider-copilot-sdk/` -- Copilot SDK provider documentation
- `docs/provider-ollama/` -- Ollama provider documentation
- `docs/provider-openai/` -- OpenAI provider documentation
- `docs/provider-openrouter/` -- OpenRouter provider documentation
- `docs/provider-shared/` -- Shared provider utilities documentation

Each subdirectory contains `.docs.md` files that describe the internal
architecture, key types, data flow, and design decisions for that crate.