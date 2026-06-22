---
description: "Use when user asks: add domain type, add newtype, add semantic wrapper, add shared type, add domain struct"
name: "Add Domain Type"
argument-hint: "type name, kind (newtype/struct/enum), and domain it belongs to"
agent: "agent"
---
Add a new domain type to `src/domain/` using this prompt's placement, newtype,
and TDD rules.

## Inputs

- Type name and kind: newtype wrapper, struct, or enum (required).
- Domain module it belongs to under `src/domain/` (required).
- Consumers that will use this type (at least one module path required).
- Plan phase spec or behavioral description if this work is plan-driven.

## Task Guidance

1. **Read required guidance** - use `0-utility-codebase-survey`,
   `0-global-critical-rules`, and the `3-implement-domain-implementation`
   language companion before editing.

2. **Confirm placement** - validate that the new type belongs in `src/domain/`
   and does not introduce wrong-direction imports or cycles.

3. **Update the file set**:
   - `src/domain/<module>.rs` - add the type definition. For primitive wrappers,
      use the project's newtype macros (location per
      `.github/local/directories.md`), or write a plain single-field struct if
     no macro is defined. If the wrapper should preserve the inner wire
     format, add `#[serde(transparent)]` (or equivalent transparent serde
     handling); use custom serde only when the type needs a different wire
     format, validation, or encoding. For structs, keep to max 5 fields;
     extract semantic sub-structs if more fields are needed. For enums, prefer
     specific variant names over generic ones.
   - `src/domain/mod.rs` - update to re-export the new type.
   - `tests/domain/<module>.tests.rs` - unit tests covering construction,
     validation, conversion, and boundary behavior.

4. **Red** - write failing tests first for valid, invalid, and boundary cases.

5. **Green** - implement the type to satisfy those failing tests. It must:
   - use the project's newtype macros per `.github/local/directories.md`, or a
     plain single-field struct if no macro is defined,
   - use transparent serde handling for single-field wrappers that should keep
     the underlying wire format,
   - avoid transparent serde when custom wire format, validation, or encoding
     is required,
   - provide Rustdoc for the type, its fields, and its public methods,
   - keep structs to max 5 fields; use semantic sub-structs for larger shapes,
   - ensure the type is exported through `src/domain/mod.rs`.

6. **Refactor** - improve clarity without changing behavior. Re-run the
   relevant validation commands.

7. **Before finishing** - confirm dependency direction, newtype and shape
   rules, Rustdoc coverage, and boundary-case tests.

## Validation

Run after implementation is complete:
```
cargo check
cargo test
```
Confirm:
- Type is in `src/domain/` and exported through `src/domain/mod.rs`.
- Primitive wrappers use the newtype macro, not bare type aliases.
- Struct fields are max 5; domain sub-structs extract additional state.
- Rustdoc covers the type, fields, and all public methods.
- Tests cover construction, validation, conversions, and boundary cases.

## Output

1. File list created or modified
2. Test summary (failing Red, passing Green)
3. Validation results and any unresolved blockers
