# Project Directory Structure

## Workspace Crates

This is a Cargo workspace. All production Rust code lives under `crates/`.

- **`Cargo.toml`** — workspace root manifest (defines workspace members, shared metadata)

### `crates/augur-app/`
CLI entrypoint, composition root, and wiring surface.

- `crates/augur-app/src/main.rs` — binary entrypoint
- `crates/augur-app/src/lib.rs` — crate exports and lib facade
- `crates/augur-app/src/actors/` — app-level actor implementations
- `crates/augur-app/src/wiring/` — composition root modules
  - `wiring/app_runtime.rs`, `chat_provider.rs`, `domain.rs`, `infrastructure.rs`, `lifecycle.rs`, `mod.rs`, `supervisor.rs`, `task_runner.rs`, `tui_wiring.rs`

### `crates/augur-core/`
Core domain logic, actors, configuration, persistence, and tools.

- `crates/augur-core/src/lib.rs` — crate root, re-exports
- `crates/augur-core/src/macros.rs` — crate-level macros
- `crates/augur-core/src/actors/` — actor implementations
  - `active_model`, `agent`, `ask`, `cache`, `catalog_manager`, `command`, `deterministic_orchestrator`,
    `file_read`, `file_scanner`, `guided_plan`, `history_adapter`, `llm_feed_consumer`, `logger`,
    `lsp`, `mod.rs`, `orchestrator`, `session`, `supervisor`, `token_tracker`, `tool`,
    `user_message_consumer`
- `crates/augur-core/src/config/` — configuration schema and loading
  - `program_settings.rs` — program-level config defaults and loaders
  - `program_settings.yml` — editable program-level defaults
  - `user_settings.rs` — user settings persistence
  - `user-settings.yaml` — user settings file
  - `loader.rs`, `provider_catalog.rs`, `endpoint_catalog_discovery.rs`, `write_section.rs`
- `crates/augur-core/src/domain/` — shared domain types, semantic wrappers, and invariants
  - `deterministic_orchestrator.rs`, `deterministic_orchestrator_ops.rs`, `support/`, `tests/`
- `crates/augur-core/src/helpers/` — test fakes and test helpers
  - `fake_ask.rs`, `fake_catalog_manager.rs`, `fake_history_adapter.rs`, `fake_llm.rs`,
    `fake_logger.rs`, `fake_orchestrator.rs`, `fake_token_tracker.rs`, `fake_tool.rs`,
    `fake_user_message_consumer.rs`, `mod.rs`
- `crates/augur-core/src/persistence/` — persistence handles, store, and persisted types
  - `handle.rs`, `mod.rs`, `plan_persistence.rs`, `store.rs`
- `crates/augur-core/src/plan_store/` — plan storage logic
- `crates/augur-core/src/token_history.rs` — token history loading

### `crates/augur-domain/`
Domain-only crate with actors, config, domain entities, persistence, and tools.

- `crates/augur-domain/src/lib.rs` — crate root
- `crates/augur-domain/src/actors/` — domain-level actors
- `crates/augur-domain/src/config/` — domain-level config
- `crates/augur-domain/src/domain/` — domain entities
- `crates/augur-domain/src/persistence/` — domain persistence
- `crates/augur-domain/src/tools/` — domain tool definitions

### `crates/augur-tui/`
Terminal UI layout, queries, picker, rendering, and actors.

- `crates/augur-tui/src/lib.rs` — crate root
- `crates/augur-tui/src/actors/` — TUI-specific actors
- `crates/augur-tui/src/domain/` — TUI domain types
- `crates/augur-tui/src/tui/` — TUI layout, rendering, and event handling

### Provider crates

- `crates/augur-provider-anthropic/` — Anthropic API provider
- `crates/augur-provider-copilot-sdk/` — Copilot SDK provider
- `crates/augur-provider-ollama/` — Ollama provider
- `crates/augur-provider-openai/` — OpenAI provider
- `crates/augur-provider-openrouter/` — OpenRouter provider (includes routing, task actors)
- `crates/augur-provider-shared/` — shared provider types, request context, retry, streaming logic

### Other crates

- `crates/augur-integration-tests/` — standalone integration/smoke test crate
- `crates/augur-graph-builder/` — documentation graph builder for static site generation

## Test Tree

Tests live **per-crate** under each crate's `tests/` directory (e.g., `crates/augur-core/tests/`).
There is no root-level `tests/` directory.

### Per-crate test layout (mirrors `src/`)

Each workspace crate with source code has a `tests/` directory:

- `crates/augur-core/tests/` — core crate tests
  - `actors/`, `config/`, `domain/`, `persistence/`, `plan_store/`, `tools/` — mirrored module coverage
  - `macros.tests.rs`, `token_history.tests.rs` — standalone module-level test files
  - `compile_fail/` — compile-fail tests specific to core
- `crates/augur-domain/tests/` — domain crate tests
- `crates/augur-tui/tests/` — TUI crate tests
- `crates/augur-app/tests/` — app crate tests
- `crates/augur-integration-tests/` — integration test crate (has its own `tests/`)
- `crates/augur-provider-*/tests/` — per-provider test directories

