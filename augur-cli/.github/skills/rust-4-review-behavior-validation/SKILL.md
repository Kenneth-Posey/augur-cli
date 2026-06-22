---
name: rust-4-review-behavior-validation
description: >
  Rust-specific behavioral validation via test execution, coverage measurement,
  and panic detection. Validates that implementation satisfies behavioral
  requirements. Use when verifying tests pass, coverage meets targets, and
  library code is panic-safe.
---

# Rust 4 Review Behavior Validation

## Overview

**Authority boundary**: Observable behavioral correctness only. Review changed
Rust code and test evidence against the feature handoff files. Do not use this
skill for architectural placement, performance tuning, or type-shape review.

## Key Files

- `README.md` - overview and usage notes

## Review Role

This skill reviews Rust behavior by combining changed code, repo-local
authorities, and test or coverage evidence, then emits the shared
`pass|fail` signal.

## Scope

### What This Skill Validates

1. **Test Execution**
   - The workspace test baseline passes under `cargo test --workspace --quiet`
   - All integration tests pass
   - Doc tests compile and execute successfully
   - Test exit code is 0 (success)
   - No test panics or timeouts

2. **Code Coverage**
   - Coverage percentage meets or exceeds threshold (default: 80%)
   - Covered lines are measured via tarpaulin or similar tool
   - Coverage report is generated and archivable
   - Uncovered lines are justified or marked as acceptable

3. **Library Code Panic Safety**
   - No `unwrap()`, `expect()`, `panic!()` in production library code
   - `?` operator is used for error propagation
   - Error handling is explicit via `match` or similar
   - Test code and binary code may use unwrap for setup
   - All panics are documented and justified

4. **Feature Completeness**
   - All planned features are implemented (not stubs)
   - Features are discoverable via public API or documentation
   - Features have corresponding test coverage
   - Feature flags are properly declared in Cargo.toml

### Coverage Boundaries

This skill assumes:
- Code compiles without errors (`cargo build` succeeds)
- Test infrastructure is present (tests/ directory or inline tests)
- Feature flags are properly declared
- Coverage tooling (tarpaulin) is available
- Relevant handoff files are available for comparison

## Key Concepts

### 1. Test Completeness

**What it is**: All code paths in library (public API and internal helpers) must have
corresponding tests. Test completeness is validated by:
- Unit tests for individual functions and modules
- Integration tests for component interactions and end-to-end flows
- Doc tests for public API usage examples

**How to validate**:
- Confirm `cargo test --workspace --quiet` passed
- Verify no test failures or panics in output
- Check that all test categories (unit, integration, doc) execute
- Confirm test count matches or exceeds the handoff authority in
  `plans/<feature-slug>/plan/test-strategy-plan.md`

**Example: Valid Test Coverage**
```
$ cargo test --workspace --quiet
   Compiling my-lib v0.1.0
    Finished test [unoptimized + debuginfo] target(s) in 0.42s
     Running unittests src/lib.rs
     Running tests/integration.rs

test result: ok. 47 passed; 0 failed; 0 ignored
```

### 2. Coverage Threshold Enforcement

**What it is**: Code coverage measures the percentage of code lines executed during tests.
Default threshold is 80% for library code.

**How to validate**:
- Measure coverage using `cargo tarpaulin --out Html --output-dir reports`
- Confirm coverage percentage >= threshold (80%)
- Identify uncovered lines and justify their absence
- Exempt test modules, binary-only code, and explicitly allowed dead code

**Example: Valid Coverage**
```
$ cargo tarpaulin --out Html --output-dir reports
   Compiling my-lib v0.1.0
Generating report
    Finished report generation

Coverage: 85.3% (102/120 lines executed)
Report written to reports/tarpaulin-report.html
```

### 3. Library Code Panic Safety

**What it is**: Library code (public API, internal helpers) must not panic at runtime
in production use. Panics crash the caller's application.

