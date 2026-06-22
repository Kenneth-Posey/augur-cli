---
name: 0-global-orchestration-pipeline
description: >
  Orchestration workflow for the full feature pipeline
  (Design → Plan → Implement → Review). Defines which steps to run, in what
  order, and how to handle pass/fail signals at each gate for interactive and
  automated runs.
---

# 0-global-orchestration-pipeline

## Pipeline Overview

The feature pipeline has four stages. Complete each stage before starting the
next. Within a stage, run steps one at a time in the listed order, except in
Stage 4, where the checkers run in parallel and `review-consolidator` merges the
results.

```
Stage 1: Design              → Stage 2: Plan                   → Stage 3: Implement              → Stage 4: Review
  Produces GWT behavioral       Produces pseudocode planning       Translates pseudocode into        Reviews the real code
  specs                         package                            real code and tests
  3 builder/reviewer            6 builder/reviewer pairs           4 builder/reviewer                11 checkers + consolidator
  pairs                         + 1 gate                           pairs                             + git-operator checkpoint
```

After each stage passes, invoke `global-writer-changelog` to write a changelog
entry, then invoke `global-git-operator` to create a checkpoint commit before
proceeding. Each checkpoint section in this skill authorizes that stage commit
when the caller provides the completed stage, this skill path/section, and the
matching changelog path to `global-git-operator`.

## Key Files

- `README.md` - overview and usage notes

## Stage Artifact Boundary (Hard Requirement)

- **Stage 1 (Design)** and **Stage 2 (Plan)** are artifact-only stages.
  During these stages, write only under `plans/<feature-slug>/` (plus required
  `changelogs/` checkpoint artifacts).
- Do **not** modify production or test implementation paths during Stage 1 or
  Stage 2, including `src/`, `tests/`, `examples/`, and equivalent runtime code
  paths for the repository.
- **Stage 3 (Implement)** is the first stage where source/test implementation
  edits are allowed.
- If source/test implementation edits are detected before Stage 3, halt with a
  stage-boundary violation and route remediation through the applicable
  stage-level quick-patch flow.

## Step Execution Contract

When you launch any background step in the pipeline:

- Launch with `mode: 'background'` unless you need the output immediately to
  decide the next step (in which case use `mode: 'sync'`).
- Wait for the step to complete before launching the next step in the sequence.
- Collect the step's output signal: `pass` or `fail`. Any signal other than `pass` is treated as `fail`.
- Do not proceed to the next step until the current step emits a clear signal.

## Pre-flight Checks

Before starting Stage 1, verify:

1. The working tree is clean (no uncommitted changes). If not clean, halt and ask
   the user to commit or stash changes before proceeding.
2. A feature request or plan file is present in `plans/` or has been provided
   inline. If absent, halt and ask the user to supply requirements.
3. No previous session is in a `stopped` state for this feature. If one exists,
   ask the user whether to resume from the last completed stage or restart.
4. Derive `<feature-slug>` from the feature request: take the 2–5 most meaningful
   words, lowercase, hyphen-separated. Example: "Add JWT authentication to the API"
   → `add-jwt-auth`. Record this slug - all Stage 1 and Stage 2 artifacts will be
   written to `plans/<feature-slug>/`.

If all pre-flight checks pass, announce the pipeline start to the user and proceed
to Stage 1.

---

## Stage 1: Design

**Purpose:** Transform the raw feature request into validated behavioral specifications.

**Artifacts produced:** requirements document, feature specification, behavior specification.

### Step 1.1 - Requirements

1. Launch `design-requirements-builder` with the raw feature request.
2. Wait for output: a requirements document in Given/When/Then form.
3. If builder produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `design-requirements-reviewer` with the requirements document.
5. If reviewer signals `pass` → proceed to Step 1.2.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 1.2 - Features

1. Launch `design-features-builder` with the approved requirements document.
2. Wait for output: a feature specification.
3. If builder produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `design-features-reviewer` with the feature specification.
5. If reviewer signals `pass` → proceed to Step 1.3.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 1.3 - Behaviors

1. Launch `design-behavior-builder` with the approved feature specification.
2. Wait for output: a behavior specification (Given/When/Then scenarios).
3. If builder produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `design-behavior-reviewer` with the behavior specification.
5. If reviewer signals `pass` → Stage 1 complete.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Stage 1 Checkpoint

