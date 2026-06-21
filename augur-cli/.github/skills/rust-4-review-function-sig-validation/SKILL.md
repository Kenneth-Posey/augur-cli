---
name: rust-4-review-function-sig-validation
description: >
  Rust-specific function signature validation for lifetimes, error handling patterns,
  visibility semantics, and trait bounds. Validates that Rust function signatures are
  safe, idiomatic, and correctly express their contracts. Use when reviewing signature
  correctness.
---

# Rust 4 Review Function Signature Validation

## Overview

**Authority boundary**: Signature correctness only. Review changed signatures
against the feature handoff files and Rust compiler constraints. Do not use this
skill for function-body logic, broader behavior validation, or style-only
review.

## Key Files

- `README.md` - overview and usage notes

## Review Role

Review changed signatures against repo authorities and any compiler or lint
evidence, then report a `pass|fail` outcome.

## Scope

### What This Skill Validates

1. **Lifetime Annotations**
   - Lifetime elision rules are correctly applied
   - Explicit lifetimes (when required) are sound and necessary
   - Output lifetimes are traceable to input parameters or `'static`
   - No disconnected or arbitrary lifetimes
   - Self-referential methods do not require explicit lifetime parameters

2. **Error Handling Patterns**
   - Functions that can fail return `Result<T, E>` or `Option<T>`, not panic
   - Error types match crate conventions (e.g., `crate::Error`, domain-specific enum)
   - Return types express all error cases without hidden panics
   - Infallible functions do not wrap return type in `Result`
   - `?` operator is properly used at error boundaries

3. **Visibility & Encapsulation**
   - Visibility modifiers (`pub`, `pub(crate)`, private) match intended layer
   - Private or crate-internal types do not leak in `pub` function signatures
   - Public functions in public modules follow hierarchy rules
   - Re-exported `pub(crate)` types in public signatures are justified

4. **Trait Bounds Satisfaction**
   - All generic type parameters have required bounds
   - Bounds are sufficient for usage in function body
   - No unnecessary or over-constraining bounds
   - Trait object bounds include required lifetime and marker traits
   - Associated types are properly specified in generic constraints

### Coverage Boundaries

This skill assumes:
- Code compiles without errors (`cargo check` passes)
- All changed or new signatures are provided in scope
- Trait definitions and plan specifications are available for comparison

## Key Concepts

### 1. Lifetime Elision and Correctness

**What it is**: Rust's lifetime elision rules allow omission of explicit lifetimes when
they can be inferred from the function signature.

**How to validate**:
- Single input lifetime → output lifetime inferred automatically
- `&self` or `&mut self` → output lifetime same as self lifetime
- Multiple input lifetimes → output lifetime must be explicit (no elision)
- All output references must be traceable to an input parameter or `'static`

**Example: Valid Elision**
```rust
// ✓ VALID: Output lifetime elided from single input
fn parse(s: &str) -> Result<Value, ParseError> { }

// ✓ VALID: Output lifetime from self
fn as_ref(&self) -> &Value { }

// ✗ INVALID: Multiple inputs, missing explicit output lifetime
fn merge(a: &str, b: &str) -> &str { }  // ERROR: lifetime mismatch

// ✓ VALID: Explicit output lifetime specified
fn merge<'a>(a: &'a str, b: &str) -> &'a str { }
```

### 2. Error Handling: Result vs. Panic

**What it is**: Rust functions must express fallible operations via return types,
never via implicit panics or unwrap calls in library code.

**How to validate**:
- Recoverable errors → `Result<T, E>`
- Optional values → `Option<T>`
- Error type matches crate conventions (not bare strings)
- No panic-inducing calls (`unwrap()`, `expect()`, `panic!()`) in library function signatures
- Return type is infallible (`T` directly) only if function truly cannot fail

**Example: Valid Error Handling**
```rust
// ✓ VALID: Error case expressed in return type
pub fn parse(input: &str) -> Result<Config, ConfigError> { }

// ✓ VALID: Optional value (None is semantically correct)
pub fn find(key: &str) -> Option<&Value> { }

// ✗ INVALID: Hidden panic in signature (caller cannot prepare)
pub fn parse_unchecked(input: &str) -> Config { }  // Will panic on invalid input

// ✗ INVALID: Direct panic in signature
pub fn unwrap_value(opt: Option<Value>) -> Value {
    opt.unwrap()  // Must return Option or Result
}
```

### 3. Visibility and Encapsulation

