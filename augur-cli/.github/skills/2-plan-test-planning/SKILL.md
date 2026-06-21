---
name: 2-plan-test-planning
description: "Designs a test strategy from behavioral specifications and function signatures: classifies scenarios into test types, builds a coverage matrix, specifies property predicates, defines pass conditions, and establishes test composition rules. Use at the Plan stage before any test code is written."
---

# Skill: 2-plan-test-planning

## Extracting Test Scenarios from Behavioral Specifications

Derive test scenarios directly from Given/When/Then behavioral specifications. Each GWT scenario should produce one or more test cases:

- **Given** → setup: all preconditions that must be established before the action under test. Translate each Given predicate into a concrete fixture, factory call, or stub configuration.
- **When** → action under test: the single function call or command that triggers the behavior. A test scenario has exactly one action under test; if a scenario's When clause describes multiple steps, decompose it into separate scenarios before planning tests.
- **Then** → assertion: each Then predicate becomes one or more assertions. All Then predicates for a scenario must be asserted in the same test; partial assertion is a coverage defect.

**One function per scenario:** map each scenario to exactly one function under test. If a scenario's When clause involves multiple functions, treat it as an integration scenario.

**Distinguishing input conditions:** for each scenario, identify the distinguishing input condition that separates it from other scenarios sharing the same function under test:

| Condition type | Description | Example |
|---|---|---|
| Happy path | All inputs valid; system in expected state | Valid credentials, correct format |
| Boundary | Input at the edge of a valid range | Empty collection, maximum-length string |
| Error path | Input violates a precondition or constraint | Missing required field, out-of-range value |
| Invalid state transition | Entity not in a state that allows the operation | Shipping a non-confirmed order |
| Concurrent access | Multiple callers interact with shared state simultaneously | Two writers, reader during write |

Every function under test must have at least one happy-path scenario and at least one error-path scenario. Functions with explicit state machine transitions must have a scenario for each invalid transition.

## Key Files

- `README.md` - overview and usage notes

## Classifying Test Scenarios into Test Types

Each scenario maps to exactly one test type. The classification rule is determined by the scope of what the Then clause asserts:

**Unit test:** one function under test; all external dependencies are replaced by mocks, stubs, or fakes; the Then clause asserts only on the return value or direct state of the object under test. Use for:
- Verifying a single behavioral rule in isolation.
- Testing error branches that are difficult to trigger with real dependencies.
- Testing all boundary conditions efficiently without I/O overhead.

**Integration test:** multiple real components interact; shared or persistent state may be involved; the Then clause asserts on state that is owned by a different aggregate or component than the one in the When clause. Use when:
- A scenario's correctness requires verifying that two components honor a shared contract.
- The behavior crosses a persistence, network, or process boundary.
- The Then clause asserts on the side effects of a command rather than its return value.

**Property-based test:** a predicate that must hold across many generated inputs rather than a fixed example. Use for:
- Domain invariants that must hold for any valid input (not just the examples in the scenario set).
- Mathematical or algebraic properties (commutativity, associativity, round-trip encoding/decoding).
- Functions whose input space is too large to enumerate with example-based tests.
- Detecting edge cases the scenario author did not anticipate.

**Performance/benchmark test:** measures wall-clock time or throughput against a defined baseline. Use only when:
- A behavioral specification explicitly states a latency or throughput requirement (e.g., "processes 10 000 records in under 500 ms").
- A regression baseline must be tracked across changes.
- Plan performance tests only when the specification requires them.

**Classification rule:** each scenario maps to exactly one test type. Document the rationale for the classification alongside the scenario entry in the coverage matrix, especially for scenarios that could plausibly be either unit or integration.

## Building the Coverage Matrix

The coverage matrix shows that every behavioral requirement has a test.

**Matrix structure:**
- **Rows:** state × event pairs from the behavior plan (every transition in the state machine, or every GWT scenario identifier if no state machine was produced).
- **Columns:** test scenarios (named by the naming convention defined in "Test Composition Rules").
- **Cell:** the test type that covers the (behavior, scenario) pair. A cell is filled when the test scenario's Given/When/Then fully covers the corresponding state × event pair.

**Coverage completeness rules:**
1. Every row must have at least one filled cell. An empty row is a coverage gap - emit it as a missing test scenario.
2. Every error type variant defined in the function signature plan must have at least one error-path test column covering it. An error variant with no test is a coverage gap.
3. Every invalid state transition must have at least one test column. An unguarded transition is a coverage gap.
4. If a row has only one cell and that cell is a unit test, consider whether an integration test is also required to verify the cross-boundary contract.

