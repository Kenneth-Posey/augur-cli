---
name: 0-global-documentation-standards
description: >
  Documentation structure, Markdown/YAML output contracts, and inline Rustdoc
  standards. Use when creating or editing `docs/` files, writing Rustdoc, or
  auditing documentation completeness.
---

# Documentation Standards

## Standard Format for Files in `docs/`

Every documentation file in `docs/` should follow this structure unless the
file is a pure index table (for example, `docs/README.md`).

1. `# Title`
   - Use a concise title naming the subsystem or topic.
   - Include common aliases in parentheses when needed.
2. `## Scope`
   - State what the file covers and what it does not cover.
   - Link to related docs for adjacent concerns.
3. `## Key Components` (or `## Concepts`)
   - Describe major entities, modules, actors, or flows.
   - Prefer tables for field-level or command-level references.
4. `## Data Flow` or `## Execution Flow`
   - Describe ordered behavior in deterministic steps.
   - Use numbered lists for sequence-sensitive behavior.
5. `## Contracts and Invariants`
   - State assumptions, required ordering, and safety guarantees.
   - Include ownership and synchronization boundaries where relevant.
6. `## Failure Modes and Recovery`
   - Document expected failure cases and fallback behavior.
   - Clarify retry behavior and no-op/idempotent paths.
7. `## Validation`
   - Explain how behavior is validated (tests, smoke checks, runtime signals).
   - Include what must be observed to consider the documented behavior correct.
8. `## References`
   - Link directly to related docs and primary source modules.

## Key Files

- `README.md` - overview and usage notes

## Rustdoc JSON Handling

- When workflows rely on `rustdoc.json`, do not read or parse the file directly
  in the caller.
- Pass the `rustdoc.json` path to the appropriate wrapper tool (for example,
  `0-external-doc-extractor` or `0-external-sig-report` `run.sh`) and let that
  tool consume it.

## File-Level Rules

- Use deterministic heading hierarchy: `#`, `##`, `###` only.
- Keep terminology consistent with code (`ActorHandle`, `CommandResult`,
  `FeedSnapshot`, etc.).
- Prefer concrete names over generic terms like "manager" or "handler" unless
  those names exist in code.
- Use relative links for repo-local references.
- Keep examples minimal and directly tied to real symbols.
- When adding or moving major modules, update `docs/structure.md` and
  `docs/README.md` in the same change.

## Documentation File Naming

Files in `docs/` must use the `.docs.md` suffix
(for example, `actor-lifecycle.docs.md`, `risk-model.docs.md`). Rust source
files use `snake_case` base names; the `.docs.md` double suffix makes the
documentation role explicit alongside that convention.

**Why this matters:** consistent suffixing lets tooling target documentation
files precisely without matching plans, changelogs, or `README` files. It also
makes file purpose obvious in directory listings.

- Do not use a plain `.md` extension for files in `docs/` except for the two
  index files `docs/README.md` and `docs/structure.md`, which keep their
  conventional names.
- When renaming an existing `docs/` file to add the `.docs.md` suffix, update
  all links to that file in the same change.

## Required Outcomes

- Documentation files in `docs/` follow the canonical section format and
  heading hierarchy.
- Markdown and YAML files follow the applicable requirements below when those
  file types are edited.
- `docs/README.md` is updated when adding or removing major docs.
- `docs/structure.md` is updated when module structure changes.
- Inline Rust docs explain usage context, parameter semantics, return
  contracts, invariants, and primary consumers for shared constants.

## Markdown Requirements

- Required elements:
  - title (`# ...`)
  - scope or goal section
  - ordered execution or requirement sections when sequence matters
  - validation or acceptance section
  - risks or notes section for implementation plans
- Formatting requirements:
  - deterministic heading hierarchy (`#`, `##`, `###`)
  - consistent list style and table formatting
  - relative links for repo-local references