**What it is**: Rust visibility modifiers control which code can access a signature and
its types. Incorrect visibility breaks encapsulation and exposes internal details.

**How to validate**:
- `pub` functions expose types that are themselves `pub` or re-exported
- `pub(crate)` functions can expose `pub(crate)` types (internal to crate)
- Private functions do not appear in public function signatures
- Module hierarchy is respected: public functions in public modules
- Type leakage: private or crate-internal types in public function parameters/return

**Example: Valid Visibility**
```rust
// VALID: Public function with public types
pub struct Request { }
pub fn handle_request(req: Request) -> Response { }

// INVALID: Public function exposing private type
struct InternalConfig { }
pub fn process(config: InternalConfig) -> Result { }  // ERROR: InternalConfig is private

// VALID: Crate-internal function with crate-internal types
pub(crate) struct CacheEntry { }
pub(crate) fn lookup(key: &str) -> Option<CacheEntry> { }
```

### 4. Trait Bounds Satisfaction

**What it is**: When a function is generic over a type parameter `T`, it may require
`T` to implement certain traits (bounds) to use methods or operations on `T`.

**How to validate**:
- Every generic type parameter used in the function body must have appropriate bounds
- Bounds are specified in the signature, not inferred
- All bounds are necessary (remove unused bounds)
- Trait objects include all required bounds (lifetime, marker traits)
- Associated types are properly constrained

**Example: Valid Bounds**
```rust
// ✓ VALID: Clone bound required for operation
pub fn clone_all<T: Clone>(items: &[T]) -> Vec<T> {
    items.iter().map(|item| item.clone()).collect()
}

// ✗ INVALID: Missing Clone bound
pub fn clone_all<T>(items: &[T]) -> Vec<T> {
    items.iter().map(|item| item.clone()).collect()  // ERROR: T does not have Clone
}

// ✓ VALID: Bounds for trait objects
pub fn invoke(callback: &dyn Fn() + Send + 'static) { }

// ✗ INVALID: Unnecessary bounds
pub fn only_clone<T: Clone + Default>(items: &[T]) -> Vec<T> {
    items.iter().map(|item| item.clone()).collect()  // Default never used
}
```

### 5. Signature Completeness

**What it is**: Function signatures must be complete, concise, and consistent with
their trait definitions (if any).

**How to validate**:
- Parameter count ≤ 3 (struct wrapper for complex inputs)
- Return type is explicitly specified (never implicit `()` when value should return)
- Function is consistent with trait method signature (if implementing trait)
- No mutable static references in signature
- Generic parameters and lifetimes are necessary

**Example: Valid Completeness**
```rust
// ✓ VALID: Concise, ≤3 parameters
pub fn build(name: &str, config: &Config) -> Result<Handle> { }

// ✗ INVALID: Too many parameters (should use struct)
pub fn create(a: i32, b: i32, c: i32, d: i32, e: String) -> Result { }

// ✓ VALID: Consistent with trait
impl MyTrait for MyType {
    fn from_str(s: &str) -> Result<Self, ParseError> { }
}

// ✗ INVALID: Return type differs from trait
trait Iterator {
    fn next(&self) -> Option<Item>;
}
impl Iterator for MyIter {
    fn next(&self) -> Option<Item> { }  // ✓ Correct signature
    // If signature differs from trait, compilation error
}
```

## Composition & References

### Review Authorities

- `plans/<feature-slug>/plan/function-sig-plan.md` - primary authority for the
  expected function signatures.
- `plans/<feature-slug>/plan/domain-spec.md` - semantic types, error taxonomy,
  and ownership expectations.
- `plans/<feature-slug>/plan/dependency-graph.md` - trait placement, visibility
  boundaries, and cross-module references.
- `plans/<feature-slug>/plan/implementation-plan.md` - runtime constraints that
  justify async, allocation, or ownership choices.
- Changed code - the concrete signatures under review.

### Review Output

```
Changed signatures + compiler or lint evidence
    ↓
Signature review in this skill
    ↓
Findings ordered by severity
    ↓
Each finding tied to the governing handoff file and overall outcome
```

## Review Signal

Use the same `pass|fail` vocabulary as the deterministic
function-signature checks, based on the scoped code and evidence set.

| Condition | Signal |
|----------|--------|
| Critical signature findings present | `fail` |
| Only major/minor cleanup or warning-level findings remain | `pass` with warnings |
| Validation timed out or required evidence is incomplete | `fail` |

### Deterministic Evidence Sources

Use current deterministic artifacts when they are part of the handoff. If fresh
evidence is required, the repo-approved commands are:

