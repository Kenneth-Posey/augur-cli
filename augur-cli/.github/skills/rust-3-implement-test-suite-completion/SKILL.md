---
name: rust-3-implement-test-suite-completion
description: >
  Rust-specific patterns for implementing comprehensive test suites. Teaches identifying missing
  test cases, mirrored `tests/**/*.tests.rs` placement, source-file bridge stubs,
  and cargo test as completion signal. Use when implementing tests to achieve
  full coverage against the test plan.
---

# Rust 3 Implement Test Suite Completion

## Prerequisites and Context

This skill assumes:

- A test plan artifact exists (test cases, behavior references, coverage goals)
- The canonical mirrored `tests/**/*.tests.rs` layout is planned
- Edge cases and error paths are identified
- `cargo test --quiet` coverage expectations are documented

It covers:

- Identifying missing test cases against a test plan
- Structuring mirrored Rust test files and bridge stubs
- Organizing test files under `tests/` using the `.tests.rs` suffix
- Treating `cargo test --quiet` output as the completion signal

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### 1. Test Plan Gap Analysis

Test plan gaps are missing test cases that prevent full coverage. Gap analysis
compares the test plan against implemented tests to identify what remains.

Prefer `cargo test --quiet` for completion checks unless you need full
verbose output to diagnose a failure.

**Gap categories**:
- **Happy path tests**: Main behavior not tested
- **Error path tests**: Error conditions not covered
- **Edge case tests**: Boundary conditions (empty, max, zero, null) not tested
- **Integration tests**: End-to-end behaviors not verified
- **Performance tests**: Regression or performance thresholds not covered

**Gap identification process**:
```
Test Plan: 
  1. User creation with valid email ✓ (test_user_creation_valid exists)
  2. User creation with invalid email ✗ (missing)
  3. User creation with duplicate email ✗ (missing)
  4. Persistence layer integration ✗ (missing)

Gaps identified:
  - Error case: invalid email
  - Error case: duplicate email
  - Integration: persistence
```

**Mapping test to plan**:
```rust
// Test plan entry
/*
Behavior: User Creation Success
Test Case: Create user with valid email and password
Expected: User object returned with ID, persisted in repository
*/

// Implementation
#[test]
fn test_user_creation_with_valid_inputs() {
    // PLAN: "Create user with valid email and password"
    let user = User::new(
        UserId::new(1),
        Email::new("test@example.com".to_string()).unwrap(),
    );

    // PLAN: "User object returned with ID"
    assert_eq!(user.id(), UserId::new(1));

    // PLAN: "persisted in repository"
    // (Covered by separate integration test)
}
```

### 2. Mirrored Test File Organization

Keep Rust test bodies in mirrored `tests/**/*.tests.rs` files. When a mirrored
test file exists, the source file keeps only a `#[cfg(test)]` bridge stub so
tests still compile in module context.

**Placement pattern**:
```rust
// src/domain/user.rs

pub struct User { /* */ }

impl User {
    pub fn new(id: UserId, email: Email) -> Self { /* */ }
    pub fn is_active(&self) -> bool { /* */ }
}

#[cfg(test)]
#[path = "../../tests/domain/user.tests.rs"]
mod tests;

// tests/domain/user.tests.rs
use super::*;

#[test]
fn test_new_user_is_created() {
    let user = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
    assert_eq!(user.id(), UserId::new(1));
}

#[test]
fn test_new_user_is_active() {
    let user = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
    assert!(user.is_active());
}

#[test]
fn test_user_deactivation() {
    let mut user = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
    user.deactivate();
    assert!(!user.is_active());
}
```

**Key discipline**:
- Tests live in mirrored `.tests.rs` files, not inline bodies in source
- `#[cfg(test)]` bridge stubs hide tests from release binaries
- `use super::*;` keeps mirrored test files in the tested module's context
- Test naming: `test_<function>_<scenario>`

### 3. Behavior Tests Under the Canonical Layout

End-to-end or subsystem behavior tests still use the mirrored `.tests.rs`
layout. Place them under the source entrypoint or module whose behavior the
test plan is exercising.

