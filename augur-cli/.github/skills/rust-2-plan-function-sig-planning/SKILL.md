---
name: rust-2-plan-function-sig-planning
description: >
  Plans and reviews Rust function signatures for idiomatic ownership,
  lifetimes, error handling, trait bounds, and attributes. Use when a feature
  plan defines the intended signature and you need a Rust-correct API shape.
---

# Rust 2 Plan Function Signature Planning

## Handoff Inputs

Use this skill once the intended function names, parameters, return values, and
constraints are documented. Prefer:

- `plans/<feature-slug>/plan/function-sig-plan.md` for function names,
  parameters, return values, visibility, and async/sync intent.
- `plans/<feature-slug>/plan/domain-spec.md` for business-level error
  categories and semantic type expectations.
- `plans/<feature-slug>/plan/dependency-graph.md` for trait boundaries, module
  ownership, and allowed cross-module references.
- `plans/<feature-slug>/plan/implementation-plan.md` for runtime constraints
  that affect ownership or allocation choices.

Focus on:

- **Ownership semantics**: Owned vs. borrowed parameters; when to consume vs. borrow.
- **Lifetime parameters**: Explicit lifetimes only when ambiguous; proper elision.
- **Trait bounds**: Generic type constraints ensuring correctness at compile time.
- **Error type mapping**: Result<T, E> wrapper for fallible operations; Option<T> for single absence case.
- **Attributes**: #[must_use], #[inline], #[deprecated] as needed.
- **Doc comments**: Safety notes, panics, invariants for API consumers.

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### 1. Ownership & Borrowing Rules

**Principle:** Ownership semantics must be explicit in the function signature.
Ownership decision determines borrowing, lifetime, and caller responsibility.

**Decision Rule:**
```
Function mutates input?
  Yes → &mut T
  No → Function reads repeatedly? 
    Yes → &T (slice if homogeneous collection)
    No → Consume ownership (T)
```

**Pattern:**
```rust
// Borrowed slice (preferred over &Vec for flexibility)
pub fn sum(values: &[i32]) -> i32 { ... }

// Mutable borrow (caller retains ownership, we modify)
pub fn sort_in_place(list: &mut [u32]) { ... }

// Owned (we take ownership)
pub fn take_ownership(data: Vec<String>) -> String { ... }
```

**When to borrow:**
- Function reads without mutation → `&T`
- Function mutates in place → `&mut T`
- Multiple calls on same data → Borrow (cheaper than move)

**When to own:**
- Function needs to extend lifetime beyond call scope.
- Function will store data in heap structure.
- Function semantics require "consuming" the input.

---

### 2. Lifetime Parameters

**Principle:** Lifetimes model borrowing relationships. Explicit naming is required only
when ambiguous; Rust compiler auto-elides in most cases.

**Elision Rules:**
- Single borrowed input → output lifetime elided to input lifetime (compiler default)
- Multiple borrowed inputs → explicit lifetimes needed if output is a reference
- No borrowed inputs → no lifetimes needed (owned types)

**Pattern:**
```rust
// Elision OK: single input borrow, return is same lifetime
pub fn process(item: &Item) -> &str { ... }

// Explicit 'a needed: two inputs with different lifetimes, one is returned
pub fn borrow_from_either<'a>(a: &'a Item, b: &Item) -> &'a str { ... }

// Trait object requires explicit lifetime
pub fn handle_logger(logger: &dyn Logger + 'a) { ... }
```

**Decision tree:**
```
Does function return a reference?
  No → No lifetimes needed
  Yes → 
    Single input parameter?
      Yes → Elide (Rust's default)
      No → 
        Return references same input as which parameter?
          Ambiguous → Error in spec; clarify
          Clear → Name explicit lifetime on that parameter and return
```

---

### 3. Trait Bounds & Where Clauses

**Principle:** Generic type parameters require trait bounds to guarantee behavior.
Simple bounds go inline; complex ones use where clauses.

**Pattern:**
```rust
// Simple bounds on type parameter
pub fn collect_sorted<T: Ord>(items: Vec<T>) -> Vec<T> { ... }

// Multiple bounds
pub fn format_data<T: Debug + Display>(data: &T) -> String { ... }

// Complex bounds use where clause
pub fn update<T, U>(val: T, other: U) 
where
    T: Clone + Default,
    U: AsRef<str>,
{ ... }

// Trait objects for polymorphism (no generic)
pub fn handle(handler: &dyn Handler) { ... }
```

**When to use bounds:**
- Generic function needs specific behavior from type parameter.
- Multiple type parameters with overlapping constraints.

