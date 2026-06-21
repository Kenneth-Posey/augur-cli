---
name: rust-3-implement-behavior-wiring
description: >
  Rust-specific patterns for implementing actor wiring and message handling. Teaches how
  to wire actor handles and feeds, construct the composition root, and verify end-to-end
  behavior through public interfaces. Use when implementing runtime orchestration that
  realizes behavioral specifications.
---

# Rust 3 Implement Behavior Wiring

## Prerequisites and Context

This skill assumes:

- A behavior plan artifact exists, mapping behaviors to component interactions
- Actor boundaries and message types are defined
- Function signatures for domain and interface layers exist
- Integration test structure is planned

Use it to:

- Wire actor handles and feed channels
- Construct a `wiring.rs` composition root
- Write integration tests that verify behaviors through public handles
- Trace a Given/When/Then behavior through the wired system

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### 1. Wiring Root Architecture

A composition root is a single module (`wiring.rs`) that instantiates actors,
connects their channels, and returns public handles. Keep initialization
coupling there.

**Key principles**:
- **Single location**: All actor construction in one place
- **Public handles only**: Return handles, not internal actor state
- **Immutable after construction**: Once wired, system is ready for messages
- **Test-friendly wiring**: Supports creating test-only or instrumented variants

**How to structure**:
```rust
// lib/wiring.rs
pub struct System {
    pub handle_a: HandleA,
    pub handle_b: HandleB,
    // All other public handles for test/integration access
}

impl System {
    pub fn new() -> Self {
        // Create actors with channels
        // Wire them together
        // Return handles
    }
}
```

**Tracing a behavior through the wired system**:
1. Start with a Given/When/Then behavior from the behavior plan
2. Identify which public handle the When step uses
3. Follow the message through actor channels and domain layer
4. Verify the Then step's observable outcome (state change, response, or side effect)

### 2. Actor Handle Wiring Pattern

Actor handles are thread-safe, reference-counted endpoints (`Handle<A>`) that
callers use to send messages. Feeds are the receiving end of a message channel.

**Wiring pattern**:
```rust
// Create channel for communication
let (tx, rx) = tokio::sync::mpsc::channel(capacity);

// Actor task spawned with receiver
let actor = MyActor::new(rx);
tokio::spawn(actor.run());

// Return handle (sender) to caller
pub handle = Handle::new(tx);
```

**Composition for multiple actors**:
```rust
pub struct System {
    pub request_handler: Handle<RequestActor>,
    pub domain_processor: Handle<DomainActor>,
    pub persistence: Handle<PersistenceActor>,
}

impl System {
    pub fn new() -> Self {
        let (req_tx, req_rx) = tokio::sync::mpsc::channel(100);
        let (dom_tx, dom_rx) = tokio::sync::mpsc::channel(100);
        let (per_tx, per_rx) = tokio::sync::mpsc::channel(100);

        // Actors hold receiver and may have handles to other actors
        let request_actor = RequestActor::new(req_rx);
        let domain_actor = DomainActor::new(dom_rx, per_tx.clone());
        let persistence_actor = PersistenceActor::new(per_rx);

        tokio::spawn(request_actor.run());
        tokio::spawn(domain_actor.run());
        tokio::spawn(persistence_actor.run());

        System {
            request_handler: Handle::new(req_tx),
            domain_processor: Handle::new(dom_tx),
            persistence: Handle::new(per_tx),
        }
    }
}
```

### 3. Behavior Verification Through Public Handles

Integration tests invoke behaviors through public handles and verify outcomes
through observable state: responses, side effects, or later queries.

**Testing pattern**:
```rust
#[tokio::test]
async fn test_behavior_create_user() {
    // Given: system is wired and ready
    let system = System::new();

    // When: send a CreateUser message through public handle
    let response = system.request_handler
        .send(RequestMsg::CreateUser {
            name: "Alice".to_string(),
        })
        .await
        .expect("request sent");

    // Then: verify observable behavior
    assert_eq!(response.status, Status::Success);

    // Verify downstream state (query via public handle)
    let user = system.query_user(response.user_id)
        .await
        .expect("user exists");
    assert_eq!(user.name, "Alice");
}
```

**Key discipline**: Test only through public handles, not internal actor state.
The test should not reach into actor internals to verify behavior.

### 4. Given/When/Then Wiring

Map each Given/When/Then behavior to an integration test that wires actors,
executes the When step, and verifies the Then step.

**Mapping pattern**:
- **Given**: Set up system state via wiring + initial messages
- **When**: Send the behavior's trigger message through a public handle
- **Then**: Assert observable outcomes (response, state query, or side effects)

**Example from behavior plan**:
```
Behavior: User Registration Success

Given: System is running with database connected
When: POST /users with valid email and password
Then: User is persisted, response includes user ID, email is confirmed in DB
```

