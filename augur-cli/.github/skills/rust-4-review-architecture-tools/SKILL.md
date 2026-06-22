---
name: rust-4-review-architecture-tools
description: >
  Deterministic tool commands for Stage 4 architecture review. Runs arch-linter,
  module-graph, and dependency-intel to detect boundary violations, dependency cycles,
  and cross-crate security issues for a scoped Rust review handoff.
---

# Skill: Rust Stage 4 - Architecture Review Tool Commands

---

## When To Use This Skill

Use this skill when a Rust review handoff needs architecture evidence for
scoped source changes. It defines the repo-local authorities, expected handoff
inputs, exact tool commands, and how to interpret the output.

---

## Key Files

- `README.md` - overview and usage notes

## Expected Handoff Inputs

- Scoped changed-file list for the Rust modules under review
- Relevant design and plan artifacts, especially:
  - `plans/<feature-slug>/plan/dependency-graph.md`
  - `plans/<feature-slug>/plan/domain-spec.md`
  - `plans/<feature-slug>/plan/implementation-plan.md`
- Repository layout guidance from `.github/local/directories.md`
- Existing output artifacts from prior runs, if available

---

## Repo-Local Authorities

- `plans/<feature-slug>/plan/dependency-graph.md` is the authority for intended
  dependency direction and approved crate relationships.
- `plans/<feature-slug>/plan/domain-spec.md` and
  `plans/<feature-slug>/plan/implementation-plan.md` provide the intended module
  responsibilities and boundaries.
- `.github/local/directories.md` is the authority for source and test path
  conventions when scoping commands.

---

## Tool Commands & Integration

### Tool 1: arch-linter

**Purpose**: Detect module boundary violations, wrong-direction dependencies, and
layer contract breaches.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-arch-linter && cargo build --release

# Run against src
.github/skills/0-external-arch-linter/run.sh \
  src \
  --output-format json \
  --fail-on-findings no
```

Capture stdout as the review artifact (for example `arch-findings.json`).

**Output Interpretation**:

JSON output fields: `findings[]` each with:
- `severity` (`critical|high|medium|low`)
- `rule` (`boundary-contract|wrong-direction|cycle`)
- `location` (file:line)
- `message`

Map each finding directly to the standard diagnostic format using
`"tool": "arch-linter"`.

---

### Tool 2: module-graph

**Purpose**: Build the full module dependency graph and surface dependency
cycles as repeated node paths.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-module-graph && cargo build --release

# Run to get dependency graph
.github/skills/0-external-module-graph/run.sh \
  --format json
```

Capture stdout as the review artifact (for example `module-graph.json`).

**Output Interpretation**:

Use the `edges` field to trace dependency direction. Cycles appear as repeated
node paths in the edge list. Map cycle findings to `"rule": "cycle"` with
`"severity": "critical"`.

---

### Tool 3: dependency-intel

**Purpose**: Detect dependency advisories and cross-crate stability issues in the
dependency tree.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-dependency-intel && cargo build --release

# Run advisory check
mkdir -p reports
cargo metadata --format-version 1 > reports/metadata.json
.github/skills/0-external-dependency-intel/run.sh \
  reports/metadata.json \
  --mode advisory \
  --output reports/advisories.json
```

Use `reports/advisories.json` as the review artifact.

**Output Interpretation**:

Map advisory findings by their embedded severity field. Treat critical/high
findings as architecture-stability blockers. Use `"tool": "dependency-intel"` on
each mapped finding.

---

## Deterministic Validation Signal

Use the command outputs above to assign the shared `pass|fail` signal
for Rust Stage 4 review.

- Any `critical` or `high` finding in arch-linter output → **`fail`**
- Cycles detected in module-graph → **`fail`** (Critical)
- Any `critical` or `high` finding from `dependency-intel` → **`fail`**
- `medium` or `low` findings only → **`pass`** with warnings

---

## Standard Diagnostic Format

All findings emitted by this skill's tools must be mapped to:

```json
{
  "checker": "architecture-checker",
  "signal": "pass|fail",
  "findings": [
    {
      "severity": "critical|high|medium|low",
      "rule": "<rule-id>",
      "location": "<file>:<line>",
      "message": "<human-readable description>",
      "tool": "arch-linter|module-graph|dependency-intel",
      "evidence": "<raw output snippet or key value>"
    }
  ]
}
```

---

## Review Notes

- Tool paths are rooted in `0-external-arch-linter`,
  `0-external-module-graph`, and `0-external-dependency-intel`.
- Interpret tool findings against the scoped changed-file list and the
  dependency and design authorities listed above.
