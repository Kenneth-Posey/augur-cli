---
name: 2-plan-integration-planning
description: "Produces specifications for component interactions across boundaries, defining integration points, contracts, and mocking strategies for cross-boundary testing. Use at the Plan stage when components interact across module boundaries and integration points, contracts, and test isolation strategies must be specified."
---

# Skill: 2-plan-integration-planning

**Output:** An integration specification that defines cross-boundary behavior and test isolation.

---

## Scope

This skill covers:

1. **Integration Points:** Explicit boundaries where modules exchange data, calls, or state.
2. **Component Contracts:** Input schemas, output schemas, side effects, error conditions, and timing assumptions for each integration point.
3. **Mocking Strategy:** How to isolate components during testing; which boundaries require mocks, stubs, or adapters; and how mocks reflect actual behavior.
4. **Dependency Injection:** Whether integration points use constructor injection, method parameters, trait objects, or configuration to decouple components.
5. **State Boundaries:** Shared state, message passing, or event-driven coordination between components.
6. **Composition Order:** Which component initializes first, initialization dependencies, and lifecycle hooks.
7. **Error Propagation:** How errors cross boundaries; whether they transform, wrap, or fail-fast.
8. **Observability Points:** Logging, tracing, metrics collection at integration boundaries.

**Out of Scope:**
- Internal component logic (address in component design documents).
- Performance benchmarking or load testing strategies.
- Deployment or infrastructure orchestration.

---

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### 1. Integration Point

An **integration point** is a location where two or more modules interact:

- **Synchronous Call:** Module A calls a function/method on Module B and waits for a response.
- **Asynchronous Message:** Module A sends a message to a queue; Module B consumes and processes it.
- **Shared State:** Modules read/write a common data structure (database, cache, message bus).
- **Event Subscription:** Module A publishes an event; Module B subscribes and reacts.

**Contract for each point:** name, direction (one-way or two-way), input/output schemas, failure modes, latency expectations.

### 2. Component Contract

A **contract** specifies what a component promises:

```
Component: PaymentProcessor
Integration Point: ProcessPayment
  Input:
    - order_id: UUID
    - amount_cents: u64
    - payment_method: "card" | "bank_transfer"
  Output:
    - transaction_id: UUID
    - status: "approved" | "declined" | "pending"
  Side Effects:
    - Writes to payment_log table (idempotent)
    - Publishes PaymentProcessed event
  Error Conditions:
    - Invalid amount: returns declined
    - Network failure: retryable after 5s
    - Duplicate order_id within 60s: returns same transaction_id
  Assumptions:
    - order_id exists in orders table
    - Caller holds database write lock if needed
  Timeout: 30s
```

### 3. Mocking Strategy

**Strategy** defines how to replace or stub a component during testing:

- **Full Mock:** Component is entirely replaced with a fake that returns pre-programmed responses.
- **Partial Mock:** Real component is used, but external dependencies (DB, API) are mocked.
- **Spy:** Real component runs; calls are logged for verification.
- **Adapter Mock:** A test adapter wraps the real component, intercepting calls for verification.

**When to Use:**

| Scenario | Strategy | Reason |
|----------|----------|--------|
| Testing ordering logic (caller) | Mock payment processor | Isolate order logic from payment complexity |
| Testing payment processor in isolation | Mock external payment gateway | Verify business logic, not third-party API |
| Testing integration of payment + audit log | Partial mock (real payment, mock DB) | Verify both components' contract without I/O |
| Testing entire checkout flow | Spy (real all components, log calls) | Verify realistic flow, detect coupling issues |

### 4. Dependency Injection Patterns

- **Constructor Injection:** Component receives dependencies in constructor/initializer.
- **Method Parameters:** Dependencies passed per call.
- **Trait Objects:** Component receives trait object; mock implements trait.
- **Configuration + Factory:** Component resolved from factory with test config.

**Preference for testability:** Constructor or trait objects (enables swapping for tests).

### 5. State Boundaries

- **No Shared State:** Each component owns its data; integration via messages or calls.
- **Shared Mutable State:** Components access common data structure; requires synchronization and testing for race conditions.
- **Event-Driven State:** Components react to events; state transitions verified via event sequence.

**Best practice:** Minimize shared mutable state; prefer message-passing or event streams.

### 6. Error Propagation

Define how errors cross boundaries:

- **Transform:** Error A becomes Error B at boundary.
- **Wrap:** Error A is wrapped in Error B context.
- **Fail-Fast:** Error causes immediate halt; caller must handle.
- **Retry:** Caller automatically retries with backoff.

**Example:**
```
PaymentGatewayError (external)
  → PaymentProcessorError::GatewayUnavailable (internal)
    → OrderError::PaymentFailed (domain)
      → HTTP 402 Payment Required (API response)
```

---

## Composition & References

### Document Structure

An **integration planning document** should include:

1. **Title & Scope:** What modules are integrated; what questions this spec answers.
2. **Module Inventory:** List of modules, their responsibilities, their owned data.
3. **Integration Points Table:**
   - Point ID
   - Source → Target
   - Synchronous/Asynchronous
   - Input schema
   - Output schema
   - Error modes
   - Latency SLA
   - Mocking strategy for testing

4. **Dependency Injection Design:**
   - Constructor signatures or factory patterns
   - Test fixture setup
   - Mock implementations

5. **State Boundary Diagram:**
   - Which module owns which data
   - Where shared access occurs
   - Read-only vs. read-write boundaries

6. **Error Propagation Map:**
   - External errors → Internal errors → Domain errors
   - Retry policies
   - Logging checkpoints

