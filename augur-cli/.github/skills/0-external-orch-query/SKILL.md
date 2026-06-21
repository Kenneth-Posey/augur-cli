---
name: 0-external-orch-query
description: >
  CLI for starting orchestration sessions, recording phase outcomes and
  signals, advancing phases, and querying session status.
---

# run.sh

## Purpose

CLI for starting orchestration sessions, recording phase outcomes and signals,
advancing phases, and querying session status.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-orch-query
cargo build --release
```

## Run

```bash
.github/skills/0-external-orch-query/run.sh <subcommand> [options]
```

## Usage

Subcommands:

- `start-session --plan-id <id> --phase <phase>` - Start a new orchestration session for a plan
- `status [--session-id <id>]` - Print session status (defaults to the active session)
- `advance-phase --session-id <id> --completed-phase <p> --next-phase <p> --outcome <pass|fail|skipped> [--notes <txt>]` - Record a phase outcome and advance
- `record-signal --session-id <id> --signal-kind <kind> --source <source> [--detail <txt>]` - Persist an orchestration signal
- `resolve-decision --decision-id <id> --resolution <txt>` - Mark a pending decision as resolved
- `stop-session --session-id <id> --reason <txt>` - Stop the session with an explicit reason
- `complete-session --session-id <id>` - Mark the session as completed (all phases passed)

**Database location**: `state/orchestrator-state.db` under the repo root (default; override with `--db <path>`). Missing parent directories are created automatically before the database is opened.

**Schema**: defined in `orchestrator-state.db.schema` at the repo root.

## Examples

```bash
# Start a new orchestration session
.github/skills/0-external-orch-query/run.sh start-session \
  --plan-id "0165" \
  --phase "design-architecture"

# Query active session status
.github/skills/0-external-orch-query/run.sh status

# Record phase completion
.github/skills/0-external-orch-query/run.sh advance-phase \
  --session-id 1 \
  --completed-phase "design-architecture" \
  --next-phase "implement-core" \
  --outcome pass \
  --notes "Architecture review passed; no blocking findings"

# Record a failure signal
.github/skills/0-external-orch-query/run.sh record-signal \
  --session-id 1 \
  --signal-kind fail \
  --source code-rust-implementer \
  --detail "Tests failed after refactor; unable to resolve"

# Resolve a pending decision
.github/skills/0-external-orch-query/run.sh resolve-decision \
  --decision-id 3 \
  --resolution "Approve splitting module into domain and adapters layers"

# Stop the session
.github/skills/0-external-orch-query/run.sh stop-session \
  --session-id 1 \
  --reason "Critical compiler error; phase halted pending investigation"
```

## Key Files

- `run.sh` - Canonical wrapper for orch query