1. **cargo check** - Compile-time signature errors and warnings
   ```sh
   cargo check --all-targets
   ```
   Extract signature-related diagnostics (lifetime, generics, visibility).

2. **cargo clippy** - Lint suggestions, especially around visibility and generics
   ```sh
   cargo clippy --all-targets -- -W clippy::all -W clippy::pedantic
   ```
   Focus on `needless_lifetimes`, `type_complexity`, `visibility`, and trait bound lints.

3. **Manual inspection** - For error handling patterns and visibility enforcement
   - Read function signatures with error handling (Result/Option)
   - Check module visibility hierarchy
   - Verify trait method signatures match trait definitions

**How to interpret diagnostics**:
- `lifetime mismatch` → Lifetime elision or annotation error
- `cannot find trait bound` → Missing generic bound
- `private type in public function` → Visibility leakage
- `unused generic parameter` → Unnecessary bound or parameter
- `trait objects must include` → Missing trait object bound

## Examples

### Example 1: Lifetime Validation

**Scenario**: Function signature added with improper lifetime handling.

**Before** (Invalid):
```rust
pub fn parse_header(response: &str) -> &str {
    let header = response.lines().next();
    &header.unwrap_or("")  // ERROR: dangling reference
}
```

**Validation Finding**: 
- Rule: "Lifetime correctness: output lifetime must be traceable to input"
- Evidence: Borrow checker error; temporary value does not live long enough
- Severity: Critical (undefined behavior risk)
- Correction: Return owned String or trace lifetime from input

**After** (Valid):
```rust
pub fn parse_header(response: &str) -> &str {
    response.lines().next().unwrap_or("")
}
```

### Example 2: Error Handling Validation

**Scenario**: Function returns implicit panic instead of Result type.

**Before** (Invalid):
```rust
pub fn parse_config(input: &str) -> Config {
    serde_json::from_str(input).unwrap()  // Will panic on invalid JSON
}
```

**Validation Finding**:
- Rule: "Error handling: functions that can fail must return Result or Option"
- Evidence: `unwrap()` in library function will cause runtime panic
- Severity: Critical (crashes caller; violates library safety contract)
- Correction: Return `Result<Config, serde_json::Error>`

**After** (Valid):
```rust
pub fn parse_config(input: &str) -> Result<Config, serde_json::Error> {
    serde_json::from_str(input)
}
```

### Example 3: Visibility Leakage Validation

**Scenario**: Public function exposes private type.

**Before** (Invalid):
```rust
struct InternalCache { data: HashMap<String, Value> }

pub fn get_cached(key: &str) -> Option<InternalCache> {
    // ERROR: InternalCache is private but exposed in pub signature
    None
}
```

**Validation Finding**:
- Rule: "Visibility: private types must not appear in public function signatures"
- Evidence: `InternalCache` is private; public function exposes it
- Severity: Critical (breaks encapsulation; type is not public API)
- Correction: Return public wrapper or `pub(crate)` if for crate-internal use

**After** (Valid):
```rust
pub struct CachedValue { data: Vec<u8> }

pub fn get_cached(key: &str) -> Option<CachedValue> {
    None
}
```

### Example 4: Trait Bounds Validation

**Scenario**: Generic function missing required bounds.

**Before** (Invalid):
```rust
pub fn process_all<T>(items: Vec<T>) -> Vec<T> {
    items.iter()
        .map(|item| {
            let _rendered = item.to_string();  // ERROR: T does not implement Display
            item.clone()  // ERROR: T does not implement Clone
        })
        .collect()
}
```

**Validation Finding**:
- Rule: "Generic bounds: all type parameters used in function must have required bounds"
- Evidence: Compiler errors; `T` needs `Display` and `Clone` bounds
- Severity: Critical (does not compile)
- Correction: Add `Display` and `Clone` bounds to generic parameter

**After** (Valid):
```rust
pub fn process_all<T: Clone + std::fmt::Display>(items: Vec<T>) -> Vec<T> {
    items.iter()
        .map(|item| {
            let _rendered = item.to_string();
            item.clone()
        })
        .collect()
}
```

## Decision Criteria

### Severity Classification

Use these criteria to classify findings and set severity:

