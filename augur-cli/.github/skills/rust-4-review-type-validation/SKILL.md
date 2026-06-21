---
name: rust-4-review-type-validation
description: >
  Rust-specific type safety validation for lifetimes, generic bounds, unsafe blocks,
  and semantic type patterns. Validates that Rust's type system is used correctly and
  defensively. Use when reviewing type correctness, memory safety, and semantic type
  usage in Rust code.
---

# Rust 4 Review Type Validation

## Overview

**Authority boundary**: Type correctness only. Review changed Rust types against
the feature handoff files and compiler-enforced constraints. Do not use this
skill for performance tuning, architectural placement, or broader behavioral
review.

## Key Files

- `README.md` - overview and usage notes

## Review Role

Review changed types alongside repo-local authorities and compiler or lint
artifacts, then emit the shared `pass|fail` signal.

## Scope

### What This Skill Validates

1. **Lifetime Correctness**
   - No dangling references or use-after-free
   - Lifetime annotations are present and correct
   - Lifetime relationships between parameters and return types are justified
   - Variance rules are not violated (covariance, contravariance, invariance)

2. **Generic Type Bounds**
   - All type parameters have required bounds
   - Bounds are sufficient for the usage within the generic function or struct
   - No unnecessary bounds that over-constrain the API
   - Trait object bounds are correctly specified

3. **Unsafe Block Justification**
   - Every unsafe block has a clear reason (e.g., FFI, low-level optimization)
   - Safety requirements are documented in inline comments
   - Safety invariants are not violated
   - Safer alternatives have been ruled out

4. **Semantic Type Usage**
   - Newtypes are used and not bypassed via direct field access
   - Type aliases are used appropriately (not hiding complexity)
   - Error types carry sufficient context
   - Type system is leveraged to encode domain invariants

### Coverage Boundaries

Assumes:
- Code compiles without errors (`cargo check` passes)
- Borrow checker warnings are resolved (`cargo build` succeeds)
- Basic API contracts are documented
- The caller provides specific code sections to validate

## Key Concepts

### 1. Lifetime Correctness

**What it is**: Rust's lifetime system ensures references do not outlive the values
they reference. A correct program has no dangling pointers and no use-after-free.

**How to validate**:
- Examine function signatures with input and output references
- Verify lifetime annotations match the actual borrowing pattern
- Check for lifetime-related compiler warnings from `cargo check`
- Verify the function's safety contract is enforced by the type system

**Example: Dangling Reference**
```rust
// INVALID: 'a outlives the borrowed value
fn bad_ref(s: &str) -> &'a str {
    let temp = "hello".to_string();
    &temp  // ERROR: borrowed value does not live long enough
}

// VALID: return lifetime matches input lifetime
fn good_ref<'a>(s: &'a str) -> &'a str {
    s
}
```

### 2. Generic Type Parameter Bounds

**What it is**: When a function or struct is generic over `T`, it may require `T`
to implement certain traits (bounds) to use operations on `T` within that function.

**How to validate**:
- Examine all `T` usages inside the generic function or struct
- Verify each operation on `T` is satisfied by the declared bounds
- Check for unnecessary bounds that over-constrain the API
- Verify bounds are specified in the generic declaration, not in `where` clauses
  unnecessarily

**Example: Missing or Unnecessary Bounds**
```rust
// INVALID: Clone is used but not required by bounds
fn clone_item<T>(t: T) -> T {
    t.clone()  // ERROR: T does not have Clone
}

// VALID: Clone is required
fn clone_item<T: Clone>(t: T) -> T {
    t.clone()
}

// VALID: Bounds are correct and necessary
fn needs_both<T: Clone + Display>(t: T) {
    let _rendered = t.to_string();
    let _cloned = t.clone();
}

// UNNECESSARY: Extra bound not used
fn only_needs_clone<T: Clone + Default>(t: T) -> T {
    t.clone()  // Default is not used; remove it
}
```

### 3. Unsafe Block Necessity

**What it is**: Unsafe blocks disable compiler checks to allow low-level operations
like dereferencing raw pointers or calling C functions. They must be justified and
carefully documented.

**How to validate**:
- Every unsafe block has a clear reason documented in comments
- The reason is one of: FFI, low-level optimization, hardware access, or other
  legitimate safety-critical need
