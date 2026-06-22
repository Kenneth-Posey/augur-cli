---
name: rust-4-review-security-tools
description: >
  Deterministic commands for Rust Stage 4 security review. Runs cargo clippy
  with unsafe focus and syn-analyzer for bare primitives and magic literals.
  Produces structured findings for unsafe block documentation, input-validation
  gaps, and integer-safety issues.
---

# Skill: Rust Stage 4 - Security Review Tool Commands

---

## When To Use This Skill

Use this skill when a Rust Stage 4 review needs deterministic security
evidence for scoped changes. Unsafe blocks without `// SAFETY:` comments are
Critical; `unsafe_code` clippy violations are High.

---

## Key Files

- `README.md` - overview and usage notes

## Expected Handoff Inputs

- Scoped changed-file list for Rust modules touching unsafe code, parsing,
  boundaries, or public APIs
- Relevant plan artifacts, especially:
  - `plans/<feature-slug>/plan/domain-spec.md`
  - `plans/<feature-slug>/plan/dependency-graph.md`
  - `plans/<feature-slug>/plan/implementation-plan.md`
- Repository layout guidance from `.github/local/directories.md`
- Existing clippy and AST-analysis artifacts, if already captured

---

## Repo-Local Authorities

- `plans/<feature-slug>/plan/domain-spec.md` is the authority for domain
  invariants and externally visible trust boundaries.
- `plans/<feature-slug>/plan/dependency-graph.md` is the authority for approved
  crate and module relationships.
- `.github/local/directories.md` is the authority for locating the Rust source
  tree under review.

---

## Tool Commands

### Tool 1: cargo clippy (unsafe focus)

**Purpose**: Surface unsafe code warnings and lint violations with a focus on
unsafe block justification and integer safety.

**Commands**:
```bash
# Run clippy with unsafe warnings enabled
cargo clippy --all-targets --message-format=json -- -W unsafe_code

# Normalize the captured clippy JSON with cargo-diagnostics
cd .github/skills/0-external-cargo-diagnostics && cargo build --release
.github/skills/0-external-cargo-diagnostics/run.sh \
  clippy-unsafe.json \
  --mode cargo-json \
  > security-clippy.json
```

Capture the `cargo clippy` JSON stream to `clippy-unsafe.json` before running
`cargo-diagnostics`.

**Output Interpretation**:

Map clippy `unsafe_code` lint violations to findings with `"tool": "cargo-clippy"`,
`"severity": "high"`, and `"rule": "unsafe-code-lint"`.

Identify unsafe blocks lacking a `// SAFETY:` comment from the raw output and
map to `"severity": "critical"`, `"rule": "unsafe-missing-safety-comment"`.

---

### Tool 2: syn-analyzer (security patterns)

**Purpose**: Detect bare primitives that should be newtypes (preventing type
confusion) and unexplained numeric magic literals.

**Commands**:
```bash
# Build first (if not already built)
cd .github/skills/0-external-syn-analyzer && cargo build --release

.github/skills/0-external-syn-analyzer/run.sh \
  src \
  --format json \
  --reports bare-primitives,magic \
  > security-syn.json
```

**Output Interpretation**:

- `bare-primitives` findings: functions using raw primitives that should be
  newtypes (prevents type confusion). Public API bare primitive findings →
  `"severity": "high"`, `"rule": "bare-primitive-public-api"`.
- `magic` findings: unexplained numeric literals → `"severity": "low"`,
  `"rule": "magic-literal"`.

Map each finding with `"tool": "syn-analyzer"`.

---

## Deterministic Validation Signal

Map the approved command outputs above to the shared `pass|fail`
signal used across Rust Stage 4 review.

- `unsafe` block without `// SAFETY:` comment → **`fail`** (Critical)
- Clippy `unsafe_code` lint violations → **`fail`** (High)
- Bare primitive findings on public API → **`fail`** (High)
- Magic literal findings only → **`pass`** with warnings

---

## Standard Diagnostic Format

Map all findings to:

```json
{
  "checker": "security-checker",
  "signal": "pass|fail",
  "findings": [
    {
      "severity": "critical|high|medium|low",
      "rule": "<rule-id>",
      "location": "<file>:<line>",
      "message": "<human-readable description>",
      "tool": "cargo-clippy|syn-analyzer",
      "evidence": "<raw output snippet or key value>"
    }
  ]
}
```

---

## Review Notes

- `cargo-diagnostics` is rooted in `0-external-cargo-diagnostics`.
- `syn-analyzer` is rooted in `0-external-syn-analyzer`.
- Interpret unsafe, bare-primitive, and magic-literal findings against the
  scoped changed-file list and the authorities listed above.