After all three reviewer pairs pass:
- Collect artifacts: `plans/<feature-slug>/design/requirements.md`, `plans/<feature-slug>/design/features.md`, `plans/<feature-slug>/design/behaviors.md`.
- Invoke `global-writer-changelog` to produce a changelog entry for the stage artifacts. Wait for confirmation the changelog file was written to `changelogs/`.
- Launch `global-git-operator` to create an authorized pipeline checkpoint commit using this
  skill's Stage 1 Checkpoint section as authorization evidence:
  `"checkpoint: design stage complete"`.
- Proceed to Stage 2.

---

## Stage 2: Plan

**Purpose:** Translate the validated behavioral specifications into a complete pseudocode
planning package. Every Stage 2 artifact is expressed in language-agnostic pseudocode:
domain models as typed pseudocode structs/enums, dependency graph as pseudocode module
declarations, function signatures as typed pseudocode stubs, behavior logic as pseudocode
state machines and algorithms, test cases as pseudocode test stubs.

**Inputs:** Design artifacts from Stage 1.

**Artifacts produced:** domain pseudocode, dependency graph pseudocode, function signature
pseudocode stubs, behavior pseudocode (state machines and algorithms), test pseudocode stubs,
implementation plan, gap report.

### Step 2.1 - Domain Planning

1. Launch `plan-domain-designer` with the features and behaviors from Stage 1.
2. Wait for output: a domain entity specification (entities, aggregates, invariants).
3. If planner produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `plan-domain-reviewer` with the domain specification.
5. If reviewer signals `pass` → proceed to Step 2.2.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 2.2 - Dependency Planning

1. Launch `plan-dependency-designer` with the domain entity specification and the behavioral specifications from Stage 1.
2. Wait for output: a module dependency graph with placement decisions and interface boundaries.
3. If designer produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `plan-dependency-plan-evaluator` with the dependency graph.
5. If evaluator signals `pass` → proceed to Step 2.3.
6. If evaluator signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 2.3 - Function Signature Planning

1. Launch `plan-function-sig-planner` with the validated domain specification,
   dependency graph, and behavior specifications.
2. Wait for output: a function signature plan with interface contracts and type
   boundaries.
3. If planner produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `plan-function-sig-reviewer` with the function signature plan.
5. If reviewer signals `pass` → proceed to Step 2.4.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 2.4 - Behavior Planning

1. Launch `plan-behavior-planner` with the validated function signature plan,
   dependency graph, domain spec, and Stage 1 behavior specs.
2. Wait for output: behavior plan at `plans/<feature-slug>/plan/behavior-plan.md`.
3. If builder produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `plan-behavior-plan-reviewer` with the behavior plan, dependency graph,
   function signature plan, domain spec, and Stage 1 behavior specs.
5. If reviewer signals `pass` → proceed to Step 2.5.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 2.5 - Test Planning

1. Launch `plan-test-planner` with the validated behavior plan, function signature
   plan, and Stage 1 behavior specs.
2. Wait for output: a test strategy plan with coverage matrix and test composition
   rules.
3. If planner produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `plan-test-reviewer` with the test strategy plan.
5. If reviewer signals `pass` → proceed to Step 2.6.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 2.6 - Plan Building

1. Launch `plan-builder` with all Stage 2 artifacts (domain spec, dependency
   graph, function signature plan, behavior plan, test strategy plan).
2. Wait for output: a phased implementation plan.
3. If builder produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `plan-evaluator` with the implementation plan.
5. If evaluator signals `pass` → proceed to Step 2.7.
6. If evaluator signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 2.7 - Gap Analysis (Final Gate)

1. Launch `plan-gap-analyst` with the full Stage 2 planning package
   (domain spec, dependency graph, function signature plan, behavior plan, test
   strategy plan, implementation plan) and Stage 1 behavior specs.
2. Wait for output: a standard validation signal (`pass` or `fail`) plus
   `plans/<feature-slug>/plan/gap-report.md`.
3. If analyst signals `pass` (no critical or major gaps) → Stage 2 complete.
4. If analyst signals `fail` → run `utility-quick-patch-plan` with the gap report and failure context; retry up to 2 times. Hard Stop after 2 retries.

### Stage 2 Checkpoint

After all seven steps pass:
- Collect artifacts:
  - `plans/<feature-slug>/plan/domain-spec.md`
  - `plans/<feature-slug>/plan/dependency-graph.md`
  - `plans/<feature-slug>/plan/function-sig-plan.md`
  - `plans/<feature-slug>/plan/behavior-plan.md`
  - `plans/<feature-slug>/plan/test-strategy-plan.md`
  - `plans/<feature-slug>/plan/implementation-plan.md`
  - `plans/<feature-slug>/plan/gap-report.md`
