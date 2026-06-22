---
name: rust-1-design-requirements-engineering
description: >
  Rust-specific guidance for design-stage requirements. Use when deriving
  Rust implementation requirements from plan and design artifacts. Checks
  implementability, testability, error handling, concurrency safety, and
  memory safety.
---

# Rust 1 Design Requirements Engineering

## Handoff Inputs

Use this skill after feature intent is captured in handoff artifacts. Start
with:

- `plans/<feature-slug>/plan/domain-spec.md` for purpose, scope, actors, data
  flows, invariants, and success criteria.
- `plans/<feature-slug>/design/behaviors.md` for state transitions, boundary
  conditions, and failure scenarios.
- `plans/<feature-slug>/plan/test-strategy-plan.md` for planned coverage and
  error-path validation expectations.
- `plans/<feature-slug>/plan/dependency-graph.md` for concurrency edges, shared
  resources, and integration boundaries.

Add these Rust-specific checks:

- **Rust-implementability gates**: can this requirement be implemented safely in Rust?
- **Error-path explicitness**: all error cases and recovery patterns.
- **Concurrency constraints**: async boundaries, shared-state safety, actor model
  alignment.
- **Memory safety rules**: ownership boundaries, lifetime constraints, heap
  allocation patterns.
- **Testability mappings**: which errors are testable, which require integration
  testing, which need property-based testing or fuzzing.
- **Type-system leverage**: how Rust's type system can enforce the requirement
  and prevent implementation errors.

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### 1. Requirement Implementability Gate

A Rust requirement is implementable when:

- **Type Safety**: It can be expressed using Rust's type system or `Result` /
  `Option` patterns without `unsafe` blocks or `unwrap()` calls in the happy
  path.
- **Ownership Clarity**: Ownership boundaries and borrowing rules can be
  expressed without lifetime complexity that obscures intent.
- **Memory Safety**: No manual memory management, dangling pointers, or
  data-race risks.
- **Compiler Verifiability**: The compiler can prove the requirement holds
  without runtime checks (or runtime checks are minimal and documented).

If a requirement cannot meet these gates, it must be refined, decomposed, or
rejected. Document the gate decision and reasoning.

### 2. Error Case Taxonomy

For each requirement, enumerate all error cases:

- **Logical errors** - invariant violations, precondition failures (e.g., empty
  input when non-empty is required).
- **Resource errors** - allocation failures, I/O errors, timeout errors.
- **Concurrency errors** - actor shutdown, channel closure, deadlock recovery.
- **Domain errors** - business rule violations (e.g., invalid state transitions).

For each error:

- **Testability**: Is it unit-testable, integration-testable, or both?
- **Recovery**: What is the recovery path? Retry? Propagate? Halt?
- **Result Type**: Does it map to `Result<T, E>`, `Option<T>`, or a custom enum?

### 3. Async and Concurrency Boundaries

Concurrency decisions affect type signatures, testability, and performance.
For each requirement, state:

- **Async/Sync boundary**: Where do async calls begin? Where do they end?
- **Actor alignment**: Does this requirement fit the project's actor model? If
  not, explain why.
- **Shared state**: What state, if any, is shared? How is it synchronized
  (`Arc<Mutex<T>>`, `Arc<RwLock<T>>`, channels)?
- **Cancellation**: Can this requirement be cancelled mid-execution? How?

### 4. Memory Safety and Ownership Constraints

Document ownership decisions:

- **Owned vs. borrowed**: When is data owned by the caller? When is it borrowed?
- **Lifetimes**: If lifetimes are non-trivial, name them and explain why they
  cannot be elided.
- **Heap allocation**: Does this requirement require heap allocation? If so, when
  and why? (Avoid "defer to implementation"; decide at requirements time.)
- **Static data**: Are any data structures static or global? Justify.

### 5. Error Handling Strategy

Choose and document the error-handling strategy:

1. **Result-based** - recoverable errors are `Err(E)`. Use when the caller can
   reasonably recover or retry.
2. **Panic-based** - unrecoverable errors cause a panic. Use only for true
   programming errors (violated invariants, contract violations).
3. **Option-based** - absence of a value is modeled as `None`. Use when
   "no value" is a valid, expected outcome (not an error).
4. **Custom enum** - multiple distinct error types. Use when different recovery
   strategies apply to different errors.
5. **Hybrid** - combinations of the above at different layers.

Document which strategy applies to each error class and why.

### 6. Testability Mapping

For each requirement, map test coverage to error paths:

- **Happy path**: The main success case. Always testable.
- **Error paths**: Which errors are covered by unit tests? Which require
  integration tests or mocks?
- **Edge cases**: Boundary conditions (zero, empty, maximum, invalid state).
  How are they tested?
