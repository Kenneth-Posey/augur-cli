---
name: rust-2-plan-behavior-planning
description: >
  Maps Rust behavior specifications (state machines, decision trees, control flows) to
  idiomatic Rust using enums, match expressions, Result types, actor traits, and
  type-state patterns. Use when turning a behavior plan into concrete Rust types and
  transitions.
---

# Rust 2 Plan Behavior Planning

## Inputs

Use this skill after the behavior is captured in plan files. Prefer:

- `plans/<feature-slug>/design/behaviors.md` for states, transitions,
  decisions, and actions.
- `plans/<feature-slug>/plan/domain-spec.md` for invariants, domain terms, and
  allowed failure outcomes.
- `plans/<feature-slug>/plan/dependency-graph.md` for actor/message edges,
  ownership direction, and module boundaries.
- `plans/<feature-slug>/plan/implementation-plan.md` for deployment or runtime
  constraints that affect the Rust shape.

Use it to define:

- **Rust type mapping**: How each behavior construct maps to Rust types (enum, struct, trait).
- **Exhaustiveness enforcement**: Rust compiler ensures all states/transitions are handled.
- **Error representation**: Result<T, E> and Option<T> for decision points and errors.
- **Actor trait patterns**: Async/concurrent behavior modeled as message-passing trait implementations.
- **Type-state patterns**: Compile-time invariant enforcement via phantom types.
- **Zero-cost abstractions**: State machines compile without extra behavior-modeling overhead.

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### 1. State Representation via Enums

**Principle:** Every distinct state in the behavior spec maps to an enum variant.
Variant fields hold state-specific data.

**Pattern:**
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MyState {
    Idle,
    Processing { task_id: u64, attempt: u8 },
    Failed { reason: String },
    Complete { result: T },
}
```

**Benefit:** Compile-time exhaustiveness checking ensures no state is forgotten in match
branches. Type system prevents invalid state combinations.

**When to use:**
- States are fixed and known upfront (not discovered at runtime).
- Each state has distinct behavior or preconditions.
- Transitions are deterministic based on state/event pair.

**When NOT to use:**
- State space is unbounded or data-driven.
- States share all fields (use generic state struct instead).

---

### 2. Transitions as Match Expressions

**Principle:** State transitions are encoded as `match` arms on (current state, input event).
Result type captures success or transition error.

**Pattern:**
```rust
fn transition(state: MyState, event: Event) -> Result<MyState, TransitionError> {
    match (state, event) {
        (MyState::Idle, Event::Start(task)) => {
            Ok(MyState::Processing { task_id: task.id, attempt: 1 })
        }
        (MyState::Processing { .. }, Event::Retry) => {
            Ok(MyState::Processing { task_id, attempt: attempt + 1 })
        }
        (MyState::Processing { .. }, Event::Cancel) => {
            Ok(MyState::Idle)
        }
        (_, Event::Reset) => Ok(MyState::Idle),
        (current, event) => Err(TransitionError::Invalid { 
            state: current, 
            event 
        }),
    }
}
```

**Benefit:**
- Rustc enforces that all state/event combinations are handled; impossible transitions
  become compilation errors.
- Default arm catches invalid transitions; no silent failures.
- Pattern guards allow conditional transitions.

**Validation:** Compile with `cargo check` and ensure no `unreachable_patterns` warnings.

---

### 3. Decisions via Result and Option

**Principle:** Behavior decision points (success/failure branches, optional paths) map
to `Result` and `Option` types.

**Pattern:**
```rust
pub trait Behavior {
    fn execute(&mut self) -> Result<Outcome, BehaviorError>;
}