**Placement pattern**:
```
src/                          tests/
  interface/
    user_api.rs     ←→         interface/user_api.tests.rs
  wiring.rs         ←→         wiring.tests.rs
  lib.rs            ←→         lib.tests.rs
```

**Test file structure**:
```rust
// src/interface/user_api.rs
pub async fn create_user(system: &System, request: CreateUserRequest) -> Result<CreateUserResponse> {
    /* ... */
}

#[cfg(test)]
#[path = "../../tests/interface/user_api.tests.rs"]
mod tests;

// tests/interface/user_api.tests.rs
use super::*;
use crate::wiring::System;

#[tokio::test]
async fn test_behavior_user_creation_end_to_end() {
    // Given: System wired with all layers
    let system = System::new();

    // When: Send user creation request through the module's public entrypoint
    let response = create_user(
        &system,
        CreateUserRequest { email: "user@example.com".to_string() },
    )
    .await
    .expect("user creation succeeds");

    // Then: Verify user is created and persisted
    assert!(response.user_id.is_some());

    let user = system.query_user(response.user_id.unwrap())
        .await
        .expect("user persisted");
    assert_eq!(user.email.as_str(), "user@example.com");
}
```

**Key discipline**:
- Use the mirrored `tests/**/*.tests.rs` layout for both narrow and broad behaviors
- Keep the source file limited to the bridge stub when mirrored tests exist
- Use the module or entrypoint that matches the planned behavior as the mirror target
- Test naming: `test_behavior_<behavior_name>`

### 4. Mirrored Test File Placement

Test file organization mirrors source code organization for clarity. Each
source module keeps a same-path partner under `tests/` with the `.tests.rs`
suffix.

**Mirroring pattern**:
```
src/                          tests/
  domain/
    user.rs         ←→         domain/user.tests.rs
    order.rs        ←→         domain/order.tests.rs
  interface/
    user_api.rs     ←→         interface/user_api.tests.rs
  lib.rs            ←→         lib.tests.rs
```

**Navigation aid**: For each source module, there is a corresponding mirrored test
file. Example:
```rust
// src/domain/user.rs
pub struct User { /* */ }
pub fn create_user(...) { /* ... */ }

#[cfg(test)]
#[path = "../../tests/domain/user.tests.rs"]
mod tests;

// tests/domain/user.tests.rs
// Module-context tests for User behavior
```

### 5. Cargo Test as Completion Signal

`cargo test` runs all tests and reports results. A passing full test suite
signals that all planned behaviors and edge cases are covered.

**Completion criteria**:
```sh
cargo test --all-targets
```

Output should show:
- ✓ All unit tests pass
- ✓ All integration tests pass
- ✓ No test failures or panics
- ✓ Coverage report (optional) shows adequately covered code

**Test output interpretation**:
```
$ cargo test

running 10 unit tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out

running 5 integration tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured
```

**Failure cases** (not complete):
```
failures:

---- test_user_creation_invalid_email stdout ----
thread 'test_user_creation_invalid_email' panicked at 'assertion failed: ...'

failures:
    test_user_creation_invalid_email

test result: FAILED. 14 passed; 1 failed;
```

**Completion signal**: When `cargo test` shows "test result: ok" with all expected
tests present (count matches plan), suite is complete.

### 6. Test Case Scenarios and Coverage

Each test case maps to a specific scenario from the test plan. Coverage
includes happy paths, error paths, edge cases, and concurrency scenarios.

