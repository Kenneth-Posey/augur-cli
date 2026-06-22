---
name: rust-4-review-type-validation-examples
description: >
  Concrete worked examples for Rust type validation - correct and incorrect
  patterns for lifetimes, generic bounds, unsafe, and semantic types. Load when
  needing annotated examples to apply during type review.
---

# Skill: Rust Type Validation - Examples

---

## Examples

PASS/FAIL labels show the review outcome each example demonstrates. Some are
compiler-enforced; others rely on reviewer judgment.

### Example 1: Correct Lifetime - ✓ PASS

```rust
// Signature: fn parse(input: &str) -> Result<Config, ParseError>
// Question: Is output lifetime correct?

fn parse(input: &str) -> Result<Config, ParseError> {
    // ...
    Ok(Config { /* fields do NOT borrow from input */ })
}

// Answer: ✓ CORRECT
// Rationale: Output (Config) does not borrow from input, so no lifetime parameter needed.
// Lifetime inference: Result has no borrowed data; 'static or no lifetime required.
```

### Example 2: Dangling Reference - ✗ FAIL

```rust
// Signature: fn get_ref(s: &str) -> &str
// Question: Why is this problematic without lifetime?

fn get_ref(s: &str) -> &str {
    // Implicit: fn get_ref<'a>(s: &'a str) -> &'a str
    &s[0..1]  // Output is derived from s; lifetime is correct (though omitted via elision)
}

// BUT if written as:
fn get_ref(s: &str) -> &str {  // <- Ambiguous: which lifetime does output have?
    static CACHED: &str = "cache";
    CACHED  // Output is 'static, not from s; lifetime is WRONG
}

// Compiler Error:
// error[E0515]: cannot return value referencing local variable
//   --> src/lib.rs:3:5
//    |
// 3  |     &s[0..1]
//    |     ^^^^^^^
//    |     |
//    |     value is borrowed from local variable
//    |     returns a reference to data owned by the current function
```

**Rationale**: If output borrows from input, the output lifetime must be derived from input lifetime. If output is `'static` or owned, use those explicitly.

### Example 3: Generic Bounds - Correct - ✓ PASS

```rust
// Signature: fn serialize<T: Serialize>(item: &T) -> String
// Question: Is the bound necessary?

use serde::Serialize;

fn serialize<T: Serialize>(item: &T) -> String {
    serde_json::to_string(item).unwrap()
    // ^^^^^^^^ serde_json needs Serialize trait; bound is necessary
}

// Answer: ✓ CORRECT
// Rationale: `T: Serialize` is necessary because `to_string` requires it.
// No extra bounds; no associated types needed (Serialize has none).
```

### Example 4: Over-Constrained Bounds - ✗ FAIL

```rust
// Signature: fn id<T: Clone>(x: i32) -> i32
// Question: Are the bounds correct?

fn id<T: Clone>(x: i32) -> i32 {
    // `T` is never used; `Clone` bound is unnecessary
    // `T` is not mentioned in the function body
    x
}

// Answer: ✗ INCORRECT
// Rationale: `T` is not used, so the bound is dead code.
// Even if `T` were used, if function only returns i32, `T: Clone` might not be needed.

// Correct:
fn id(x: i32) -> i32 { x }

// Cargo check output:
// $ cargo clippy
// warning: unused generic parameter
//   --> src/lib.rs:5:16
//    |
// 5  | fn id<T: Clone>(x: i32) -> i32 { x }  // <- Remove it
```

### Example 5: Unsafe Block - Justified and Minimal - ✓ PASS

```rust
// CORRECT: Unsafe block with clear safety justification
unsafe fn read_memory(ptr: *const u8) -> u8 {
    // SAFETY: Caller must ensure:
    // 1. ptr is a valid pointer (non-null, properly aligned)
    // 2. ptr points to initialized memory containing a valid u8
    // 3. no other references to this memory exist
    // Violation of these invariants is undefined behavior.
    
    *ptr  // Dereference pointer; safe only under conditions above
}

// Usage (caller responsible for invariants):
#[test]
fn test_read_memory() {
    let value: u8 = 42;
    let ptr = &value as *const u8;
    unsafe {
        assert_eq!(read_memory(ptr), 42);
    }
}
```

### Example 6: Unsafe Block - Missing Justification - ✗ FAIL

```rust
// INCORRECT: Unsafe block without safety comment
fn buggy_transmute<T, U>(t: T) -> U {
    unsafe {
        std::mem::transmute(t)  // <- No comment; what invariants must hold?
    }
}

// Compiler warning / review rejection:
// ✗ Missing safety comment
// ✗ Unsafe block not minimally scoped
// ✗ Caller invariants not documented

// Correct version:
fn safe_transmute<T: Into<U>, U>(t: T) -> U {
    t.into()  // No unsafe; use trait instead
}

// OR if unsafe is actually necessary:
// SAFETY: Transmute is only safe if T and U have identical memory layout.
// This is verified at compile time by the trait bound (not shown here).
unsafe fn transmute_same_layout<T, U>(t: T) -> U {
    std::mem::transmute(t)
}
```

### Example 7: Semantic Type (Newtype) - Correct - ✓ PASS

```rust
// Newtype pattern: Wrapper enforces invariant (positive integer)
#[derive(Clone, Copy)]
pub struct PositiveInt(u32);

impl PositiveInt {
    pub fn new(value: u32) -> Result<Self, String> {
        if value == 0 {
            Err("PositiveInt must be > 0".to_string())
        } else {
            Ok(PositiveInt(value))
        }
    }

    pub fn get(&self) -> u32 {
        self.0
    }
}

// Conversion methods
impl From<PositiveInt> for u32 {
    fn from(p: PositiveInt) -> u32 {
        p.0
    }
}

// Usage:
#[test]
fn test_positive_int() {
    let positive = PositiveInt::new(5).unwrap();
    assert_eq!(positive.get(), 5);
    
    let invalid = PositiveInt::new(0);
    assert!(invalid.is_err());  // ✓ Invariant enforced at construction
}
```

**Validation**:
- [ ] ✓ Newtype pattern correct: single field of wrapped type
- [ ] ✓ Conversion methods present: `From`, `get()`
- [ ] ✓ Type bypass prevented: field is private, no direct access
- [ ] ✓ Invariant upheld: `new()` validates, constructor cannot be bypassed

### Example 8: Semantic Type - Invariant Bypassed - ✗ FAIL

```rust
// INCORRECT: Invariant can be bypassed via public field
pub struct PositiveInt {
    pub value: u32,  // <- Public field: invariant can be violated!
}

impl PositiveInt {
    pub fn new(value: u32) -> Result<Self, String> {
        if value == 0 {
            Err("PositiveInt must be > 0".to_string())
        } else {
            Ok(PositiveInt { value })
        }
    }
}

// Invariant violated:
#[test]
fn test_invariant_bypass() {
    let mut positive = PositiveInt::new(5).unwrap();
    positive.value = 0;  // <- Bypass constructor validation!
    assert_eq!(positive.value, 0);  // ✗ Invariant violated
}

// Correct version (private field):
pub struct PositiveInt(u32);  // Private field
impl PositiveInt {
    pub fn new(value: u32) -> Result<Self, String> {
        if value == 0 {
            Err("...".to_string())
        } else {
            Ok(PositiveInt(value))
        }
    }
}
// Now invariant cannot be bypassed
```

---

## Key Files

- `README.md` - overview and usage notes