**When to avoid over-binding:**
- Bound is not actually used in function body → Remove it.
- Type parameter only appears in owned form, no method calls → No bounds needed.

---

### 4. Error Handling: Result<T, E>

**Principle:** Fallible operations return `Result<OutputType, ErrorType>`.
Infallible operations return the bare type (or `Option<T>` for single absence case).

**Decision Rule:**
```
How many error cases?
  0 → Bare type (e.g., i32, String)
  1 (and it's "not found") → Option<T>
  2+ or domain-specific → Result<T, ErrorEnum>
```

**Pattern:**
```rust
// Infallible operation
pub fn sum(values: &[i32]) -> i32 { ... }

// Single error case: "not found"
pub fn get_user(id: u64) -> Option<User> { ... }

// Multiple error cases
pub fn load_config(path: &Path) -> Result<Config, ConfigError> { ... }

// Custom error enum for domain specificity
pub enum ParseError {
    InvalidFormat,
    Truncated,
    Utf8Invalid,
}

pub fn parse(input: &str) -> Result<Data, ParseError> { ... }
```

**Never return bare Option for domain errors:**
- `Option<T>` signals "this value may not exist", not "operation failed"
- Use `Result<T, E>` for exceptions, validation failures, I/O errors

---

### 5. Attributes

**Principle:** Attributes guide compiler and document API contracts.

| Attribute | When to Use | Example |
|-----------|-----------|---------|
| `#[must_use]` | Function computes important value; ignoring result is likely a bug | `#[must_use] pub fn verify() -> bool` |
| `#[inline]` | Only on trivial wrappers (< 10 lines); compiler decides most cases | `#[inline] fn unwrap_or_panic(x: Option<T>) -> T` |
| `#[deprecated]` | Function is being phased out; pair with migration path in doc comment | `#[deprecated(since = "1.2", note = "use new_api instead")]` |
| `#[allow(dead_code)]` | Only in test modules or intentional stubs | Rare; avoid in public API |

---

## Examples

### Example 1: Simple Read-Only Operation

**Input spec:**
```
Name: verify_checksum
Parameters: data (bytes), expected (hash value)
Return: boolean
Error: None (always succeeds)
Attributes: #[must_use]
```

**Rust Signature:**
```rust
/// Verifies that data matches the expected checksum.
///
/// # Example
///
/// ```
/// let data = b"hello";
/// let checksum = b"world";
/// assert!(verify_checksum(data, checksum));
/// ```
#[must_use]
pub fn verify_checksum(data: &[u8], expected: &[u8]) -> bool {
    // ...
}
```

**Reasoning:**
- `data: &[u8]` - borrowed slice; read-only, caller retains ownership.
- `expected: &[u8]` - same; no mutation.
- `bool` - no Result; spec says always succeeds.
- No lifetime params - both inputs are independent (Rust elides).
- `#[must_use]` - caller must check result to avoid security bug.

---

### Example 2: Fallible Operation with Error Mapping

**Input spec:**
```
Name: load_config
Parameters: path (file path)
Return: configuration object
Error: file not found, parse error, permission denied
Attributes: None
```

**Rust Signature:**
```rust
/// Loads configuration from the given file path.
///
/// # Errors
///
/// Returns `Err` if:
/// - The file does not exist (`ConfigError::NotFound`)
/// - The file cannot be parsed (`ConfigError::ParseError`)
/// - Permission denied (`ConfigError::PermissionDenied`)
pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    // ...
}

#[derive(Debug)]
pub enum ConfigError {
    NotFound,
    ParseError(String),
    PermissionDenied,
}
```

**Reasoning:**
- `path: &Path` - borrowed filesystem reference; read-only.
- `Result<Config, ConfigError>` - three error cases; use custom enum.
- No lifetime - Path ref is input-scoped.

---

### Example 3: Generic with Trait Bounds

**Input spec:**
```
Name: collect_sorted
Parameters: items (collection), comparator (behavior)
Return: sorted vector of items
Error: comparison failed or invalid state
Attributes: None
```

**Rust Signature:**
```rust
/// Collects items into a sorted vector.
///
/// # Errors
///
/// Returns `Err` if comparison fails or comparison order is unstable.
pub fn collect_sorted<T>(items: impl IntoIterator<Item = T>) -> Result<Vec<T>, SortError>
where
    T: Ord,
{
    // ...
}
```

**Reasoning:**
- `impl IntoIterator<Item = T>` - flexible input (Vec, slice, iterator).
- `T: Ord` - comparator provided by trait; avoids extra parameter.
- `where T: Ord` - trait bound for readability.
- `Result<Vec<T>, SortError>` - captures error enum for "comparison failed".
- No lifetime - owned Vec returned; generic T is concrete after monomorphization.

