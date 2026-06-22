---
name: 3-implement-function-sig-implementation
description: "Implements planned public contracts in language-neutral terms by keeping interfaces minimal, using semantic input and output types, containing signature complexity, and preserving boundary direction. Use at Stage 3 when turning the function signature plan into executable interfaces and adapters."
---

# Skill: 3-implement-function-sig-implementation

## When to Use

Use this skill when Stage 2 has already defined function signatures, failure modes, and boundary contracts, and Stage 3 must implement those contracts without weakening type clarity or dependency boundaries.

Before choosing interface syntax, helpers, packaging, or visibility, consult [`.github/local/language-companions.md`](../../local/language-companions.md) for the language-specific companion for the current stack.

## Key Files

- `README.md` - overview and usage notes

## Inputs and Dependencies

- Signature plans from `2-plan-function-sig-planning`
- Domain plans from `2-plan-domain-planning`
- Behavior plans from `2-plan-behavior-planning`
- TDD discipline from `0-global-tdd-workflow`

## Stage 3 Guardrails

1. **Honor the planned contract exactly.** Do not silently widen inputs, collapse error cases, or add speculative outputs.
2. **Prefer semantic types over bare primitives.** Inputs and outputs should communicate domain meaning directly.
3. **Bound signature complexity.** When a call needs too many related inputs, bundle them into a named request or command type instead of extending a long positional list.
4. **Keep dependency direction clean.** Public interfaces may depend on domain types and approved boundary models; they must not force domain code to depend on transport or storage details.
5. **Use compile-target stubs only before Red, and remove them by Green.** A temporary placeholder that exists only so tests can compile does not satisfy the contract.

## Core Pattern

Function-signature implementation produces three concrete pieces:

1. **Executable contract surface** - the callable operation and its documented failure vocabulary
2. **Boundary models** - named input/output types that carry domain meaning
3. **Adapters or translators** - narrow conversions between external representations and internal semantic types

Each piece should stay small and purpose-specific.

## Workflow

### 1. Start from the planned contract

For each planned operation, preserve:

- operation intent
- input meaning
- success output
- typed failure cases
- preconditions and postconditions

Refine internals if needed, but do not casually rewrite the contract.

### 2. Replace ambiguous primitives with named types

Introduce named types when a parameter or return value represents:

- an identity
- a validated user input
- a bounded quantity
- a state transition request
- a domain-specific error

If callers could accidentally swap two same-shaped values, the signature is under-typed.

### 3. Keep interfaces minimal and focused

Refactor when a signature:

- takes many unrelated arguments
- uses boolean switches to choose fundamentally different behavior
- exposes storage or transport details that callers should not know
- returns loosely typed structures that make callers rediscover meaning

Preferred responses:

- introduce a named request type
- split a multi-purpose operation into separate operations
- return a named result model
- move representation translation to an adapter at the boundary

### 4. Isolate boundary translation

External representations and internal domain types often differ. Keep that conversion:

- explicit
- narrow
- validated
- local to the boundary

Do not leak raw external payload shapes into the domain for convenience at the boundary.

### 5. Remove placeholders before Green

If a contract surface was temporarily stubbed so Red tests could compile, replace it before declaring Green. The final implementation must exercise real validation, branching, and delegation.

## Validation Checklist

- [ ] Each implemented operation matches the planned name, intent, and failure vocabulary
- [ ] Domain-significant inputs and outputs use semantic or wrapper types where appropriate
- [ ] Long or mixed-purpose signatures have been decomposed into named request/result types
- [ ] Boundary adapters isolate external representation details from the domain
- [ ] Dependency direction remains from interface/orchestration toward domain, not the reverse
- [ ] Any temporary compile-target stub used before Red has been removed or replaced
- [ ] All planned contract tests pass with no production placeholders remaining

## Relationship to Other Stage 3 Skills

- `3-implement-domain-implementation` supplies the domain types and invariants that the contract surface should expose safely
- `3-implement-behavior-wiring` composes these contracts into runtime flows
- `3-implement-test-suite-completion` validates that the implemented contract behaves exactly as planned
