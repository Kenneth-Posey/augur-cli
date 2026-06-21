---
name: rust-2-plan-behavior-reviewing
description: "Rust-specific additions for behavior plan review. Validates that planned state machines, decision trees, and actor patterns are expressible as sound, idiomatic Rust using compiler-enforceable criteria."
---

# Skill: rust-2-plan-behavior-reviewing

## Handoff Inputs

Review repository handoff artifacts, not other skills. Prefer:

- `plans/<feature-slug>/design/behaviors.md` for scenario traceability, states,
  transitions, and guards.
- `plans/<feature-slug>/plan/domain-spec.md` for invariants and error taxonomy.
- `plans/<feature-slug>/plan/dependency-graph.md` for actor/message boundaries
  and dependency direction.

This skill adds Rust-specific correctness checks that the handoff files do not
cover.

## Key Files

- `README.md` - overview and usage notes

## Checking State Enum Exhaustiveness

For each state machine in the plan, verify that its states map cleanly to a
Rust enum:

- Every state variant must appear in at least one `match` arm in every
  transition function for that state machine.
- Flag any state that would force a `_` or `..` catch-all arm with "do
  nothing" behavior. That silently swallows transitions and must be explicit.
- Verify that each state's carried data is documented. Missing field
  documentation makes exhaustive handling unclear at implementation time.

**Fail condition:** Any state variant lacks documented fields or requires a
wildcard fallback.

## Verifying Result and Option Usage for Decision Points

For each decision tree branch and each error path in the plan:

- Branches representing "success or recoverable failure" must be planned as `Result<T, E>` with a documented error variant for the failure leaf.
- Branches representing "present or absent" must be planned as `Option<T>`.
- Flag any decision branch that produces an error outcome with no
  corresponding error variant in the plan's error catalog.
- Flag any decision branch that returns a sentinel value (for example `-1`,
  empty string, or `0`) instead of a typed `Result` or `Option`.

## Checking Actor Trait Pattern Alignment

For each actor identified in the plan:

- Verify the actor has a planned trait covering its message-handling
  interface. Without it, the actor cannot be tested or swapped in isolation.
- Verify each mailbox message has a planned concrete type (struct or enum
  variant). Flag untyped messages such as `Box<dyn Any>`.
- For async actors, verify the planned trait is `async`-compatible (either
  `async fn` or a returned `Future`) and note whether `Send` bounds are
  required.

## Checking Typestate Pattern Usage

For each invariant in the behavior plan that requires a state transition to be unreachable after a specific point:

- Determine whether the invariant can be enforced at compile time using a typestate pattern (phantom type parameter advancing through states as a type-level witness).
- Flag invariants documented as "must never happen" when the plan enforces
  them only with runtime checks. These are candidates for typestate
  promotion.
- For each typestate candidate, verify the plan documents the phantom type
  names and the transition functions that advance the type parameter.

Do not flag invariants where runtime enforcement is the right choice, such as
conditions that depend on runtime data.
