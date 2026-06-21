# Agent Routing Guide

## Core Routing Rules

- Treat the primary context as a dispatcher. Delegate suitable whole subtasks to
  custom agents before loading heavy skills, reading many files, or doing broad
  investigation inline.
- Dispatch agents by executable name (`name:` in agent frontmatter). Treat
  numbered filenames and markdown headings as artifact identifiers only.
- Always launch delegated agents as background tasks (`mode: 'background'`).
  Use `mode: 'sync'` only when you need immediate output to choose the next
  step and expect the task to be brief.
- For broad repository `rg`/`grep`/`find`/recursive `ls` or large read
  operations, use `size-check` pre-flight estimates when available so the
  agent can narrow, paginate, or split before issuing high-volume commands.
- Route git status, diff, log, commit, push, and other git actions only through
  `global-git-operator`.
- After an agent reports back, do not repeat the same large investigation in the
  primary context unless a concrete blocker or contradiction requires follow-up.
- If no current agent is a good fit, propose adding a new agent before
  continuing with a large specialized task in the primary context.

## Feature Pipeline - Interactive Sessions (Primary Path)

When a user asks to implement a feature end-to-end in an interactive session:

- **Read the `0-global-orchestration-pipeline` skill** at session start. The
  main conversation is the dispatcher; do not route interactive feature work to
  an orchestrator agent.
- Use the pipeline skill for execution order, checkpoints, and failure flow.
- Enforce stage boundaries from the pipeline skill: Stage 1/2 are artifact-only
  (`plans/<feature-slug>/` + checkpoint changelogs), and implementation edits
  (`src/`, `tests/`, runtime code paths) begin only at Stage 3.
- Route stage work to the agents identified by that skill, launching delegated
  agents as background tasks.
- Route structured failure analysis to `global-triage-failure` when the skill
  indicates failure triage is needed.

This is the primary path for interactive feature work. The orchestrator agents
below are for automated or CI contexts only.

## Feature Pipeline - Automated / CI Contexts (Secondary Path)

For fully-automated runs where no human is present to manage the pipeline:

- **Use `global-pipeline-orchestrator`** to drive the full four-stage pipeline
  (Design → Plan → Implement → Review) end-to-end. It follows the
  `0-global-orchestration-pipeline` skill internally.
- **Use `global-session-resume-orchestrator`** to drive an existing multi-phase plan through
  deterministic stored orch-query state. It follows the pipeline skill and
  maintains orch-query session state.

Do not use these agents for interactive sessions. In interactive sessions, the
main conversation follows the pipeline skill directly.

## DelegateFix Recovery Path (Quick-Patch Protocol)

When a reviewer or evaluator emits `fail` (a Hold), the orchestrator follows
this three-tier recovery protocol before escalating to a full worker re-run:

1. **Hold → Quick-Patch Attempt 1:** Route the failure notes and artifact path
   to the appropriate quick-patch agent (`utility-quick-patch-design`,
   `utility-quick-patch-plan`, `utility-quick-patch-code`, or `utility-quick-patch-tests`). The
   quick-patch agent emits `pass` or `fail`.
2. **Re-run same reviewer:** If the quick-patch emits `pass`, re-run the same
   reviewer against the patched artifact.
   - If the reviewer emits `pass` → continue the pipeline normally.
   - If the reviewer still emits `fail` (Hold) → proceed to attempt 2.
3. **Hold → Quick-Patch Attempt 2:** Route the updated failure notes to the
   same quick-patch agent for a second targeted attempt.
4. **Re-run same reviewer again:** If the quick-patch emits `pass`, re-run the
   same reviewer a second time.
   - If the reviewer emits `pass` → continue the pipeline normally.
   - If the reviewer still emits `fail` (Hold) → proceed to BacktrackTo.
5. **BacktrackTo(worker) → Halt:** After two failed quick-patch attempts, route
   back to the originating worker agent (e.g., `design-behavior-builder`,
   `plan-behavior-planner`, `implement-behavior-builder`) for a full rework. If the rework
   also fails, halt and escalate to the user.

Quick-patch agents must never be used in place of full worker agents for
initial artifact creation or broad redesign.

## global-code-reviewer

- Route here for code, test, and standards reviews of implementation changes.
- Use this for diff review, standards conformance, and plan-scope completeness
 checks on code changes.
- Do not use for `.github/` customization review, plan approval, dependency
  audits, or cargo-output triage; use the specialist reviewer for those cases.

## review-activation-checker

- Route here for deterministic replacement-work activation validation.
- Use this for wiring proof, legacy-bypass proof, runtime assertion evidence,
 and active-path verification when a change replaces existing behavior.
