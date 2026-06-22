# Agent Behavior Quick Guide

Use this file for quick agent behavior and routing rules. The full routing
matrix is in [`.github/routing.md`](routing.md).

## Dispatch Rules

- Dispatch agents by executable `name:` value from agent frontmatter.
- Treat numbered filenames and markdown headings as artifact identifiers only,
  not launch names.
- Launch delegated agents as background tasks unless you need immediate brief
  output to choose the next step.
- Route repository changelog writing only to `global-writer-changelog`. Use it
  for stage checkpoint changelogs.
- Route git status, diff, log, commit, push, and other git work only to
  `global-git-operator`.
- Route `.github/` customization authoring, updates, and removals to
  `global-customization-author`, and route review to `global-customization-reviewer`. Use the
  appropriate add/update/remove prompts for agents, skills, prompts,
  instructions, and tools.

## Routing Summary

- **Interactive feature sessions:** stay in the main conversation, read the
  `0-global-orchestration-pipeline` skill, and use it as the dispatcher. Do not
  hand interactive feature work to automation orchestrators.
- During interactive pipeline execution, enforce stage boundaries: Stage 1/2 are
  artifact-only (`plans/<feature-slug>/` + checkpoint changelogs). Do not write
  implementation code paths until Stage 3.
- For implementation and replacement work, treat deferred wiring as incomplete;
  the activation gate must be satisfied by `review-activation-checker` unless the work
  is explicitly scaffold-only.
- **Automation / CI paths:** use `global-pipeline-orchestrator` or
  `global-session-resume-orchestrator`.
- **Stage orchestrators (automation only):** use `design-orchestrator`,
  `plan-orchestrator`, `implement-orchestrator`, and `review-orchestrator`.
- **Checkpoint support:** use `global-writer-changelog` for checkpoint
  changelog artifacts and `global-git-operator` for authorized git actions when the
  orchestration surface requires them.
- **Reviewer executable names:** use `plan-domain-reviewer`,
  `implement-domain-reviewer`, `plan-function-sig-reviewer`,
  `implement-function-sig-reviewer`, `review-activation-checker`, and
  `review-completeness-checker`.
- **Stage 4 merge agent (internal only):** `review-consolidator`. Launch only through
  `review-orchestrator` or `0-global-orchestration-pipeline` Stage 4; do not
  dispatch directly from general routing surfaces.
- **Src deadcode audits:** use `external-code-src-deadcode-analysis` for read-only Rust
  `src/` symbol deadcode reporting.
- **Src stub detection:** use `external-code-stub-detector` for read-only Rust `src/`
  deferred pattern reporting (`todo!()`, `unimplemented!()`, etc.).
- **Actor delegation audits:** use `external-code-actor-ops-detector` for read-only Rust
  `actor.rs`/`actor_ops.rs` pairing and delegation-hygiene reporting.
- **Cargo-resolved dependency direction audits:** use `external-code-rustc-dependency-check`
  for read-only package-layer direction checks from `cargo metadata`.
- **Actor topology regeneration:** use `external-code-topology-extractor` (via the
  `utility-topology-extractor` agent) to regenerate `.github/local/system-actor-graph.yml`
  from current wiring code.
- **Quick-patch recovery agents:** after a reviewer Hold, use
  `utility-quick-patch-design` (design artifacts), `utility-quick-patch-plan` (plan
  artifacts), `utility-quick-patch-code` (Rust source files), or
  `utility-quick-patch-tests` (test files) for the DelegateFix recovery path.
  See `.github/routing.md` for the three-tier recovery protocol.

For the full routing matrix, scenario guidance, and delegation rules, see
[`.github/routing.md`](routing.md).
