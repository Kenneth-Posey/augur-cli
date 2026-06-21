---
name: 2-plan-function-sig-planning
description: "Designs function signatures, parameter types, return types, error types, and interface contracts from domain operations and behavioral specifications. Use at the Plan stage when translating domain entities and Given/When/Then behaviors into typed operation signatures."
---

# Skill: 2-plan-function-sig-planning

## Extracting Operations from Domain and Behavior Specs

Domain entities and behavioral specifications jointly determine the set of operations a system must expose. Extract operations by reading both sources:

**From domain entities:**
- For each entity in the domain spec, identify what a caller can do to it: create it, read it, mutate it, validate it, delete it, or query across a collection of it.
- Each distinct action on an entity is a candidate function.
- Aggregate roots expose operations; internal entities and value objects typically do not - their mutations happen through the aggregate root.

**From Given/When/Then scenarios:**
- **Given** → precondition context: these predicates become input constraints and preconditions on parameters.
- **When** → triggering command or query: the verb and subject of the When clause map directly to the function name and the type that holds it.
- **Then** → expected outcome: the success branch maps to the return type; each named failure mode maps to an error variant.

**Deriving function names from When clauses:**

Apply consistent verb prefixes so that names are predictable across the operation set:

| Intent | Verb prefix | Example |
|---|---|---|
| Construct a new entity | `create_` | `create_order` |
| Partially update state | `update_` | `update_shipping_address` |
| Remove or archive | `delete_` / `archive_` | `delete_account` |
| Read without side effects | `get_` / `find_` / `list_` | `get_user_by_id` |
| Check validity | `validate_` / `check_` | `validate_payment_method` |
| Trigger a domain event | use the event verb directly | `submit_order`, `approve_request` |

When the same verb prefix appears on multiple functions for the same entity, distinguish them by the distinguishing noun (e.g., `get_order_by_id` vs. `get_orders_for_customer`).

## Key Files

- `README.md` - overview and usage notes

## Designing Parameter Types

**Core principle:** each parameter should carry the minimum information needed to perform the operation and no more. Parameters must not leak internal representation details.

**Required vs. optional parameters:**
- Required parameters appear as positional typed arguments.
- Optional parameters must be typed explicitly as optional (a container type that signals absence, such as `Option<T>`, a nullable type, or a dedicated `Maybe` wrapper - not a default value hidden inside the function body).
- Do not use boolean flag parameters to toggle fundamentally different behaviors; split into two functions instead.

**Parameter bundling rule:**
When three or more parameters share a logical context (they always change together, describe the same concept, or form a natural domain grouping), define a named input type and replace the individual parameters with it. Named input types are easier to extend and easier to reference in errors and documentation.

**Avoiding representation leakage:**
- Parameters must express what the caller knows, not how the system stores it.
- Do not expose storage IDs, internal sequence numbers, or persistence-layer keys as raw primitive types; wrap them in identity types (see "Type Consistency Rules" below).
- If a parameter requires the caller to understand internal state layout, that is a design smell - redesign the parameter to accept a domain concept instead.

## Designing Return Types

Every function has exactly one of the following return categories:

| Category | When to use | Type shape |
|---|---|---|
| Value (pure query) | Function reads and returns a domain value; no failure modes exist | The value type directly |
| Unit/void (command) | Function mutates state; the only outcome is success or failure | Unit type, or nothing |
| Discriminated success/failure (fallible) | Function may fail for domain or infrastructure reasons | Typed result wrapping both the success value and the error type |

**Rules:**
- Fallible operations must use a typed error, not an untyped or stringly-typed error channel. Every caller must be able to pattern-match on the failure variant.
- Never return a raw null or sentinel value (e.g., `-1`, `""`, `null`) to indicate failure - use the discriminated type.
- If a function returns a collection that may be empty (but "no results" is not an error), return the empty collection, not an error variant.

**Async/deferred return conventions:**
- When a function performs I/O or depends on an external resource, wrap the return type in the platform-appropriate future or promise type.
- The deferred return type wraps the same success/failure discriminated type - it does not flatten it.
- Functions that stream results return an asynchronous sequence or channel type rather than a single future.
- Document whether the caller must await, poll, or subscribe to observe the result.

## Designing Error Types

Before finalizing a function signature, enumerate every failure mode for that function. Failure modes come from three sources:

1. **Precondition violations** - the caller passed an input that violates a stated precondition (wrong format, out-of-range value, null where a value is required).
2. **Invalid state transitions** - the entity's current state does not allow the requested operation (e.g., attempting to ship an order that has not been confirmed).
3. **Resource and infrastructure failures** - external dependencies (databases, network, clocks, queues) returned an error or timed out.

**Organizing error variants hierarchically:**

Group error variants into two top-level categories:

- **Domain errors** - failures that a domain-aware caller can handle and recover from (invalid state transition, constraint violation, not-found). These must be modeled explicitly; callers must be able to match on them.
- **Infrastructure errors** - failures from external systems or platform layers that callers typically log and propagate rather than recover from inline.