**How to validate**:
- Scan library code for panic-inducing functions: `unwrap()`, `expect()`, `panic!()`
- Verify `?` operator is used for error propagation
- Check that error handling is explicit via `match` or similar
- Permit panics only in test code (`#[cfg(test)]`) or binary code (`src/bin/`)

**Example: Valid Panic Safety**
```rust
// VALID: Error propagated via ?
pub fn parse(input: &str) -> Result<Config> {
    let json = serde_json::from_str(input)?;
    Ok(Config::from(json))
}

// INVALID: Panic in library code
pub fn parse_unchecked(input: &str) -> Config {
    serde_json::from_str(input).unwrap()  // Will panic on error
}

// VALID: Panic in test code
#[cfg(test)]
mod tests {
    #[test]
    fn test_parsing() {
        let result = parse("{}").unwrap();  // OK in test
        assert!(result.is_valid());
    }
}
```

### 4. Feature Completeness

**What it is**: All planned features must be implemented (not stubs), discoverable
via public API, and tested.

**How to validate**:
- List planned features from specification or Cargo.toml
- Check that each feature has corresponding code in `src/`
- Verify feature is exported in `lib.rs` or public module
- Confirm at least one test references the feature

**Example: Valid Feature Implementation**
```toml
[features]
feature_foo = []
feature_bar = []
```

```rust
#[cfg(feature = "feature_foo")]
pub mod foo {
    pub fn do_something() { }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(feature = "feature_foo")]
    fn test_foo() {
        foo::do_something();
    }
}
```

## Composition & References

### Review Authorities

- `plans/<feature-slug>/design/behaviors.md` - expected behavior transitions,
  visible outputs, and failure modes.
- `plans/<feature-slug>/plan/test-strategy-plan.md` - planned coverage,
  fixtures, and execution scope.
- `plans/<feature-slug>/plan/implementation-plan.md` - completion claims and
  feature scope to verify.
- `Cargo.toml`, `src/`, and `tests/` - implemented feature flags, public
  surfaces, and executable tests.
- [`.github/local/directories.md`](../../local/directories.md) - canonical test
  placement and mirroring rules.

### Review Output

```
Changed code + review artifacts
    ↓
Behavior review in this skill
    ↓
Evidence (test output, coverage report, panic scan, feature audit)
    ↓
Findings mapped back to handoff files with a `pass|fail` signal
```

## Review Signal

Use the shared `pass|fail` vocabulary for this review.

| Condition | Signal |
|----------|--------|
| Critical behavioral findings present | `fail` |
| Only warning-level concerns or supportive evidence remains | `pass` with warnings |
| Validation timed out or required evidence is incomplete | `fail` |

### Evidence Sources

Use current review artifacts when they are part of the handoff. If fresh
evidence is required, the repo-approved commands are:

1. **cargo test** - Establish the workspace test baseline
    ```sh
    cargo test --workspace --quiet
    ```
    Extract test counts, failing cases, and error messages. Treat narrower reruns
    such as `cargo test --all-features --quiet` as diagnostic follow-up only;
    they do not replace the workspace baseline.

2. **cargo tarpaulin** - Measure code coverage
   ```sh
   cargo tarpaulin --out Html --output-dir reports --timeout 300
   ```
   Generate LCOV and HTML reports; extract coverage percentage.

3. **grep + cargo expand** - Scan for panics in library code
   ```sh
   grep -r "unwrap\|expect\|panic\|unreachable" src/ | grep -v "^src/bin/" | grep -v "#\[cfg(test)\]"
   ```
   Identify panic-inducing functions in production paths.

4. **cargo doc** - Verify feature discoverability
   ```sh
   cargo doc --all-features --no-deps
   ```
   Check that public API is documented and discoverable.

**How to interpret tool output**:
- `test result: ok` → All tests passed
- `Coverage: 85%` → Coverage target met
- No output from panic grep → Library code is panic-safe
- Feature in public module → Feature is discoverable

## Examples

### Example 1: Supportive Evidence

**Input**: Implementation with tests and coverage.

