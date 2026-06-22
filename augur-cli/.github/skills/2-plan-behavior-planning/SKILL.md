---
name: 2-plan-behavior-planning
description: "Translates Given/When/Then behavioral specifications into a concrete behavior plan: state machines, decision trees, actor/message-passing patterns, and behavior contracts. Use at the Plan stage when a feature introduces stateful behavior, conditional flows, or actor interactions."
---

# Skill: 2-plan-behavior-planning

## Reading Given/When/Then Specifications

Each GWT scenario describes one observable behavior:

- **Given** - preconditions (initial state, existing entities, environment)
- **When** - the triggering event or action
- **Then** - the expected outcome (state change, emitted event, returned value, error)

Read it as follows:

1. Extract nouns from Given/When/Then clauses - these are state holders or actors.
2. Extract verbs from the When clause - these are transitions or commands.
3. Extract assertions from the Then clause - these become guards (conditions that gate the outcome) or effects (post-state or emitted values).
4. Group scenarios by shared subject noun to identify the state machine owner.

## Key Files

- `README.md` - overview and usage notes

## Mapping to State Machines

For each state-bearing entity:

- **States** - distinct named conditions of the entity. A state is required for each unique set of valid next transitions.
- **Transitions** - labeled edges between states, named after the When-clause verb.
- **Guards** - Boolean conditions derived from Given-clause context that must hold for a transition to fire.
- **Effects** - observable changes captured in the Then clause: updated field values, emitted events, or return values.

State machine construction rules:

1. Every state must be reachable from the initial state via at least one transition chain.
2. Every GWT scenario must map to exactly one transition (or a guard-separated branch on one transition).
3. Dead-end states (no outgoing transitions) must be explicitly named terminal states.
4. Cyclic transitions are permitted only when the cycle is explicitly justified by a scenario.

Record each state machine as a table: `(current state, event) → (guard, next state, effects)`.

## Decision Trees and Guard Conditions

When one When-clause can lead to multiple Then outcomes based on context, model it as a decision tree:

- Each decision node is a Boolean guard derived from Given-clause predicates.
- Leaf nodes are state transitions or direct effects.
- All branches must be mutually exclusive and exhaustive - no uncovered input combination.

Consolidate shared guard predicates into reusable named conditions (for example, `timeout_elapsed`, `resource_available`).

## Actor and Message-Passing Patterns

When GWT scenarios involve multiple subjects communicating asynchronously:

- Identify each **actor**: an independent lifecycle owner that sends and receives messages.
- Identify each **message**: a named, typed command or event crossing actor boundaries.
- Map each When-clause action to either an internal state change (same actor) or a message send (crossing actor boundary).
- Document the **mailbox protocol**: which messages each actor accepts, which it rejects, and in which states.

Use an actor model when you see:

- Multiple Given-clause subjects each with their own state
- Then-clause assertions on a different subject than the When-clause subject
- Time-delayed effects or retry-on-failure patterns

## Behavior Contracts

For each state machine or decision tree node, document:

- **Preconditions** - facts that must hold before the transition fires (derived from Given clauses)
- **Postconditions** - facts guaranteed to hold after the transition completes (derived from Then clauses)
- **Invariants** - facts that must hold in all states for the entity (never violated by any transition)

Express contracts as verifiable predicates, not prose.

## Resolving Conflicts and Ambiguities

Common conflict patterns and resolution:

| Conflict | Resolution |
|---|---|
| Two scenarios share the same (state, event) but produce different outcomes | Introduce a guard to split the transition into two branches |
| A scenario references a state not present in any other scenario | Determine if it is a new state or a misnamed variant of an existing state |
| A Then-clause asserts on an entity that has no identified state machine | Decide whether the entity is a state holder (add it as an actor) or a value object (no machine needed) |
| Cyclic transitions without a termination scenario | Flag and request a clarifying scenario that exits the cycle |

Record all ambiguities and chosen resolutions in the behavior plan.