- Invoke `global-writer-changelog` to produce a changelog entry for the stage artifacts. Wait for confirmation the changelog file was written to `changelogs/`.
- Launch `global-git-operator` to create an authorized pipeline checkpoint commit using this
  skill's Stage 2 Checkpoint section as authorization evidence:
  `"checkpoint: plan stage complete"`.
- Proceed to Stage 3.

---

## Stage 3: Implement

**Purpose:** Translate the Stage 2 pseudocode planning package into working implementation code and
a passing test suite.

**Inputs:** Pseudocode planning package from Stage 2.

**Artifacts produced:** domain implementation code, function compile-target stubs,
behavior-wired logic, and a complete test suite.

For production file paths, language-specific deferred-implementation markers,
and Green/test/check commands, consult
[`.github/local/language-companions.md`](../../local/language-companions.md)
and the applicable local/language-specific companion guidance. This stage
defines sequencing, TDD gates, and zero-stub completion requirements; companion
guidance defines the language/runtime details.

### Step 3.1 - Domain Implementation

1. Launch `implement-domain-builder` with the domain entity specification.
2. Wait for output: domain types, data structures, invariant methods, and only the
   minimal explicitly labeled `compile-target stubs` needed so later tests compile.
3. If builder produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `implement-domain-reviewer` with the generated domain code.
5. If reviewer signals `pass` → proceed to Step 3.2.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 3.2 - Function Signature Implementation

1. Launch `implement-function-sig-builder` with the function signature plan and
    approved domain code.
2. Wait for output: function signatures with full contracts and documentation plus
   only the minimal explicitly labeled `compile-target stubs` needed so later tests compile.
3. If builder produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `implement-function-sig-reviewer` with the function stub implementations.
5. If reviewer signals `pass` → proceed to Step 3.3.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 3.3 - Test Authoring (TDD Red)

1. Launch `implement-test-author` with the test strategy plan and behavior plan from
   Stage 2, the behavioral specification from Stage 1, and the approved
   compile-target stubs from Step 3.2. Tests may rely on those targets so the
   suite compiles, but they must still fail in Red. If this is replacement work,
   include a runtime assertion test proving the legacy path is not used and the
   new path is active.
2. Wait for output: failing test artifacts that follow the applicable test-file
   layout and documentation conventions from
   [`.github/local/language-companions.md`](../../local/language-companions.md)
   and related local guidance, with Red state confirmed.
3. If author produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `implement-test-tdd-reviewer` with the written tests and the test strategy
   plan.
5. If reviewer signals `pass` → proceed to Step 3.4.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Step 3.4 - Behavior Wiring

1. Launch `implement-behavior-builder` with the behavior plan from Stage 2, the approved
   domain code from Step 3.1, the approved compile-target stubs from Step 3.2,
   the behavioral specification from Stage 1, and the failing test suite from
   Step 3.3. This step must replace every temporary compile-target stub with
   real production behavior before it can pass.
2. Wait for output: wired implementations with business logic and state transitions that
   satisfy all tests written in Step 3.3 with zero remaining production compile-target stubs.
3. If builder produces no output or signals `fail` → run the stage-appropriate quick-patch step (see Quick-Patch Routing below) with the failure context; retry up to 2 times. Hard Stop after 2 retries.
4. Launch `implement-behavior-implementation-reviewer` with the behavior-wired code, the
   behavior plan, and the test suite. The reviewer validates code-to-pseudocode
   traceability, the deterministic remaining-stub scan required by the
   applicable local/language-specific guidance, and the language-appropriate
   Green verification commands from that guidance to confirm all Stage 3 tests pass.
5. If reviewer signals `pass` (traceability complete + zero remaining production stubs + Green verification passes) → Stage 3 complete;
   proceed to Stage 3 Checkpoint.
6. If reviewer signals `fail` → run the stage-appropriate quick-patch step with the reviewer's failure report; retry the full step (builder + reviewer) up to 2 times. Hard Stop after 2 retries.

### Stage 3 Checkpoint

After Steps 3.1–3.4 pass:
1. Run the deterministic production-stub scan over the Stage 3 production implementation
   paths, following the applicable local/language-specific guidance. Zero remaining production
   matches is a hard requirement.
   - The scan must include `compile-target stub` markers and any language-specific
     deferred-implementation markers named by that guidance.
   - If any match remains → do not proceed; run `utility-quick-patch-code` with the failure context.
   - If zero matches remain → continue.
