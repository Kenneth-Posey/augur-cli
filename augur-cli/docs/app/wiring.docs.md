# wiring

## Scope

The `wiring` module (`crates/augur-app/src/wiring/mod.rs`) is the composition root
of the `augur-app` crate. It owns the actor-graph construction, dependency ordering,
and lifecycle management for every runtime component in the process. All handles,
channels, and spawn logic are centralised here; no other module in the crate
spawns infrastructure or domain actors.

## Key Components

The module is divided into seven sub-modules:

- **`infrastructure`** - Spawns the lowest-level service actors: LLM client,
  file-read actor, cache actor, tool (registry) actor, logger, token tracker,
  history adapter, LSP actor, and the OpenRouter orchestrator. Builds the built-in
  tool registry with all filesystem, LSP, spawn-agent, and query-user tools. This
  is where `spawn_core_runtime` and `build_registry` live, and it is the first
  layer wired at startup.

- **`domain`** - Spawns the domain-layer actors (agent, session, ask-agent,
  deterministic orchestrator) and the planning actors (file scanner, guided plan).
  These actors depend on the infrastructure handles from `CoreRuntime` and
  communicate through channels established by the wiring layer.

- **`app_runtime`** - Orchestrates the assembly of the full application runtime.
  Spawns the non-UI actors (domain, supervisor, chat, planning), wires the
  auto-message bridge from the deterministic orchestrator to the LLM for
  hands-free pipeline continuation, and then finalises by spawning the TUI
  actor. Returns the complete `RunRuntime` bundle.

- **`chat_provider`** - Implements the `ChatProvider` trait that the TUI uses to
  communicate with the agent. The `EndpointRoutingChatProvider` routes submit,
  interrupt, restore, compact, and background-task commands through the agent
  handle, session handle, and OpenRouter orchestrator. Also handles saved-model
  restoration on startup.

- **`supervisor`** - Optionally spawns the executor and supervisor actors for
  plan-driven execution. The supervisor holds the `PlanTreeStore` and drives
  the executor through plan steps. Wired only when the Copilot/executor feature
  is active.

- **`task_runner`** - Defines `OpenRouterTaskRunner`, a concrete
  `BackgroundTaskRunnerPort` that dispatches background agent tasks through the
  OpenRouter orchestrator for non-Copilot endpoints. Also provides the hybrid
  intent-action routing adapter (`TaskRunner`) that builds and submits execution
  plans through orchestrator ingestion.

- **`tui_wiring`** - Assembles the TUI actor dependencies from the handles and
  channels produced by the other wiring sub-modules. Spawns the TUI sub-actors
  (main feed, agent panel, ask panel, chat menu, spinner, controls) and the
  feed-consumer actors (LLM feed, user message), then bridges decoded feed
  events to the TUI panels.

- **`lifecycle`** - Owns `shutdown_runtime` and `await_runtime`. Shutdown
  proceeds in reverse dependency order: UI layer first, then domain layer, then
  infrastructure layer last. The LSP actor receives a `kill()` signal before
  the join handle is awaited to prevent orphaned rust-analyzer processes.

The module also re-exports key public symbols from its sub-modules (`build_registry`,
`spawn_core_runtime`, `shutdown_runtime`, `await_runtime`, etc.) and provides a
family of test-visible runtime bundles (`SpawnedAppActors`, `SpawnedDomainActors`,
`ActorRuntime<H>`, etc.) that integration tests use to construct a partial wiring
graph without launching the full application.

## Role in the Ecosystem

The `wiring` module is the architectural centrepiece of the `augur-app` crate and
the entire application. It converts flat configuration into a directed actor graph
where each actor receives only the handles of the actors it depends on - no raw
shared state is passed. The module enforces a strict layers-upon-layers dependency
order: infrastructure (LLM, tools, observability) → domain (agent, session,
orchestrator) → UI (TUI, panels). This ordering guarantees that when the TUI
signals shutdown, every actor above it has already received its termination signal,
preventing deadlocks and orphaned tasks. The module's public surface is minimal:
the `run()` async function, plus the re-exported test helpers that integration
tests use to wire actors in isolation.