---

### Example 4: Output Lifetime Depends on Input

**Input spec:**
```
Name: extract_field
Parameters: record (struct), key (string)
Return: field value reference
Error: key not found
Attributes: None
```

**Rust Signature:**
```rust
/// Extracts a field value from a record by key.
///
/// # Errors
///
/// Returns `Err` if the key is not found in the record.
pub fn extract_field<'a>(record: &'a Record, key: &str) -> Result<&'a str, FieldError> {
    // ...
}
```

**Reasoning:**
- `'a` on `record` and return - returned reference lives as long as record.
- `key: &str` - no lifetime needed; used only for lookup, not returned.
- Lifetime is explicit here - return type `&'a str` depends on `record`'s lifetime.
- `Result<&'a str, FieldError>` - fallible operation (key may not exist).

---

## Decision Criteria

### Signature Choice Matrix

| Scenario | Ownership | Lifetime | Trait Bounds | Error Handling |
|----------|-----------|----------|--------------|----------------|
| Read-only shared data | `&T` or `&[T]` | Auto-elided if single input | None (use T directly) | Option or Result |
| Mutable shared access | `&mut T` | Name if multiple borrows | None unless needed | Result |
| Consumed data | `T` | None (owned) | Use trait objects if generic | Result |
| Generic transformation | `T` | Explicit if output borrows | Usually needed (Ord, Clone, etc.) | Result |
| Trait objects for flexibility | `dyn Trait` | May need lifetime | Define trait | Result |

### Elision Decision Tree
```
Does function return a reference?
  No → No lifetimes needed
  Yes → 
    Single input parameter?
      Yes → Elide (Rust's default)
      No → 
        Return references same input as which parameter?
          Ambiguous → Error in spec (clarify)
          Clear → Name explicit lifetime on that parameter and return
```

### Error Type Decision Tree
```
How many error cases?
  0 → Bare type
  1 (and it's "not found") → Option<T>
  1+ (domain-specific) → Result<T, ErrorEnum>
  External library errors → Result<T, Box<dyn Error>>
```

---

## Validation Rules

Every planned Rust signature should pass these checks:

### Compile-Time Checks (Rust Compiler)
1. **Syntax:** Signature parses without errors
2. **Type Checking:** All types are in scope and valid
3. **Lifetime Coherence:** No lifetime parameter mismatches
4. **Trait Bounds:** All traits in bounds are in scope; no circular bounds
5. **Generic Parameters:** Used in at least one parameter or return type

### Semantic Checks (Human Review)
1. **Ownership Semantics:** Matches intent (owns/borrows/mutates)
2. **Error Model:** Fallible operations return `Result`; infallible do not
3. **Trait Bound Necessity:** No gratuitous bounds; each serves a purpose
4. **Lifetime Explicitness:** Explicit only when ambiguous; elided otherwise
5. **Idiomatic Naming:** Type parameters follow convention (`T`, `E`, not `Ty`)
6. **Documentation:** Signature includes safety notes if unsafe or has invariants

### Safety Checks
1. **Unsafe Boundary:** If signature uses `unsafe`, document invariants in doc comment
2. **Panics:** If function may panic, document in doc comment (#[doc])
3. **Memory Safety:** No use of raw pointers without explicit reasoning in plan

---

## Composition & References

### Primary references
- `plans/<feature-slug>/plan/function-sig-plan.md` - primary authority for
  expected signatures, parameters, returns, and async intent.
- `plans/<feature-slug>/plan/domain-spec.md` - semantic meaning, invariants, and
  error taxonomy for types used in the signature.
- `plans/<feature-slug>/plan/dependency-graph.md` - module and trait boundaries
  that determine visibility and trait placement.
- `plans/<feature-slug>/plan/implementation-plan.md` - performance or runtime
  constraints that affect ownership and allocation choices.
- [`.github/local/directories.md`](../../local/directories.md) - canonical path
  layout for locating the signature's owning module.

---

## Resolve unclear inputs

- **Issue:** Ambiguous lifetime rules → Clarify the borrow source and output
  ownership in `plans/<feature-slug>/plan/function-sig-plan.md`
- **Issue:** Generic bound mismatch → Reconcile trait/module boundaries in
  `plans/<feature-slug>/plan/dependency-graph.md`
- **Issue:** Error type conflict → Reconcile the error taxonomy in
  `plans/<feature-slug>/plan/domain-spec.md`