- This agent emits only pass/fail and does not own broader behavior review.

## review-consolidator

- Internal-only Stage 4 merge agent. Do not route to this agent directly from
  general dispatch surfaces.
- Use only through `0-global-orchestration-pipeline` Stage 4 or
  `review-orchestrator`, after all eleven Stage 4 checker signals have been
  collected.
- Merge the eleven review-stage signals into the final pass/fail decision.

## external-code-topology-extractor

- Route here to regenerate or verify `.github/local/system-actor-graph.yml`
  by running the deterministic topology-extractor tool against the wiring code.
- Use when the topology file needs to be created from scratch, or when a batch
  of wiring changes has occurred and the file needs to be brought up to date.
- Scope is deterministic and read-only on source; writes only to `.github/local/`.

## external-code-src-deadcode-analysis

- Route here for read-only deadcode analysis of Rust `src/` trees.
- Use this when you need symbols reported as unreferenced from other source
  files.
- Scope is deterministic and source-only; this agent reports findings and does
  not apply fixes.

## external-code-stub-detector

- Route here for read-only stub detection of Rust `src/` trees.
- Use this when you need deferred patterns (`todo!()`, `unimplemented!()`, etc.)
  reported from source code.
- Scope is deterministic and source-only; this agent reports findings and does
  not apply fixes.

## external-code-actor-ops-detector

- Route here for read-only actor delegation audits of Rust `src/` trees.
- Use this when you need `actor.rs`/`actor_ops.rs` pairing gaps, orphaned files,
  or non-trivial `actor.rs` logic reported.
- Scope is deterministic and source-only; this agent reports findings and does
  not apply fixes.

## external-code-rustc-dependency-check

- Route here for Cargo-resolved dependency-direction audits of Rust workspaces.
- Use this when source-text dependency scans are not enough and you need
  `cargo metadata` resolved edges validated against package-layer policy.
- Scope is deterministic and read-only; this agent reports findings and does
  not apply fixes.

## global-customization-author

- Route here for any authoring or restructuring work under `.github/agents/`,
  `.github/skills/`, `.github/prompts/`, `.github/instructions/`, or
  `.github/local/`.
- Use this when routing, baseline guidance, or customization cross-links must be
  updated together.

## global-customization-reviewer

- Route here after `.github/` customization artifacts are created or updated.
- Use this for standards-conformance, cross-link, and routing-consistency review
  of agents, skills, prompts, and instructions.
- Do not use this agent as the author; it is read-only.

## 0-global-debug-analyst (skill)

- Invoke this skill first for failing tests, compiler errors, clippy failures, or
  cargo failures when the root cause is not yet known.
- Use it to isolate the failure mechanism and propose the minimal fix.

## design-behavior-builder

- Route here to produce a complete behavior specification in Given/When/Then
  form from a validated feature specification.
- Use this for comprehensive behavior documentation and testing specifications.

## design-behavior-reviewer

- Route here for final design-stage validation of behavior specifications.
- Use this to validate completeness, consistency, and traceability of behaviors.

## design-features-builder

- Route here to decompose a requirements document into a feature specification
  by identifying, decomposing, and organizing requirements into implementable
  features.
- Use this for feature specification from requirements.

## design-features-reviewer

- Route here to validate feature specifications for requirements coverage and
  implementability.
- Use this to confirm every requirement is addressed, no orphaned features exist,
  and all features are implementable.

## design-orchestrator

- **Secondary/automation path.** Route here only in automated or CI contexts
  to run Stage 1 (Design) as a dedicated background agent. It follows the
  `0-global-orchestration-pipeline` skill for Stage 1 only.
- In interactive sessions, the main conversation follows the pipeline skill
  directly; do not route to this agent.
- Use this when a CI pipeline needs an isolated Design-stage executor that
  surfaces a `pass`/`fail` signal to a calling automation.

## design-requirements-builder

- Route here to transform a raw user feature request into a structured
  requirements document in Given/When/Then form.
- Use this for requirements authoring within the Design stage.

## design-requirements-reviewer

- Route here to validate requirements documents against completeness criteria,
  consistency rules, and testability principles.
- Use this for requirements validation within the Design stage.

## plan-orchestrator

- **Secondary/automation path.** Route here only in automated or CI contexts
  to run Stage 2 (Plan) as a dedicated background agent. It follows the
  `0-global-orchestration-pipeline` skill for Stage 2 only.
- In interactive sessions, the main conversation follows the pipeline skill
  directly; do not route to this agent.
