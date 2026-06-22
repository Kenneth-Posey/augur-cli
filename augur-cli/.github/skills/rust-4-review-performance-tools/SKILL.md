---
name: rust-4-review-performance-tools
description: >
  Tool commands for Stage 4 Rust performance review. Uses syn-analyzer to detect
  high cyclomatic complexity, deep conditional chains, oversized functions, and
  magic literals that indicate performance risk in scoped changes.
---

# Skill: Rust Stage 4 - Performance Review Tool Commands

---

## When To Use This Skill

Use this skill for Stage 4 Rust performance reviews that need tool-based
evidence for scoped changes. Critical complexity (cyclomatic > 5) or
unbounded recursion is an immediate `fail`.

---

## Key Files

- `README.md` - overview and usage notes

## Expected Inputs

- Scoped changed-file list for Rust modules with algorithmic or hot-path changes
- Relevant plan artifacts, especially:
  - `plans/<feature-slug>/plan/implementation-plan.md`
  - `plans/<feature-slug>/design/behaviors.md`
  - `plans/<feature-slug>/plan/test-strategy-plan.md`
- Repository layout guidance from `.github/local/directories.md`
- Existing complexity-analysis artifacts, if already captured

---

## Repo-Local Authorities

- `plans/<feature-slug>/plan/implementation-plan.md` is the authority for the
  intended structure of performance-sensitive logic.
- `plans/<feature-slug>/design/behaviors.md` and
  `plans/<feature-slug>/plan/test-strategy-plan.md` provide expected runtime
  behavior and any performance-focused test coverage.
- `.github/local/directories.md` is the authority for locating the Rust source
  tree under review.

---

## Tool Commands & Integration

### Tool 1: syn-analyzer (all performance checks)

**Purpose**: Detect cyclomatic complexity violations, deep conditional chains,
oversized functions, and unexplained magic literals by parsing the AST.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-syn-analyzer && cargo build --release

# Run all performance-relevant checks
.github/skills/0-external-syn-analyzer/run.sh \
  src \
  --format json \
  --reports complexity,chain,length,magic \
  --max-complexity 5 \
  --max-chain 5 \
  --max-lines 50 \
  > perf-analysis.json

# Stricter thresholds for performance-critical paths
.github/skills/0-external-syn-analyzer/run.sh \
  src \
  --format json \
  --reports complexity \
  --max-complexity 8 \
  --severity warning \
  > perf-strict.json
```

**Output Interpretation**:

JSON output: `findings[]` each with:
- `rule` (`complexity|chain|length|magic`)
- `location` (file:line)
- `message`
- `severity` (`error|warning|info`)

**Severity mapping to checker report**:

| syn-analyzer rule | Checker finding severity |
|---|---|
| `complexity` | High |
| `chain` | Medium |
| `length` | Medium |
| `magic` | Low |

Cyclomatic complexity > 5 → override to Critical regardless of syn-analyzer
severity.

The chain threshold flags `if`/`else if` chains at 5 or deeper, so the default
allows up to 4 chained branches before a finding is emitted.

Map each finding with `"tool": "syn-analyzer"`.

---

## Validation Signal

Map the approved command outputs above to the shared `pass|fail`
signal used in Rust Stage 4 review.

- Critical complexity (cyclomatic > 5) → **`fail`** (Critical)
- Unbounded recursion detected → **`fail`** (Critical)
- Advisory warning threshold (cyclomatic 4) → **`pass`** with warnings

---

## Standard Diagnostic Format

All findings emitted by this skill's tools must be mapped to:

```json
{
  "checker": "performance-checker",
  "signal": "pass|fail",
  "findings": [
    {
      "severity": "critical|high|medium|low",
      "rule": "<rule-id>",
      "location": "<file>:<line>",
      "message": "<human-readable description>",
      "tool": "syn-analyzer",
      "evidence": "<raw output snippet or key value>"
    }
  ]
}
```

---

## Review Notes

- `syn-analyzer` is rooted in `0-external-syn-analyzer`.
- Interpret complexity and size findings against the scoped changed-file list
  and the repo-local authorities listed above.
