---
name: 0-utility-session-orchestrator
description: >
  Deterministic session orchestrator: advances through an approved multi-phase
  plan using stored state, explicit stop signals, and specialized roles.
  Provides one SQLite-backed CLI tool (orch-query) for all state reads and
  writes. Use when reading or updating session state, advancing a plan phase,
  or determining which role should run next.
---

# Session Orchestrator

All state changes must go through `orch-query`; no raw SQL or ad hoc decision logic is permitted.

## Orchestration Tool

**Database location**: `state/orchestrator-state.db` under the repo root (default; override with `--db <path>`). Missing parent directories are created automatically before the database is opened.

**Schema**: defined in `orchestrator-state.db.schema` at the repo root.

**Commands**:

| Command | Purpose |
|---|---|
| `start-session --plan-id <id> --phase <phase>` | Start a new orchestration session |
| `status [--session-id <id>]` | Print full session status (defaults to active session) |
| `advance-phase --session-id <id> --completed-phase <p> --next-phase <p> --outcome <pass\|fail\|skipped> [--notes <txt>]` | Record phase outcome and advance |
| `record-signal --session-id <id> --signal-kind <kind> --source <source> [--detail <txt>]` | Persist an orchestration signal |
| `resolve-decision --decision-id <id> --resolution <txt>` | Mark a pending decision as resolved |
| `stop-session --session-id <id> --reason <txt>` | Stop the session with an explicit reason |
| `complete-session --session-id <id>` | Mark the session as completed (all phases passed) |

## Key Files

- `README.md` - overview and usage notes

## Hard Stop Conditions

Halt immediately when any condition below occurs. Record the mapped
`SignalKind`, then take the required action.

| Condition | Signal Kind | Required Action |
|---|---|---|
| A phase emits `Fail` outcome | `fail` | Record phase log with `fail`, call `stop-session` with reason |
| A `stop` signal is recorded | `stop` | Call `stop-session` with the signal detail as reason |
| A dependency-direction violation is detected by `module-graph` | `fail` | Record as a fail signal, stop the session |
| A role explicitly refuses to proceed | `stop` | Record the refusal as the stop reason |

### Signal Kind Taxonomy

Every state transition must map to one of these three values in the `signals`
table:

| Signal Kind | Label | Meaning |
|---|---|---|
| Proceed | `proceed` | Current phase completed successfully; advance to the next phase |
| Stop | `stop` | Explicit stop requested; halt the session with a reason |
| Fail | `fail` | Phase or role failure; halt and record for review |

### Session Lifecycle

```
active  →  (proceed signals advance through phases)
active  →  completed  (all phases pass)
active  →  stopped    (explicit stop or fail signal)
```

A session that is `stopped` or `completed` cannot be advanced. A new session
must be started to retry from a known-good phase.

## The Decision Loop

Use `orch-query` state to choose the next action. Follow this loop exactly:

```
1. query_status (orch-query status)
2. If pending_decisions is non-empty → HALT. Do not advance.
   Prompt the human to run: orch-query resolve-decision --decision-id <id> --resolution <txt>
3. If session.status == stopped or completed → HALT. Report final state.
4. Identify the current phase from session.progress.current_phase.
5. Delegate the phase to the appropriate role (see the matching skill).
6. On role success → record-signal proceed → advance-phase → loop to step 1.
7. On role failure → record-signal fail → stop-session.
```

Do not replace stored signals with prose judgment. End every branch through
`orch-query`.

## Deterministic Signal Sources

| Source | Signal Produced | Condition |
|---|---|---|
| `design-orchestrator` | `proceed` | Stage 1 completes with checkpoint-ready outputs |
| `design-orchestrator` | `fail` | Stage 1 fails hard-stop conditions or exhausts retries |
| `plan-orchestrator` | `proceed` | Stage 2 completes with checkpoint-ready outputs |
| `plan-orchestrator` | `fail` | Stage 2 fails hard-stop conditions or exhausts retries |
| `implement-orchestrator` | `proceed` | Stage 3 completes with checkpoint-ready outputs |
| `implement-orchestrator` | `fail` | Stage 3 fails hard-stop conditions or exhausts retries |
| `review-orchestrator` | `proceed` | Stage 4 consolidator decision is pass and checkpoint is complete |
| `review-orchestrator` | `fail` | Stage 4 consolidator decision is fail or retries are exhausted |
| `global-git-operator` | `proceed` | Commit created successfully |
| `global-git-operator` | `stop` | Authorization missing or changelog absent |
| `global-writer-changelog` | `proceed` | Required stage changelog artifact written successfully |
| `global-writer-changelog` | `fail` | Required stage changelog artifact missing or write failed |
| Human (via `resolve-decision`) | `proceed` | Decision answered; orchestrator resumes |

## Non-Goals

1. No direct code writing by the orchestrator.
2. No branch switching or merge automation.
3. No replacement for plan approval or human decision points.

## External Tools

This skill uses the following external tool:

- [`0-external-orch-query`](../0-external-orch-query/SKILL.md) - Start orchestration sessions, advance phases, record signals, resolve decisions, and query session status