| Finding Type | Severity | Reason |
|---|---|---|
| Lifetime elision violation → compilation error | Critical | Does not compile |
| Output lifetime disconnected from input | Critical | Dangling reference; UB risk |
| Recoverable error returns panic instead | Critical | Crashes caller on valid input |
| Private type in public signature | Critical | Breaks encapsulation |
| Missing generic bound → compilation error | Critical | Does not compile |
| Trait method signature differs from trait | Critical | Trait object will not work |
| Result used for infallible operation | Major | Adds unnecessary complexity |
| Over-constrained generic bounds | Major | API unnecessarily restrictive |
| Visibility modifier incorrect | Major | Encapsulation violation (non-critical) |
| Redundant lifetime annotation | Minor | Cleanliness issue; not wrong |

### Finding Interpretation Guidance

Use these criteria to interpret the review evidence and describe the findings:

1. **Critical findings present**: Treat them as blocking signature issues that
   must be called out explicitly.
2. **Pattern of major findings**: Multiple major findings usually indicate a
   broader signature-design problem worth describing as a concentrated risk.
3. **Isolated major finding**: Explain the local impact, whether the issue is
   contained, and what follow-up is needed.
4. **Only minor findings**: Record them as cleanup or idiomaticity notes.
5. **No findings**: Summarize the signature evidence that supports the reviewed
   contract.

**Suggested summary pattern**:
- Critical findings → describe the compilation, lifetime, visibility, or
  contract break directly.
- Several major findings → describe the repeated pattern and its effect on the
  API contract.
- One or two major findings → document the issue, the affected signature, and
  why the rest of the review may still be sound.
- Minor-only findings or no findings → note the cleanup items or the evidence
  supporting signature correctness.

## Validation Rules

### Lifetime Annotation Rules

1. **Elision Rules Respected**: Lifetimes follow Rust's three elision rules:
   - Single input lifetime inferred to output
   - `&self` lifetime inferred to output
   - Multiple input lifetimes require explicit output lifetime

2. **Output Lifetime Traceable**: Every lifetime appearing in return type must
   be traceable to an input parameter or explicitly `'static`.

3. **Self References Correct**: Methods with `&self` or `&mut self` do not require
   explicit lifetime parameters unless additional borrowed inputs exist.

4. **No Disconnected Lifetimes**: Return type lifetime cannot be arbitrary;
   it must correlate with input lifetimes.

### Error Handling Rules

1. **Result/Option for Fallible Operations**: Functions that can fail return
   `Result<T, E>` or `Option<T>`, never panic in signature.

2. **Error Type Matches Convention**: Error type is from crate error enum,
   standard library (e.g., `io::Error`), or domain-specific enum. Not bare strings.

3. **No Hidden Panics**: Signature does not hide panics via `unwrap()`, `expect()`,
   or `panic!()` calls in library code.

4. **Infallible Functions Unwrapped**: Functions that cannot fail do not wrap
   return type in `Result`; return `T` directly.

5. **Error Propagation Explicit**: `?` operator is used at error boundaries,
   not within library signatures.

### Visibility Rules

1. **Correct Modifier Applied**: Visibility is `pub` (public API), `pub(crate)`
   (crate-internal), or no modifier (module-private).

2. **No Type Leakage**: Private or crate-internal types never appear in `pub`
   function signatures.

3. **Hierarchy Respected**: Public functions in public modules; crate-internal
   functions in internal modules.

4. **Re-export Justified**: If `pub(crate)` type appears in `pub` function,
   justify why it is re-exported via that function.

### Trait Bounds Rules

1. **All Bounds Necessary**: Every generic bound used in signature must appear
   in function body or other signature parameters. Remove unused bounds.

2. **Bounds Sufficient**: All operations on generic parameter `T` are supported
   by its bounds.

3. **No Conflicting Bounds**: Bounds do not contradict (e.g., `T: Fn() + Clone`
   is OK; duplicates removed).

4. **Lifetime Bounds Correct**: `T: 'a` used only when `T` contains references;
   `'a: 'b` used when `'a` outlives `'b`.

5. **Trait Object Bounds Complete**: Trait objects include all required bounds
   (lifetime, `Send`, `Sync`, etc.).

### Signature Completeness Rules

1. **Parameter Count ≤ 3**: Function accepts ≤3 parameters. Use struct wrapper
   for more complex inputs.

2. **Return Type Explicit**: Never implicit `-> ()` when semantic value should
   be returned.

3. **No Unsafe Patterns**: Function signature does not accept `&mut static`
   or similar patterns that bypass safety guarantees.

4. **Consistency with Trait**: If implementing trait method, signature matches
   trait definition exactly.

5. **Generic Parameters Necessary**: Every generic parameter and lifetime used
   in signature is necessary for correctness. No unused generic cruft.
