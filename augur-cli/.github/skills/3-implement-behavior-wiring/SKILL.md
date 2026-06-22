---
name: 3-implement-behavior-wiring
description: "Use at Stage 3 to turn planned behavior into executable flow while keeping orchestration thin, dependencies one-way, and state ownership in domain or infrastructure layers."
---

# Skill: 3-implement-behavior-wiring

## When to Use

Use this skill when Stage 2 behavior plans define triggers, sequencing, guards, collaborators, and observable outcomes, and Stage 3 must turn them into executable flow.

Before choosing syntax or framework patterns, consult [`.github/local/language-companions.md`](../../local/language-companions.md) for the language-specific version of this guidance.

## Key Files

- `README.md` - overview and usage notes

## Inputs and Dependencies

- Behavior plans from `2-plan-behavior-planning`
- Domain implementation guidance from `3-implement-domain-implementation`
- Signature contracts from `2-plan-function-sig-planning`
- Test expectations from `2-plan-test-planning`
- TDD discipline from `0-global-tdd-workflow`

## Stage 3 Guardrails

1. **Dependency flow stays one-way:** orchestration -> domain -> persistence/integration. Wiring may call downward; lower layers must not depend on orchestration.
2. **Orchestration is not the business-logic dump.** It sequences work, applies flow control, and delegates rules to the domain.
3. **Complexity must stay bounded.** Large handlers, oversized branching trees, and mixed responsibilities are refactoring triggers.
4. **Temporary compile stubs are allowed only before Red.** If a missing dependency blocks the first failing test, add the thinnest stub needed to compile, then replace it immediately.
5. **Green requires real behavior.** All planned behavior tests must pass, and no production stub or fake-success branch may remain.

## Core Pattern

Behavior wiring should answer four questions:

1. **What triggers the flow?**
2. **Which domain operation owns each decision or mutation?**
3. **Which infrastructure dependency is invoked, and at what boundary?**
4. **What observable outcome proves the flow succeeded or failed correctly?**

If any step cannot be assigned clearly, the implementation is mixing responsibilities.

## Workflow

### 1. Start from planned behavior, not framework mechanics

Map each Given/When/Then scenario to:

- setup or precondition checks
- one triggering action
- delegated domain calls
- boundary calls to persistence or integrations
- observable outputs

Do not invent extra branches, retries, or side effects unless the plan requires them.

### 2. Keep orchestration thin

Wiring code may:

- validate request shape or trigger preconditions
- select the next domain operation
- choose between planned branches
- translate dependency failures into the planned outward contract

Wiring code should not:

- own durable state
- re-implement domain invariants
- hide missing dependencies behind implicit globals
- mix unrelated flows in one handler

### 3. Break long flows into named subflows

Extract helpers when a flow has:

- multiple independent branches
- repeated guard logic
- repeated boundary conversion
- reusable sub-sequences that appear in more than one behavior

Each helper should represent one meaningful step or one reusable branch, not an arbitrary slice of lines.

### 4. Keep state ownership explicit

State mutations belong to the layer that owns that state:

- domain mutations in domain operations
- persistence writes in persistence adapters or repositories
- external side effects in integration adapters

The wiring layer coordinates those calls but does not become the state owner.

### 5. Make failure routing observable

Every planned failure branch should correspond to:

- a distinct delegated failure from a lower layer, or
- an explicit guard failure at the wiring boundary

Avoid catch-all behavior that erases the difference between validation, domain, and infrastructure failures.

## Complexity Control Heuristics

Refactor the wiring when:

- one handler coordinates too many collaborators
- one function contains multiple unrelated branches
- request parsing, business rules, persistence, and response formatting all live together
- a caller must know internal sequencing details to use the public entrypoint correctly

Preferred responses:

- split boundary translation from orchestration
- move business rules into domain operations
- extract named branch handlers or subflows
- introduce a small coordinator object only when it reduces, not increases, coupling

## Validation Checklist

- [ ] Each behavior path maps back to a planned scenario or explicit plan-approved branch
- [ ] Wiring code preserves orchestration -> domain -> persistence/integration direction
- [ ] Domain rules remain in the domain layer instead of being re-implemented in wiring
- [ ] Stateful concerns are owned by the layer that persists or governs them
- [ ] Branching and helper extraction keep each wiring unit focused and understandable
- [ ] Any temporary compile stub used before Red has been removed or replaced
- [ ] All planned behavior tests pass through real wiring paths with no production placeholders

## Relationship to Other Stage 3 Skills

- `3-implement-domain-implementation` defines the domain operations and invariants that wiring composes
- `3-implement-function-sig-implementation` supplies the executable contract surfaces that wiring calls
- `3-implement-test-suite-completion` verifies the wired system through the planned scenarios