Where multiple functions share the same domain error types, define an error type at the module or aggregate level rather than per-function. Do not define a new error type for each function unless the failure vocabulary is truly distinct.

**Error variant context rules:**
- Each error variant must carry enough context to diagnose the failure without inspecting caller state.
- Include: the entity ID or key involved, the operation attempted, and the constraint that was violated.
- Do not embed stack traces or log messages in error variants - those belong in the infrastructure layer.

## Defining Interface Contracts

For every function in the signature set, document three contract elements:

**Preconditions** - what must be true about inputs and system state before the function is called:
- State the constraint in terms of the parameter types and their domain meaning.
- Distinguish between validated preconditions (the function checks and returns an error) and assumed preconditions (the function panics or is undefined if violated - document which, and why).

**Postconditions** - what is guaranteed about the return value and system state after a successful call:
- State the guarantee in terms of the return type and any relevant system state.
- Each Then-clause assertion from a GWT scenario becomes a postcondition.

**Invariants** - facts that must hold before and after every call on an entity:
- Invariants are drawn from the domain spec's entity definitions, not from individual scenarios.
- If calling a function would violate an invariant, the function must return an error (not silently allow the invariant to break).

**Expressibility rule:** contracts must be expressed as verifiable predicates, not prose descriptions. "The returned order has a non-empty ID" is verifiable. "The order is created correctly" is not.

## Type Consistency Rules

Inconsistent types across a signature set produce bugs that are invisible to callers until runtime. Apply these rules:

**Same-concept rule:** the same domain concept must map to the same type everywhere it appears. If `CustomerId` is a wrapped identifier in `create_order`, it must be the same `CustomerId` type in `get_orders_for_customer` - not a raw integer in one place and a string in another.

**Structural compatibility rule (type drift):** if two functions operate on the same entity, their parameter and return types must be structurally compatible. Example: if `create_order` returns an `Order` and `update_shipping_address` takes an `OrderId`, the `Order` returned by `create_order` must expose an `id` field of type `OrderId`. Mismatched types between producer and consumer functions are a design error, not an implementation concern.

**Identity type wrapping rule:** domain entity identifiers must be wrapped in a newtype rather than exposed as bare primitives. This prevents callers from passing a `CustomerId` where an `OrderId` is expected.

**Naming consistency:** if the same parameter concept appears across multiple functions, use the same parameter name. Inconsistent naming (`user_id`, `userId`, `id`, `uid`) for the same concept is a maintainability defect.

## Behavior-to-Signature Traceability

A complete function signature set must cover every Given/When/Then scenario in the behavior spec. Verify coverage explicitly:

**Traceability matrix construction:**
For each scenario, record:

| Scenario ID | Function name | Parameters covering the When inputs | Return type covering the Then outcome | Gap? |
|---|---|---|---|---|

A scenario is covered when:
- The When clause's inputs are fully typed by the function's parameter list.
- The Then clause's success outcome is expressed in the return type's success branch.
- Each Then clause's named failure is expressed as a distinct error variant in the return type's error branch.

**Identifying gaps:**
- Any scenario row with an empty "Function name" cell is a missing operation - add the function.
- Any scenario row where the Then outcome is not covered by any return type branch is an incomplete return type - extend the discriminated type.
- Any scenario row where a failure mode appears in the Then clause but has no corresponding error variant is an incomplete error type - add the variant.

Include the completed traceability matrix in the plan. The plan is incomplete until every row is filled and every gap is resolved.

## Grouping Same-Pattern Methods

When multiple methods share an identical structural pattern - same ownership
model, same error type, and same parameter shape - consolidate them into a
shared table with one pseudocode example rather than giving each a full
subsection. This avoids repeating boilerplate that adds no information.

**Format for grouped methods:**

```
### <Group Name> (<pattern description>)

Pattern: `fn method_name(&self, ...) → <return type>`, <additional pattern notes>.

| Method | Parameters | Notes |
|--------|-----------|-------|
| method_one | param_a: TypeA, param_b: TypeB | - |
| method_two | param_a: TypeA, param_c: TypeC | triggers side effect X |
| method_three | - | flushes all pending |
```

**Pseudocode example (one per group):**

```
fn method_one(&self, param_a: TypeA, param_b: TypeB) → () {
    // fire-and-forget: spawn internally, no return value
}
```

**Grouping rule:** Apply this format when ≥2 methods satisfy ALL of:
- Same ownership/borrowing model (e.g., all `&self`, all `&mut self`)
- Same top-level return type (e.g., all `()`, all `Result<T, SameError>`)
- Same parameter shape (e.g., all take one domain ID + one value type)

Reserve full per-method subsections for complex or unique functions that do
not share a common structural pattern.

## Language-Specific Companion

This skill defines language-agnostic operation names, parameter concepts, return categories, error hierarchies, and contracts. To translate them into language-idiomatic type annotations, ownership or borrowing semantics, trait bounds, error wrapping patterns, and compiler-enforced constraints, look up the `2-plan-function-sig-planning` capability key in [`.github/local/language-companions.md`](../../local/language-companions.md) and invoke the listed companion skill.