- Safety invariants are explained (what must be true for the unsafe code to be safe)
- Safer alternatives have been ruled out or acknowledged
- No unnecessary unsafe blocks (e.g., wrapping safe code)

**Example: Justified vs. Unjustified Unsafe**
```rust
// INVALID: Unsafe without reason
unsafe {
    let x = 42;
}

// VALID: FFI requires unsafe, reason documented
// SAFETY: Called only after validating the FFI contract.
unsafe {
    c_function(42)  // Assumes c_function is a valid FFI binding
}

// INVALID: Wrapping safe code unnecessarily
unsafe {
    let s = "hello".to_string();  // No unsafe operations; remove unsafe block
}

// VALID: Justified by ownership pattern
// SAFETY: We own both references and guarantee no aliasing.
unsafe {
    *ptr = value;  // Setting a value through a raw pointer
}
```

### 4. Semantic Type Usage

**What it is**: Newtypes and semantic types encode domain invariants in Rust's type
system, making it impossible to misuse them. Correct usage means not bypassing the
type through direct field access or transmute.

**How to validate**:
- Newtypes are constructed via explicit constructors or pub fields
- Direct field access is only used when intentional and documented
- Type aliases are used for clarity, not hiding complexity
- Error types carry sufficient context (not just strings)
- Semantic meaning is preserved across API boundaries

**Example: Newtype Misuse**
```rust
// VALID: Newtype with private field
pub struct UserId(u64);

impl UserId {
    pub fn new(id: u64) -> Self {
        UserId(id)
    }
}

// INVALID: Bypassing the newtype safety
let user_id: UserId = UserId::new(42);
let raw_id: u64 = user_id.0;  // Direct field access defeats the purpose

// VALID: Explicit newtype getter when needed
pub fn raw_id(&self) -> u64 {
    self.0
}
```

## Composition & References

### Review Authorities

- `plans/<feature-slug>/plan/domain-spec.md` - semantic type intent,
  invariants, and error context requirements.
- `plans/<feature-slug>/plan/function-sig-plan.md` - ownership, lifetimes, and
  generic-bound expectations visible at API boundaries.
- `plans/<feature-slug>/plan/dependency-graph.md` - boundary direction and
  cross-module type exposure rules.
- `plans/<feature-slug>/plan/implementation-plan.md` - declared unsafe, FFI, or
  low-level surfaces that need justification.
- Changed code - the concrete types, impls, and unsafe blocks under review.

### Review Output

```
Changed code + deterministic review artifacts
    ↓
Type review in this skill
    ↓
Findings ordered by severity
    ↓
Each finding linked back to the governing handoff file with a shared pass|fail signal
```

## Review Signal

Use the same `pass|fail` vocabulary as the deterministic type-tooling
layer. Apply it through review of the scoped code and evidence.

| Condition | Signal |
|----------|--------|
| Critical type-safety findings present | `fail` |
| Only major/minor warning-level findings remain | `pass` with warnings |
| Validation timed out or required evidence is incomplete | `fail` |

### Deterministic Evidence Sources

Use current deterministic artifacts when they are part of the handoff. If fresh
evidence is required, use these repo-approved commands:

1. **cargo check** - Compile-time type errors and warnings
   ```sh
   cargo check --all-targets
   ```
   Extract type-related diagnostics (lifetime, generics, borrow checker).

2. **cargo clippy** - Lint suggestions, especially around unsafe and type usage
   ```sh
   cargo clippy --all-targets -- -W clippy::all -W clippy::pedantic
   ```
   Focus on `unsafe_code`, `type_complexity`, and other type-related lints.

3. **Manual inspection** - For semantic type usage and unsafe justification
   - Read function signatures with references and lifetimes
   - Review unsafe block comments and invariant documentation
   - Check newtype usage patterns

**How to interpret diagnostics**:
- `lifetime mismatch` → Lifetime annotation error (critical)
- `the trait bound ... is not satisfied` → Missing or wrong bounds (major)
- `unsafe function call` → Unsafe block needed; verify documented (depends)
- `mutation of non-mutable binding` → Generic bound issue with Mut/Copy (major)

## Examples

### Example 1: Lifetime Validation

**Scenario**: Code review finds function signature change with lifetimes.

**Before** (Invalid):
```rust
pub fn parse_header(response: &str) -> &str {
    let header = response.lines().next();
    &header.unwrap_or("")  // ERROR: dangling reference
}
```