- **Concurrency**: If async, how are concurrency errors (timeouts, cancellation,
  message loss) tested?
- **Property-based**: Are there invariants that should be verified with
  property-based testing (e.g., quickcheck)?

## Composition and References

### Handoff Authorities

Read the relevant handoff files first. They should provide:

- **Purpose** - what problem does this solve?
- **Scope** - what is in scope? What is out?
- **Actors** - who or what interacts with this feature?
- **Data flows** - what data moves where?
- **Success criteria** - how do we know it works?
- **Constraints** - resource limits, performance targets, compliance rules.

### Rust-Specific Output Format

Use this structure for the Rust-specific requirements document:

```
# Rust Implementability: [Feature/Requirement Name]

## Overview
[Brief summary of what this requirement does in Rust context]

## Handoff Anchor
[Reference to the handoff file(s) this enriches]

## Implementability Gate
[PASS / CONDITIONAL / BLOCKED + reasoning]

## Type Safety and Memory Safety
[Ownership, borrowing, lifetime decisions]

## Async and Concurrency Model
[Async/sync boundaries, actor alignment, shared state]

## Error Cases and Recovery
[Table: error case, category, testability, recovery strategy, Result type]

## Error Handling Strategy
[Which error types use Result, panic, Option, or custom]

## Testability Mapping
[Happy path, error paths, edge cases, concurrency scenarios, property-based
coverage plan]

## Verification and Validation
[How to prove this requirement is implemented correctly]

## Design Decisions and Rationale
[Key tradeoffs, why this design over alternatives]

## Implementation Notes
[Guidance for the implementer: patterns to use, anti-patterns to avoid]
```

## Decision Criteria

Use these criteria to decide whether a requirement is ready for Rust
implementation:

### 1. Clarity Gate

**Question**: Can a Rust developer read the requirement and immediately
understand how to type the API?

- **Pass**: The requirement specifies input/output types, error conditions, and
  ownership.
- **Conditional**: The requirement is clear but lifetime or async complexity
  needs clarification.
- **Block**: The requirement is ambiguous about ownership, error handling, or
  concurrency.

### 2. Testability Gate

**Question**: Can all error paths and edge cases be covered by tests?

- **Pass**: All paths are testable via unit or integration tests.
- **Conditional**: Some paths require fuzzing, property-based testing, or
  adversarial scenarios that need special tooling.
- **Block**: The requirement is untestable (e.g., requires timing-sensitive
  behavior or true randomness with no seed).

### 3. Concurrency Gate

**Question**: Is the concurrency model explicit and implementable?

- **Pass**: The requirement clearly states sync or async, actor boundaries, and
  shared-state constraints.
- **Conditional**: Concurrency is optional; document both sync and async paths.
- **Block**: The requirement mixes sync and async in a way that violates Rust's
  async model or creates deadlock risk.

### 4. Type Safety Gate

**Question**: Can the type system enforce the requirement?

- **Pass**: The requirement can be expressed as a type signature or trait bound.
- **Conditional**: The requirement requires runtime checks or custom validation.
- **Block**: The requirement contradicts Rust's type system or requires unsafe
  code in the happy path.

For single-field semantic wrappers, call out whether the wrapper should
preserve the underlying wire format. If it should, `#[serde(transparent)]` (or
equivalent transparent serde handling) is the default serialization boundary.
If it needs a custom wire format, validation, or encoding, document that
explicitly instead.

### 5. Feasibility Gate

**Question**: Can this requirement be implemented within the project's
constraints (dependencies, performance, deployment model)?

- **Pass**: The requirement aligns with project architecture and dependencies.
- **Conditional**: The requirement requires a new dependency or architectural
  change; document the tradeoff.
- **Block**: The requirement requires external resources, exact timing, or
  capabilities the project cannot provide.

## Validation Rules

Before marking a Rust requirement ready for implementation:

### R1. Error Taxonomy is Complete

- [ ] All error cases are enumerated (logical, resource, concurrency, domain).
- [ ] Each error case is mapped to a Result type or panic decision.
- [ ] Recovery paths are documented for each error.
- [ ] Test coverage for error cases is planned.

### R2. Async Boundaries are Explicit

- [ ] If async: actor boundaries, message types, and cancellation points are
  named.
- [ ] If sync: shared-state access patterns are documented (Arc, Mutex, RwLock,
  or none).
- [ ] Deadlock risks, if any, are identified and mitigated.

### R3. Ownership is Clear

- [ ] Owned vs. borrowed data is explicit in the API sketch.
- [ ] Heap allocation decisions are justified.
- [ ] Lifetime constraints, if any, are named and explained (not deferred to
  implementation).

### R4. Type Safety is Leveraged