**Implementation**:
```rust
#[tokio::test]
async fn given_system_ready_when_post_user_then_persisted() {
    // GIVEN: wire system with all actors
    let system = System::new();
    
    // WHEN: send registration request through HTTP adapter
    // (which sends through request_handler public handle)
    let response = system.request_handler
        .send(RequestMsg::RegisterUser {
            email: "user@example.com".to_string(),
            password: "secure123".to_string(),
        })
        .await
        .unwrap();

    // THEN: verify three aspects
    // 1. Response includes user ID
    assert!(response.user_id.is_some());
    
    // 2. Verify persistence (query through public handle)
    let stored_user = system.query_user(response.user_id.unwrap())
        .await
        .expect("user persisted");
    assert_eq!(stored_user.email, "user@example.com");
    
    // 3. Verify email confirmation workflow started
    // (may be verified via side effect capture or observer pattern)
}
```

### 5. Composition Root Patterns

A composition root can support different configurations (production, test,
instrumented) so you can test specific behaviors in isolation.

**Simple production wiring**:
```rust
pub fn production() -> System {
    System::new()  // Standard wiring with all actors
}
```

**Test-friendly wiring with observability**:
```rust
pub fn test_with_recording() -> (System, Arc<RecordingObserver>) {
    let observer = Arc::new(RecordingObserver::new());
    
    // Construct actors with observer handles cloned in
    let (tx, rx) = mpsc::channel(100);
    let actor = MyActor::new(rx, observer.clone());
    // ... wire rest of system
    
    (system, observer)
}

// In integration test:
let (system, observer) = wiring::test_with_recording();
system.request_handler.send(...).await?;

// Verify message flow through observer
assert!(observer.recorded_message(MessageType::UserCreated));
```

## Examples

### Example 1: Simple Request/Response Wiring

**Scenario**: Implement behavior "Create item returns success response"

**Behavior Plan Entry**:
```
Behavior: Item Creation Success
Given: System wired, item database available
When: Send CreateItem message via request handle
Then: Receive CreateItemResponse with new item ID
```

**Implementation**:
```rust
// lib/wiring.rs
pub struct System {
    pub request_handler: Handle<RequestActor>,
}

impl System {
    pub fn new() -> Self {
        let (req_tx, req_rx) = tokio::sync::mpsc::channel(100);
        let domain_handler = {
            let (tx, rx) = tokio::sync::mpsc::channel(100);
            let actor = DomainActor::new(rx);
            tokio::spawn(actor.run());
            Handle::new(tx)
        };

        let request_actor = RequestActor::new(req_rx, domain_handler);
        tokio::spawn(request_actor.run());

        System {
            request_handler: Handle::new(req_tx),
        }
    }
}

// tests/integration_test.rs
#[tokio::test]
async fn test_behavior_create_item_success() {
    // Given
    let system = System::new();

    // When
    let response = system.request_handler
        .send(RequestMsg::CreateItem {
            name: "Widget".to_string(),
        })
        .await
        .unwrap();

    // Then
    assert_eq!(response.status, Status::Success);
    assert!(response.item_id.is_some());
}
```

**Valid pattern**: Behavior flows through public handle, observable outcome
verified. Test is simple, deterministic, and doesn't reach into internals.

### Example 2: Multi-Actor Choreography

**Scenario**: Behavior involving request → domain → persistence flow

**Behavior Plan Entry**:
```
Behavior: Persisted Item Creation
Given: System wired with all layers, persistence ready
When: CreateItem message sent
Then: Item persisted to database, CreateItemResponse returned with ID
```

**Implementation**:
```rust
pub struct System {
    pub request_handler: Handle<RequestActor>,
    pub query_handler: Handle<QueryActor>, // For Then verification
}

impl System {
    pub fn new() -> Self {
        // Wire persistence
        let (persist_tx, persist_rx) = mpsc::channel(100);
        let persist_actor = PersistenceActor::new(persist_rx);
        tokio::spawn(persist_actor.run());

        // Wire domain with persistence handle
        let (domain_tx, domain_rx) = mpsc::channel(100);
        let domain_actor = DomainActor::new(domain_rx, persist_tx.clone());
        tokio::spawn(domain_actor.run());

        // Wire request with domain handle
        let (req_tx, req_rx) = mpsc::channel(100);
        let request_actor = RequestActor::new(req_rx, domain_tx);
        tokio::spawn(request_actor.run());

        // Query actor for Then verification
        let (query_tx, query_rx) = mpsc::channel(100);
        let query_actor = QueryActor::new(query_rx);
        tokio::spawn(query_actor.run());

        System {
            request_handler: Handle::new(req_tx),
            query_handler: Handle::new(query_tx),
        }
    }
}

#[tokio::test]
async fn test_behavior_item_persisted() {
    // Given
    let system = System::new();

    // When: Send CreateItem request
    let response = system.request_handler
        .send(RequestMsg::CreateItem {
            name: "Widget".to_string(),
            price: Money::from(99.99),
        })
        .await
        .unwrap();

    // Allow a small delay for persistence to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Then: Verify item is persisted
    let query_response = system.query_handler
        .send(QueryMsg::GetItem {
            item_id: response.item_id.clone(),
        })
        .await
        .unwrap();

    assert_eq!(query_response.item.name, "Widget");
    assert_eq!(query_response.item.price, Money::from(99.99));
}
```