7. **Observability Plan:**
   - Logs at each boundary (input, output, error)
   - Metrics (latency, success rate per point)
   - Trace instrumentation

8. **Testing Strategy:**
   - Which points use full mocks, partial mocks, spies
   - Fixture setup for each scenario
   - Integration test scenarios (happy path, error cases, concurrency)

### References to Related Documents

- **Component Design Specs:** Describe individual module internals (referenced in module inventory).
- **Data Schema Docs:** Define input/output schemas (referenced in integration points table).
- **Error Catalog:** Lists domain errors and their meanings (referenced in error propagation).
- **Dependency Design:** If multi-tier architecture, shows layer order (referenced in composition).

---

## Examples

### Example 1: Payment + Order Integration

**Modules:**
- OrderService: Manages order lifecycle
- PaymentProcessor: Processes payment transactions
- AuditLogger: Records all financial transactions

**Integration Points:**

| Point | Source | Target | Direction | Input | Output | Mock Strategy |
|-------|--------|--------|-----------|-------|--------|----------------|
| ProcessPayment | OrderService | PaymentProcessor | Sync call | {order_id, amount, method} | {tx_id, status} | Full mock (returns "approved") |
| LogTransaction | PaymentProcessor | AuditLogger | Async event | {tx_id, amount, timestamp} | ack | Spy (verify called, don't mock) |
| FetchOrder | PaymentProcessor | OrderService | Sync call | {order_id} | {order, status} | Partial mock (real OrderService, mock DB) |

**Mocking for test scenario "Payment succeeds, order fulfilled":**
```
Setup:
  - OrderService: real (in-memory DB)
  - PaymentProcessor: real
  - AuditLogger: spy (log calls without writing)
  
Steps:
  1. Create order in OrderService
  2. Call PaymentProcessor.ProcessPayment(order_id, amount, "card")
  3. Verify:
     - OrderService.FetchOrder called
     - PaymentProcessor returns {tx_id: "123", status: "approved"}
     - AuditLogger.LogTransaction called with tx_id, amount
```

### Example 2: Event-Driven Integration (Message Queue)

**Modules:**
- Producer: Publishes events to queue
- Consumer: Subscribes to queue, processes events

**Integration Point:**
```
Point: OrderCreatedEvent
  Source: OrderService
  Target: NotificationService
  Direction: Async (message queue)
  
  Input Schema:
    - order_id: UUID
    - customer_email: String
    - total_amount: Currency
  
  Output: None (fire-and-forget)
  
  Mock Strategy:
    - Mock queue: in-memory list
    - Verify OrderService publishes event
    - Verify NotificationService consumes and sends email
```

### Example 3: Trait-Based Injection

**Component Contract (Rust example, pattern applies to other languages):**
```rust
trait PaymentGateway {
  fn charge(&self, order: &Order) -> Result<TransactionId, PaymentError>;
}

struct OrderProcessor {
  gateway: Box<dyn PaymentGateway>,
}

// Production
let processor = OrderProcessor {
  gateway: Box::new(StripeGateway::new(api_key)),
};

// Testing
let processor = OrderProcessor {
  gateway: Box::new(MockPaymentGateway {
    response: Ok(TransactionId("123")),
  }),
};
```

---

## Decision Criteria

Use this skill when:

1. **Two or more modules must work together** to deliver a feature or behavior.
2. **Unclear how modules exchange data** (synchronous calls? events? shared state?).
3. **Testing strategy depends on isolation:** Need to mock or stub boundaries.
4. **Error handling crosses boundaries:** Errors from one module affect another.
5. **Composition order matters:** Some modules must initialize before others.

**Inputs Required:**
- Module list and their responsibilities.
- Dependency graph (which modules use which).
- High-level flow (happy path and error cases).

**Outputs Produced:**
- Integration specification document.
- Component contract table.
- Mocking strategy per integration point.
- Dependency injection design.
- Error propagation map.

---

## Validation Rules

### Before Accepting an Integration Spec

1. **Completeness:**
   - Every integration point has a contract (input, output, side effects, error modes).
   - Every module in the scope appears in the module inventory.
   - Every dependency has a mocking strategy assigned.

2. **Correctness:**
   - No circular dependencies (if present, justified as event-driven and explicitly marked).
   - Error propagation map covers all error modes from integration points.
   - Mocking strategy is testable (i.e., mock doesn't require the real component).

3. **Clarity:**
   - Each integration point has a unique ID and clear description.
   - Schemas are concrete (not "some object" or "data structure").
   - Latency SLAs and retry policies are explicit (not "reasonable" or "as fast as possible").

4. **Feasibility:**
   - Dependency injection pattern is applicable to language/framework in use.
   - Mock implementations do not require implementing the entire real component.
   - State boundaries do not create deadlocks or race conditions under test load.

### Rejection Criteria

- **Vague contracts:** "Component returns success" without defining success.
- **Unmockable dependencies:** Design requires mocking a third-party library that is tightly coupled.
- **Circular hard dependencies:** Module A must initialize Module B, but Module B must initialize Module A (no event-driven justification).
- **Missing error paths:** Error cases not covered in integration points or error propagation map.

---

## Notes for Users

- **Start with a dataflow diagram:** Sketch how data moves between modules before writing contracts.
- **Name integration points clearly:** Use domain language (e.g., "PaymentAuthorization" not "call_func_1").
- **Be specific with schemas:** If input is "order ID", specify its type (UUID, Integer, String) and constraints (required, max length, format).
- **Test the spec:** Have a reviewer confirm the contracts are clear enough to implement without follow-up questions.
- **Iterate:** If implementation reveals an ambiguous or unmockable contract, revise the spec before coding.