- Use this when a CI pipeline needs an isolated Plan-stage executor that
  surfaces a `pass`/`fail` signal with the full plan package to a calling
  automation.

## plan-domain-designer

- Route here to design domain entities, aggregates, value objects, and invariants
  from validated design features and behavioral specifications.
- Use this when Stage 2 work is focused on domain modeling.

## plan-domain-reviewer

- Route here for semantic review of domain entity specifications: correctness,
  invariant consistency, and entity lifecycle completeness.
- Use for domain-spec review within Stage 2 planning.

## plan-dependency-designer

- Route here to design the module dependency graph: identify boundaries, define
  DAG structure, and assign ownership direction from validated domain entities.
- Use this when Stage 2 work is focused on dependency-graph design.

## plan-dependency-plan-evaluator

- Route here to validate a Stage 2 pseudocode dependency graph for acyclicity,
  single-direction flow, entity placement completeness, and behavioral
  communication coverage.
- Use for dependency-graph validation within Stage 2 planning.
- Works entirely with plan files; does not read source code or run build tools.

## plan-function-sig-planner

- Route here to design function signatures, parameter types, return types, and
  interface contracts from validated domain specification and behavioral specifications.
- Use this to transform domain operations into concrete function signatures.

## plan-function-sig-reviewer

- Route here for semantic review of function signature plans: type correctness,
  completeness, interface contract validity, and consistency with domain specifications.
- Use for function-signature-plan review within Stage 2 planning.

## plan-behavior-planner

- Route here to translate Given/When/Then behavioral specifications, domain
  entities, dependency graph, and function signature plan into a concrete behavior
  plan: state machines, decision trees, actor protocols, and behavior contracts.
- Use this when Stage 2 work is focused on behavior planning.

## plan-behavior-plan-reviewer

- Route here for semantic review of behavior plans: GWT scenario traceability,
  state/transition coverage, guard exhaustiveness, contract testability, and
  language-specific correctness.
- Use for behavior-plan review within Stage 2 planning.

## plan-test-planner

- Route here to design comprehensive test strategies, coverage matrices, and test
  composition rules from validated behavioral specifications and function signatures.
- Use this to create a test contract that spans unit, integration, and property-based tests.

## plan-test-reviewer

- Route here for semantic review of test strategy plans: coverage completeness,
  traceability to behaviors, test type appropriateness, and pass condition clarity.
- Use for test-strategy review within Stage 2 planning.

## utility-doc-author

- Route here for documentation-only work in `docs/**/*.docs.md`, `README`-style docs,
  or Rustdoc comments.
- Use this when behavior should stay unchanged and only documentation needs
  authoring or correction.
- Do not use for `.github/` customization markdown; route those to
  `global-customization-author`.

## utility-topology-extractor

- Route here to regenerate or verify `.github/local/system-actor-graph.yml`
  from the current wiring code. Delegates to the deterministic
  `external-code-topology-extractor` tool.
- Use when the topology file needs to be created from scratch, or when a batch
  of wiring changes has occurred and the file needs to be brought up to date.
- Does not modify src/ files. Writes only to `.github/local/`.

## global-git-operator## global-git-operator

- Route every git workflow here: status, diff, log, show, branch queries,
  commits, pushes, and other git-only tasks.
- Use this agent for authorized pipeline checkpoint commits and other git work.
- This is the only agent allowed to run git commands.
- If another agent needs git state, have that agent request the needed git
  evidence from `global-git-operator` instead of running git directly.

## global-pipeline-orchestrator

- **Secondary/automation path.** Route here only in automated or CI contexts
  to drive the full four-stage pipeline (Design → Plan → Implement → Review)
  end-to-end. It follows the `0-global-orchestration-pipeline` skill internally.
- In interactive sessions, the main conversation follows the pipeline skill
  directly; do not route to this agent.
- Use this when a CI pipeline or non-interactive automation needs a single agent
  to manage the entire feature pipeline with orch-query session tracking.

## global-triage-failure

- Route here to analyze review-stage failures and classify their likely cause.
- Use this to produce structured diagnostics, failure taxonomy, and recovery
  considerations for the session orchestrator.
- Does not own retry or routing control; the orchestrator decides the next action.

## global-writer-changelog

- Route here to write repository changelog entries for completed changes and
  pipeline stage checkpoints.
- Use this when a passed pipeline stage or other commit-ready change needs a
  `changelogs/` entry; follow the orchestration or pipeline skill for any
  checkpoint sequencing.

## utility-code-newtype-migrator

- Route here when the task is to replace bare domain primitives with semantic
  newtype wrappers across an existing area.
