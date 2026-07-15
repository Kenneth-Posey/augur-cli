---
name: Prompts Guide
description: >
  Catalog of all .github/prompts/ files, their trigger phrases, purpose, and
  relationships. Equivalent in design and purpose to routing.md for the prompt
  collection: a reference guide for awareness and correct selection.
---

# Prompts Guide

This file catalogs every prompt under `.github/prompts/`. Prompts are
workflow-entrypoint commands that define a repeatable task flow for the main
conversation context. Use this guide to recognize which prompt a user request
maps to, and to understand how prompts relate to each other.

## How to Use This Guide

- **User says a trigger phrase** - look up the trigger column below and read
  the prompt's purpose and workflow summary to confirm the match.
- **Planning a multi-step session** - check the `Related` column to ensure
  dependent or follow-up prompts are available.
- **Need task awareness** - scan the table to know what capabilities exist
  without re-reading every prompt file.

---

## Prompt Catalog

| # | Prompt | Trigger Phrases | Purpose | Workflow Summary | Related |
|---|--------|----------------|---------|------------------|---------|
| 1 | `add-actor` | "add actor", "create actor", "new actor", "implement actor" | Create a new actor using thin-shell/functional-core pattern | Survey codebase, confirm placement, create actor shell + ops + wiring + tests (Red/Green/Refactor) | `add-domain-type`, `add-tool`, `build-plan` |
| 2 | `add-agent` | "add agent", "create agent", "new agent" | Create a custom agent profile under `.github/agents/` | Decision gate (agent vs prompt vs skill vs instruction), write agent.yml, tools, skills, step-by-step behavior, handoff | `add-prompt`, `add-skill`, `add-instructions`, `review-customization` |
| 3 | `add-domain-type` | "add domain type", "add newtype", "add semantic wrapper", "add shared type", "add domain struct" | Create a domain type (newtype/struct/enum) in `src/domain/` | Survey codebase, confirm placement, create type + domain module export + tests (Red/Green/Refactor) | `add-actor`, `add-tool` |
| 4 | `add-instructions` | "add instruction", "add rule", "create instruction", "create rule" | Add or restructure instructions in the correct `.github/` layer | Decision gate (layer selection), write to correct file with frontmatter and validation | `add-agent`, `add-prompt`, `add-skill`, `review-customization` |
| 5 | `add-prompt` | "add prompt", "create prompt", "new prompt" | Create a reusable prompt command under `.github/prompts/` | Decision gate (prompt vs agent vs skill vs instruction), write prompt file with workflow, inputs, and output contract | `add-agent`, `add-skill`, `add-instructions`, `review-customization` |
| 6 | `add-skill` | "add skill", "create skill", "new skill" | Create a skill directory and `SKILL.md` under `.github/skills/` | Decision gate (skill vs prompt vs agent vs instruction), write skill with frontmatter and body, add supporting files only when needed | `add-agent`, `add-prompt`, `add-instructions`, `review-customization` |
| 7 | `add-tool` | "add tool", "create tool", "new tool", "implement tool handler" | Add a tool to the project's tool registry | Survey codebase, confirm placement, create handler + ops + registry update + tests (Red/Green/Refactor) | `add-actor`, `add-domain-type`, `build-plan` |
| 8 | `architecture-audit` | "architecture audit", "whole-tree audit", "run all analyzers", "analyze codebase architecture" | Run the full analyzer suite for a whole-tree architecture audit | Run 12 analyzers in fixed order (syn-analyzer, module-graph, arch-linter, doc-extractor, test-gap-fusion, sig-report, deadcode, stubs, consolidator, rustc-dependency, actor-ops), consolidate findings | `code-audit-rust`, `standards-check` |
| 9 | `build-plan` | "create plan", "build plan", "implementation plan", "new plan", "plan this feature" | Create an implementation plan for a feature, refactor, or migration | Gather scope, apply architecture clarity gate, draft plan files, review for gaps, present to user for confirmation | `execute-plan`, `review-implementation` |
| 10 | `changelog-author` | "write changelog", "create changelog", "changelog entry" | Write a changelog entry for completed work | Get timestamp, build filename, query git for diff/commits, write sections (Summary, Issues Resolved, Root Causes, Solutions, Files Changed, Status) | `create-commit` |
| 11 | `code-audit-rust` | "rust audit", "code audit", "audit rust code", "deterministic audit" | Run a deterministic Rust code audit using repo-supported tooling | Read local guidance, run compiler/clippy/tests, run coverage-gap, syn-analyzer, stub-detector, deadcode, module-graph, arch-linter, rustc-dependency checks; separate supported from unsupported findings | `architecture-audit`, `standards-check` |
| 12 | `create-commit` | "create commit", "commit this phase", "commit completed phase", "create message and commit" | Create a phase-scoped commit through `global-git-operator` | Confirm authorization, build message, run test-doc consistency check, delegate staging/commit to `global-git-operator` | `changelog-author`, `execute-plan` |
| 13 | `execute-plan` | "execute plan phase", "run plan phase", "implement plan phase", "start phase execution" | Execute one phase of an implementation plan | Read plan root, identify phase, route to matching orchestrator (design/plan/implement/review), report result with validation | `build-plan`, `review-implementation` |
| 14 | `init-local` | "init local", "initialize local", "setup project", "initialize repo" | Initialize `.github/local/` files from current repo state | Discover identity, map source tree, establish project rules, detect language, populate language companions, initialize plan execution contract, link from core files | None (one-time setup) |
| 15 | `pr-description` | "pr description", "pull request", "write pr", "create pr description" | Write a pull request description from commits and diffs | Determine target branch, gather git metadata from `global-git-operator`, read plan files if present, write PR description | `create-commit`, `changelog-author` |
| 16 | `review-customization` | "review customization", "review artifact", "review .github" | Review `.github/` customization artifacts against the matching `add-*` prompt | Classify artifact type, run customization-analyzer, read matching add prompt, apply semantic checks, report gate result | `add-agent`, `add-prompt`, `add-skill`, `add-instructions` |
| 17 | `review-implementation` | "review implementation", "validate plan implementation", "verify plan completion" | Review implementation against the most recent relevant plan | Read plan, build phase-by-phase checklist, verify Red/Green/Refactor sequence, file changes, removals, modular reuse, validation criteria; create follow-up plan files for gaps | `build-plan`, `execute-plan` |

