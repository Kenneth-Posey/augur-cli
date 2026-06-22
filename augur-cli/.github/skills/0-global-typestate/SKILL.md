---
name: 0-global-typestate
description: >
  Guidance for encoding state machines and lifecycle phases into the type system
  using the typestate pattern. Use when designing or reviewing types where illegal
  state transitions should be prevented at compile time.
---

# Typestate Pattern

## What Is Typestate

Typestate encodes a value's current phase in its type. Transitions consume one
phase type and produce another. Illegal transitions fail to compile.

## Key Files

- `README.md` - overview and usage notes

## When to Use

- A domain type has distinct lifecycle phases (e.g., `Pending → Active → Closed`).
- Certain operations are only valid in specific phases (e.g., you can only send a
  message to an `Active` session).
- A `bool` field (e.g., `is_initialized`, `is_closed`) guards whether an operation
  is allowed - this is a signal that typestate applies.
- A function panics or returns an error because it was called in the wrong phase -
  typestate should make that call unrepresentable.

## When Not to Use

- The state set is open or determined at runtime from external config.
- States share most fields and behavior - prefer a single struct with an explicit
  state `enum` field combined with decision enums (see Actor Standards).
- The state machine is simple enough that a decision enum is sufficient.

## Pattern

Define each phase as a unit type:

```text
state Pending
state Active
state Closed
```

Parameterize the domain type over the phase:

```text
type Session<State>:
  id: SessionId
  _state: phantom<State>  // carries type information only, no runtime data
```

Implement methods only on the valid phases:

```text
impl Session<Pending>:
  // Creates a new session in the Pending phase.
  fn new(id: SessionId) -> Session<Pending>

  // Activates the session. Consumes Pending, produces Active.
  fn activate(self: Session<Pending>) -> Session<Active>

impl Session<Active>:
  // Sends a message. Only callable while the session is Active.
  fn send(self, msg: Message)

  // Closes the session. Consumes Active, produces Closed.
  fn close(self: Session<Active>) -> Session<Closed>
```

`Session<Pending>.send(...)` does not exist. Calling it is a compile error.

## Carrying State Fields Across Transitions

When fields vary by phase, use a wrapper that carries them:

```text
type Session<State>:
  id: SessionId
  phase: State

type Active:
  started_at: TimestampMs

impl Session<Active>:
  fn started_at(self) -> TimestampMs:
    return self.phase.started_at
```

This keeps phase-specific data co-located with the phase type.

## Integration With the Actor Pattern

- The **actor's internal state** uses typestate for owned domain values whose
  lifecycle must be enforced.
- The **actor's public handle** expresses state changes through command variants,
  not typestate - handles are shared across threads and cannot own exclusive state.
- The **ops module** implements typestate transitions as pure functions. The actor
  calls the ops function and replaces its owned value with the returned type.

## Relation to Flow Constraints

Typestate enforces one-way progression at the type level:
`Session<Closed>` cannot become `Session<Active>` because no such function
exists. The Pipeline Constraint and Ports and Adapters rules express the same
idea at the data-flow and dependency levels.

## Review Heuristics

- If a struct has a `bool is_initialized` or `bool is_closed` field, replace with typestate.
- If a function panics because it was called in the wrong phase, typestate should
  prevent that call from compiling.
- If a `match` on a state enum produces the same output type for all arms but
  different capabilities, the arms likely represent different typestates.
- If code must check a condition before performing an operation, consider whether
  the type system can make the check unnecessary.

---

For language-specific implementation patterns, see the companion documentation
for your language (for example, `rust-*`).