- [ ] The requirement can be expressed as a Rust type signature or trait bound.
- [ ] Invariants that the type system can enforce are identified.
- [ ] Runtime checks, if any, are minimal and necessary.

### R5. Testability Plan is Concrete

- [ ] Happy path test is sketched.
- [ ] Error-path tests are identified (unit, integration, or special tooling).
- [ ] Edge cases and boundary conditions are listed.
- [ ] Concurrency scenarios, if applicable, are included.

### R6. Implementer Guidance is Actionable

- [ ] Decision rationale is documented (not just "why not X" but "why Y").
- [ ] Patterns or idioms the implementer should use are named (e.g., newtype,
  typestate, builder).
- [ ] Anti-patterns to avoid are called out.
- [ ] Open design questions, if any, are noted for the implementation phase.

## Examples

### Example 1: Simple Synchronous Requirement

**Universal Requirement**: Parse a comma-separated value string into a list of
fields.

**Rust Enrichment**:

```
# Rust Implementability: CSV Field Parser

## Implementability Gate: PASS

## Type Safety
- Input: `&str` (borrowed string)
- Output: `Result<Vec<&str>, ParseError>`
- Ownership: No allocations for the output; fields are borrowed from input.

## Error Cases
- Empty input → Result::Ok(vec![])
- Unterminated quote → Result::Err(ParseError::UnterminatedQuote)
- Invalid encoding → Result::Err(ParseError::InvalidUtf8)

## Testability
- Happy path: multiple field types, quoted fields, escaped quotes
- Error paths: all three errors can be unit-tested with mock inputs
- Edge cases: empty string, single field, trailing delimiter

## Error Handling
Result<Vec<&str>, ParseError> - recoverable parsing error.
```

### Example 2: Async Requirement

**Universal Requirement**: Fetch configuration from a remote server, with a
timeout and automatic retry on transient failures.

**Rust Enrichment**:

```
# Rust Implementability: Remote Config Fetch

## Implementability Gate: PASS (with concurrency constraints)

## Async/Concurrency Model
- API: async fn fetch_config() -> Result<Config, FetchError>
- Actor boundary: spawned as a one-shot task in an actor's message handler
- Timeout: tokio::time::timeout() enforces deadline
- Retry: external loop (up to 3 retries) on FetchError::Transient

## Error Cases
- Connection timeout (after 5s) → FetchError::Timeout (retriable)
- DNS failure → FetchError::DnsFailure (retriable, up to 3x)
- Invalid JSON → FetchError::InvalidJson (non-retriable)
- Server error (5xx) → FetchError::ServerError (retriable)
- Malformed response → FetchError::MalformedResponse (non-retriable)

## Error Handling
Result<Config, FetchError> with retriable flag in FetchError::
- Transient errors (timeout, DNS, 5xx): retry up to 3x with exponential backoff
- Non-transient errors (JSON, malformed): fail immediately

## Testability
- Happy path: mock HTTP server with valid JSON
- Timeout: set mock delay to exceed timeout threshold
- Retries: mock transient failure on 1st attempt, success on 2nd
- Non-retriable error: mock invalid JSON, verify no retry
- Cancellation: spawn task, cancel via handle; verify cleanup

## Concurrency
- Spawned as tokio task; caller gets JoinHandle<Result<Config, FetchError>>
- No shared state; config is immutable once fetched
- Cancellation: dropping JoinHandle cancels the task
```

### Example 3: Ownership and Lifetime Decision

**Universal Requirement**: Store a reference to user-provided data and return it
in a response later.

**Rust Enrichment** (CONDITIONAL → requires clarification):

```
# Rust Implementability: User Data Reference

## Implementability Gate: CONDITIONAL

Problem: Storing a reference requires a lifetime parameter, which complicates
API design.

## Ownership Options (pick one)

Option A: Clone the data (simple, but allocates)
  - Input: T (owned or borrowed, copied in)
  - Storage: owned Vec<T>
  - Return: &T (borrowed from storage)
  - Tradeoff: extra allocation, but API is simple

Option B: Use a lifetime parameter (zero-copy, but complex)
  - Input: &'a T
  - Storage: &'a T
  - Return: &'a T
  - Tradeoff: caller is responsible for keeping data alive; API complexity

Option C: Use Rc/Arc (thread-unsafe shared reference, or thread-safe)
  - Input: Arc<T> (caller or implementer allocates)
  - Storage: Arc<T> (shared ownership)
  - Return: Arc<T> (caller clones the Arc, shares reference)
  - Tradeoff: reference counting overhead, but simple API

## Recommendation
Clarify: is this data small and often-copied (choose Option A), long-lived and
expensive (choose Option B), or shared across threads (choose Option C)?
```
