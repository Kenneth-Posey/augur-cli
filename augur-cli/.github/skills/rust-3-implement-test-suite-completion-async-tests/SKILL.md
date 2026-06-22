---
name: rust-3-implement-test-suite-completion-async-tests
description: >
  Async test patterns for Rust using tokio::test and async-std. Load when
  implementing tests for async functions, futures, or actor message flows.
---

# Skill: Rust Test Suite Completion - Async Testing Patterns

## Async Test Patterns

### `#[tokio::test]` for Async Unit Tests

Use `#[tokio::test]` for most async unit tests:

```rust
#[tokio::test]
async fn test_fetch_user_success() {
    // Arrange
    let client = AsyncClient::new();
    
    // Act
    let result = client.fetch_user(1).await;
    
    // Assert
    assert!(result.is_ok());
    let user = result.unwrap();
    assert_eq!(user.id, 1);
}

#[tokio::test]
async fn test_fetch_user_not_found() {
    // Arrange
    let client = AsyncClient::new();
    
    // Act
    let result = client.fetch_user(99999).await;
    
    // Assert
    assert!(result.is_err());
}
```

**Key Points**:
- Use `#[tokio::test]` instead of `#[test]` for async functions.
- The default current-thread runtime is enough for most unit tests.
- Apply it only to `async fn` tests; the runtime awaits the future for you.

---

### Multi-Threaded Runtime for Concurrency Tests

For concurrent behavior, use the multi-threaded Tokio runtime:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_concurrent_requests() {
    // Arrange: Create a channel for collecting results
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let client = AsyncClient::new();
    
    // Act: Spawn multiple concurrent tasks
    for i in 0..10 {
        let client = client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let result = client.fetch_user(i).await;
            let _ = tx.send(result).await;
        });
    }
    drop(tx);
    
    // Assert: Collect results from all tasks
    let mut success_count = 0;
    while let Some(result) = rx.recv().await {
        if result.is_ok() {
            success_count += 1;
        }
    }
    assert_eq!(success_count, 10);
}
```

**Key Points**:
- Use `#[tokio::test(flavor = "multi_thread")]` when the test relies on concurrency.
- It supports `tokio::spawn` and interactions across multiple tasks.
- Drop the original sender so `recv().await` can finish.
- Tasks can run on multiple OS threads.

---

### Timeout Handling

Wrap slow or blocking async operations in explicit timeouts:

```rust
#[tokio::test]
async fn test_request_timeout() {
    // Arrange
    let client = AsyncClient::new();
    let timeout_duration = tokio::time::Duration::from_secs(1);
    
    // Act
    let result = tokio::time::timeout(
        timeout_duration,
        client.fetch_user_with_delay(10), // Would take 10 seconds
    ).await;
    
    // Assert
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), tokio::time::error::Elapsed { .. }));
}
```

**Key Points**:
- Use `tokio::time::timeout` to keep tests from hanging.
- Timeouts document expected timing behavior and catch regressions.

---

### Cancellation Testing

Verify that cancelled tasks clean up correctly:

```rust
#[tokio::test]
async fn test_cancellation_cleanup() {
    // Arrange
    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();
    
    // Act
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut cancel_rx => {
                    // Cleanup on cancellation
                    return true;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Do work
                }
            }
        }
    });
    
    // Wait briefly, then cancel
    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    let _ = cancel_tx.send(());
    
    // Assert
    let result = task.await;
    assert!(result.is_ok());
}
```

**Key Points**:
- Use `tokio::select!` to model cancellation paths.
- Assert cleanup behavior when the task is cancelled.
- This protects graceful shutdown behavior.

---

## Key Files

- `README.md` - overview and usage notes
