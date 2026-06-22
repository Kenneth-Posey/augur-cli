---
name: rust-4-review-function-sig-tools
description: >
  Deterministic tool commands for Stage 4 function signature review. Runs sig-report
  and syn-analyzer to verify that implemented signatures match the Function Signature
  Plan and that parameter lists are not oversized for a scoped Rust review handoff.
---

# Skill: Rust Stage 4 - Function Signature Review Commands

---

## When To Use This Skill

Use this skill when a Rust review handoff needs command-based evidence for
scoped API changes. Missing planned functions are Critical, and oversized
parameter lists are High per project standards.

---

## Key Files

- `README.md` - overview and usage notes

## Expected Handoff Inputs

- Scoped changed-file list for Rust modules with public or cross-module API changes
- Relevant plan artifacts, especially:
  - `plans/<feature-slug>/plan/function-sig-plan.md`
  - `plans/<feature-slug>/plan/domain-spec.md`
  - `plans/<feature-slug>/plan/implementation-plan.md`
- Repository layout guidance from `.github/local/directories.md`
- Existing deterministic signature and AST-analysis artifacts, if already captured

---

## Repo-Local Authorities

- `plans/<feature-slug>/plan/function-sig-plan.md` is the authority for expected
  function names, parameters, and return types.
- `plans/<feature-slug>/plan/domain-spec.md` and
  `plans/<feature-slug>/plan/implementation-plan.md` provide the intended API
  responsibilities and ownership boundaries.
- `.github/local/directories.md` is the authority for locating the Rust source
  tree under review.

---

## Tool Commands

### Tool 1: sig-report

**Purpose**: Generate a structured snapshot of implemented function signatures.

**Rustdoc JSON handling rule**:
- Do not read or parse `rustdoc.json` directly in this workflow.
- Generate or provide the path, then pass it to `.github/skills/0-external-sig-report/run.sh`.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-sig-report && cargo build --release

# Option A: use generated snapshot mode (auto-detects from Cargo.toml)
mkdir -p reports
.github/skills/0-external-sig-report/run.sh \
  --snapshot generated \
  --function-signatures \
  --output-format json \
  > reports/sig-report.json

# Option B: broader consolidation evidence when needed
.github/skills/0-external-sig-report/run.sh \
  --snapshot generated \
  --consolidation \
  --output-format json \
  > reports/sig-report.json

# Option C: fallback to text output
.github/skills/0-external-sig-report/run.sh \
  --snapshot generated \
  --function-signatures \
  > reports/sig-report.txt
```

**Output Interpretation**:

JSON output is findings-only. Compare each finding against
`plans/<feature-slug>/plan/function-sig-plan.md`.

- Function in plan but missing from `reports/sig-report.json` → `"severity": "critical"`, `"rule": "missing-plan-function"`
- Type mismatch between `reports/sig-report.json` and plan → `"severity": "critical"`, `"rule": "signature-type-mismatch"`

Map each finding with `"tool": "sig-report"`.

---

### Tool 2: syn-analyzer (parameter counts)

**Purpose**: Detect functions with parameter lists exceeding the project
maximum of 3 parameters.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-syn-analyzer && cargo build --release

mkdir -p reports
.github/skills/0-external-syn-analyzer/run.sh \
  src \
  --format json \
  --reports params \
  --max-params 3 \
  > reports/param-report.json
```

**Output Interpretation**:

Each entry in `reports/param-report.json` identifies a function with more than 3
parameters. Map each to a finding with `"tool": "syn-analyzer"`,
`"severity": "high"`, and `"rule": "oversized-param-list"`.

---

## Validation Signal

Map the command outputs above to the shared `pass|fail` signal used in
Rust Stage 4 review.

- Missing plan functions → **`fail`** (Critical)
- Type mismatches between sig-report and Function Signature Plan → **`fail`** (Critical)
- Oversized parameter lists (> 3 params) → **`fail`** (High)

---

## Diagnostic Format

Map findings from these tools to:

```json
{
  "checker": "function-sig-checker",
  "signal": "pass|fail",
  "findings": [
    {
      "severity": "critical|high|medium|low",
      "rule": "<rule-id>",
      "location": "<file>:<line>",
      "message": "<human-readable description>",
      "tool": "sig-report|syn-analyzer",
      "evidence": "<raw output snippet or key value>"
    }
  ]
}
```

---

## Review Notes

- `sig-report` and `syn-analyzer` are rooted in `0-external-sig-report` and
  `0-external-syn-analyzer`.
- Compare all findings against
  `plans/<feature-slug>/plan/function-sig-plan.md` and the scoped changed-file
  list before recording the final diagnostic set.