**Test Output**:
```
$ cargo test --workspace
test result: ok. 42 passed; 0 failed; 0 ignored
```

**Coverage Output**:
```
$ cargo tarpaulin --out Html --output-dir reports
Coverage: 85.2% (102/120 lines executed)
```

**Panic Scan**:
```
$ grep -r "unwrap\|expect" src/ | grep -v bin | wc -l
0
```

**Feature Checklist**:
- `feature_foo`: Implemented in `src/foo.rs`, tested in `tests/foo_integration.rs`
- `feature_bar`: Implemented in `src/bar.rs`, tested in `tests/bar_integration.rs`

**Interpretation**: The evidence supports the behavioral contract. Tests pass,
coverage exceeds the stated target, no production-library panic paths were
found, and the planned features are discoverable.

---

### Example 2: Coverage Gap Evidence

**Coverage Output**:
```
$ cargo tarpaulin --out Html --output-dir reports
Coverage: 62.3% (74/119 lines executed)

Uncovered lines:
  - src/error_handler.rs:45-52 (error recovery path)
  - src/cache.rs:88-105 (eviction policy)
```

**Issue**: Coverage is below 80% threshold.  
**Root Cause**: Error recovery path and cache eviction policy lack test coverage.

**Remediation**:
- Add tests for error recovery scenarios
- Add tests for cache eviction under memory pressure
- Target 80%+ coverage

**Interpretation**: The evidence shows a blocking behavioral gap. Coverage is
below the stated threshold, and the untested paths include error recovery and
cache eviction behavior.

---

### Example 3: Panic-Safety Evidence

**Panic Scan Output**:
```
$ grep -r "unwrap\|expect" src/
src/parser.rs:42:    json.get("config").unwrap()  // Direct unwrap in public function
src/handler.rs:18:   options.unwrap_or_default()  // OK: uses unwrap_or with default

Issue: src/parser.rs line 42 has unwrap() in public code path
```

**Issue**: Library code contains unwrap that will panic on error.  
**Root Cause**: Public `parse_config()` function panics if "config" key is missing.

**Remediation**:
- Change to `json.get("config").ok_or(ParseError::MissingKey)?`
- Return error to caller instead of panicking

**Interpretation**: The evidence shows a blocking behavioral mismatch. Public
library code still contains a panic-inducing path, so malformed input can crash
the caller instead of producing an error result.

---

### Example 4: Supportive Evidence with Review Notes

**Coverage Output**:
```
Coverage: 78.5% (94/120 lines executed)  # 1.5% below threshold

Uncovered lines:
  - src/diagnostics.rs:15-20 (debug logging, low priority)
  - src/legacy_compat.rs:5-12 (deprecated path, will be removed)
```

**Justification**: Uncovered lines are debug logging and deprecated paths.

**Interpretation**: The evidence is broadly supportive, but it carries notable
review notes. Coverage is slightly below the stated target, and the uncovered
lines are limited to debug and deprecated paths that should still be tracked.

---

### Example 5: Test-Failure Evidence

**Test Output**:
```
$ cargo test --workspace
test result: FAILED. 39 passed; 3 failed; 0 ignored

failures:

---- tests::integration::test_concurrent_access stdout ----
thread 'tests::integration::test_concurrent_access' panicked at 'assertion failed: ...

---- tests::integration::test_timeout_behavior stdout ----
thread 'tests::integration::test_timeout_behavior' panicked at 'timeout exceeded'
```

**Issue**: 3 tests failed.  
**Root Causes**: 
- Concurrent access test assertion failed
- Timeout test did not meet timing expectations

**Interpretation**: The evidence shows blocking behavioral issues. The failing
tests point to a concurrent-access defect and incorrect timeout handling.

## Decision Criteria

### Severity Classification

Use these criteria to classify findings and set severity:

