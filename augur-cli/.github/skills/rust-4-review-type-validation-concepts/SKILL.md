---
name: rust-4-review-type-validation-concepts
description: >
  Key concepts for Rust type validation: lifetime correctness, generic bounds,
  unsafe justification, semantic types, and tool checks. Use when reviewing
  Rust type-system usage and need quick validation criteria for these areas.
---

# Skill: Rust Type Validation - Key Concepts

---

## Key Concepts

### 1. Lifetime Correctness & Variance

**Concept:** Lifetimes ensure references are valid when used. Variance rules determine when one lifetime can substitute for another.

**Rules:**
- **Output Lifetimes:** Must be traceable to input(s) or `'static`. No "phantom" output lifetimes.
- **Covariance:** `'a: 'b` means `'a` can be used where `'b` is required (longer lifetime substitutes for shorter).
- **Contravariance:** Function traits are contravariant in parameter lifetimes (opposite of return types).
- **Invariance:** Mutable references are invariant; cannot substitute a different lifetime.

**Rust Validation:**
```rust
// ✓ CORRECT: Output lifetime traceable from single input
fn first(s: &str) -> &str { &s[..1] }

// ✗ WRONG: Output lifetime not traceable
fn dangle() -> &'static str { 
    let s = String::from("hi");
    &s  // Error: `s` does not live long enough
}

// ✓ CORRECT: Multiple inputs with explicit output lifetime
fn longer<'a>(x: &'a str, y: &str) -> &'a str { x }

// ✓ CORRECT: Self lifetime correct for method
impl MyType {
    fn as_ref(&self) -> &MyType { self }  // Output lifetime = &self lifetime
}

// ✗ WRONG: Variance violated-contravariant where covariant required
fn process<'a>(f: &dyn Fn(&'a str) -> &'static str) { }
// Cannot pass `&dyn Fn(&'static str) -> &'static str` here (wrong direction)
```

### 2. Generic Bounds Reasoning

**Concept:** Generic bounds restrict types to ensure safe, sound code. Over-constraining limits reusability; under-constraining fails to compile.

**Rules:**
- **Necessity:** Bound appears in body, constrains other parameters, or is required by invariant
- **Coherence:** Bounds do not conflict (e.g., `T: Copy + Drop` is invalid)
- **Associated Types:** Named or constrained; ambiguous types rejected
- **Lifetime Bounds:** `T: 'a` only for types containing references; `'a: 'b` for outlives

**Rust Validation:**
```rust
// ✓ CORRECT: Clone bound used in body
fn duplicate<T: Clone>(item: T) -> (T, T) { 
    (item.clone(), item.clone()) 
}

// ✗ WRONG: Clone bound not used (over-constrained)
fn identity<T: Clone>(item: T) -> T { item }  // Remove Clone

// ✓ CORRECT: Associated type specified
fn store<I: IntoIterator<Item = String>>(iter: I) { }

// ✗ WRONG: Associated type not specified (ambiguous)
fn store<I: IntoIterator>(iter: I) { }  // Iterator<Item = ?> unknown

// ✓ CORRECT: Lifetime bound for reference-containing type
fn process<'a, T: 'a>(items: &'a [T]) { }

// ✗ WRONG: Lifetime bound for non-reference type
fn store<T: 'static>(item: T) { }  // OK if T contains refs; wrong if T = i32
```

### 3. Unsafe Justification & Minimal Scope

**Concept:** Unsafe code is allowed when safety invariants can be proven, but must be minimal and documented.

**Rules:**
- **Invariant Documented:** Comment explains which safety requirement is needed
- **Actually Upheld:** Code genuinely preserves invariant; not assumed
- **Minimally Scoped:** Only necessary lines inside `unsafe { }`
- **Single Reason:** One invariant per block; split complex cases

**Rust Validation:**
```rust
// ✓ CORRECT: Justified unsafe with proper scope
unsafe fn deref_ptr<T>(p: *const T) -> &'static T {
    // SAFETY: Caller must ensure `p` is valid, properly aligned, 
    //         and points to initialized, never-modified `T`.
    &*p
}

// ✗ WRONG: Over-scoped unsafe (initialization is safe)
unsafe {
    let v = vec![1, 2, 3];  // Safe, not unsafe
    let ptr = v.as_ptr();   // Safe, not unsafe
    let ref_val = &*ptr;    // Unsafe only here
}

// ✓ CORRECT: Multiple invariants = separate blocks
unsafe {
    // SAFETY: Caller ensures `ptr` is valid and initialized
    *ptr = value;
}
unsafe {
    // SAFETY: Caller ensures `ptr` is aligned for type T
    let val = *(ptr as *const T);
}

// ✗ WRONG: No justification
unsafe {
    *ptr = value;  // What invariant? Why is this safe?
}
```

### 4. Semantic Types (Newtype) Pattern

**Concept:** Newtype pattern wraps a type to enforce invariants or create distinct types at compile time.

**Rules:**
- **Single Field:** Wrapper struct contains exactly one field of wrapped type
- **Invariant Clear:** Comment or type name expresses what invariant is enforced
- **Conversions:** `From`, `Into`, `Deref`, `AsRef` present as needed
- **Bypass Prevention:** Private field enforces invariant; public field only if invariant is voluntary
- **Transparent Serde:** If the wrapper should serialize identically to the inner
  type, use `#[serde(transparent)]` or equivalent transparent serde handling;
  custom wire formats or validation require explicit serde attributes or impls.

**Rust Validation:**
```rust
// ✓ CORRECT: Newtype pattern with invariant
pub struct UserId(u64);  // Invariant: IDs are non-zero

impl UserId {
    pub fn new(id: u64) -> Option<Self> {
        if id > 0 { Some(UserId(id)) } else { None }
    }
}

// ✓ CORRECT: Conversion methods for ergonomics
impl From<UserId> for u64 {
    fn from(id: UserId) -> u64 { id.0 }
}

impl Deref for UserId {
    type Target = u64;
    fn deref(&self) -> &u64 { &self.0 }
}

// ✗ WRONG: Public field bypasses invariant
pub struct UserId(pub u64);  // Invariant violated: can set to 0

// ✗ WRONG: Invariant enforced nowhere
pub struct UserId {
    value: u64,  // Invariant: what should this be?
}

// ✓ CORRECT: Conversions preserve invariant
impl From<&UserId> for u64 {
    fn from(id: &UserId) -> u64 { id.0 }  // Still non-zero; invariant OK
}
```

### 5. Tool Checks: cargo check, clippy, borrow checker

**Concept:** Automated tools detect many type errors; review must address tool findings.

**Rules:**
- **cargo check:** Must pass; all compilation errors resolved
- **cargo clippy:** Lint warnings addressed (unless explicitly ignored with rationale)
- **Borrow Checker:** No lifetime or reference validity errors
- **Tool Output:** Use relevant errors or warnings as review evidence

**Rust Validation:**
```bash
# ✓ CORRECT: cargo check succeeds
$ cargo check
   Compiling mylib v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in 0.2s

# ✗ WRONG: cargo check fails (lifetime error)
$ cargo check
error[E0106]: missing lifetime specifier
  --> src/lib.rs:3:12
   |
3  | fn merge(a: &str, b: &str) -> &str { }
   |                                    ^ expected named lifetime parameter

# ✗ WRONG: clippy warning not addressed
$ cargo clippy
warning: unused generic parameter
  --> src/lib.rs:5:16
   |
5  | fn id<T: Clone>(x: i32) -> i32 { x }  // Clone not used; remove it
```

---

## Key Files

- `README.md` - overview and usage notes
