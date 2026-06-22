---
name: rust-4-review-behavior-tools
description: >
  Deterministic tool commands for Stage 4 behavior review. Runs the repository
  workspace test baseline, test-gap-fusion, and optionally cargo-tarpaulin to
  verify all tests pass and identify structural coverage gaps for a scoped Rust
  review handoff.
---

# Skill: Rust Stage 4 - Behavior Review Tool Commands

---

## When To Use This Skill

Use this skill to gather deterministic behavior evidence for a scoped Rust
review. Start with `cargo test --workspace --quiet`; treat narrower follow-up
runs as diagnostic only.

---

## Key Files

- `README.md` - overview and usage notes

## Expected Handoff Inputs

- Scoped changed-file list for Rust source and mirrored test files
- Relevant plan artifacts, especially:
  - `plans/<feature-slug>/design/behaviors.md`
  - `plans/<feature-slug>/plan/test-strategy-plan.md`
  - `plans/<feature-slug>/plan/implementation-plan.md`
- Repository layout guidance from `.github/local/directories.md`
- Existing deterministic test and coverage artifacts, if already captured

---

## Repo-Local Authorities

- `plans/<feature-slug>/design/behaviors.md` is the authority for expected
  runtime behavior.
- `plans/<feature-slug>/plan/test-strategy-plan.md` is the authority for test
  scope and intended coverage shape.
- `.github/local/directories.md` is the authority for mapping `src/` files to
  mirrored `tests/**/*.tests.rs` files.

---

## Tool Commands & Integration

### Tool 1: cargo test (primary baseline)

**Purpose**: Establish the workspace test baseline for the review.

**Commands**:
```bash
# Workspace test baseline
cargo test --workspace --quiet
```

**Output Interpretation**:

- Non-zero exit code → immediate **`fail`** (Critical). Capture the smallest
  useful output and
  map each failing test to a finding with `"tool": "cargo-test"`,
  `"severity": "critical"`, and `"rule": "workspace-test-failure"`.
- Zero exit code establishes the workspace baseline for scoped interpretation.
- Do not replace this baseline with narrower `--lib`, `--test`, or
  feature-limited runs. Narrow reruns are diagnostic only, and should stay
  quiet unless verbose logs are needed to diagnose the failure.

---

### Tool 2: test-gap-fusion

**Purpose**: Perform structural gap analysis to identify source files, modules,
or behaviors lacking sufficient test coverage (unit, integration, or doc).

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-test-gap-fusion && cargo build --release

# Run structural gap analysis
mkdir -p reports
.github/skills/0-external-test-gap-fusion/run.sh \
  --src src \
  --tests tests \
  --output reports/gap-report.json

# With tarpaulin coverage (if available)
mkdir -p reports
cargo tarpaulin --workspace --out Xml --output-dir reports 2>/dev/null && \
.github/skills/0-external-test-gap-fusion/run.sh \
  --src src \
  --tests tests \
  --cobertura reports/cobertura.xml \
  --cobertura-full \
  --output reports/gap-report.json
```

**Output Interpretation**:

JSON output: `gaps[]` each with:
- `source_file` - path of the source file with the gap
- `missing_coverage_type` (`unit|integration|doc`)
- `priority` (`high|medium|low`)

Add `--cobertura-full` when file-level coverage detail is needed.

Map each gap to a finding with `"tool": "test-gap-fusion"`. Use
`"rule": "coverage-gap-<missing_coverage_type>"`.

---

## Finding Severity Guidance

- Any test failure (`cargo test --workspace --quiet` non-zero exit) → critical finding
- Gap report `high`-priority gaps → high-severity finding
- `medium` or `low` gaps → warning-level findings to document

---

## Deterministic Validation Signal

Map the workspace baseline and structural gap results to the shared
`pass|fail` signal:

- `cargo test --workspace --quiet` non-zero exit → **`fail`** (Critical)
- High-priority structural gaps against the scoped behavior/test authorities → **`fail`** (High)
- Workspace baseline clean with only medium/low gaps or no gaps → **`pass`** with warnings if needed

---

## Standard Diagnostic Format

All findings emitted by this skill's tools must be mapped to:

```json
{
  "checker": "behavior-checker",
  "signal": "pass|fail",
  "findings": [
    {
      "severity": "critical|high|medium|low",
      "rule": "<rule-id>",
      "location": "<file>:<line>",
      "message": "<human-readable description>",
      "tool": "cargo-test|test-gap-fusion",
      "evidence": "<raw output snippet or key value>"
    }
  ]
}
```

---

## Review Notes

- `test-gap-fusion` is rooted in `0-external-test-gap-fusion`.
- Interpret workspace test failures and structural gaps against the scoped
  changed-file list and the behavior/test authorities listed above.