**Validation Finding**: 
- Rule: "Lifetime correctness: no dangling references"
- Evidence: Borrow checker error; temporary value from `unwrap()` does not live
  long enough
- Severity: Critical (undefined behavior risk)
- Correction: Return owned String or take lifetime from input

**After** (Valid):
```rust
pub fn parse_header<'a>(response: &'a str) -> &'a str {
    response.lines().next().unwrap_or("")  // Correct: lifetime matches input
}
```

### Example 2: Generic Bounds Validation

**Scenario**: Generic function added with insufficient bounds.

**Before** (Invalid):
```rust
pub fn apply_all<T>(items: Vec<T>, f: impl Fn(T) -> T) -> Vec<T> {
    items.iter().map(|item| f(*item)).collect()
    // ERROR: T does not implement Copy; cannot *item
}
```

**Validation Finding**:
- Rule: "Generic bounds satisfaction: all type parameters have required bounds"
- Evidence: Compiler error; cannot copy T without Copy bound
- Severity: Critical (does not compile)
- Correction: Add Copy or Clone bound, or change signature

**After** (Valid):
```rust
pub fn apply_all<T: Copy>(items: Vec<T>, f: impl Fn(T) -> T) -> Vec<T> {
    items.iter().map(|item| f(*item)).collect()
}
```

### Example 3: Unsafe Block Justification

**Scenario**: New unsafe code added without documentation.

**Before** (Invalid):
```rust
pub fn raw_transmute(bytes: Vec<u8>) -> u64 {
    unsafe {
        *(bytes.as_ptr() as *const u64)
    }
}
```

**Validation Finding**:
- Rule: "Unsafe block necessity: every unsafe block must be justified"
- Evidence: No safety comment; alignment and bounds not verified
- Severity: Critical (potential UB: unaligned access, dangling pointer)
- Correction: Document safety invariants or use safe alternative

**After** (Valid):
```rust
pub fn bytes_to_u64(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 8 {
        return None;
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[..8]);
    Some(u64::from_le_bytes(buf))  // Safe, no unsafe needed
}

// Or if unsafe is truly justified:
pub fn raw_cast_aligned(ptr: *const u64) -> u64 {
    // SAFETY: Caller must ensure ptr is:
    // - properly aligned for u64
    // - valid and initialized
    // - not aliased for the duration of this call
    unsafe { *ptr }
}
```

### Example 4: Semantic Type Validation

**Scenario**: Newtype usage changed; direct field access introduced.

**Before** (Invalid):
```rust
pub struct RequestId(u64);

impl RequestId {
    pub fn new(id: u64) -> Self {
        RequestId(id)
    }
}

pub fn handle_request(id: RequestId) {
    let raw = id.0;  // Bypasses newtype safety
    log_request(raw);  // Lost type context
}
```

**Validation Finding**:
- Rule: "Semantic types: newtypes used correctly, not bypassed"
- Evidence: Direct field access defeats the purpose of the newtype
- Severity: Major (type safety violation)
- Correction: Provide explicit getter or pass newtype through

**After** (Valid):
```rust
pub fn handle_request(id: RequestId) {
    let _request_id = log_request(id);  // Preserve type through API
}

pub fn log_request(id: RequestId) -> u64 {
    id.as_u64()
}

impl RequestId {
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}
```

## Decision Criteria

### Severity Classification

Use these criteria to classify findings and set severity:

| Finding Type | Severity | Reason |
|---|---|---|
| Lifetime mismatch → dangling reference | Critical | UB risk: use-after-free |
| Missing bounds → compiler error | Critical | Does not compile |
| Generic variance violation | Critical | Type safety violation (can cause UB) |
| Unsafe block missing comment | Major | Difficult to audit; violates safety culture |
| Unsafe block with unjustified reason | Critical | Potential UB; breaks safety contract |
| Newtype bypassed via direct field access | Major | Type safety violation; defeats purpose |
| Over-constrained bounds (unnecessary) | Minor | API too restrictive; not wrong but suboptimal |
| Error type loses context | Major | Makes debugging harder; not immediate safety risk |
| Trait object bounds missing | Major | Runtime panic risk in some contexts |

### Finding Interpretation Guidance

Use these criteria to interpret type-review evidence and explain the resulting
findings:

