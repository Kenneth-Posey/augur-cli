---
name: rust-3-implement-test-suite-completion-validation
description: >
  Decision rules and validation checklists for Rust test suite completion. Load
  when validating test coverage gaps or deciding which test categories to apply.
---

# Rust Test Suite Completion - Validation & Decision Rules

## Validation Rules

### Organization Validation

**Rule 1**: Unit tests for a public function MUST be in `#[cfg(test)]` module adjacent to implementation  
**Rule 2**: Integration tests (> 1 module) MUST be in `tests/` directory at crate root  
**Rule 3**: Each test MUST follow naming pattern: `test_<subject>_<when>_<then>`  
**Rule 4**: Each test MUST have Arrange/Act/Assert structure (no mixing phases)  

### Coverage Validation

**Rule 5**: All public functions MUST have >= 1 test covering happy path  
**Rule 6**: All error-returning functions MUST have >= 1 test covering error case(s)  
**Rule 7**: All conditional logic MUST be tested for both branches (minimum)  
**Rule 8**: Async code MUST use `#[tokio::test]` with explicit timeout handling  
**Rule 9**: State-dependent code MUST test all reachable state transitions  

### Test Quality Validation

**Rule 10**: Each test MUST test exactly ONE behavior (single assertion focus)  
**Rule 11**: Tests MUST NOT depend on test execution order  
**Rule 12**: Mock/fixture code MUST be separate from test logic  
**Rule 13**: Tests MUST NOT commit side effects (files, environment changes)  
**Rule 14**: Property-based tests MUST encode mathematical invariants, not random assertions  

### Async Pattern Validation

**Rule 15**: Timeout-sensitive async code MUST have explicit timeout tests  
**Rule 16**: Cancellation-capable async code MUST test cancellation paths  
**Rule 17**: Concurrent code MUST use `tokio::sync` primitives in tests  

---

## Key Files

- `README.md` - overview and usage notes

## Workflows

### Test Gap Identification Workflow

Given a code module with existing tests:
1. Identify all public functions
2. List reachable execution paths (branches, error cases, state transitions)
3. Cross-reference against existing test coverage
4. Flag uncovered paths as gaps

Result: a gap list with scenario descriptions.

---

### Test Implementation Workflow

Given a gap list from earlier analysis:
1. For each gap: Choose test pattern (inline vs. integration vs. property-based)
2. Write the test with that pattern
3. Run test to verify it fails (Red phase of TDD)
4. Implement code to pass test
5. Verify all existing tests still pass
6. Check coverage with `tarpaulin --out Html`

Result: implemented tests and a coverage report.

---

### Code Review Workflow (Test Reviewer)

Given a test implementation PR:
1. Verify test names follow `test_<subject>_<when>_<then>` pattern
2. Verify Arrange/Act/Assert structure is clear
3. Check that test is testing ONE behavior
4. Verify error paths tested for all error-returning functions
5. Check async code uses appropriate timeout/cancellation tests
6. Run tests locally: `cargo test --all-features`
7. Verify coverage threshold met: `tarpaulin --exclude-files tests/`

Result: approval or review feedback.

---