### Test naming patterns

- The dominant convention is `.tests.rs` suffix (e.g., `macros.tests.rs`, `token_history.tests.rs`).
- Mirrored module directories use subdirectory names matching `src/` structure.
- Not every test file uses the `.tests.rs` suffix or a 1:1 mirror path; verify the existing
  pattern in the target crate before adding a file.

## Documentation

- `docs/README.md` — documentation index
- `docs/structure.md` — source tree and module placement
- `docs/INSTALL.md` — installation instructions
- `docs/core/` — per-module docs for the `augur-core` crate
  - `actors.docs.md`, `config.docs.md`, `crate-overview.docs.md`, `domain.docs.md`,
    `helpers.docs.md`, `macros.docs.md`, `persistence.docs.md`, `plan_store.docs.md`,
    `README.md`, `token_history.docs.md`, `tools.docs.md`
- `docs/app/` — docs for the `augur-app` crate
- `docs/tui/` — docs for the `augur-tui` crate
- `docs/provider-anthropic/`, `docs/provider-copilot-sdk/`, `docs/provider-ollama/`,
  `docs/provider-openai/`, `docs/provider-openrouter/`, `docs/provider-shared/` —
  per-provider documentation

## Changes and Tracking

- `changelogs/` — changelog entries (one per change)
  - Files: `MM-DD-YYYY-HHMM-<slug>.md`
  - Sections: Summary, Issues Resolved, Root Causes, Solutions, Files Changed, Status

## Planning

- `plans/` — canonical planning root
  - Use `plans/<feature-slug>/...` for feature planning artifacts
  - `plans-ecosystem/` may contain legacy/template planning material; use it only when a task explicitly targets that path
  - For plan-writing standards, use [`0-global-plan-implementation`](../skills/0-global-plan-implementation/SKILL.md)

## Configuration

- `Cargo.toml` — workspace manifest
- `Cargo.lock` — dependency lockfile
- `configs/application.yaml` — application configuration
- `configs/application.secrets.yaml` — **actual secrets with SDK keys (not published)**; excluded from publish-to-public.sh output
- `configs/application.secrets.template.yaml` — secrets template with placeholder values (published, required for `augur-cli` to build)
- `configs/providers/` — provider-specific configuration
- `crates/augur-core/src/config/program_settings.yml` — program-level excluded directory defaults
- `crates/augur-core/src/config/user-settings.yaml` — user settings file
- `state/token-history.json` — token history data
- `state/orchestrator-state.db` — orchestration state database
- `.github/plan_execution.yml` — pipeline execution contract (base template, shipped with bundle)
- `.github/local/plan_execution.yml` — per-repo plan execution contract (generated by init-local)
- `launch-dev.sh` — repo-local run helper script (development config)
- `launch-release.sh` — repo-local run helper script (installed ~/.augur-cli/ config)
- `.github/` — GitHub customization and tooling
  - `.github/AGENTS.md` — agent behavior guidelines
  - `.github/copilot-instructions.md` — baseline CLI instructions
  - `.github/routing.md` — centralized agent-routing guidance
  - `.github/instructions/` — path-specific instruction files
  - `.github/skills/` — on-demand knowledge skills
  - `.github/agents/` — custom agents
  - `.github/prompts/` — workflow prompts
  - `.github/local/` — this file and project-specific metadata

## Logs and Research Snapshots

- `logs/` — runtime/session log output
  - Timestamped `<unix_seconds>_msg.jsonl` session logs
  - Timestamped `<unix_seconds>_app.log` tracing output
  - Timestamped `<unix_seconds>_tui.log` TUI-specific tracing output
- No verified `logs/research/` subdirectory is currently present in this repo snapshot
  - If a workflow needs a persisted `codebase-probe` snapshot, choose a path in an existing directory and verify it before writing
  - Do not assume a canonical committed `research-snapshot.json` path exists in this repository
  - Do not commit ad hoc research snapshots unless an explicit reproducibility baseline is required

## Other Root-Level Artifacts

- `scripts/` — utility scripts
- `reports/` — generated report artifacts
- `sessions/` — session data (gitignored)
- `public-html/` and `public-html-temp/` — static site generation output
- `README.md` — project overview
- `to-do-items.md` — outstanding work items
- `install.sh` — installation script
- `cargo-build-quiet.sh`, `cargo-test-quiet.sh` — quiet build/test wrappers
- `html-build-site.sh`, `html-serve-site.sh`, `publish-to-public.sh` — site generation/publishing scripts

## Critical Rules

- **Never hallucinate paths** — always verify against this list
- **Use repository-root-relative paths** — prefix paths with the repository root, e.g., `$REPO_ROOT/...`
- **Mirror test layout when the target area already does so** — prefer `<crate>/tests/<src-path>.tests.rs`, but preserve established standalone harness files where the repo already uses them
- **Changelog every change** — every feature or fix gets a dated entry in `changelogs/`
- **Use canonical planning paths** — default to `plans/<feature-slug>/...` unless a task explicitly requires a different verified planning location
