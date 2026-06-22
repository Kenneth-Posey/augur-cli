---
description: "Use when user asks: run plan, start plan, resume plan, orchestrate plan"
name: "Run Plan"
argument-hint: "plan root path; optional: --session-id <id> to resume an existing session"
agent: "agent"
---
Start or resume a full multi-phase plan run with `global-session-resume-orchestrator`.
Establish session context, then hand execution to `global-session-resume-orchestrator`,
`orch-query`, and `0-global-orchestration-pipeline`.

Use this prompt for whole-plan orchestration. For a single phase only, use
`execute-plan`.

## Inputs

- Path to the plan root file in `plans/` (required).
- Optional `--session-id <id>` to resume an existing session.
- Current repository state (working tree must be clean before starting).

## Workflow

1. **Read the plan root file.** Follow all part-file links. Identify the plan
   id (for example, `"0165"`) and the ordered phase list.

2. **Establish session state.**
   - Without `--session-id`, start a new session with `orch-query`.
   - With `--session-id`, read the existing session status from `orch-query`.
   - Treat stored session state as authoritative; do not ask for a manual phase
     recap.

3. **Check session readiness.**
   - If pending decisions exist, report them and stop.
   - If the session is already `stopped` or `completed`, report that terminal
     status and stop.

4. **Invoke `global-session-resume-orchestrator`.**
   Pass the plan path, session id, and any reusable research snapshot that
   meets the independent-research contract. `global-session-resume-orchestrator` determines
   phase order, retries, failure routing, checkpoint flow, and session
   advancement.

5. **Report session outcome.**
   Print the resulting `orch-query` status, including phase history, final
   status, or stop reason.

## Start vs Resume Decision

| Condition | Action |
|---|---|
| No `--session-id` and no active session | Start new session with `start-session` |
| No `--session-id` but an active session exists | Resume via `status` (active session) |
| `--session-id` provided | Resume that session via `status --session-id <id>` |

Do not consult conversation history to determine the current phase. The
authoritative source is always `orch-query status`.

## Research Snapshot Integration

Before reusing any `research-snapshot.json`, check the reuse contract in
`.github/skills/0-utility-independent-research/SKILL.md`. Reuse a snapshot
only when it contains the required snapshot fields for the task. Otherwise
regenerate it before invocation.

## Output

1. Session id and status (`active`, `stopped`, or `completed`)
2. Phase history: list of phases with outcomes
3. Any pending decisions requiring human input
4. Final commit reference(s) or stop reason