**Test scenarios**:
```rust
// tests/domain/user.tests.rs
use super::*;

// HAPPY PATH: Normal operation
#[test]
fn test_user_creation_success() {
    let user = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
    assert!(!user.is_deleted());
}

// ERROR PATH: Precondition violation
#[test]
fn test_invalid_email_format() {
    let result = Email::new("notanemail".to_string());
    assert!(matches!(result, Err(EmailError::InvalidFormat)));
}

// EDGE CASE: Boundary condition
#[test]
fn test_very_long_email() {
    let long_email = format!("{}@example.com", "a".repeat(250));
    let result = Email::new(long_email);
    assert!(matches!(result, Err(EmailError::TooLong)));
}

// EDGE CASE: Empty/zero
#[test]
fn test_empty_email_rejected() {
    let result = Email::new(String::new());
    assert!(matches!(result, Err(EmailError::Missing)));
}

// STATE MACHINE: Valid transitions
#[test]
fn test_user_state_transitions() {
    let mut user = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
    assert_eq!(user.status(), UserStatus::Active);

    user.deactivate();
    assert_eq!(user.status(), UserStatus::Inactive);

    user.reactivate().unwrap();
    assert_eq!(user.status(), UserStatus::Active);
}

// CONCURRENCY: Message ordering
#[tokio::test]
async fn test_concurrent_user_creation() {
    let system = System::new();

    let handle1 = tokio::spawn({
        let system = system.clone();
        async move {
            system.create_user(Email::new("user1@example.com".into()).unwrap()).await
        }
    });

    let handle2 = tokio::spawn({
        let system = system.clone();
        async move {
            system.create_user(Email::new("user2@example.com".into()).unwrap()).await
        }
    });

    let result1 = handle1.await.unwrap().unwrap();
    let result2 = handle2.await.unwrap().unwrap();

    assert_ne!(result1.user_id, result2.user_id);
}
```

## Examples

### Example 1: Closing a Unit Test Gap

**Scenario**: Test plan requires "Email validation rejects invalid format"

**Gap**: Test doesn't exist

**Implementation**:
```rust
// src/domain/email.rs

pub struct Email { /* */ }

impl Email {
    pub fn new(address: String) -> Result<Self, EmailError> { /* */ }
}

#[cfg(test)]
#[path = "../../tests/domain/email.tests.rs"]
mod tests;

// tests/domain/email.tests.rs
use super::*;

#[test]
fn test_email_validation_accepts_valid() {
    let email = Email::new("user@example.com".to_string()).unwrap();
    assert_eq!(email.as_str(), "user@example.com");
}

// NEW TEST: Fill gap from test plan
#[test]
fn test_email_validation_rejects_invalid_format() {
    // PLAN: "Email validation rejects invalid format"
    let test_cases = vec![
        "notanemail",        // No @
        "@example.com",      // No local part
        "user@",             // No domain
        "user@.com",         // Missing domain name
    ];

    for invalid_email in test_cases {
        let result = Email::new(invalid_email.to_string());
        assert!(
            matches!(result, Err(EmailError::InvalidFormat)),
            "Email '{}' should be invalid",
            invalid_email
        );
    }
}
```

**Valid pattern**: Test maps to plan entry, covers multiple invalid formats,
clear assertion message.

### Example 2: Closing an Integration Test Gap

**Scenario**: Test plan requires "User creation end-to-end with persistence"

**Gap**: Integration test doesn't exist

**Implementation**:
```rust
// src/interface/user_api.rs
#[cfg(test)]
#[path = "../../tests/interface/user_api.tests.rs"]
mod tests;

// tests/interface/user_api.tests.rs
use super::*;
use crate::wiring::System;
use crate::domain::Email;

#[tokio::test]
async fn test_behavior_user_creation_persisted() {
    // PLAN: "User creation end-to-end with persistence"
    
    // Given: System wired with all layers
    let system = System::new();

    // When: Create user through the mirrored module entrypoint
    let response = create_user(
        &system,
        CreateUserRequest {
            email: "newuser@example.com".to_string(),
        },
    )
        .await
        .expect("request succeeds");

    // Then: Verify user is persisted
    assert!(response.user_id.is_some(), "Response includes user ID");

    // Query to verify persistence
    let user = system.query_user(response.user_id.unwrap())
        .await
        .expect("user persisted");

    assert_eq!(user.email.as_str(), "newuser@example.com");
    assert!(!user.is_deleted());
}
```

**Valid pattern**: End-to-end test through public API, Given/When/Then structure,
verifies both behavior and persistence.

### Example 3: Identifying and Closing Multiple Gaps

**Scenario**: Test plan specifies 12 test cases; only 8 are implemented

**Gap Analysis**:
```
Plan Test Cases:
  1. Create user success ✓
  2. Create user invalid email ✗
  3. Create user duplicate email ✗
  4. Deactivate user success ✓
  5. Deactivate inactive user ✓
  6. Reactivate user success ✓
  7. Reactivate suspended user (error) ✗
  8. User persistence integration ✗
  9. Concurrent user creation ✗
 10. Query user not found ✓
 11. Delete user success ✓
 12. Delete user not found ✗

Missing (gaps): 2, 3, 7, 8, 9, 12
```

