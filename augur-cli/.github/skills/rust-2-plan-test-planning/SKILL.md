---
name: rust-2-plan-test-planning
description: >
  Maps test strategy to Rust tooling, idioms, and patterns. Turns unit,
  integration, property, and performance requirements into concrete plans using
  cargo test, proptest, criterion, and trait-based mocking. Use when planning
  or reviewing a Rust test suite before implementation.
---

# Rust 2 Plan Test Planning

## Use When

Use this skill after test intent is defined in plan artifacts. Prefer:

- `plans/<feature-slug>/plan/test-strategy-plan.md` for test categories,
  coverage targets, fixtures, and execution environments.
- `plans/<feature-slug>/design/behaviors.md` for scenario coverage, transitions,
  and error-path expectations.
- `plans/<feature-slug>/plan/function-sig-plan.md` for API surfaces that need
  unit, integration, or property coverage.
- `plans/<feature-slug>/plan/implementation-plan.md` for performance-sensitive
  or async scenarios that influence benchmark and integration coverage.

Use it to decide:

- **Test module organization**: Inline `#[cfg(test)]` vs. `tests/` directory; structure and boundaries.
- **Rust test framework mapping**: Unit tests (`#[test]`), integration tests, doc tests.
- **Property-based testing**: `proptest` setup, Arbitrary implementations, strategy composition.
- **Benchmark suite**: `criterion` structure with `black_box()`, input parameterization, baseline tracking.
- **Mock trait patterns**: Trait injection, sealed trait test implementations, builder mocks.
- **Cargo test profiles**: Fast (local), thorough (pre-commit), comprehensive (CI).

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### 1. Test Module Organization

#### Inline Tests (`#[cfg(test)]`)

**When to use:**
- Unit tests for private implementation details.
- Fast feedback loop required (no separate compilation).
- Tests access internal/private APIs.

**Pattern:**
```rust
// In src/lib.rs or src/main.rs

pub fn public_api() -> String {
    // ...
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_api_happy_path() {
        let result = public_api();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_public_api_edge_case() {
        // Test boundary condition
    }
}
```

**Benefits:**
- Direct access to private types and functions.
- Fast compilation and execution (no separate binary).
- Tightly coupled to implementation.

**Constraints:**
- No test code appears in release binaries (removed by `#[cfg(test)]`).
- Tests must fit in single module.

---

#### Integration Tests (`tests/` directory)

**When to use:**
- Public API surface testing.
- Multi-crate integration scenarios.
- Each test file is compiled as separate binary.
- Real-world linking behavior validation.

**Pattern:**
```rust
// In tests/integration_test.rs

use my_crate::api::Client;
use my_crate::config::Config;

#[test]
fn test_client_initialization() {
    let config = Config::default();
    let client = Client::new(config);
    assert!(client.is_ready());
}

#[tokio::test]
async fn test_async_operation() {
    let result = my_crate::async_fn().await;
    assert_eq!(result, expected);
}
```

**Benefits:**
- Tests only public API (cannot access private internals).
- Each test file compiles to separate binary (real linking).
- Simulates external user environment.

**Constraints:**
- Slower compilation (separate binary per test file).
- Cannot test private types directly.

---

#### Doc Tests

**When to use:**
- Public API examples that also serve as documentation.
- Executable documentation that stays synchronized with code.
- API usage examples for crate consumers.

**Pattern:**
```rust
/// Adds two numbers and returns the sum.
///
/// # Example
///
/// ```
/// use my_crate::add;
/// let result = add(2, 3);
/// assert_eq!(result, 5);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

**Benefits:**
- Documentation and test in one.
- Rustdoc compiles and runs tests automatically.
- Ensures examples in docs never go stale.

**Constraints:**
- Limited scope (public APIs only).
- Cannot import private types.
- Output captured and compared (less flexible assertions).

---

### 2. Property-Based Testing with proptest

**Principle:** Test that specific properties hold for all generated inputs (not just hand-written cases).

#### Arbitrary Implementation

```rust
use proptest::prelude::*;

#[derive(Clone, Debug)]
struct Point { x: i32, y: i32 }

impl Arbitrary for Point {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    
    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (any::<i32>(), any::<i32>())
            .prop_map(|(x, y)| Point { x, y })
            .boxed()
    }
}
```

#### Strategy Composition

```rust
use proptest::prelude::*;

// Simple scalar generation
let ints = any::<i32>();

// Bounded collection
let vec = prop::collection::vec(0..100, 1..10);

// String matching regex pattern
let emails = r#"[a-z]+@[a-z]+\.[a-z]+"#;

// Custom composite strategy
let user = (r#"\PC+"#, 18..120)
    .prop_map(|(name, age)| User { name, age });
```

#### Invariant Checking

```rust
#[test]
fn test_serialize_deserialize_roundtrip() {
    proptest!(|value in any::<MyType>()| {
        let json = serde_json::to_string(&value).unwrap();
        let roundtrip: MyType = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(value, roundtrip);
    });
}
```