| Finding Type | Severity | Reason |
|---|---|---|
| Test failure (any test fails) | Critical | Behavioral contract not met |
| Coverage < threshold (default 80%) | Critical | Code paths untested |
| Panic detected in library code | Critical | Library will crash caller on error |
| Feature listed but not implemented | Critical | Feature requirement not met |
| All tests pass | Supporting evidence | Behavioral contract currently supported |
| Coverage >= threshold | Supporting evidence | Code paths adequately exercised |
| Library code panic-safe | Supporting evidence | Safety expectation currently supported |
| All features implemented | Supporting evidence | Feature requirements appear implemented |

### Finding Interpretation Guidance

Use these criteria to interpret the review evidence and set the shared
`pass|fail` signal:

1. **Critical findings present**: Describe them as blocking behavioral issues.
    - Test failures
    - Coverage below threshold
    - Panics in library code
    - Missing features

2. **Warnings present**: Describe them as notable review concerns and explain
   why they may or may not block follow-up work.
    - Coverage slightly below threshold with good justification
    - Minor test flakiness (isolated)
    - Deprecated code that will be removed

3. **No critical findings**: Describe the evidence as supportive and call out
   any remaining limits or assumptions.
    - All tests pass
    - Coverage >= threshold
    - Library code is panic-safe
    - All features implemented

**Suggested review summary pattern**:
- If critical findings exist, list them first and tie each one to the affected
  behavioral contract.
- If only warning-level concerns remain, explain their scope, justification, and
  follow-up expectations.
- If the evidence is clean, state which test, coverage, panic-scan, and feature
  checks support that conclusion.

## Validation Rules

### Test Execution Rules

1. **All Tests Pass**: Exit code from `cargo test --workspace --quiet` is 0.
   No test failures, panics, or timeouts.

2. **Test Count Meets or Exceeds Plan**: Number of passing tests >= planned test count
   from `plans/<feature-slug>/plan/test-strategy-plan.md`.

3. **All Test Categories Included**: Unit tests, integration tests, and doc tests
   all execute successfully.

4. **No Test Skips**: `#[ignore]` tests are skipped only for documented reasons
   (performance, environment-dependent, etc.).

5. **Deterministic Results**: Tests pass consistently when run multiple times;
   no flakiness or race conditions.

### Coverage Rules

1. **Coverage >= Threshold**: Coverage percentage >= 80% (default threshold).
   Measured via `cargo tarpaulin` or equivalent tool.

2. **Uncovered Lines Justified**: Lines not covered are documented as acceptable
   (debug code, deprecated, unreachable).

3. **Critical Paths Covered**: Public API and error paths have >90% coverage.
   Internal helpers have >= threshold coverage.

4. **Coverage Report Archived**: Coverage reports (HTML, LCOV) are generated
   and archivable for trend analysis.

5. **No Coverage Regressions**: Coverage >= prior release coverage (if available).

### Library Panic Safety Rules

1. **No unwrap() in Production**: Library code does not call `unwrap()` on
   `Result` or `Option` unless immediately followed by error handling.

2. **No expect() in Production**: Library code does not call `expect()` on
   `Result` or `Option` (except in test utilities).

3. **No panic!() Calls**: Library code does not directly call `panic!()` except
   in documented debug assertions.

4. **Error Propagation Explicit**: Errors are propagated via `?` operator or
   explicit `match`; never via implicit panic.

5. **Test Code May Panic**: Test code (under `#[cfg(test)]` or in `tests/`) may
   use `unwrap()` for setup and assertions.

### Feature Completeness Rules

1. **All Features Implemented**: Every planned feature has corresponding code;
   no stubs or incomplete implementations.

2. **Features Discoverable**: Feature implementations are exported in `lib.rs`
   or public modules; `#[cfg(feature = "...")]` is used correctly.

3. **Features Tested**: Every feature has at least one corresponding test;
   tests use the same `#[cfg(feature = "...")]` conditions.

4. **Feature Flags Declared**: All features are listed in `Cargo.toml` under
   `[features]` section.

5. **No Dead Features**: All declared features have corresponding code; no
   orphaned feature flags.