**Implementation**:
```rust
// src/domain/user.rs
#[cfg(test)]
#[path = "../../tests/domain/user.tests.rs"]
mod tests;

// tests/domain/user.tests.rs
// ... existing tests 1, 4, 5, 6, 10, 11 ...

// GAP 2: Invalid email error
#[test]
fn test_create_user_invalid_email() {
    let result = User::new(UserId::new(1), Email::new("notanemail".into()));
    assert!(matches!(result, Err(_)));
}

// GAP 3: Duplicate email error
#[test]
fn test_create_user_duplicate_email() {
    let repo = setup_repo_with_user("user@example.com");
    let result = repo.save(&User::new(UserId::new(2), Email::new("user@example.com".into()).unwrap()));
    assert!(matches!(result, Err(RepositoryError::DuplicateEmail)));
}

// GAP 7: Reactivate suspended fails
#[test]
fn test_reactivate_suspended_user_fails() {
    let mut user = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
    user.suspend().unwrap();
    let result = user.reactivate();
    assert!(matches!(result, Err(UserError::InvalidTransition)));
}

// GAP 12: Delete not found error
#[test]
fn test_delete_user_not_found() {
    let mut repo = Repository::new();
    let result = repo.delete(UserId::new(999));
    assert!(matches!(result, Err(RepositoryError::NotFound)));
}

// GAP 8 & 9: Behavior tests in a mirrored entrypoint file
// tests/interface/user_api.tests.rs

#[tokio::test]
async fn test_user_persistence_integration() {
    // GAP 8
    let system = System::new();
    // ... verify user creation and persistence
}

#[tokio::test]
async fn test_concurrent_user_creation() {
    // GAP 9
    let system = System::new();
    // ... spawn concurrent requests, verify no conflicts
}
```

**Valid pattern**: Each gap identified and mapped to implementation, test count
now matches plan (12 tests).

## Tool Integration

### 1. Running Full Test Suite

Complete test run:
```sh
cargo test --all-targets
```

Run with output:
```sh
cargo test --all-targets -- --nocapture
```

Run specific test:
```sh
cargo test test_email_validation_rejects_invalid_format -- --exact
```

### 2. Coverage Analysis

Install and run tarpaulin:
```sh
cargo install cargo-tarpaulin
cargo tarpaulin --lib --out Html --output-dir reports
```

Identify uncovered lines and implement tests for them.

### 3. Test Organization Verification

Check test file structure:
```sh
find src -name "*.rs" -exec grep -l "#\[cfg(test)\]" {} \;
find tests -name "*.tests.rs" -type f
```

Verify each mirrored test file uses the `.tests.rs` suffix and each source file
with mirrored tests keeps only the bridge stub.

### 4. Clippy for Test Quality

Check test code quality:
```sh
cargo clippy --tests -- -W clippy::all
```

Watch for:
- Panicking in tests that should use `assert!` or `Result`
- Unreadable assertions (use descriptive messages)
- Test functions that don't actually test anything

## Decision Criteria

### When Implementing Tests

1. **Gap Coverage**: All missing test cases from plan are implemented
2. **Scenario Coverage**: Happy path, error paths, edge cases all covered
3. **Organization Correctness**: Tests live in mirrored `tests/**/*.tests.rs`
   files and source files use bridge stubs
4. **Naming Clarity**: Test names clearly indicate what they test
5. **Passing Suite**: `cargo test --all-targets` shows all tests passing

### When Reviewing Test Completion

1. **Test Count Match**: Count matches plan expectations (e.g., 12 tests planned = 12+ tests)
2. **Case Coverage**: Each plan test case is implemented
3. **Scenario Completeness**: Happy path, errors, and edge cases covered
4. **File Placement**: Mirrored `tests/**/*.tests.rs` files exist and source
   files keep only bridge stubs when mirrored tests exist
5. **Execution Success**: `cargo test` output shows all tests passing
