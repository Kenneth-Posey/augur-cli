---
name: 0-external-orch-query
description: >
  CLI for starting orchestration sessions, recording phase outcomes and
  signals, advancing phases, and querying session status.
---

# orch-query

## When to use

Use this skill at every phase transition of a multi-phase orchestrated pipeline.
Start a session before the first phase, record outcomes and advance after each
phase, record signals from executors, resolve pending decisions, and stop or
complete the session when finished.

## Purpose

CLI for starting orchestration sessions, recording phase outcomes and signals,
advancing phases, and querying session status. Manages a SQLite database at a
configurable path that persists all session state across tool invocations.

## Run

```bash
.github/skills/0-external-orch-query/run.sh <subcommand> [options]
```

## Arguments

### Global Options

| Option | Description | Default |
|--------|-------------|---------|
| `--db <DB>` | Path to the SQLite database file | `state/orchestrator-state.db` |
| `-h, --help` | Print help | |

### Subcommands

**`start-session`** - Start a new orchestration session for a plan

| Option | Description | Required |
|--------|-------------|----------|
| `--plan-id <PLAN_ID>` | Plan identifier to orchestrate (e.g. "0165") | Yes |
| `--phase <PHASE>` | Name of the starting phase | Yes |
| `-h, --help` | Print help | |

**`status`** - Print the full status of a session (defaults to the active session)

| Option | Description | Required |
|--------|-------------|----------|
| `--session-id <SESSION_ID>` | Session id to query; omit to query the current active session | No |
| `-h, --help` | Print help | |

**`advance-phase`** - Record the outcome of the current phase and advance to the next

| Option | Description | Required |
|--------|-------------|----------|
| `--session-id <SESSION_ID>` | Session to advance | Yes |
| `--completed-phase <COMPLETED_PHASE>` | Name of the phase that has just completed | Yes |
| `--next-phase <NEXT_PHASE>` | Name of the phase to advance to | Yes |
| `--outcome <OUTCOME>` | Outcome of the completed phase: `pass`, `fail`, or `skipped` | Yes |
| `--notes <NOTES>` | Optional notes from the completing agent | No |
| `-h, --help` | Print help | |

**`record-signal`** - Record an orchestration signal for a session

| Option | Description | Required |
|--------|-------------|----------|
| `--session-id <SESSION_ID>` | Session this signal belongs to | Yes |
| `--signal-kind <SIGNAL_KIND>` | Signal kind: `proceed`, `stop`, `fail`, or `decision-required` | Yes |
| `--source <SOURCE>` | Agent or phase that emitted the signal | Yes |
| `--phase <PHASE>` | Name of the phase that raised this signal | Yes |
| `--detail <DETAIL>` | Optional detail text (required question text for `decision-required`) | No |
| `-h, --help` | Print help | |

**`resolve-decision`** - Resolve a pending decision point

| Option | Description | Required |
|--------|-------------|----------|
| `--decision-id <DECISION_ID>` | Row id of the decision point to resolve | Yes |
| `--resolution <RESOLUTION>` | Human-provided resolution text | Yes |
| `-h, --help` | Print help | |

**`stop-session`** - Stop an active session with an explicit reason

| Option | Description | Required |
|--------|-------------|----------|
| `--session-id <SESSION_ID>` | Session to stop | Yes |
| `--reason <REASON>` | Human-readable reason for stopping | Yes |
| `-h, --help` | Print help | |

**`complete-session`** - Mark a session as completed (all phases passed)

| Option | Description | Required |
|--------|-------------|----------|
| `--session-id <SESSION_ID>` | Session to mark as completed | Yes |
| `-h, --help` | Print help | |

## Output

The tool prints human-readable status and confirmation text to stdout. Error
messages are printed to stderr.

**Exit codes:**
- `0` - Success
- Non-zero - Error (invalid arguments, database errors, missing session, etc.)

**Database**: SQLite database at the path specified by `--db` (default:
`state/orchestrator-state.db` under the repository root). Missing parent
directories are created automatically before the database is opened. The
database file is created on first use if it does not exist.

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
  --source "code-rust-implementer" \
  --phase "implement-core" \
  --detail "Tests failed; unable to resolve"

# Resolve a pending decision
.github/skills/0-external-orch-query/run.sh resolve-decision \
  --decision-id 3 \
  --resolution "Approve splitting module into domain and adapters layers"

# Stop the session
.github/skills/0-external-orch-query/run.sh stop-session \
  --session-id 1 \
  --reason "Critical compiler error; phase halted pending investigation"

# Complete the session
.github/skills/0-external-orch-query/run.sh complete-session \
  --session-id 1
```

## Key Files

- `.github/skills/0-external-orch-query/run.sh` - Canonical wrapper for orch-query