---
name: rust-4-review-consistency-tools
description: >
  Deterministic Stage 4 consistency-review commands for scoped Rust changes.
  Uses doc-extractor and syn-analyzer to find missing documentation, naming
  violations, and doc-to-code mismatches.
---

# Skill: Rust Stage 4 - Consistency Review Tool Commands

---

## When To Use This Skill

Use this skill when a Rust review handoff needs deterministic evidence for
scoped API, documentation, or naming changes. Missing documentation on public
API items is a High-severity finding.

---

## Key Files

- `README.md` - overview and usage notes

## Expected Handoff Inputs

- Scoped changed-file list for Rust source, tests, and docs-adjacent API changes
- Relevant plan artifacts, especially:
  - `plans/<feature-slug>/plan/function-sig-plan.md`
  - `plans/<feature-slug>/plan/implementation-plan.md`
  - `plans/<feature-slug>/design/behaviors.md`
- Repository layout guidance from `.github/local/directories.md`
- Existing documentation and AST-analysis artifacts, if already captured

---

## Repo-Local Authorities

- `plans/<feature-slug>/plan/function-sig-plan.md` is the authority for expected
  public function names and signatures.
- `plans/<feature-slug>/plan/implementation-plan.md` and
  `plans/<feature-slug>/design/behaviors.md` provide the intended terminology
  and externally visible contracts.
- `.github/local/directories.md` is the authority for locating the Rust source
  tree and mirrored tests.

---

## Tool Commands & Integration

### Tool 1: doc-extractor

**Purpose**: Find undocumented public API items and summarize documentation
coverage across the source tree.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-doc-extractor && cargo build --release

# Find missing documentation
.github/skills/0-external-doc-extractor/run.sh \
  src --tier missing-docs \
  > doc-gaps.json

# Full summary
.github/skills/0-external-doc-extractor/run.sh \
  src --tier summary \
  > doc-summary.json
```

**Output Interpretation**:

Each entry in `doc-gaps.json` identifies a public item without a doc comment.
Map each one to a finding with `"tool": "doc-extractor"`,
`"severity": "high"`, and `"rule": "missing-public-doc"`.

---

### Tool 2: syn-analyzer (doc and naming)

**Purpose**: Detect missing documentation and naming convention violations by
parsing the AST.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-syn-analyzer && cargo build --release

mkdir -p reports
.github/skills/0-external-syn-analyzer/run.sh \
  src \
  --format json \
  --reports missing-docs \
  > reports/syn-docs-report.json
```

**Output Interpretation**:

`findings[]` includes:
- `rule` (`missing-docs`)
- `location` (file:line)
- `message`
- `severity`

Map each finding to the standard diagnostic format using
`"tool": "syn-analyzer"`.

---

## Deterministic Validation Signal

Map the approved command outputs above to the shared `pass|fail`
signal used in Rust Stage 4 review.

- Missing docs on public API items (from doc-extractor or syn-analyzer) → **`fail`** (High)
- Internal undocumented items → **`pass`** with warning

---

## Standard Diagnostic Format

All findings emitted by this skill's tools must be mapped to:

```json
{
  "checker": "consistency-checker",
  "signal": "pass|fail",
  "findings": [
    {
      "severity": "critical|high|medium|low",
      "rule": "<rule-id>",
      "location": "<file>:<line>",
      "message": "<human-readable description>",
      "tool": "doc-extractor|syn-analyzer",
      "evidence": "<raw output snippet or key value>"
    }
  ]
}
```

---

## Review Notes

- `doc-extractor` is rooted in `0-external-doc-extractor`.
- `syn-analyzer` is rooted in `0-external-syn-analyzer`.
- Interpret findings against the scoped changed-file list and the repo-local
  authorities listed above.