pub enum Outcome {
    Success(Data),
    PartialSuccess { completed: Vec<Data>, failed: Vec<Error> },
    Retry { delay_ms: u64, reason: String },
}
```

**Benefit:** Forces explicit error handling at compile time; no silent failures.
Caller must handle both success and error branches.

**Decision rule:**
- Decision with two outcomes (success/failure) → `Result<Success, Error>`
- Optional value → `Option<T>` (only if "not present" is not an error)
- Multiple distinct outcomes → Custom `enum` wrapping `Result`

---

### 4. Actor Pattern for Concurrency

**Principle:** When behavior involves concurrent tasks, model each concurrent worker
as a trait implementing behavior operations. Messages flow through async channels.

**Pattern:**
```rust
#[async_trait::async_trait]
pub trait Actor {
    async fn handle(&mut self, msg: Message) -> Result<Response, ActorError>;
}
```

**Benefits:**
- Clear message-passing boundaries.
- Easier to reason about concurrency and test isolation.
- Actor handles its own state; no shared mutable state needed.
- Compiler prevents data races if actor state is not Send/Sync.

**When to use:**
- Behavior requires multiple concurrent tasks.
- Tasks can be modeled as long-lived entities receiving messages.
- State is isolated per actor.

**When NOT to use:**
- Single-threaded, synchronous control flow (use plain functions).
- Tasks are short-lived and spawned once (use spawned tasks with channels).

---

### 5. Type-State for Invariants

**Principle:** Compile-time enforcement of behavior preconditions via phantom type parameters.
Transitions between type states make illegal state changes compilation errors.

**Pattern:**
```rust
pub struct Handler<State> {
    data: Data,
    _state: std::marker::PhantomData<State>,
}

pub struct Uninitialized;
pub struct Ready;
pub struct Shutdown;

impl Handler<Uninitialized> {
    pub fn new() -> Self {
        Handler { data: Data::default(), _state: PhantomData }
    }
    
    pub fn initialize(self) -> Result<Handler<Ready>, InitError> {
        // Validate and transition
        Handler { data: self.data, _state: PhantomData }
    }
}

impl Handler<Ready> {
    pub fn execute(&mut self) -> Result<(), ExecError> { /* ... */ }
    
    pub fn shutdown(self) -> Handler<Shutdown> {
        Handler { data: self.data, _state: PhantomData }
    }
}

// This won't compile:
// let h = Handler::<Uninitialized>::default();
// h.execute(); // Error: no method `execute` on uninitialized state
```

**Benefit:**
- Impossible states become unrepresentable.
- Illegal transitions fail at compile time.
- No runtime checks needed for state preconditions.
- Performance: PhantomData has zero runtime cost.

**When to use:**
- Strict ordering of operations (must initialize before use, etc.).
- Preconditions that must hold for the entire lifetime of an object.
- Rich state representation (many methods available only in certain states).

**When NOT to use:**
- State is dynamic and data-driven.
- Many states with complex inter-state methods (gets unwieldy).
- Performance-critical path where type instantiation adds latency.

---

### 6. Zero-Cost Abstractions

**Principle:** Behavior modeling should not add runtime overhead; states and transitions should
compile to efficient machine code.

**Guidance:**
- Use enums (not boxed trait objects) in hot paths for state representation.
- Prefer `match` over `if`-chains; compiler optimizes exhaustive match to direct jumps.
- Use `#[inline]` hints sparingly; compiler decides most cases.
- Leverage `const fn` for compile-time computation where applicable.
- Avoid heap allocation in tight state-machine loops.

**Validation:** Inspect generated assembly (`cargo asm` via cargo-asm) to confirm
no malloc calls or unexpected indirection in state machine hot paths.

---

## Examples

### Example 1: Simple State Machine

**Behavior Spec:**
- States: `Ready`, `Processing`, `Complete`
- Events: `Start(task_id)`, `Finish`, `Cancel`
- Invariant: Can only transition to `Processing` from `Ready`; only `Processing` can finish.

**Rust Implementation:**
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Ready,
    Processing { task_id: u64 },
    Complete,
}

pub enum Event {
    Start(u64),
    Finish,
    Cancel,
}