**Benefits:**
- Auto-shrinking: proptest minimizes failing cases to root cause.
- High coverage: tests 256+ random inputs by default.
- Invariant-focused: properties must hold for ALL inputs.

---

### 3. Benchmark Testing with criterion

**Principle:** Structured performance measurement with statistical analysis and regression detection.

#### Basic Structure

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_algorithm(c: &mut Criterion) {
    c.bench_function("algorithm_name", |b| {
        b.iter(|| target_function(black_box(input)))
    });
}

criterion_group!(benches, bench_algorithm);
criterion_main!(benches);
```

#### Key Components

- **`black_box()`:** Prevents compiler optimizations from skewing results.
- **Parameterized benchmarks:** `c.bench_with_input()` for input size variation.
- **Baseline files:** Tracked for regression detection.
- **Statistical analysis:** Criterion produces plots and variance reports.

#### Cargo.toml Setup

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio"] }

[[bench]]
name = "my_benchmarks"
harness = false  # Use criterion's harness, not cargo test
```

#### Async Benchmarking

```rust
fn bench_async(c: &mut Criterion) {
    c.bench_function("async_op", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async { target_async_function().await })
    });
}
```

---

### 4. Mock Trait Patterns

#### Pattern A: Trait Injection (Preferred)

```rust
pub trait Logger {
    fn log(&self, msg: &str);
}

pub struct SystemUnderTest {
    logger: Box<dyn Logger>,
}

#[cfg(test)]
mod tests {
    struct MockLogger { messages: RefCell<Vec<String>> }
    
    impl Logger for MockLogger {
        fn log(&self, msg: &str) {
            self.messages.borrow_mut().push(msg.into());
        }
    }
}
```

**Benefits:**
- Production code never depends on test mocks.
- Mock implements exact trait interface.
- Easy to swap implementations.

---

#### Pattern B: Sealed Traits with Test Impl

```rust
pub trait Service: sealed::Sealed {
    fn perform(&self) -> Result<(), Error>;
}

pub mod sealed {
    pub trait Sealed {}
}

#[cfg(test)]
mod test_impl {
    struct TestDouble;
    impl sealed::Sealed for TestDouble {}
    impl Service for TestDouble { /* ... */ }
}
```

**Benefits:**
- Prevents external implementations of trait.
- Test implementation hidden behind sealed trait.
- Production code safe from accidental test mock usage.

---

#### Pattern C: Builder Mock (with bon/builder derive)

```rust
#[derive(bon::Builder)]
pub struct MockRequest {
    #[builder(default = "\"GET\".to_string()")]
    pub method: String,
    pub path: String,
    pub headers: Option<Vec<String>>,
}
```

**Benefits:**
- Fluent API for test data setup.
- Boilerplate reduced via derive.
- Defaults allow minimal test setup.

---

### 5. Test Data Builders and Fixtures

#### Builder Pattern for Complex Setup

```rust
#[cfg(test)]
mod fixtures {
    pub struct UserBuilder { 
        name: String, 
        age: u32,
        email: Option<String>,
    }
    
    impl UserBuilder {
        pub fn new() -> Self { 
            Self { 
                name: "test".into(), 
                age: 0,
                email: None,
            } 
        }
        
        pub fn with_name(mut self, name: &str) -> Self { 
            self.name = name.into(); 
            self 
        }
        
        pub fn with_age(mut self, age: u32) -> Self { 
            self.age = age; 
            self 
        }
        
        pub fn build(self) -> User { 
            User { 
                name: self.name, 
                age: self.age,
                email: self.email,
            } 
        }
    }
}
```

**Benefits:**
- Fluent API for readable test setup.
- Defaults reduce boilerplate.
- Easy to add new variation.

---

#### Proptest Fixture Generation

```rust
fn arb_valid_user() -> impl Strategy<Value = User> {
    (r#"\PC+"#, 18..120)
        .prop_map(|(name, age)| User { name, age })
}

#[test]
fn test_with_arbitrary() {
    proptest!(|user in arb_valid_user()| {
        let serialized = user.to_json();
        let deserialized: User = User::from_json(&serialized).unwrap();
        prop_assert_eq!(user, deserialized);
    });
}
```

---

## Examples

### Example 1: Unit Test Planning

**Strategy Input:** "Unit tests for validation module, 80% line coverage"

**Rust Output:**
```rust
// src/validation.rs
pub fn validate_email(email: &str) -> Result<(), ValidationError> { /* ... */ }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_email_passes() {
        assert!(validate_email("user@example.com").is_ok());
    }

    #[test]
    fn test_invalid_email_fails() {
        assert!(validate_email("not-an-email").is_err());
    }

    #[test]
    fn test_empty_string_fails() {
        assert!(validate_email("").is_err());
    }
}
```

---

### Example 2: Integration Test Planning

**Strategy Input:** "HTTP API integration test, verify client-server round-trip"

