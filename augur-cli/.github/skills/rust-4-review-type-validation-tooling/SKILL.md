---
name: rust-4-review-type-validation-tooling
description: >
  Deterministic Rust type-review guidance using workspace cargo baselines,
  clippy, and related commands for scoped review evidence.
---

# Skill: Rust Type Validation - Tooling & Output Interpretation

---

## When To Use This Skill

Use this skill when a Rust review needs deterministic type-validation evidence
for scoped type, lifetime, generic, or unsafe-adjacent changes.

---

## Key Files

- `README.md` - overview and usage notes

## Expected Handoff Inputs

- Scoped changed-file list for Rust modules containing the reviewed types
- Relevant plan artifacts, especially:
  - `plans/<feature-slug>/plan/domain-spec.md`
  - `plans/<feature-slug>/plan/function-sig-plan.md`
  - `plans/<feature-slug>/plan/implementation-plan.md`
- Repository layout guidance from `.github/local/directories.md`
- Existing cargo, clippy, test, or doc output artifacts, if already captured

---

## Repo-Local Authorities

- `plans/<feature-slug>/plan/domain-spec.md` is the authority for type-level
  invariants and semantic intent.
- `plans/<feature-slug>/plan/function-sig-plan.md` is the authority for exposed
  type signatures and generic surfaces.
- `.github/local/directories.md` is the authority for locating source and
  mirrored tests for the reviewed types.

---

## Tool Commands & Integration

### Primary Tool: cargo check

**Purpose**: Type-check Rust code

**Command**:
```bash
cargo check --workspace --all-targets
```

**Output Interpretation**:
- `error[E...]`: Type system violation (blocking)
- `warning[...]`: Lint or deprecation warning (may be blocking depending on rule)

**Integration**: Treat the workspace-wide result as the compile baseline for
the review.

---

### Tool: cargo clippy

**Purpose**: Linting and type-related diagnostics

**Commands**:
```bash
# Workspace clippy baseline from .github/local/identity.md
cargo clippy --workspace -- -D warnings

# Structured diagnostics pass
cargo clippy --workspace --message-format=json -- -D warnings

# Targeted unsafe follow-up when needed for diagnosis
cargo clippy --workspace --message-format=json -- -W unsafe_code
```

**Output Interpretation**:
- `warning: ...` under `clippy::*`: Lint violation that may need addressing
- `#[allow(...)]` attribute: Explicit opt-out (must have justification comment)

**Integration**: The workspace-wide `-D warnings` run is the review baseline.
Use narrower follow-up commands only for diagnosis after recording that result.

---

### Tool: cargo test

**Purpose**: Verify type changes behave correctly in practice

**Commands**:
```bash
# Run workspace tests when type changes need execution confirmation
cargo test --workspace

# Run in release mode when optimizations may reveal issues
cargo test --workspace --release

# Re-run with backtraces when diagnosing failures
RUST_BACKTRACE=1 cargo test --workspace
```

**Output Interpretation**:
- Test failures may indicate unsound type design
- Review failures for broken semantic assumptions

---

### Tool: rustdoc

**Purpose**: Build documentation and verify examples

**Commands**:
```bash
# Build workspace docs
cargo doc --no-deps --workspace

# Optional doc-test follow-up when diagnosing documentation breakage
cargo test --workspace --doc
```

**Output Interpretation**:
- Doc test failures indicate usage examples don't compile
- Documentation clarity can confirm semantic intent

---

## Finding Severity Guidance

- `cargo check --workspace --all-targets` non-zero exit → critical finding
- `cargo clippy --workspace -- -D warnings` non-zero exit → high-severity finding
- Workspace test failures tied to changed types → high-severity finding
- Doc test failures on changed type surfaces → medium-severity finding

---

## Deterministic Validation Signal

Use the repo-approved command outputs above to produce the shared
`pass|fail` signal for Rust Stage 4 review.

- Any critical or high-severity tool finding → **`fail`**
- Medium-only findings, such as scoped doc-test breakage, → **`pass`** with warnings
- Clean required baselines or warning-free outputs → **`pass`**
- Required command timed out or evidence is incomplete → **`fail`**

---

## Review Notes

- Use the changed-file list and repo-local authorities above to decide which
  diagnostics apply.
- Prefer existing tool artifacts when available; otherwise run the commands in
  this skill and capture their outputs as review evidence.

---
