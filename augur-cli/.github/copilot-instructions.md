# Copilot Instructions

Agent behavior quick guide: [`.github/AGENTS.md`](AGENTS.md)
Project identity (root, build commands, branch policy): [`.github/local/identity.md`](local/identity.md)
Source tree, test layout, path rules: [`.github/local/directories.md`](local/directories.md)
Commit policy, TDD rules, definition of done: [`.github/local/rules.md`](local/rules.md)
Language-specific skill routing: [`.github/local/language-companions.md`](local/language-companions.md)
Detailed agent-routing guidance: [`.github/routing.md`](routing.md)

## Always-On Rules

- Keep tool output out of primary context. Cap all searches and `shell_exec`
  calls that could produce large output. Never run broad searches over
  `logs/`. Summarize before carrying findings forward. See
  `.github/local/rules.md` `## Tool Output and Context Discipline` for the
  full rules.
- Use `size-check` for broad `rg`/`grep`/`find`/recursive `ls` and large file
  reads when available so command scope can be filtered, paginated, or split
  before high-volume output is requested.
- Never use em dash characters; use a regular hyphen (`-`) instead.

## Orchestration Entry Guidance

- For interactive phased implementations, use
  `0-global-orchestration-pipeline`.
- In interactive pipeline runs, keep Stage 1/2 artifact-only: write to
  `plans/<feature-slug>/` (and checkpoint changelogs) and do not edit
  implementation code paths such as `src/`/`tests/` until Stage 3.
- For automated or CI Stage 4 review runs, use `review-orchestrator`; it
  launches the eleven Stage 4 checkers, including `review-activation-checker`, and hands
  the merge decision to internal-only `review-consolidator`.
- For automated or resumable orchestration, use `global-pipeline-orchestrator`
  or `global-session-resume-orchestrator` with `orch-query` state.
- Treat plan files, current repository state, and `orch-query` state as the
  source of truth. Do not invent separate local workflow graphs.

## Tooling

- Use the `skill` tool to invoke skills for specialized knowledge (architecture, standards, planning).
- Use available custom agents for specialized work. **Always launch as
  background tasks** (`mode: 'background'`). Use `mode: 'sync'` only when
  immediate output is required to choose the next step. See
  [`.github/routing.md`](routing.md) for the full routing guide and
  [`.github/agents/`](agents/) for agent specs.
- Route `.github/` customization work to `global-customization-author` and review to
  `global-customization-reviewer`; use the matching `add-*` prompts when creating or
  updating agents, skills, prompts, instructions, or tools.
- Let deterministic formatters own layout/formatting; avoid style-only edits or
  LLM/formatter ping-pong on whitespace, import grouping, or similar churn.
- Route replacement-work cutover/legacy-bypass verification to
  `review-activation-checker`.
- When dispatching an agent, use the executable agent name from the agent
  frontmatter (for example `global-pipeline-orchestrator`, `design-orchestrator`,
  `global-git-operator`), not numbered filenames or headings.
- Workflow prompts live in `.github/prompts/`.
- `global-git-operator` is the only agent allowed to run git commands. Route commit,
  push, status, diff, log, and other git work through that agent.
- Route repository changelog authoring to `global-writer-changelog`, especially
  for commit-ready changes and pipeline stage checkpoints before `global-git-operator`.
- Path-specific instruction files (`.github/instructions/*.instructions.md`) inject
  based on `applyTo` glob matching. `applyTo: "**"` injects on every request.
  Path-scoped patterns (e.g. `**/*.rs`) inject when Copilot is actively working on
  matching files in CLI or VS Code.

## Primary Context Routing

- Treat the primary context as a dispatcher and delegate suitable whole
  subtasks to background agents before loading heavy context inline.
- Use [`.github/routing.md`](routing.md) for the full agent-by-agent routing
  matrix and scenario guidance.
- If no suitable agent exists, propose a new agent before continuing with a
  large specialized task in the primary context.

## Phase 3: Implementation Stage Routing

- Use `0-global-orchestration-pipeline` as the canonical interactive entrypoint
  for end-to-end feature implementation.
- Use `global-pipeline-orchestrator` or `global-session-resume-orchestrator` only for
  automated, resumable, or otherwise non-interactive orchestration.
- Use the selected orchestration surface plus [`.github/routing.md`](routing.md)
  for stage sequencing, checkpoints, and specialized delegation. Do not restate
  or invent alternate workflow graphs here.
- For feature replacement work, Stage 4 must include `review-activation-checker`, and
  the final merge decision comes from internal-only `review-consolidator`; deferred
  wiring stays incomplete unless the scope is explicitly scaffold-only.
- For ad-hoc changes outside the phased flow, use the appropriate specialist
  agent such as `utility-code-rust-implementer`, `external-code-src-deadcode-analysis`,
  `external-code-stub-detector`, `external-code-actor-ops-detector`,
  `external-code-rustc-dependency-check`, or `utility-quick-patch-code`.
