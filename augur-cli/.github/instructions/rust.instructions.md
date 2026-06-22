---
description: "Applies to Rust source work. Points to Rust capability routing and key repo rules."
applyTo: "**/*.rs"
---

# Rust File Context

## Skills To Invoke

- Use [`.github/local/language-companions.md`](../local/language-companions.md)
  as the authoritative Rust capability map.
- Common capability keys surfaced in Rust work include
  `2-plan-architecture-planning` for placement decisions,
  `2-plan-test-planning` for test planning, and
  `4-review-architecture-validation` for architecture review.
- For Rust test implementation and Red/Green execution guidance, consult
  [`.github/local/language-companions.md`](../local/language-companions.md) for
  the Rust mapping instead of assuming a separate universal test
  implementation skill.
- Do not invent or hardcode Rust-specific aliases.

## TDD and Commit Policy

See [`.github/local/rules.md`](../local/rules.md) for the full policy.
Quick reminders:
- Write the failing test **first** (Red → Green → Refactor). No exceptions.
- Bug fixes require a regression test that fails before the fix is applied.
- Definition of done: all tests pass, no stubs, no deferred behavior.

## High-Priority Rust Reminders

These rules are commonly missed. See
[`.github/local/language-companions.md`](../local/language-companions.md) for
capability mapping and [`.github/local/directories.md`](../local/directories.md)
for layout details.

**Decomposition limits**
- Max **3 parameters** per function - bundle excess into a named struct.
- Max **5 fields** per struct - extract semantic sub-structs.
- Any non-exempt struct with **3+ fields** requires `#[derive(bon::Builder)]`.

**Type safety**
- Wrap domain-significant numeric and string values in **newtypes**.

## Newtypes Required For

- Any struct field that's `String` and represents a domain value (`FilePath`,
  `FileName`, `Email`, etc.).
- Any struct field that's `u32`, `u64` and represents a measured/counted value
  (`ByteCount`, `TokenCount`, `LineNumber`, etc.).
- Any struct field that's `f64` and represents a semantic measurement
  (`Price`, `Duration`, `Percentage`).
- Any struct field that's `bool` and represents domain state or policy
  (`IsArchived`, `HasMfaEnabled`, `CanRetry`, etc.).

DO NOT leave bare primitives in:
- Request/Response DTOs
- Public domain types
- API struct fields
- For single-field semantic newtypes that should preserve the inner wire
  format, prefer `#[serde(transparent)]` (or equivalent transparent serde
  handling) at serialization boundaries.
- Do not use transparent serde when the type needs a custom wire format,
  custom validation, or custom encoding/decoding behavior.
- **Parse, don't validate** - convert raw input to validated domain types at
  the outermost boundary; never pass raw data inward.

**Observability**
- Use `tracing` for all runtime output. **Never** use `println!` or `eprintln!`
  in production code.

**Constants**
- No magic numbers - use named constants or descriptively named helpers.

**Tests**
- Test files live in `tests/` and mirror the `src/` directory structure.
- Mirrored test file naming: `tests/<src_directory>/<src_file>_test.rs`
  - Example: `src/domain/user.rs` → `tests/domain/user_test.rs`
- This repo may also contain standalone harness files and other non-1:1 cases; verify the existing local pattern before adding a file.
- In source files, keep only a `#[cfg(test)] #[path = "..."] mod tests;` stub
  when a mirrored external test file already exists for that module.

**No unsafe without approval**
- Do not introduce `unsafe` blocks without explicit user approval.

**Avoid shims**
- Do not create shim functions, type aliases, or wrapper modules that add no
  semantic value. This applies to both functionality and types.
- Type redirects (e.g., `pub type UserId = String;`) hide the actual type being
  used and obscure important semantic information about what you're working with.
  If `String` is what callers need, use `String` directly. If semantic wrapping is
  needed, use a proper newtype (`struct UserId(String)`), not a type alias.
- Exception: re-exports from `lib.rs` files are acceptable and encouraged to shape
  the public API (e.g., `pub use crate::domain::User;` for convenient access).
- Shims and type redirects hide the actual implementation without adding safety,
  validation, or semantic meaning. Callers and implementers need to know what
  types and functions they're actually using.