**Valid pattern**: Wiring establishes message flow; Given sets up system;
When exercises the behavior through public handle; Then verifies outcomes
via query handle or observable side effects.

### Example 3: Invalid Wiring Patterns

**Invalid pattern 1: Actor state mutation in tests**
```rust
#[test]
fn test_bad_state_mutation() {
    let system = System::new();
    
    // BAD: Direct access to internal state defeats public interface testing
    let actor_ref = &system.request_handler.actor;  // INVALID: internal!
    actor_ref.items.push(Item::new("test"));
    
    // This doesn't test real behavior; it's a false positive
}
```

**Correction**: Use public handles only:
```rust
#[tokio::test]
async fn test_state_via_public_api() {
    let system = System::new();
    
    // VALID: Use public handle to create item
    let response = system.request_handler
        .send(RequestMsg::CreateItem { ... })
        .await
        .unwrap();
    
    // VALID: Query via public handle to verify state
    let item = system.query_handler
        .send(QueryMsg::GetItem { id: response.item_id })
        .await
        .unwrap();
}
```

**Invalid pattern 2: Wiring logic scattered across tests**
```rust
#[test]
fn test_scattered_wiring() {
    // BAD: Each test rebuilds the wiring differently
    let (tx1, rx1) = mpsc::channel(100);
    let actor = RequestActor::new(rx1);
    // ... inline wiring in test
    
    // Another test:
    let (tx2, rx2) = mpsc::channel(200);  // Different capacity!
    let actor2 = RequestActor::new(rx2);
    // ... different wiring
}
```

**Correction**: Centralize wiring in `System::new()`:
```rust
// lib/wiring.rs
pub struct System { /* ... */ }
impl System {
    pub fn new() -> Self {
        // Single wiring, reused by all tests
    }
}

// tests/
#[tokio::test]
async fn test_1() {
    let system = System::new();
    // test behavior 1
}

#[tokio::test]
async fn test_2() {
    let system = System::new();
    // test behavior 2
}
```

## Tool Integration

### 1. Cargo Test Execution

Run all integration tests to verify wiring:
```sh
cargo test --test '*'  # Run all integration tests
cargo test --test integration_test -- --nocapture  # With output
```

Verify behavior-level tests pass:
```sh
cargo test behavior_  # Run all tests with "behavior_" prefix
```

### 2. Module Graph Analysis

Verify wiring.rs is at the correct layer:
```sh
module-graph wiring.rs
```

Should show:
- Depends on: domain, interface, actor definitions
- Depended on by: tests, main (for app setup)
- No cycles

### 3. Clippy Lints for Message Passing

Check for common wiring mistakes:
```sh
cargo clippy --all-targets -- -W clippy::all
```

Watch for:
- Unused channel sends (message dropped)
- Inefficient channel capacity
- Blocking operations in async actors

### 4. Integration Test Instrumentation

For debugging wiring issues, add logging to the composition root:
```rust
impl System {
    pub fn new() -> Self {
        tracing::debug!("Wiring system...");
        
        let (req_tx, req_rx) = mpsc::channel(100);
        tracing::debug!("Request channel created");
        
        let request_actor = RequestActor::new(req_rx);
        tokio::spawn(request_actor.run());
        tracing::debug!("Request actor spawned");
        
        System { request_handler: Handle::new(req_tx) }
    }
}
```

Run with tracing enabled:
```sh
RUST_LOG=debug cargo test -- --nocapture
```

## Decision Criteria

### For behavior-builder

Use these criteria to validate wiring:

1. **Centralized Wiring**: All actor construction in one `System::new()` or similar
2. **Public Handles Only**: Tests receive only thread-safe handles, not actor internals
3. **Behavior Coverage**: Each Given/When/Then behavior has a passing test
4. **Message Flow**: Messages flow from public handle through actors to domain layer
5. **Observable Outcomes**: Behaviors verify outcomes through public API (handles or
   queries), not internal state inspection

### For behavior-reviewer

Use these criteria to validate wiring correctness:

1. **Composition Determinism**: Wiring produces the same actor graph every time
2. **No Synchronization Bugs**: No race conditions in actor startup or shutdown
3. **Handle Availability**: All behaviors have corresponding public handles
4. **Test Isolation**: Each test constructs its own `System` instance (or uses proper
   fixtures)
5. **Given/When/Then Structure**: Tests follow structure; Given wires system, When
   sends message, Then verifies outcome
