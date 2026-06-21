---
name: rust-3-implement-test-suite-completion-examples
description: >
  Annotated code examples for Rust test suite completion patterns - unit,
  integration, property, and async tests. Load when needing concrete
  implementation references.
---

# Rust Test Suite Completion - Examples

---

## Examples

### Example 1: Closing a Coverage Gap (Error Path)

**Scenario**: Code review identifies uncovered error handling branch.

```rust
// src/file_reader.rs - BEFORE (untested error path)
pub fn read_file(path: &str) -> Result<String, Error> {
    let contents = std::fs::read_to_string(path)?;  // ← Uncovered error path
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_file_success() {
        // Existing test covers happy path only
        let result = read_file("test.txt");
        assert!(result.is_ok());
    }
}
```

**Pattern**:

```rust
// src/file_reader.rs - AFTER (error path tested)
pub fn read_file(path: &str) -> Result<String, Error> {
    let contents = std::fs::read_to_string(path)?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_file_success() {
        let result = read_file("test.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_file_not_found() {
        let result = read_file("nonexistent.txt");
        assert!(result.is_err());
        match result {
            Err(Error::NotFound(_)) => { /* expected */ }
            _ => panic!("Expected NotFound error"),
        }
    }
}
```

**Why**: Untested error paths leave production risk.

---

### Example 2: Async Integration Gap

**Scenario**: Async handler function has no tests for timeout/cancellation paths.

```rust
// src/async_handler.rs - BEFORE (missing cancel test)
pub async fn fetch_with_timeout(url: &str, timeout_secs: u64) -> Result<String, Error> {
    tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        fetch(url),
    )
    .await
    .map_err(|_| Error::Timeout)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_with_timeout_success() {
        let result = fetch_with_timeout("http://httpbin.org/delay/1", 10).await;
        assert!(result.is_ok());
    }
}
```

**Pattern**:

```rust
// src/async_handler.rs - AFTER (timeout tested)
pub async fn fetch_with_timeout(url: &str, timeout_secs: u64) -> Result<String, Error> {
    tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        fetch(url),
    )
    .await
    .map_err(|_| Error::Timeout)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_with_timeout_success() {
        let result = fetch_with_timeout("http://httpbin.org/delay/1", 10).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_with_timeout_exceeds_duration() {
        // Mock endpoint that takes longer than timeout
        let result = fetch_with_timeout("http://httpbin.org/delay/30", 1).await;
        assert!(matches!(result, Err(Error::Timeout)));
    }
}
```

**Why**: Happy-path tests miss timeout behavior, a critical error path.

---

### Example 3: State Machine Coverage Gap

**Scenario**: State transitions untested due to hidden control flow paths.

```rust
// src/state_machine.rs - BEFORE (incomplete test coverage)
pub enum State { Idle, Running, Stopped }

pub struct Task {
    state: State,
}

impl Task {
    pub fn transition(&mut self) -> Result<(), Error> {
        match self.state {
            State::Idle => {
                self.state = State::Running;
                Ok(())
            }
            State::Running => {
                self.state = State::Stopped;
                Ok(())
            }
            State::Stopped => Err(Error::InvalidTransition),  // ← Untested
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_to_running() {
        let mut task = Task { state: State::Idle };
        assert!(task.transition().is_ok());
        assert!(matches!(task.state, State::Running));
    }

    #[test]
    fn test_running_to_stopped() {
        let mut task = Task { state: State::Running };
        assert!(task.transition().is_ok());
        assert!(matches!(task.state, State::Stopped));
    }
}
```

**Pattern**:

```rust
// src/state_machine.rs - AFTER (all transitions tested)
pub enum State { Idle, Running, Stopped }

pub struct Task {
    state: State,
}

impl Task {
    pub fn transition(&mut self) -> Result<(), Error> {
        match self.state {
            State::Idle => {
                self.state = State::Running;
                Ok(())
            }
            State::Running => {
                self.state = State::Stopped;
                Ok(())
            }
            State::Stopped => Err(Error::InvalidTransition),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_to_running() {
        let mut task = Task { state: State::Idle };
        assert!(task.transition().is_ok());
        assert!(matches!(task.state, State::Running));
    }

    #[test]
    fn test_running_to_stopped() {
        let mut task = Task { state: State::Running };
        assert!(task.transition().is_ok());
        assert!(matches!(task.state, State::Stopped));
    }

    #[test]
    fn test_stopped_transition_error() {
        let mut task = Task { state: State::Stopped };
        let result = task.transition();
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::InvalidTransition)));
        assert!(matches!(task.state, State::Stopped)); // State unchanged
    }
}
```

**Why**: Test invalid transitions and verify state is unchanged.

---

## Key Files

- `README.md` - overview and usage notes