pub fn transition(state: State, event: Event) -> Result<State, String> {
    match (state, event) {
        (State::Ready, Event::Start(task_id)) => {
            Ok(State::Processing { task_id })
        }
        (State::Processing { .. }, Event::Finish) => {
            Ok(State::Complete)
        }
        (State::Processing { .. }, Event::Cancel) => {
            Ok(State::Ready)
        }
        (s, e) => Err(format!("Invalid transition: {:?} -> {:?}", s, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_sequence() {
        let s1 = transition(State::Ready, Event::Start(1)).unwrap();
        assert_eq!(s1, State::Processing { task_id: 1 });
        
        let s2 = transition(s1, Event::Finish).unwrap();
        assert_eq!(s2, State::Complete);
    }

    #[test]
    fn test_invalid_transition() {
        let err = transition(State::Ready, Event::Finish);
        assert!(err.is_err());
    }
}
```

---

### Example 2: Actor with Result-Based Decisions

**Behavior Spec:**
- Actor receives task requests.
- Attempts execution; can succeed, fail with retry, or fail permanently.
- Failure is logged and passed to error handler.

**Rust Implementation:**
```rust
#[derive(Debug, Clone)]
pub enum Task {
    Process(String),
    Cancel,
}

#[derive(Debug)]
pub enum TaskResult {
    Success(String),
    Retry { attempt: u8, next_delay_ms: u64 },
    Failed { reason: String },
}

#[async_trait::async_trait]
pub trait TaskHandler {
    async fn handle(&mut self, task: Task) -> Result<TaskResult, TaskError>;
}

pub struct DefaultHandler {
    max_retries: u8,
}

#[async_trait::async_trait]
impl TaskHandler for DefaultHandler {
    async fn handle(&mut self, task: Task) -> Result<TaskResult, TaskError> {
        match task {
            Task::Process(item) => {
                match attempt_process(&item).await {
                    Ok(result) => Ok(TaskResult::Success(result)),
                    Err(e) if should_retry(&e) => {
                        Ok(TaskResult::Retry { attempt: 1, next_delay_ms: 100 })
                    }
                    Err(e) => Ok(TaskResult::Failed {
                        reason: e.to_string(),
                    }),
                }
            }
            Task::Cancel => Ok(TaskResult::Success("Cancelled".to_string())),
        }
    }
}

async fn attempt_process(item: &str) -> Result<String, ProcessError> {
    // Implementation
    Ok(format!("Processed: {}", item))
}

fn should_retry(e: &ProcessError) -> bool {
    // Retry logic
    true
}

#[derive(Debug)]
pub struct ProcessError(String);
impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for ProcessError {}
```

---

### Example 3: Type-State for Safety

**Behavior Spec:**
- Handler must be initialized before use.
- Execution only valid after initialization.
- Once shutdown, all operations forbidden.

**Rust Implementation:**
```rust
pub struct Handler<S> {
    data: String,
    _state: std::marker::PhantomData<S>,
}

pub struct Uninitialized;
pub struct Initialized;
pub struct Shutdown;

impl Handler<Uninitialized> {
    pub fn new(name: &str) -> Self {
        Handler {
            data: name.to_string(),
            _state: std::marker::PhantomData,
        }
    }

    pub fn initialize(self) -> Result<Handler<Initialized>, String> {
        if self.data.is_empty() {
            return Err("Invalid name".to_string());
        }
        Ok(Handler {
            data: self.data,
            _state: std::marker::PhantomData,
        })
    }
}

impl Handler<Initialized> {
    pub fn execute(&self) -> Result<String, String> {
        Ok(format!("Executing: {}", self.data))
    }

    pub fn shutdown(self) -> Handler<Shutdown> {
        Handler {
            data: self.data,
            _state: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_state_safety() {
        let h = Handler::<Uninitialized>::new("test");
        // This would not compile:
        // h.execute(); // Error: no method `execute` on Uninitialized

        let h_init = h.initialize().unwrap();
        let result = h_init.execute();
        assert!(result.is_ok());
    }
}
```

---

## Decision Criteria

Use this skill to choose:

### 1. State Representation
- **Use enums** for fixed, known states.
- **Use generic state structs** only if state is data-rich and variants share structure.
- **Validate:** "Can I write a safe, exhaustive match over all states?"

### 2. Transition Logic
- **Encode as pure functions** returning `Result<NewState, Error>` first.
- **Add internal mutation** only if performance testing justifies it.
- **Validate:** "Does rustc prevent invalid transitions?"

### 3. Error Handling
- **Use Result<T, E>** for recoverable errors (retry, fallback, logging).
- **Use Option<T>** for optional outcomes (not failure, just absence).
- **Use panic (!)** only for truly unrecoverable programmer errors.
- **Validate:** "Is error path explicit and testable?"

### 4. Concurrency Pattern
- **Use actors** (trait-based) for concurrent, message-driven tasks.
- **Use RwLock/Mutex** only when shared mutable state is unavoidable; minimize lock scope.
- **Use channels** for producer-consumer flows.
- **Validate:** "Does the code avoid deadlocks? Use lock-free primitives where possible?"

### 5. Type-State vs. Runtime Checks
- **Use type-state** for preconditions that must hold across the entire lifetime.
- **Use runtime checks** for dynamic, per-call conditions.
- **Validate:** "Is the invariant statically verifiable, or does it depend on runtime data?"

### 6. Abstraction Level
- **Provide trait abstractions** only for truly polymorphic behavior.
- **Avoid over-factoring** if only one implementation will exist.
- **Validate:** "Is the trait boundary clear and minimal?"

---

## Validation Rules

### Rule 1: Exhaustiveness
**Check:** All behavior states and transitions specified in upstream spec are represented
in Rust enum/match.  
**Validation:** Rustc compilation must succeed with no `unreachable_patterns` or
`non_exhaustive_patterns` warnings.  
**Failure Mode:** Missing state variant or transition arm.

### Rule 2: Type Safety
**Check:** No `unsafe` blocks or `unwrap()` in hot paths unless explicitly justified.  
**Validation:** Code review confirms justification; profiling validates necessity.  
**Failure Mode:** Silent panics or undefined behavior.

### Rule 3: Error Propagation
**Check:** All error paths (including actor message failures) are captured in `Result`
or logged.  
**Validation:** Test suite exercises error branches; code coverage ≥ 90% for error handling.  
**Failure Mode:** Dropped errors; silent failures.

### Rule 4: Zero-Cost Abstraction
**Check:** State machine hot paths produce no heap allocation or indirect calls.  
**Validation:** `cargo asm` inspection confirms direct jumps/branches, no malloc calls in loop.  
**Failure Mode:** Unexpected runtime overhead; allocation in tight loops.

### Rule 5: Actor Isolation
**Check:** Each actor trait implementation handles its own message types without global state.  
**Validation:** Trait implementation contains no `thread_local!` or `static mut`.  
**Failure Mode:** Data races; difficult debugging.

### Rule 6: State Invariants
**Check:** Type-state (if used) enforces all documented preconditions at compile time.  
**Validation:** Attempt to call a method on the wrong state type; compilation fails.  
**Failure Mode:** Type-state not enforcing invariants; runtime panics still possible.

---

## Composition & References

### Handoff Authorities
- `plans/<feature-slug>/design/behaviors.md` - authoritative behavior states,
  transitions, and decision points.
- `plans/<feature-slug>/plan/domain-spec.md` - domain invariants, terminology,
  and error semantics that the Rust types must preserve.
- `plans/<feature-slug>/plan/dependency-graph.md` - actor boundaries, message
  flow, and permitted dependency direction.
- `plans/<feature-slug>/plan/implementation-plan.md` - runtime constraints that
  affect async, actor, or typestate choices.
- [`.github/local/directories.md`](../../local/directories.md) - canonical
  source and test layout when deciding module placement.
