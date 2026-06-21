---
name: 3-implement-domain-implementation
description: "Implements planned domain models in language-neutral terms by enforcing invariants, introducing semantic types, bounding complexity, and keeping domain code independent of orchestration and infrastructure concerns. Use at Stage 3 when turning the domain plan into executable domain types and operations."
---

# Skill: 3-implement-domain-implementation

## When to Use

Use this skill when Stage 2 domain planning has identified entities, value objects, aggregates, lifecycles, and invariants, and Stage 3 must turn that plan into concrete domain code.

Before writing code, consult [`.github/local/language-companions.md`](../../local/language-companions.md) for the language-specific companion for the current stack.

## Key Files

- `README.md` - overview and usage notes

## Inputs and Dependencies

- Domain model from `2-plan-domain-planning`
- Signature contracts from `2-plan-function-sig-planning`
- Behavior expectations from `2-plan-behavior-planning`
- Test expectations from `2-plan-test-planning`
- TDD discipline from `0-global-tdd-workflow`

## Stage 3 Guardrails

1. **Use semantic types at domain boundaries.** Replace bare identifiers, raw strings, free-form numbers, and loosely typed maps with named domain types or wrapper types whenever the value has distinct business meaning.
2. **Keep complexity bounded.** Split oversized entities, long constructors, and large transition methods into smaller domain concepts or focused helpers. Avoid implementations that require callers to reason about too many fields or parameters at once.
3. **Preserve one-way dependency flow.** Domain code may depend on domain-local helpers, but it must not depend on orchestration details, transport formats, UI concerns, or persistence-specific representations.
4. **Use temporary compile stubs only to reach Red.** If a missing symbol blocks the first failing tests from compiling, add the thinnest stub that lets tests compile, then replace it immediately. A stub is never Green.
5. **Green means complete.** All planned tests pass, and no production stub, placeholder branch, or fake-success path remains.

## Workflow

### 1. Translate the plan into domain types

For each planned concept, decide whether it is:

- an entity with identity and lifecycle
- a value object defined by validated data
- an aggregate root that enforces consistency for a cluster
- a domain service or helper that holds pure domain logic but no cross-layer concerns

Do not let storage shape, API payload shape, or transport naming decide the domain model.

### 2. Introduce semantic types before writing behavior

Wrap domain-significant primitives early:

- identity values
- constrained text values
- measured quantities
- bounded numeric values
- state or status concepts

If two inputs would both be represented by the same primitive but mean different things, they should not share the same type at the domain boundary.

### 3. Enforce invariants at creation and transition boundaries

Every constructor, factory, or state-transition operation must either:

- produce a valid domain object, or
- fail in a typed, inspectable way

Do not create invalid objects first and “fix them later.” Invalid state should be rejected at the boundary where it is introduced.

### 4. Decompose complex domain logic

Use focused helpers when:

- a constructor validates many independent rules
- an operation mixes calculation, transition checks, and formatting
- a type carries too many unrelated fields
- the same rule appears in multiple places

Keep the public domain surface small by moving repeated or dense logic into named domain-local helpers.

### 5. Keep the domain layer pure in direction and responsibility

The domain layer owns:

- invariants
- lifecycle transitions
- calculations
- domain-level validation

The domain layer does **not** own:

- request routing
- transport parsing/serialization
- direct persistence orchestration
- infrastructure retry or delivery policy

If a rule depends on infrastructure, model that dependency as an input contract. Keep the decision in the domain only when it is truly business logic.

## Complexity Control Heuristics

Treat these as refactoring triggers:

- a public operation needs a long list of unrelated inputs
- a domain type collects many fields from unrelated responsibilities
- a method contains multiple branches for unrelated business rules
- callers must remember positional primitive arguments to use the API correctly

Preferred responses:

- bundle related inputs into a named request/value type
- split a large aggregate into a root plus contained concepts
- extract a helper that owns one rule or calculation
- move cross-cutting coordination out to behavior wiring

## Validation Checklist

- [ ] Each domain-significant primitive is represented by a semantic type or documented exception
- [ ] Entities, value objects, and aggregates match the Stage 2 domain plan
- [ ] Constructors and transitions enforce invariants at the boundary
- [ ] Public operations stay within bounded complexity and use named inputs when needed
- [ ] No domain code depends directly on orchestration, transport, or persistence details
- [ ] Any temporary compile-target stub used before Red has been removed or replaced
- [ ] All planned domain tests pass with no production placeholders remaining

## Relationship to Other Stage 3 Skills

- `3-implement-function-sig-implementation` realizes the planned public contracts around this domain model
- `3-implement-behavior-wiring` composes domain operations into end-to-end flows without reversing dependency direction
- `3-implement-test-suite-completion` proves the domain implementation is Green