- Use this for broad primitive-migration work that starts with surveying current
  usage and updates the related boundaries.
- Prefer `utility-code-rust-implementer` for ordinary feature delivery that only
  happens to touch a small number of types.

## plan-builder

- Route here to synthesize all Stage 2 artifacts (domain spec, dependency graph,
  function signatures, behavior plan, test strategy) into a single phased
  implementation plan document.
- Use this when Stage 2 work is focused on plan synthesis.

## plan-evaluator

- Route here to review or approve a written plan in `plans/` before work starts.
- Use this for phase-gate validation, invalid agent checks, and plan-quality
  findings.
- Do not use it for implementation review after code changes; use
  `global-code-reviewer` for that.

## utility-question-answering

- Route broad repository questions here when answering requires reading many
  files, tracing behavior, or synthesizing cross-cutting context.
- Use this for investigation and explanation, not for audits or code changes.
- If the question is really a review, route to the correct review agent instead.

## utility-quick-patch-design

- Route here to apply targeted surgical fixes to design-stage artifacts
  (`requirements.md`, `features.md`, `behaviors.md`) after any
  `1-design-*-reviewer` Hold.
- Use this for minimal gap-filling only: read the failure notes, patch the
  exact gaps, emit `pass` or `fail`. Do not regenerate from scratch.
- Do not use for initial artifact creation; use the corresponding builder
  agent instead.

## utility-quick-patch-plan

- Route here to apply targeted surgical fixes to plan-stage artifacts
  (`domain-spec.md`, `dependency-graph.md`, `function-sig-plan.md`,
  `behavior-plan.md`, `test-strategy-plan.md`, or `implementation-plan*.md`)
  after any `2-plan-*-reviewer` or `2-plan-*-evaluator` Hold.
- Use this for minimal gap-filling only: read the failure notes, patch the
  exact gaps, emit `pass` or `fail`. Do not regenerate from scratch.
- Do not use for initial artifact creation; use the corresponding planner
  agent instead.

## utility-quick-patch-code

- Route here to apply targeted surgical fixes to Rust source files after any
  `3-implement-*-reviewer` or `4-review-*-checker` Hold citing source code
  failures.
- Use this for minimal gap-filling only: read the failure notes, patch the
  exact gaps, run `cargo test --lib --quiet`, emit `pass` or `fail`. Do not
  regenerate from scratch.
- For general small bounded updates outside the reviewer-hold flow,
  prefer `utility-quick-patch-code` over initiating a full `utility-code-rust-implementer` run.

## utility-quick-patch-tests

- Route here to apply targeted surgical fixes to test files after a reviewer
  Hold specifically citing test coverage or test correctness failures.
- Use this for minimal gap-filling only: read the failure notes, patch the
  exact missing or incorrect test cases, emit `pass` or `fail`. Do not
  regenerate from scratch.
- Do not use for initial test authoring; use `implement-test-author` instead.

## utility-code-refactorer

- Route here for behavior-preserving structural cleanup driven by a known
  standards or decomposition violation.
- Use this when observable behavior should stay the same and the goal is better
  structure, not new functionality.
- Do not use it for root-cause diagnosis or new behavior delivery; use
  `0-global-debug-analyst` skill or `utility-code-rust-implementer` instead.

## utility-code-rust-implementer

- Route planned or clearly specified Rust behavior changes here once the desired
   behavior is known.
- Use this for feature delivery, bug fixes, and other complete implementation
   work that must finish with tests and no deferred behavior.
- For unknown failures, invoke `0-global-debug-analyst` skill first; for very small bounded
   updates, `utility-quick-patch-code` may be the better fit.

## implement-orchestrator

- **Secondary/automation path.** Route here only in automated or CI contexts
  to run Stage 3 (Implement) as a dedicated background agent. It follows the
  `0-global-orchestration-pipeline` skill for Stage 3 only.
- In interactive sessions, the main conversation follows the pipeline skill
  directly; do not route to this agent.
- Use this when a CI pipeline needs an isolated Implement-stage executor that
  surfaces a `pass`/`fail` signal with the full implementation package to a
  calling automation.

## Stage 3 concrete routing responsibilities

- Route Stage 3 implementation work among these executable agents, with exact
  execution order owned by the orchestration or pipeline skill:
  - `implement-domain-builder`
  - `implement-domain-reviewer`
  - `implement-function-sig-builder`
  - `implement-function-sig-reviewer`
  - `implement-test-author`
  - `implement-test-tdd-reviewer`
  - `implement-behavior-builder`
  - `implement-behavior-implementation-reviewer`