**Rust Output:**
```rust
// tests/api_integration_test.rs
use my_crate::api::Client;
use my_crate::config::Config;

#[tokio::test]
async fn test_create_user_roundtrip() {
    let config = Config::test_default();
    let client = Client::new(config).await.expect("client init");
    
    let user = client.create_user("Alice").await.expect("create");
    assert_eq!(user.name, "Alice");
    
    let fetched = client.get_user(user.id).await.expect("fetch");
    assert_eq!(fetched.id, user.id);
    assert_eq!(fetched.name, "Alice");
}
```

---

### Example 3: Property Test Planning

**Strategy Input:** "Reversible operations: serialize/deserialize invariant"

**Rust Output:**
```rust
// src/serialization.rs
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_serialize_deserialize_roundtrip(
            value in any::<MyType>()
        ) {
            let json = serde_json::to_string(&value).unwrap();
            let roundtrip: MyType = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(value, roundtrip);
        }
    }
}
```

---

### Example 4: Benchmark Test Planning

**Strategy Input:** "Performance regression detection for algorithm module"

**Rust Output:**
```rust
// benches/algorithm_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_sorting(c: &mut Criterion) {
    let mut group = c.benchmark_group("sorting_algo");
    for size in [100, 1000, 10000].iter() {
        group.bench_with_input("input_size", size, |b, &size| {
            let data: Vec<i32> = (1..=size as i32).collect();
            b.iter(|| sort_algorithm(black_box(&data)));
        });
    }
}

criterion_group!(benches, bench_sorting);
criterion_main!(benches);
```

---

## Validation Rules

Planned tests should satisfy:

### 1. Module Organization
- [ ] Inline unit tests use `#[cfg(test)]` guard
- [ ] Integration tests live in `tests/` directory
- [ ] No test code in public modules without `#[cfg(test)]`
- [ ] Test modules follow naming: `tests` (not `test`, `testing`, etc.)

### 2. Test Naming
- [ ] Functions: `test_<component>_<behavior>_<expected_outcome>`
- [ ] Clear assertion messages describing what failed
- [ ] No abbreviations in test names (spell out full intent)

### 3. Proptest Usage
- [ ] `Arbitrary` impl provided for all generated types
- [ ] Strategies bounded (no unbounded collections by default)
- [ ] Shrinking enabled (uses proptest default)
- [ ] At least 256 test iterations (proptest default)

### 4. Criterion Setup
- [ ] `black_box()` wraps inputs to prevent optimization
- [ ] Benchmark names match measured operation (not generic "bench1")
- [ ] Sample size ≥ 100 runs (criterion default)
- [ ] Baseline files tracked (for regression detection)

### 5. Mock Traits
- [ ] Test mocks never used in production code
- [ ] Trait injection preferred over monolithic mocks
- [ ] Mock interfaces match production trait exactly

### 6. Dependency Direction
- [ ] Test code depends on implementation
- [ ] Never: implementation depends on test code
- [ ] Test-specific crates (`proptest`, `criterion`) in `[dev-dependencies]`

---

## Composition & References

### Primary References
- `plans/<feature-slug>/plan/test-strategy-plan.md` - primary authority for
  coverage targets, test categories, and fixture expectations.
- `plans/<feature-slug>/design/behaviors.md` - scenario sequencing and failure
  paths that tests must exercise.
- `plans/<feature-slug>/plan/function-sig-plan.md` - public and internal API
  surfaces that need direct test coverage.
- `plans/<feature-slug>/plan/implementation-plan.md` - async, performance, and
  environment constraints that shape test execution.
- [`.github/local/directories.md`](../../local/directories.md) - canonical Rust
  source/test mirroring and fixture placement.

---

## Appendix A: Cargo Test Profiles

### Profile 1: Fast Local Development
```bash
cargo test --lib  # Inline tests only
# Typical: 2-5 seconds
```

### Profile 2: Thorough (Pre-commit)
```bash
cargo test --lib --doc --test '*'  # All test targets
cargo test -- --ignored            # Run ignored tests
# Typical: 15-30 seconds
```

### Profile 3: Comprehensive (CI)
```bash
cargo test --all-targets -- --nocapture  # With output
cargo test --doc                          # Doc examples
cargo bench --no-run                       # Compile benchmarks
# Typical: 1-2 minutes
```

---

## Appendix B: Proptest Version Constraints

| Constraint Type | Guidance |
|-----------------|----------|
| **Minimum Version** | `proptest = "1.0"` (stable API) |
| **Test Feature** | `#[proptest]` requires `proptest` in `[dev-dependencies]` |
| **Max Test Count** | Override with `PROPTEST_MAX_TESTS=1000 cargo test` |
| **Seed Control** | `PROPTEST_SEED=<hex> cargo test` for reproducible failures |

---

## Appendix C: Mock Trait Checklist

Before finalizing mock trait decisions:
- [ ] Mock trait implements exactly the same interface as production trait
- [ ] Mock stored behind `Box<dyn TraitName>` or monomorphized generic
- [ ] Test code never directly references mock type in production code path
- [ ] Trait methods documented with expected test behavior
- [ ] Consider `mockall` crate for complex mocks (valid alternative)
