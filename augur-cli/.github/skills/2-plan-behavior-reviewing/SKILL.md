---
name: 2-plan-behavior-reviewing
description: "Validates a behavior plan against Given/When/Then specifications. Checks state/transition coverage, guard completeness, reachability, contract testability, and conflict-free guards. Emits pass or fail with structured diagnostics."
---

# Skill: 2-plan-behavior-reviewing

## Tracing GWT Scenarios to States and Transitions

Map each GWT scenario to the behavior plan:

1. For each scenario, identify the (current state, event) pair from the Given/When clauses.
2. Locate the corresponding row in the state machine table.
3. Verify the guard condition in that row is compatible with the Given-clause predicates.
4. Verify the next state and effects in that row satisfy the Then assertions.

**Fail condition:** If a scenario cannot be mapped to a specific (current state, event, guard) row, emit a `fail` diagnostic that identifies the scenario and the missing row.

## Key Files

- `README.md` - overview and usage notes

## Checking for Missing and Unreachable States

After tracing all scenarios:

- **Missing transitions:** For each state, verify that every event type used in the scenarios has a row in that state's transition table. Flag (state, event) pairs with no row.
- **Unreachable states:** Walk the transition table from the initial state. Any state not visited is unreachable. Flag it as dead code unless it is explicitly documented as reserved or future state.
- **Missing terminal states:** Verify that every terminal path in the GWT scenarios leads to a documented terminal state. Flag flows that end in non-terminal states.

## Validating Guard Conditions

For each (state, event) row that has multiple guarded branches:

1. **Exhaustiveness:** The disjunction of all guards in the branch set must cover all possible inputs. Flag branches that leave an uncovered case.
2. **Mutual exclusivity:** No two guards in the same branch set may be simultaneously true. Flag overlapping guards.
3. **Named guard consistency:** If the same named guard (e.g., `timeout_elapsed`) appears in multiple rows, verify it references the same predicate definition everywhere. Flag inconsistent reuse.

## Validating Behavior Contracts

For each contract (precondition, postcondition, invariant) in the plan:

- **Completeness:** Every state machine and decision tree node must have at least one documented precondition and one postcondition. Flag missing contracts.
- **Testability:** Each precondition and postcondition must be a verifiable predicate, not free-form prose. Flag untestable contracts.
- **Invariant preservation:** For every transition, verify that no effect in the Then clause contradicts a documented invariant. Flag invariant violations.

## Emitting Pass or Fail

Aggregate all findings and emit one of:

- **`pass`** - no untraced scenarios, no missing transitions, no unreachable states, no guard gaps, no untestable contracts, no invariant violations.
- **`fail`** - one or more findings from the checks above. Include:
  - Finding type (untraced scenario / missing transition / unreachable state / guard gap / untestable contract / invariant violation)
  - Specific scenario ID or state/event pair affected
  - Remediation guidance (what the planner must add or change)

Do not emit `pass` if any finding is unresolved. Do not emit `fail` without actionable diagnostics.