- Documentation requirements:
  - use explicit action labels such as `Current`, `New`, `Validation`,
    `Acceptance`, and `Cleanup` for implementation plans
  - avoid ambiguous placeholders like "as needed" unless followed by exact
    resolution rules

## YAML Requirements

- Required elements:
  - top-of-file purpose block
  - `Constant Relationship Map` comment section mapping config keys to Rust
    config or domain fields and their primary consumers
  - grouped key sections with stable ordering
- Documentation requirements:
  - every configurable constant documented inline or in the relationship map
  - documentation identifies semantic meaning, units or range, and primary
    consumer(s)
  - when a YAML key mirrors or feeds a Rust field or constant, name that
    relationship explicitly
- Formatting requirements:
  - keep comments directly above related keys where practical
  - preserve YAML validity and existing schema shape
  - do not change secrets-handling conventions

## Inline Rust Code Documentation Requirements

Inline documentation must explain purpose and usage, not just restate names.

### Required Coverage

All of the following must be documented:

- Every public function.
- Every private function that contains domain logic or non-trivial
  transformations.
- Every shared constant.
- Every public type (`struct`, `enum`, `trait`, `type` alias).
- Every field in public structs where the meaning is not obvious from the field
  name.

If a function, type, or constant is intentionally internal-only and obvious, a
short one-line doc is acceptable; omission is not.

### Function Documentation Standard

- Functions: purpose, call context, parameter meaning/constraints, return
  contract, side effects, and error behavior.

Recommended Rustdoc template:

```rust
/// Computes <result> for <domain purpose>.
///
/// Use this when <call context>.
///
/// Parameters:
/// - `input_a`: <meaning, unit/range, constraints>
/// - `input_b`: <meaning, unit/range, constraints>
///
/// Returns:
/// - `<type>`: <guarantee/invariant on output>
///
/// Side effects:
/// - <state/channel/log behavior>
///
/// Errors:
/// - Returns `<ErrorType>::<Variant>` when <condition>.
fn example(input_a: TypeA, input_b: TypeB) -> Result<TypeOut, ErrorType> { ... }
```

### Constant Documentation Standard

- Constants: semantic meaning, units, rationale, and primary consumers.

Recommended template:

```rust
/// Maximum buffered events before producer backpressure applies.
///
/// Units: count of events.
/// Rationale: prevents unbounded memory growth on bursty feeds while preserving
/// enough headroom for normal peak traffic.
/// Consumers: `wiring`, `actors::event_source`.
pub const CHANNEL_CAPACITY: usize = 65_536;
```

### Type Documentation Standard

- Types: domain role, ownership/lifecycle, invariants, and variant/field
  semantics.

Recommended template:

```rust
/// Snapshot published by the upstream producer actor for downstream consumers.
///
/// Ownership:
/// - Constructed by `ProducerActor`.
/// - Read by `ConsumerActor` on each tick.
///
/// Invariants:
/// - `score` is normalized to [-1.0, 1.0].
/// - `direction` is `None` when a decision has not been reached.
#[derive(Clone, Debug)]
pub struct ActorSnapshot {
    /// Signed score value in [-1.0, 1.0].
    pub score: f64,
    /// Direction derived from score thresholding.
    pub direction: Option<Side>,
}
```

## Documentation Quality Checklist

Use this checklist before accepting documentation or Rust API changes.

- The file has a clear scope and links to adjacent docs.
- Ordered flows are described in deterministic step order.
- Behavior claims match current code and test behavior.
- New or changed APIs include updated inline docs.
- Function docs specify parameter meaning and return guarantees.
- Constant docs include units, rationale, and consumers.
- Type docs define invariants and ownership/lifecycle.
- `docs/README.md` and `docs/structure.md` were updated if navigation or
  structure changed.

## References

- [`.github/local/directories.md`](../../local/directories.md)

## External Tools

- [`0-external-doc-extractor`](../0-external-doc-extractor/SKILL.md) - Extract public items into summary, index, full-doc, or missing-docs tiers from Rust source or rustdoc JSON
