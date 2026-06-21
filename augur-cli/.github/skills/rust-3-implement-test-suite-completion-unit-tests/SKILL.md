---
name: rust-3-implement-test-suite-completion-unit-tests
description: >
  Unit test patterns for Rust using #[cfg(test)] modules, test fixtures, and
  mock traits. Load when implementing unit tests co-located in source files.
---

# Skill: Rust Test Suite Completion - Unit Test Patterns

---

## Unit Test Patterns

### Naming Convention: `test_<subject>_<when>_<then>`

Use test names that document:
- **subject**: What is being tested
- **when**: The condition or scenario
- **then**: The expected outcome

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_divide_when_divisor_is_zero_then_returns_err() {
        let result = divide(10, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_divide_when_inputs_are_valid_then_returns_quotient() {
        let result = divide(10, 2);
        assert_eq!(result, Ok(5));
    }
}
```

Clear names make failures self-explanatory.

---

### Arrange/Act/Assert

Structure tests as Arrange/Act/Assert:

```rust
#[test]
fn test_user_creation_with_valid_email() {
    // Arrange: Set up initial state and inputs
    let email = "test@example.com";
    let name = "Test User";

    // Act: Call the function under test
    let user = User::new(name, email);

    // Assert: Check the result matches expectations
    assert_eq!(user.email, email);
    assert_eq!(user.name, name);
    assert!(user.email.contains("@"));
}
```

- Keeps setup, execution, and assertions distinct
- Makes each test easier to scan

---

### Testing Private Functions

Use `#[cfg(test)]` modules to test private functions directly:

```rust
pub fn public_calculate(x: i32) -> i32 {
    private_helper(x)
}

fn private_helper(x: i32) -> i32 {
    x * 2 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_helper_transforms_input() {
        let result = private_helper(5);
        assert_eq!(result, 11);
    }
}
```

This avoids adding public APIs only for tests.

---

### Mock Trait Pattern for Dependency Injection

Use traits to mock dependencies:

```rust
pub trait Database {
    fn find_user(&self, id: u32) -> Result<User, Error>;
}

pub fn get_user_by_id(db: &dyn Database, id: u32) -> Result<User, Error> {
    db.find_user(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDatabase {
        user: Option<User>,
    }

    impl Database for MockDatabase {
        fn find_user(&self, _id: u32) -> Result<User, Error> {
            self.user.clone().ok_or(Error::NotFound)
        }
    }

    #[test]
    fn test_get_user_by_id_with_existing_user() {
        let mock_db = MockDatabase {
            user: Some(User::new("Alice", "alice@example.com")),
        };
        let result = get_user_by_id(&mock_db, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_user_by_id_with_missing_user() {
        let mock_db = MockDatabase { user: None };
        let result = get_user_by_id(&mock_db, 1);
        assert!(result.is_err());
    }
}
```

This isolates behavior without pulling external dependencies into tests.

---

## Key Files

- `README.md` - overview and usage notes
