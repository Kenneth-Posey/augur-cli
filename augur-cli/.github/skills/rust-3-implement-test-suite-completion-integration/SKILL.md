---
name: rust-3-implement-test-suite-completion-integration
description: >
  Integration testing patterns for Rust using the tests/ directory, test
  fixtures, and service/database mocks. Load when implementing integration
  tests across module boundaries.
---

# Skill: Rust Test Suite Completion - Integration Testing Patterns

## Integration Testing Organization

### Project Layout

```
my_crate/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── module_a.rs
│   └── module_b.rs
├── tests/
│   ├── common/
│   │   └── mod.rs           # Shared utilities
│   ├── test_full_workflow.rs
│   └── test_error_handling.rs
```

**Key Points**:
- Put integration tests in `tests/` at crate root, not `src/`.
- Each test file compiles as a separate binary.
- Put shared utilities in `tests/common/mod.rs` or its subdirectories.
- Tests can only use public API.

---

### Multi-File Integration Test Pattern

```rust
// tests/common/mod.rs - Shared test utilities
use my_crate::{Client, Config};

pub fn setup_test_client() -> Client {
    let config = Config::new()
        .with_timeout(Duration::from_secs(5))
        .with_retries(3);
    Client::new(config)
}

pub fn assert_response_valid(response: &Response) {
    assert!(!response.body.is_empty());
    assert!(response.status == 200 || response.status == 201);
}
```

```rust
// tests/test_full_workflow.rs - Integration test
use my_crate::{Request, Response};

mod common;
use common::{setup_test_client, assert_response_valid};

#[test]
fn test_client_request_response_cycle() {
    // Arrange
    let client = setup_test_client();
    let request = Request::new("https://api.example.com/users");

    // Act
    let response = client.execute(request);

    // Assert
    assert!(response.is_ok());
    let resp = response.unwrap();
    assert_response_valid(&resp);
}

#[test]
fn test_client_handles_network_error() {
    // Arrange
    let client = setup_test_client();
    let request = Request::new("https://invalid.unreachable.local/endpoint");

    // Act
    let response = client.execute(request);

    // Assert
    assert!(response.is_err());
}
```

**Key Points**:
- Each test file can declare `mod common;` to use shared utilities.
- Shared code lives under `tests/common/`; test files still compile independently.
- Tests only access public API, not `pub(crate)` or private items.
- Each test file becomes a separate binary under `target/debug/deps/`.

---

### Testing Multi-Module Interactions

```rust
// tests/test_user_workflow.rs - Multi-module integration test
use my_crate::users::{UserService, UserId};
use my_crate::auth::AuthService;
use my_crate::database::Database;

#[test]
fn test_user_creation_and_authentication() {
    // Arrange: Create services
    let db = Database::in_memory();
    let user_service = UserService::new(&db);
    let auth_service = AuthService::new(&db);

    // Act: Create user and authenticate
    let user_result = user_service.create_user("alice", "password123");
    assert!(user_result.is_ok());

    let user = user_result.unwrap();
    let auth_result = auth_service.authenticate(user.id, "password123");

    // Assert: User created and authenticated successfully
    assert!(auth_result.is_ok());
    assert!(auth_result.unwrap().is_authenticated);
}

#[test]
fn test_user_workflow_with_invalid_credentials() {
    // Arrange: Create services
    let db = Database::in_memory();
    let user_service = UserService::new(&db);
    let auth_service = AuthService::new(&db);

    // Act: Create user with one password, try to auth with another
    user_service.create_user("bob", "correct_password").unwrap();
    let auth_result = auth_service.authenticate(
        UserId::new(1),
        "wrong_password",
    );

    // Assert: Authentication should fail
    assert!(auth_result.is_err());
}
```

**Key Points**:
- Exercise workflows that span modules.
- Verify public contracts between services.
- Catch integration failures unit tests can miss.
- Prefer in-memory or test databases for isolation.

---

### Testing Error Paths in Integration

```rust
// tests/test_error_scenarios.rs
use my_crate::api::{Client, Error};
use std::time::Duration;

#[test]
fn test_api_request_timeout() {
    let client = Client::new().with_timeout(Duration::from_millis(100));
    let request = client.request("https://slow-server.example.com/data");

    let result = request.execute();

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::Timeout));
}

#[test]
fn test_api_returns_error_on_invalid_json() {
    // Arrange: Mock server returns invalid JSON
    let client = Client::new();

    // Act: Call API
    let result = client.get_json::<MyStruct>("https://api.example.com/invalid");

    // Assert: Should get parse error
    assert!(result.is_err());
}

#[test]
fn test_cascading_error_recovery() {
    // Arrange
    let db = Database::in_memory();
    let service = Service::new(&db);

    // Act: First operation fails, second should still work
    let first = service.operation_a().err(); // Intentional error
    let second = service.operation_b();      // Should still work

    // Assert: First failed, second succeeded
    assert!(first.is_some());
    assert!(second.is_ok());
}
```

**Key Points**:
- Verify error handling across module boundaries.
- Test timeout behavior at the system level.
- Verify recovery after failures.
- Use real or stubbed infrastructure as needed.

---

## Key Files

- `README.md` - overview and usage notes
