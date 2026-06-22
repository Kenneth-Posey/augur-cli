---
name: rust-4-review-completeness-tools
description: >
  Deterministic tool commands for Stage 4 completeness review. Runs cargo-diagnostics
  and test-gap-fusion to detect missing artifacts, unimplemented stubs, and structural
  coverage gaps for a scoped Rust review handoff.
---

# Skill: Rust Stage 4 - Completeness Review Tool Commands

---

## When To Use This Skill

Use this skill when a Rust review handoff needs deterministic completeness
checks for scoped implementation changes. `todo!()` and `unimplemented!()`
macros in production code are Critical failures.

---

## Key Files

- `README.md` - overview and usage notes

## Expected Handoff Inputs

- Scoped changed-file list for production and test modules under review
- Relevant plan artifacts, especially:
  - `plans/<feature-slug>/plan/implementation-plan.md`
  - `plans/<feature-slug>/plan/test-strategy-plan.md`
  - `plans/<feature-slug>/design/behaviors.md`
- Repository layout guidance from `.github/local/directories.md`
- Existing deterministic cargo and gap-analysis artifacts, if already captured

---

## Repo-Local Authorities

- `plans/<feature-slug>/plan/implementation-plan.md` is the authority for which
  implementation artifacts should exist.
- `plans/<feature-slug>/design/behaviors.md` and
  `plans/<feature-slug>/plan/test-strategy-plan.md` are the authorities for the
  behavior and test coverage that should accompany the change.
- `.github/local/directories.md` is the authority for locating `src/` and
  mirrored `tests/` paths.

---

## Tool Commands

### Tool 1: cargo-diagnostics

**Purpose**: Normalize `cargo check` JSON output and surface stub macros
(`todo!()`, `unimplemented!()`, `unreachable!()`) as structured findings.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-cargo-diagnostics && cargo build --release

# Emit cargo check JSON for the review artifact
cargo check --all-targets --message-format=json

# Normalize the captured cargo JSON
.github/skills/0-external-cargo-diagnostics/run.sh \
  cargo-check.json \
  --mode cargo-json \
  > completeness-diag.json
```

Save the `cargo check` JSON stream to `cargo-check.json` before running
`cargo-diagnostics`.

**Output Interpretation**:

Look for `todo!()`, `unimplemented!()`, `unreachable!()` in warnings. Any such
finding in production code (not test modules) indicates an unfinished stub.
Map each to a finding with `"tool": "cargo-diagnostics"`,
`"severity": "critical"`, and `"rule": "stub-macro"`.

---

### Tool 2: test-gap-fusion (structural completeness)

**Purpose**: Detect structural gaps - source files or behaviors with no
corresponding tests.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-test-gap-fusion && cargo build --release

mkdir -p reports
.github/skills/0-external-test-gap-fusion/run.sh \
  --src src \
  --tests tests \
  --output reports/gap-report.json
```

**Output Interpretation**:

Map `high`-priority gaps from `reports/gap-report.json` to findings with
`"tool": "test-gap-fusion"`, `"severity": "high"`, and
`"rule": "coverage-gap-<missing_coverage_type>"`.

---

## Deterministic Validation Signal

Use the command outputs above to produce the shared `pass|fail`
signal for Rust Stage 4 review.

- `todo!()` or `unimplemented!()` in production code → **`fail`** (Critical)
- High-priority structural gaps → **`fail`** (High)
- Medium/low gaps or `unreachable!()` in documented unreachable paths → **`pass`** with warnings

---

## Standard Diagnostic Format

All findings emitted by this skill's tools must be mapped to:

```json
{
  "checker": "completeness-checker",
  "signal": "pass|fail",
  "findings": [
    {
      "severity": "critical|high|medium|low",
      "rule": "<rule-id>",
      "location": "<file>:<line>",
      "message": "<human-readable description>",
      "tool": "cargo-diagnostics|test-gap-fusion",
      "evidence": "<raw output snippet or key value>"
    }
  ]
}
```

---

## Review Notes

- `cargo-diagnostics` is rooted in `0-external-cargo-diagnostics`.
- `test-gap-fusion` is rooted in `0-external-test-gap-fusion`.
- Interpret stub and gap findings against the scoped changed-file list and the
  implementation and test authorities listed above.
