# augur-cli Documentation

This is the documentation root for the augur-cli project, a multi-crate Rust
workspace that provides a terminal-based AI assistant and LLM orchestration
tool. The workspace is organized into crate groups that separate application
bootstrap, core domain logic, terminal UI rendering, and provider-specific LLM
backend integrations.

## Workspace Architecture

The application is assembled from ten crates under `crates/`. The dependency
direction flows from application entrypoint inward through the core and domain
layers, with provider crates and the TUI crate depending on both the core and
domain crates.

### Application Crate

- **`augur-app`** (`crates/augur-app/`) -- CLI entrypoint, wiring composition
  root, runtime bootstrap, and lifecycle management. Assembles all actors and
  runs the application. Documentation: [`app/`](app/), starting with
  [Crate Overview](app/crate-overview.docs.md).

### Core Domain Crates

- **`augur-core`** (`crates/augur-core/`) -- Core actor implementations covering
  agent, LLM, tool, session, logging, file access, cache, commands, file
  scanner, guided plan, supervisor, executor, history adapter, token tracker,
  user message consumer, active model, catalog manager, LSP, deterministic
  orchestrator, ask, LLM feed consumer, configuration loading, program
  settings, persistence, plan store, macros, and token history. This is the
  largest crate and contains most of the runtime logic. Documentation:
  [`core/`](core/), starting with
  [Crate Overview](core/crate-overview.docs.md).

- **`augur-domain`** (`crates/augur-domain/`) -- Domain types, traits, semantic
  newtypes, events, protocols, plan tree and state types, tool definitions,
  tool execution contracts, registry, context management, background event
  types, scheduling, agent spec parsing, DAG validation, effort levels, stream
  state, thinking mode, channels, data structures, feeds, and reply events.
  Documentation: [`domain/`](domain/), starting with
  [`domain/crate-overview.docs.md`](domain/crate-overview.docs.md).

### TUI Crate

- **`augur-tui`** (`crates/augur-tui/`) -- Terminal UI actor, Ratatui
  rendering, key dispatch, layout engines, assistant panels (ask, agent, chat
  menu, dynamic controls, main feed, spinner), TUI state management, and
  domain models for TUI input and rendering. Documentation:
  [`tui/`](tui/), starting with
  [Crate Overview](tui/crate-overview.docs.md).

### Provider Crates (LLM Backend Integrations)

- **`augur-provider-shared`** (`crates/augur-provider-shared/`) -- Shared
  provider utilities: Anthropic body construction, retry logic, SSE streaming,
  and request context. Documentation:
  [`provider-shared/`](provider-shared/), starting with
  [`provider-shared/crate-overview.docs.md`](provider-shared/crate-overview.docs.md).

- **`augur-provider-openrouter`** (`crates/augur-provider-openrouter/`) --
  OpenRouter provider with its own LLM actor, orchestrator actor, and task
  actor for routing and managing OpenRouter API calls. Documentation:
  [`provider-openrouter/`](provider-openrouter/), starting with
  [Crate Overview](provider-openrouter/crate-overview.docs.md).

- **`augur-provider-copilot-sdk`** (`crates/augur-provider-copilot-sdk/`) --
  GitHub Copilot chat SDK integration including the chat actor, executor actor,
  guided-plan hooks, background agent dispatch, and feed routing.
  Documentation: [`provider-copilot-sdk/`](provider-copilot-sdk/), starting with
  [Crate Overview](provider-copilot-sdk/crate-overview.docs.md). Uses a cloned fork of 
  the official rust repo which has some bugs that needed patching. 

- **Not completely implemented `augur-provider-anthropic`** (`crates/augur-provider-anthropic/`) --
  Anthropic Messages API streaming integration. Documentation:
  [`provider-anthropic/`](provider-anthropic/), starting with
  [Crate Overview](provider-anthropic/crate-overview.docs.md).

- **Not completely implemented `augur-provider-ollama`** (`crates/augur-provider-ollama/`) -- Local Ollama
  provider integration via an OpenAI-compatible path. Documentation:
  [`provider-ollama/`](provider-ollama/), starting with
  [Crate Overview](provider-ollama/crate-overview.docs.md).

- **Not completely implemented `augur-provider-openai`** (`crates/augur-provider-openai/`) -- OpenAI-
  compatible chat completions streaming integration. Documentation:
  [`provider-openai/`](provider-openai/), starting with
  [Crate Overview](provider-openai/crate-overview.docs.md).

## Navigation

Detailed module documentation lives in the per-module subdirectories listed
above. Each subdirectory covers its crate's internal architecture, key types,
data flow, and design decisions. For the source tree layout and file placement
conventions, see [`structure.md`](structure.md).