2. Run the repository-scope Green/test/check commands required by the applicable
   local/language-specific guidance. All Stage 3 tests and required implementation
   checks must pass.
   - If any required Green/test/check command fails → do not proceed; run `utility-quick-patch-code` with the failure context.
   - If all required Green/test/check commands pass → proceed to the checkpoint commit.
3. Invoke `global-writer-changelog` to produce a changelog entry for the stage artifacts. Wait for confirmation the changelog file was written to `changelogs/`.
4. Launch `global-git-operator` to create an authorized pipeline checkpoint commit using this
   skill's Stage 3 Checkpoint section as authorization evidence:
   `"checkpoint: implement stage complete"`.
5. Proceed to Stage 4.

---

## Stage 4: Review

**Purpose:** Validate the full implementation across eleven review dimensions and
produce a consolidated approval decision.

**Inputs:** Implementation artifacts from Stage 3.

**Artifacts produced:** validation reports from all eleven checkers, a merged
approval decision from `review-consolidator`.

### Step 4.1 - Launch Checkers (Background, Parallel)

Launch all eleven checkers as background steps simultaneously. They run in
parallel; do not wait for one to finish before launching the next:

| Checker | Validates |
|---|---|
| `review-architecture-checker` | Module dependency DAG, boundary violations |
| `review-behavior-checker` | All tests pass, coverage ≥ 80%; essential GWT scenarios 100% covered |
| `review-activation-checker` | Deterministic cutover/wiring evidence, legacy bypass evidence, replacement-work activation state |
| `review-type-checker` | type safety, constraints, ownership |
| `review-function-sig-checker` | Signatures match plan, error handling |
| `review-performance-checker` | Algorithmic complexity, regressions |
| `review-security-checker` | Unsafe code, vulnerability patterns |
| `review-consistency-checker` | Naming, documentation, cross-artifact consistency |
| `review-completeness-checker` | All planned behaviors implemented; essential GWT scenarios 100% covered |
| `external-code-stub-detector` | No surviving production stubs or placeholders |
| `review-consolidation-checker` | No dead code, duplicate functions, or chain-collapse candidates above confidence threshold |

### Step 4.2 - Collect All Signals

Wait for all eleven checkers to complete. Collect each checker's signal. Any signal other than `pass` is treated as `fail`.

### Step 4.3 - Consolidation

1. Launch `review-consolidator` with all eleven checker signals and their report artifacts.
2. If consolidator signals `pass` → Stage 4 complete; proceed to Stage 4 Checkpoint.
3. If consolidator signals `fail` → run `utility-quick-patch-code` with the consolidated failure report and all checker findings; then re-run Stage 4 from Step 4.1. Allow up to 2 retries of the full Stage 4. Hard Stop after 2 retries.

### Stage 4 Checkpoint

After consolidator signals `pass`:
- Collect all reviewer artifacts and the consolidator's approval report.
- Invoke `global-writer-changelog` to produce a changelog entry for the stage artifacts. Wait for confirmation the changelog file was written to `changelogs/`.
- Launch `global-git-operator` to create an authorized pipeline checkpoint commit using this
  skill's Stage 4 Checkpoint section as authorization evidence:
  `"checkpoint: review stage complete - pipeline done"`.
- Emit pipeline completion summary to the user.

---

## Quick-Patch Routing

When any step fails, run the stage-appropriate quick-patch step with the full failure context (step output, failure details, relevant artifacts), then retry the step from the beginning. Allow up to 2 retries per step. Hard Stop after 2 retries.

| Stage | Quick-Patch Step |
|---|---|
| Stage 1: Design | `utility-quick-patch-design` |
| Stage 2: Plan | `utility-quick-patch-plan` |
| Stage 3: Implement (domain, signatures, behavior) | `utility-quick-patch-code` |
| Stage 3: Implement (test authoring) | `utility-quick-patch-tests` |
| Stage 4: Review | `utility-quick-patch-code` |

---

## Hard-Stop Conditions

Halt immediately and report to the user. Do not retry.

1. **Step fails after 2 retries** - quick-patch could not resolve the failure in 2 attempts.
2. **Pre-flight checks fail** - unclean working tree, missing feature slug, or missing plan file.
3. **`global-git-operator` fails** - checkpoint commit could not be created (missing changelog, authorization error, or dirty state).
4. **Session context corrupted** - orch-query unavailable or session state is inconsistent.
5. **Stage-boundary violation** - any source/test implementation path was modified during Stage 1 or Stage 2.
