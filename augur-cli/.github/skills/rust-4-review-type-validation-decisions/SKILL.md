---
name: rust-4-review-type-validation-decisions
description: >
  Decision criteria and validation rules for Rust type review. Use when
  evaluating evidence about type usage.
---

# Skill: Rust Type Validation - Decision Criteria

---

## Decision Criteria

### How to Evaluate Review Evidence

**Review inputs**:
- Rust source files with type definitions, function signatures, unsafe blocks
- `cargo check` output
- `cargo clippy` output
- `plans/<feature-slug>/plan/domain-spec.md`
- `plans/<feature-slug>/plan/function-sig-plan.md`

**Evaluation process**:

1. **Run `cargo check`**
   - If errors: Record them as critical type-system findings.
   - If warnings: Check whether `cargo clippy` adds related diagnostics.

2. **Run `cargo clippy`**
   - Lint violations: Record them unless explicitly allowed with
     `#[allow(...)]` and rationale.
   - Type warnings (e.g., unused generics): Record them as review findings.

3. **Apply Lifetime Rules**
   - For each type or function with lifetime parameters, verify that output
     lifetimes trace to inputs or `'static`, variance is correct, and elision
     does not hide a problem.

4. **Apply Generic Bounds Rules**
   - For each generic parameter, verify that every bound is justified, bounds
     do not conflict, and associated types are named or constrained.

5. **Apply Unsafe Justification Rules**
   - For each `unsafe` block, verify that it is scoped minimally, justified by
     a safety comment, and valid at every call site.

6. **Apply Semantic Type Rules**
   - For each newtype, verify that it derives needed traits, exposes explicit
     conversions, and cannot be bypassed through public fields.

7. **Summary Decision**
   - If all rules hold, note that the evidence supports the reviewed type usage.
   - If any rule fails, record the specific rule violation and supporting
     evidence.

### How to Summarize the Review

**Review inputs**:
- Changed type definitions and related function signatures
- `cargo check` and `cargo clippy` outputs
- Relevant plan files listed above

**Review process**:

1. Review the code and `cargo check`/`cargo clippy` output.
2. Identify the semantic intent of each type.
3. Apply the rules above.
4. Decide whether each type fits its semantic purpose.
5. Summarize supporting evidence, rule violations, uncertainties, and follow-up
   notes.

---

## Key Files

- `README.md` - overview and usage notes

## Validation Rules

### Rule Set 1: Lifetimes

**Rule 1.1**: All explicit lifetime parameters MUST have output lifetime traceability

```
If a function has input parameters with lifetimes, output lifetimes MUST be:
  - Traceable to one or more inputs (e.g., `fn foo<'a>(x: &'a T) -> &'a U`)
  - Or explicitly `'static` (e.g., `fn get_constant() -> &'static str`)
  - NOT phantom (e.g., `fn bad<'a>() -> &'a str` is invalid)
```

**Rule 1.2**: Variance rules MUST be respected

```
For function parameters and return types:
  - Covariance (T: U): Longer lifetimes can substitute for shorter
  - Contravariance (opposite for function traits)
  - Invariance (mutable references): No substitution allowed
```

**Rule 1.3**: Self lifetimes in methods MUST match output lifetime rules

```
Example CORRECT:
  impl MyType {
      fn as_ref(&self) -> &MyType { self }
  }

Example INCORRECT:
  impl MyType {
      fn as_ref(&self) -> &'static MyType { ... }  // Self can't guarantee 'static
  }
```

---

### Rule Set 2: Generic Bounds

**Rule 2.1**: Every generic bound MUST be justified

```
A bound appears only if:
  - It is used in the function/method body
  - It constrains another generic parameter
  - It is required by an invariant
  - It enforces trait object safety
```

**Rule 2.2**: Generic bounds MUST not conflict

```
Example INCORRECT:
  fn foo<T: Copy + Drop>() { }  // Conflict: Can't implement both

Example CORRECT:
  fn foo<T: Clone>() { }
```

**Rule 2.3**: Associated types MUST be named or constrained

```
Example CORRECT:
  fn foo<I: Iterator<Item = String>>() { }

Example INCORRECT:
  fn foo<I: Iterator>() { }  // Item type is ambiguous
```

---

### Rule Set 3: Unsafe Blocks

**Rule 3.1**: Unsafe code MUST be scoped minimally

```
Example CORRECT:
  let ptr = box_ptr as *const _;
  let value = unsafe { *ptr };  // Only the dereference is unsafe

Example INCORRECT:
  unsafe {
      let ptr = box_ptr as *const _;
      let value = *ptr;
      process(value);  // process() doesn't need to be unsafe
  }
```

**Rule 3.2**: Unsafe blocks MUST be justified with comments

```
// SAFETY: box_ptr was allocated by Box and is valid for dereference here
let value = unsafe { *ptr };
```

**Rule 3.3**: Safety invariants MUST hold at all call sites

```
If unsafe block assumes `ptr` is valid, all callers MUST ensure this.
Document the assumption in public function docs.
```

---

### Rule Set 4: Semantic Types (Newtypes)

**Rule 4.1**: Newtype wrappers MUST prevent accidental misuse

```
Example CORRECT:
  pub struct UserId(u32);
  impl UserId {
      pub fn get(&self) -> u32 { self.0 }
  }

Example INCORRECT:
  pub struct UserId(pub u32);  // Direct field access defeats type safety
```

**Rule 4.2**: Conversions MUST be explicit

```
Example CORRECT:
  impl From<u32> for UserId { ... }
  impl Into<u32> for UserId { ... }

Example INCORRECT:
  pub fn new(id: u32) -> UserId { UserId(id) }  // Inconsistent patterns
```

---

### Rule Set 5: Tool Integration

**Rule 5.1**: `cargo check` MUST pass with no errors

```
If cargo check produces errors, the type is fundamentally invalid.
No further review until errors resolved.
```

**Rule 5.2**: `cargo clippy` MUST pass or violations MUST be explicitly allowed

```
Lint violations that are dismissed MUST have an explicit #[allow(...)] attribute
with a comment explaining why the lint is safe to ignore.
```

---