| 18 | `standards-check` | "standards audit", "standards check", "run standards" | Run a standards audit using cargo-diagnostics and syn-analyzer | Run customization-analyzer for supported artifacts, delegate to `external-code-tool-analyst`, present findings grouped by remediation domain and severity | `architecture-audit`, `code-audit-rust` |

---

## Prompt Relationships (Dependency / Follow-up Map)

```
init-local (one-time setup)
  └── establishes all .github/local/ files

build-plan (planning entrypoint)
  ├── execute-plan (single-phase execution)
  │     └── create-commit (after phase passes)
  │           ├── changelog-author (if changelog needed)
  │           └── pr-description (when PR is needed)

review-implementation (post-execution review)
  └── build-plan (new follow-up plan if gaps found)

add-actor / add-domain-type / add-tool (implementation building blocks)
  └── may be preceded by build-plan

add-agent / add-prompt / add-skill / add-instructions (customization building blocks)
  └── reviewed by review-customization

architecture-audit / code-audit-rust / standards-check (analysis/audit)
  └── independent, no required follow-up
```

---

## Prompt vs Agent vs Skill vs Instruction Decision

When in doubt about whether to invoke a prompt or route to an agent, use this
guidance:

| Request Type | Best Fit | Why |
|---|---|---|
| "Add an agent/actor/domain-type/tool" | **Prompt** (add-*) | Repeatable workflow in main context with clear steps |
| "Run an audit" | **Prompt** (architecture-audit, code-audit-rust, standards-check) | Repeatable multi-tool workflow best driven from main context |
| "Create an implementation plan" | **Prompt** (build-plan) | Single workflow that waits for user confirmation |
| "Execute a plan phase" | **Prompt** (execute-plan) | Routes to orchestrator agents for the actual work |
| "Write a changelog/PR/commit" | **Prompt** (changelog-author, pr-description, create-commit) | Brief templated output, delegates git to `global-git-operator` |
| "Review .github artifacts" | **Prompt** (review-customization) | Structured evaluation against add-* standards |
| Heavy research / batch impl / long-running | **Agent** (via task_spawn) | Dedicated context, parallel execution |
| General policy/rule enforcement | **Instruction** | Loaded automatically on matching paths |
| Specialized on-demand guidance | **Skill** | Loaded explicitly when needed, not always in context |

---

## Prompts That Accept Arguments

These prompts can be invoked with an optional argument to scope the work:

| Prompt | Argument | Default |
|--------|----------|---------|
| `code-audit-rust` | Rust path, crate, or module | Full repo Rust surface |
| `architecture-audit` | Scope path within `src/` | Full `src/` tree |
| `standards-check` | File path or module | Full repo |
| `build-plan` | Task description or feature scope | (required) |
| `execute-plan` | Plan root path and phase name/number | (required) |
| `review-implementation` | Plan root path | Most recent plan root in `plans/` |
| `create-commit` | Phase scope or summary | (required context) |
| `changelog-author` | Brief change description (used in slug) | (required) |
| `pr-description` | Target branch | Copilot merge target from identity.md |

---

## Prompts That Delegate to Agents

These prompts hand off work to specific agents rather than doing it inline:

| Prompt | Delegates To | Mode |
|--------|-------------|------|
| `execute-plan` | `design-orchestrator`, `plan-orchestrator`, `implement-orchestrator`, `review-orchestrator` | Background |
| `create-commit` | `global-git-operator` | Background |
| `pr-description` | `global-git-operator` (for git metadata) | Background |
| `changelog-author` | `global-git-operator` (for diff context) | Background |
| `standards-check` | `external-code-tool-analyst` | Background |
| `init-local` | `global-git-operator` (for git queries) | Background |
| `review-customization` | customization-analyzer (run.sh) | Inline command |