- Keep this routing language-neutral: these agents implement and review the
  approved Stage 2 artifacts in the repository's target language and current
  project layout.

## implement-domain-builder

- Route here for Stage 3 domain implementation responsibilities: approved domain
  types, lifecycle models, and invariant-enforcing domain operations.
- Use this when Stage 3 work is focused on concrete domain implementation.

## implement-domain-reviewer

- Route here to validate `implement-domain-builder` output against the approved domain
  specification within Stage 3 implementation work.
- Use this for review of concrete domain implementation; follow the orchestration
  or pipeline skill for any next-step routing.

## implement-function-sig-builder

- Route here for Stage 3 function-signature implementation responsibilities:
  implement approved
  contract surfaces, signatures, and only the minimal labeled compile-target
  stubs needed for TDD-oriented implementation.
- Use this when Stage 3 work is focused on contract-surface implementation.

## implement-function-sig-reviewer

- Route here to validate `implement-function-sig-builder` output against the approved
  function signature plan and domain implementation.
- Use this for review of contract-surface implementation; when the signatures
  replace an existing entrypoint, also verify cutover evidence, legacy-bypass
  proof, and that the old path is removed, unreachable, or feature-flagged off
  by default.
- Follow the orchestration or pipeline skill for any next-step routing.

## review-orchestrator

- **Secondary/automation path.** Route here only in automated or CI contexts
  to run Stage 4 (Review) as a dedicated background agent. It follows the
  `0-global-orchestration-pipeline` skill for Stage 4 only, launches all eleven
  review-stage checkers in parallel, including `review-activation-checker`, and
  invokes the internal-only `review-consolidator`.
- In interactive sessions, the main conversation follows the pipeline skill
  directly; do not route to this agent.
- Use this when a CI pipeline needs an isolated Review-stage executor that
  surfaces a `pass`/`fail` signal to a calling automation.

## global-session-resume-orchestrator

- **Secondary/automation path.** Route here only in automated or CI contexts
  to drive an existing multi-phase plan through deterministic stored orch-query
  state. It follows the `0-global-orchestration-pipeline` skill and maintains
  orch-query signal history.
- In interactive sessions, the main conversation follows the pipeline skill
  directly; do not route to this agent.
- Use this when the task is coordinating plan execution across specialized
  agents in a non-interactive run, not when writing the plan or implementing
  code directly.

## implement-test-author

- Route here for TDD Red work: failing tests, regression tests, explicit
  behavioral specifications for implementation work, and runtime cutover
  assertion tests for replacement work.
- Use this when the next step is to define expected behavior in tests rather
  than write production code.

## implement-test-tdd-reviewer

- Route here to validate test suite completeness against the test strategy plan.
- Use this for TDD-review responsibilities: checks coverage matrix, Red state,
  path mirroring, doc comments, runtime cutover assertions for replacement
  work, and that no production code was written.
- Emits `pass` or `fail`; follow the orchestration or
  pipeline skill for resulting routing.

## implement-behavior-builder

- Route here for Stage 3 behavior-wiring responsibilities: wire approved runtime
  behavior into the implemented contracts so the planned Red tests reach Green,
  and complete the old-to-new cutover in the same implementation phase unless
  the scope is explicitly scaffold-only.
- Use this when Stage 3 work is focused on behavior wiring, not new test
  authoring or Stage 2 redesign.

## implement-behavior-implementation-reviewer

- Route here to validate `implement-behavior-builder` output against the approved behavior
  plan, Green-state expectations, zero-placeholder requirement, and activation-
  gate/cutover-complete evidence, including legacy-bypass proof and runtime
  assertion coverage.
- Use this for behavior-implementation review; follow the orchestration or
  pipeline skill for any resulting routing.

## plan-gap-analyst

- Route here for Stage 2 coverage-gap analysis to verify that every Stage 1 GWT
  behavioral scenario is fully traceable through the complete Stage 2
  pseudocode package.
- Use this to check end-to-end coverage across domain spec, dependency graph,
  function signatures, behavior plan, and test strategy.
- Reads markdown instruction/planning artifacts and writes only
  `plans/<feature-slug>/plan/gap-report.md`.
- Emits standard pipeline signals: `pass` when all scenarios are covered with no
  critical/major gaps, `fail` when blocking gaps remain or required markdown
  inputs are missing or contradictory.

## external-code-tool-analyst

- Route here for cargo check, clippy, and test-output analysis that maps findings
  to standards, remediation domains, and supporting evidence.
- Use this when the main need is structured diagnostics triage rather than a fix
  or a review of already understood changes.