1. **Critical findings present**: Describe them as blocking type-safety issues.
2. **Pattern of major findings**: Several major findings usually indicate a
   broader misuse of the type system that should be described as a pattern.
3. **Isolated major finding**: Explain the local impact, the surrounding safe
   evidence, and the needed correction.
4. **Only minor findings**: Record them as API-shaping or maintainability notes.
5. **No findings**: Summarize the evidence showing that the reviewed types
   uphold the intended invariants.

**Suggested summary pattern**:
- Critical findings → state the unsafe, lifetime, bounds, or invariants issue
  and the concrete evidence for it.
- Several major findings → describe the repeated type-design weakness and where
  it appears.
- One or two major findings → document the specific break and the surrounding
  context.
- Minor-only findings or no findings → note cleanup items or the evidence that
  supports the reviewed type design.

## Validation Rules

### Lifetime Correctness Rules

1. **No Dangling References**: Every returned reference must be backed by an input
   parameter or a value with a longer lifetime than the function. Compiler enforces
   this; flagged by `cargo check`.

2. **Lifetime Annotations Present**: Functions that take or return references must
   have explicit lifetime annotations (unless elision rules apply). If elision is
   used, verify it correctly reflects intent.

3. **Lifetime Variance Respected**: Lifetimes must follow Rust's variance rules:
   - Covariance (OK to use shorter lifetime where longer expected)
   - Contravariance (parameters only)
   - Invariance (unusual; only for data types, not function parameters)

4. **Mutable Reference Exclusivity**: At most one mutable reference to a value at
   any time. Compiler enforces; flag if workarounds (e.g., `Cell`) are used
   incorrectly.

### Generic Bounds Rules

1. **All Parameters Have Required Bounds**: Every use of a generic parameter `T`
   must be covered by a bound on `T`. For example, if `T::clone()` is called, `T`
   must have `Clone` bound.

2. **Bounds are Sufficient**: If the function calls a method on a parameter, that
   method must be available via a bound. Example: if `t.to_string()` is called, `T`
   must implement `Display` or similar.

3. **Bounds are Necessary**: Remove bounds that are not actually used. Over-constraining
   limits API usability.

4. **Trait Object Bounds**: Trait objects (e.g., `dyn Trait`) must specify all
   required lifetime and static bounds (e.g., `dyn Trait + Send + 'static`).

5. **Associated Type Bounds**: If a generic parameter has associated types, they must
   be constrained where needed (e.g., `T: Iterator<Item = String>`).

### Unsafe Block Rules

1. **Every Unsafe Block Has a Comment**: Inline comment must explain why unsafe is
   necessary and document the safety invariants.

2. **Safety Invariants Documented**: Comment must state what preconditions must hold
   for the unsafe code to be sound (e.g., "Caller must ensure alignment").

3. **Unsafe is Minimal**: Wrap only the unsafe operations, not surrounding safe code.

4. **No `unsafe` Functions Without Reason**: If a function is declared `unsafe fn`,
   there must be a safety contract documented in its Rustdoc.

5. **`#[allow(unsafe_code)]`**: If unsafe code must be suppressed from clippy, the
   `allow` directive must be on the specific line or block, not at module level.

6. **No Unsafe Except For**: Valid reasons for unsafe:
   - Foreign Function Interface (FFI) calls
   - Raw pointer dereference (low-level memory access)
   - Transmute or other type conversions that compiler cannot verify
   - Atomic operations (when needed for performance)
   - Other well-documented, unavoidable safety-critical needs

### Semantic Type Rules

1. **Newtypes Not Bypassed**: Newtype fields should not be accessed directly outside
   the module that defines them, unless explicitly designed as pub struct. Use getter
   methods instead.

2. **Type Aliases Used for Clarity**: Type aliases like `type Seconds = u64` should
   clarify intent, not hide complexity.

3. **Error Types Carry Context**: Error types (custom enums, not bare strings)
   should provide enough context for debugging and recovery.

4. **Semantic Meaning Preserved**: Types that carry semantic meaning (e.g., `UserId`
   vs `u64`) should not be transmuted or cast away without explicit justification.

5. **Generic Newtypes Constructed Properly**: If a newtype wraps a generic type
   (e.g., `struct Id<T>(T)`), construction should use trait bounds to ensure `T` is
   valid for the use case.