Include the matrix in the test plan. The plan is not complete until every row is filled and every gap is resolved or explicitly deferred with a documented rationale.

## Specifying Property-Based Tests

For each domain invariant identified in the domain spec and for each algebraic property implied by the function signature set, define a property-based test specification:

**Invariant identification sources:**
- The "Invariants" section of each entity in the domain spec.
- Mathematical properties implied by operations (e.g., a `sum` function is commutative; an `encode`/`decode` pair is a round-trip).
- Monotonic or conservation properties (sequence numbers only increase; total balance is conserved across transfers).

**For each property, specify:**

1. **Property name:** a declarative statement of what must always hold (e.g., `created_order_id_is_always_unique`, `encode_decode_round_trip`).
2. **Generator strategy:** how inputs are generated - the domain of valid inputs, any constraints on the generated values, and whether generation should be biased toward boundary values.
3. **Shrinking strategy:** when a failing input is found, how it should be minimized to the smallest failing case. Prefer structural shrinking (shrink each field independently) over opaque shrinking.
4. **Number of trials (N):** the minimum number of generated inputs that must pass before the property is considered verified. State N explicitly; do not leave it as a framework default.

**Property specification format:**

```
Property: <property-name>
Invariant: <the predicate that must hold, in verifiable form>
Generator: <description of input domain and constraints>
Shrink: <shrinking approach>
Trials: <N>
```

## Defining Pass Conditions

Every test scenario must have an explicit, measurable pass condition. Reject prose-only pass conditions such as "the test passes if it works correctly."

**Unit test pass condition:**
- All assertions pass.
- No unhandled exception or panic occurs within the test body.
- No mocked dependency is called with arguments outside its expected call specification.

**Integration test pass condition:**
- All state transitions and consistency checks defined in the Then clauses pass.
- No leaked state is observable in subsequent tests (each test leaves shared resources in the same state they were in before the test ran, or explicitly resets them).
- All cross-component contracts asserted in the Then clauses hold.

**Property-based test pass condition:**
- The property predicate holds for all N generated inputs (N specified per property).
- When a counterexample is found, the framework produces the shrunk minimal failing input.
- The test run completes within the time budget defined for the test profile.

**Performance test pass condition:**
- Mean latency is within X% of the documented baseline (X specified per test; typical values: 5–15%).
- Throughput meets or exceeds the floor stated in the behavioral specification.
- The measurement is taken after a warm-up period of at least one full iteration of the workload.
- Outlier percentiles (p95, p99) are reported alongside the mean.

**Rejection rule:** if a pass condition cannot be expressed as a Boolean predicate over observable outputs and state, it must be rewritten or the test scenario must be decomposed until it can.

## Test Composition Rules

Use these rules to organize the full test suite:

**Isolation:** no test depends on the execution order of any other test. Each test must produce the same result whether it runs first, last, or in a random sequence. Shared state that persists across tests is a design defect.

**Setup and teardown ownership:** each test owns its setup and teardown. If two tests require the same precondition, they each construct it independently - they do not share a mutable instance. Shared read-only fixtures (immutable reference data, pre-computed constants) may be referenced from a shared definition, but mutable state must be freshly constructed per test.

**Naming convention:** test names follow the pattern:

```
test_<function>_<input_condition>_<expected_result>
```

Where:
- `<function>` is the name of the function under test.
- `<input_condition>` is a concise label for the distinguishing input condition (e.g., `valid_credentials`, `empty_collection`, `expired_token`).
- `<expected_result>` is the observable outcome (e.g., `returns_user`, `returns_empty_list`, `returns_auth_error`).

Names must be self-documenting: a reader who has not seen the test body should be able to predict what the test verifies from its name alone.

**Single responsibility:** each test verifies exactly one behavioral outcome. If a test body contains multiple independent assertions about unrelated outcomes, split it into separate tests.

**Determinism:** tests must produce the same result on every run. Non-determinism sources to eliminate: wall-clock time, random number generators without fixed seeds, file system state not owned by the test, network calls not intercepted by the test.

## Language-Specific Companion

This skill is language-agnostic. For framework setup, assertion libraries, property-based testing packages, benchmark harnesses, and mock patterns, look up the `2-plan-test-planning` capability key in [`.github/local/language-companions.md`](../../local/language-companions.md) and use the listed companion skill.
