---
name: 0-global-behavioral-specification
description: >
  Given/When/Then behavioral specification format: how to structure, write, validate, and
  review behavior specifications. Covers atomicity rules, completeness criteria, review
  pass/fail conditions, and examples. Use during Design when writing or reviewing behavior
  specifications, and during Review when validating test coverage against behavioral
  contracts.
---

# 0-global-behavioral-specification

## Specification Format

1. **Given/When/Then Structure**
   - Given: context/preconditions (what is true before the action)
   - When: action/trigger (what happens)
   - Then: outcome/assertion (what is true after; may include side effects or observable state)

2. **Behavioral Specification Construction**
   - Turning requirements into behaviors
   - Identifying atomic behavioral units (one behavior per scenario)
   - Ensuring behaviors are testable and implementation-independent
   - Mapping preconditions to test setup
   - Mapping actions to code execution paths
   - Mapping outcomes to assertions and observable side effects

3. **Completeness Criteria**
   - Every requirement must be expressible as one or more Given/When/Then behaviors
   - Behaviors must be unambiguous (no temporal ambiguity, no missing context)
   - Behaviors must be complete (all necessary context stated, no implicit assumptions)
   - Behaviors must be atomic (one logical assertion per Then clause)
   - Edge cases explicitly identified and specified

4. **Validation and Review**
   - Criteria for a complete behavior specification
   - Criteria for an incomplete or ambiguous specification
   - How to detect missing, redundant, or overlapping behaviors
   - Review pass/fail logic

## Per-Scenario Document Format

Each scenario in `behaviors.md` MUST open with a compact inline header that
carries the behavior ID, feature reference, requirement reference, and an
optional essential marker:

```
### BH-XXX-NNN [FE-XXX-NN / REQ-XXX-NN] - Scenario Title
### BH-XXX-NNN [FE-XXX-NN / REQ-XXX-NN] - Scenario Title [essential]
```

Where:
- `BH-XXX-NNN` - stable behavior ID (unique within the behavior document)
- `FE-XXX-NN` - feature reference from the feature specification
- `REQ-XXX-NN` - requirement reference from the requirements document
- `Scenario Title` - brief, descriptive title that identifies the scenario
- `[essential]` - optional marker; omit for supplementary scenarios (see below)

Do **not** open scenarios with a YAML metadata code fence. The inline header
carries all required traceability information in one line.

**Essential scenarios** are behaviors whose absence would make the feature
fundamentally broken - the dominator behaviors that every successful execution
path must satisfy. Mark a scenario `[essential]` when failing to cover it would
leave core functionality unverifiable regardless of other coverage.

**Coverage contract:**
- Essential scenarios (marked `[essential]`): require **100% test coverage**.
  `review-behavior-checker` and `review-completeness-checker` both gate on 100% essential-scenario
  coverage regardless of the overall coverage target.
- Supplementary scenarios (no `[essential]` marker): use the standard threshold
  (default: 80%).

**Acceptance Criteria (when needed):** If the `Then` clause does not fully
express all testable conditions, add ≤2 inline bullet points immediately after
the `Then` clause:

```
- AC: <condition not captured by Then>
```

When all criteria are already expressed in the `Then` clause, omit the AC
block entirely. Do **not** use a `#### Acceptance Criteria` heading or a full
bulleted list - the `Then` clause is the primary assertion surface.

**Example scenario using the required format:**

```
### BH-CART-001 [FE-CART-01 / REQ-CART-1] - Item added to cart successfully

Given a user is browsing the product catalog
  AND the product "Widget A" is in stock with price=$19.99
When the user clicks "Add to Cart" for "Widget A"
Then the item is added to the user's cart
  AND the cart item count increments by 1
  AND the cart subtotal increases by $19.99
```

```
### BH-CART-002 [FE-CART-01 / REQ-CART-2] - Out-of-stock item cannot be added

Given a user is browsing the product catalog
  AND the product "Widget B" has stock quantity = 0
When the user attempts to add "Widget B" to their cart
Then the system displays an "out of stock" message
  AND the item is NOT added to the cart
- AC: CartError::OutOfStock is returned, not a generic error
```

## Key Concepts

### 1. Behavioral Specification as Contract

A Given/When/Then behavior is a **minimal executable specification**:
- **Given** describes the test setup (test fixtures, initial state, mock configuration)
- **When** describes the operation being tested (function call, message send, user action)
- **Then** describes the observable outcome (return value, state change, side effect)

**Principle:** If code passes all Given/When/Then scenarios, the feature meets the specification.

### 2. Atomicity: One Logical Assertion Per Behavior

Each Given/When/Then is a **single testable claim**, not a sequence:

**Anti-pattern (sequence):**
```
Given a user is logged in
When they click the submit button
Then the form submits
  AND the user is redirected
  AND an email is sent
  AND the database is updated
```

**Pattern (atomic):**
```
Behavior: User form submission succeeds
Given a logged-in user with a valid form
When the user clicks submit
Then the form submission returns success

Behavior: Form submission triggers email
Given a logged-in user with a valid form
When the user clicks submit
Then an email notification is sent to the user

Behavior: Form submission persists state
Given a logged-in user with a valid form
When the user clicks submit
Then the submitted data is stored in the database
```

### 3. Preconditions Must Be Complete and Testable

**Given** clauses must state **all context needed to execute the behavior**:

**Anti-pattern (incomplete):**
```
Given a user
When they view the dashboard
Then they see their data
```
(What user? What data? What dashboard state? What authorization?)

**Pattern (complete):**
```
Given an authenticated user with role=viewer
  AND the user has 5 active projects
  AND the dashboard cache is fresh (≤5 minutes old)
When the user navigates to /dashboard
Then the user sees exactly 5 project cards
  AND each card displays the project's current status
```

**Rule:** A tester reading only the Given clause should be able to construct the test setup without guessing.

### 4. Actions Must Be Observable and Singular

**When** clauses must describe one **externally observable action**:

**Anti-pattern (sequence):**
```
When the user opens the app
  AND enters their credentials
  AND clicks login
```

**Pattern (singular action):**
```
When the user submits the login form
(The Given clause specifies that credentials are already entered.)
```

**Rule:** "When" does not describe steps; it describes the boundary event being tested. Steps belong in the Given setup or in a separate behavior.

### 5. Outcomes Must Be Observable

**Then** clauses must specify outcomes that are **testable and observable**:

**Anti-pattern (untestable):**
```
Then the system is fast
Then the user is happy
```

**Pattern (observable):**
```
Then the login response time is ≤500ms
Then the success page displays the user's name
Then an audit log entry is created with timestamp and user ID
```

**Rule:** "Observable" means: measurable, checkable, or verifiable by examining state or output.

### 6. Independence vs. Composition

Behaviors are **independent in specification** but may **compose in implementation**:
- Each behavior is a complete scenario that could be tested in isolation
- Implementation may optimize by sharing setup, reusing functions, or batching operations
- A behavior specification does NOT prescribe "first do X, then do Y"

**Example:**
```
Behavior: User can create an account
Given no account exists for email alice@example.com
When the user submits a registration form with email=alice@example.com, password=secret123
Then a new account is created with email=alice@example.com

Behavior: User can log in with newly created account
Given an account exists for email alice@example.com with password=secret123
When the user submits a login form with email=alice@example.com, password=secret123
Then the user receives an authentication token
```
(The behaviors are independent. Implementation may reuse account creation logic, but each behavior is testable alone.)

### 7. Equivalence: Behavior ↔ Requirement

Every requirement must be expressible as one or more behaviors:

**Requirement:** "Users shall be able to reset their password via email"

**Behaviors:**
```
Behavior: Password reset request accepted
Given an authenticated user
  AND an email is configured for the account
When the user requests a password reset
Then the system sends a reset email to the configured address

Behavior: Password reset link is valid
Given a password reset email was sent
When the user clicks the reset link within 24 hours
Then the system presents a password reset form

Behavior: Password reset completes successfully
Given the user is on a valid reset form
  AND they enter a new password meeting policy (min 8 chars, 1 uppercase, 1 digit)
When the user submits the new password
Then the password is updated
  AND the user can log in with the new password
  AND previous reset links are invalidated
```

**Rule:** If a requirement cannot be expressed as a behavior, it is incomplete or non-testable.

### 8. Completeness: Coverage Matrix

A behavior specification is **complete** when:
- Every requirement has ≥1 behavior
- Every happy path scenario is specified
- Major edge cases are specified:
  - Invalid inputs (malformed, out-of-range, wrong type)
  - Missing required state (no account, no permissions, expired token)
  - Boundary conditions (empty list, max size, zero, negative)
  - Concurrent access (race conditions, resource contention)
  - Failure modes (service unavailable, timeout, partial failure)

**Incompleteness Markers:**
- "What if X fails?" is unanswerable
- Two behaviors contradict each other
- A behavior references undefined state ("the user's data" without specifying what data)
- Requirements map to 0 behaviors

## Examples

### Example 1: E-Commerce Add to Cart

**Requirement:** "Users can add items to their shopping cart"

**Behavioral Specification:**

```
Behavior: Item added to cart successfully
Given a user is browsing the product catalog
  AND the product "Widget A" is in stock with price=$19.99
When the user clicks "Add to Cart" for "Widget A"
Then the item is added to the user's cart
  AND the cart item count increments by 1
  AND the cart subtotal increases by $19.99

Behavior: Out-of-stock item cannot be added
Given a user is browsing the product catalog
  AND the product "Widget B" has stock quantity = 0
When the user attempts to add "Widget B" to their cart
Then the system displays an "out of stock" message
  AND the item is NOT added to the cart
  AND the cart remains unchanged

Behavior: Duplicate item in cart increments quantity
Given a user has "Widget A" (qty=1) already in their cart
  AND they view the product page for "Widget A" again
When the user clicks "Add to Cart"
Then the cart item quantity for "Widget A" becomes 2
  AND the cart subtotal increases by $19.99
  AND the cart item count does not change (same product, qty incremented)

Behavior: Add to cart preserves existing items
Given a user has ["Widget A" (qty=1), "Widget C" (qty=2)] in their cart
When the user adds "Widget B" (qty=1) to the cart
Then the cart contains ["Widget A" (qty=1), "Widget B" (qty=1), "Widget C" (qty=2)]
  AND all prices are correct
```

- ✓ Every requirement facet is covered
- ✓ Happy path (success) specified
- ✓ Edge case (out of stock) specified
- ✓ Edge case (duplicate) specified
- ✓ Each behavior is atomic (one logical test case)
- ✓ Preconditions fully specify test setup
- ✓ Outcomes are observable (count, price, message)

---

### Example 2: Authentication

**Requirement:** "Users shall authenticate via username and password"

**Behavioral Specification:**

```
Behavior: Valid credentials grant access
Given a user account exists with username="alice" and password hash for "SecurePass123"
  AND the account is active (not locked or suspended)
When the user submits login form with username="alice" and password="SecurePass123"
Then the user receives an authentication token
  AND the token is valid for the next 24 hours
  AND an audit log entry is recorded with timestamp, username, and "login success"

Behavior: Invalid password denied
Given a user account exists with username="alice" and password hash for "SecurePass123"
When the user submits login form with username="alice" and password="WrongPassword"
Then authentication fails
  AND the user does not receive a token
  AND an audit log entry is recorded with "login failure" and the username
  AND the account is NOT locked (first failed attempt)

Behavior: Account locked after repeated failures
Given a user account exists with username="bob"
  AND the account has 4 failed login attempts in the last 15 minutes
When the user submits login form with username="bob" and any password
Then authentication fails
  AND the account is marked as "locked" (temporary, 30-minute cooldown)
  AND a security alert is sent to bob's registered email
  AND an audit log entry records "account locked"

Behavior: Nonexistent user rejected safely
Given no account exists for username="nobody"
When the user submits login form with username="nobody" and password="anything"
Then authentication fails
  AND no error message reveals whether the username exists
  AND an audit log entry is recorded with "login failure - user not found"
```

- ✓ Happy path (valid credentials) specified
- ✓ Negative path (wrong password) specified
- ✓ Security edge case (repeated failures) specified
- ✓ Security best practice (no user enumeration) specified
- ✓ Audit trail observable in all scenarios
- ✓ Account lockout time bounds specified (30 minutes)

---

### Example 3: Incomplete Specification (Anti-Pattern)

**Requirement:** "The report generation feature shall work"

**Incomplete Behavioral Specification:**

```
Behavior: Generate report
Given a user
When they generate a report
Then a report is generated
```

**Problems:**
- ❌ "a user" - no role, no permissions specified
- ❌ "a report" - what type? What data? What format? Unspecified.
- ❌ "they generate a report" - what action exactly? What parameters?
- ❌ "a report is generated" - how is it observable? Where is it? In what format?
- ❌ No edge cases (no data, permission denied, format error, timeout)
- ❌ Not testable without guessing

**Improved Behavioral Specification:**

```
Behavior: Analyst generates sales report for date range
Given a user with role="analyst"
  AND the date range January 1 to January 31 has 150 sales records
When the user requests a report with type="sales_summary" and date_range=[Jan-01, Jan-31]
Then the system generates a PDF report containing:
  - Total sales amount
  - Sales by region (pie chart)
  - Top 10 products (table)
  - Row count matches sales records: 150
  AND the report is available for download at the user's dashboard
  AND an audit log entry records the report generation

Behavior: Non-analyst cannot generate reports
Given a user with role="viewer" (not "analyst")
When the user attempts to request a report
Then the system denies the request with a 403 Forbidden error
  AND no report is generated
  AND an audit log entry records "unauthorized report access attempt"

Behavior: Report generation with no data succeeds
Given a user with role="analyst"
  AND the date range February 1 to February 29 has 0 sales records
When the user requests a report for that empty range
Then the system generates a PDF report with:
  - Total sales amount: $0
  - "No data for this date range" message
  - Row count: 0
  AND the report is available for download
```

---

## Decision Criteria

### When to Apply This Skill

1. **Design Stage (1-design-3):** Behavior builders use this skill to convert feature requirements into Given/When/Then specifications
2. **Behavior Review Gate (1-design-3-2):** Use this skill to validate behavior completeness and atomicity before accepting the design behavior specification
3. **Implementation Review (4-review-4):** Reviewers use this skill to map test cases back to behaviors and validate coverage
4. **Behavior Gate (4-review-4-2):** Final gate uses this skill to confirm all behaviors are satisfied by tests

### Common Pitfalls

| Pitfall | Consequence | Prevention |
|---------|-------------|-----------|
| **Behaviors describe sequences** | Not independently testable; coupling introduces fragility | Specify one atomic behavior per scenario |
| **Given clauses are incomplete** | Testers must guess setup; tests become flaky | Checklist: can I setup this Given without reading the When? |
| **Then clauses are vague** | Unmeasurable outcomes; reviewer cannot gate pass/fail | Each Then must be observable: measurable, stateful, or traceable |
| **Requirements not covered** | Gaps in specification; implementation surprises | Coverage matrix: req ↔ behavior traceability |
| **Behaviors contradict** | Impossible to satisfy all; implementation blocked | Consistency audit: do any two behaviors conflict? |
| **Behaviors are coupled to implementation** | Spec breaks when implementation details change | When/Then must describe contract, not implementation details |
| **Too many edge cases** | Specification bloat; unclear priority | Apply Pareto: specify happy path + top 3 risk edge cases first |

---

## Validation Rules

### Gate Pass Conditions

A behavior specification passes review when:

1. **Coverage:** Every stated requirement maps to ≥1 behavior
2. **Atomicity:** Each behavior has exactly one logical assertion (one reason to pass/fail)
3. **Completeness Given:** Every Given clause contains all context needed to construct the test; no required assumptions
4. **Observable When:** The When describes one externally observable action
5. **Observable Then:** Each Then clause is measurable, testable, or verifiable by state inspection
6. **Consistency:** No two behaviors are logically contradictory
7. **Independence:** Each behavior can be tested in isolation (may reuse setup, but no mandatory ordering)
8. **Non-Redundancy:** No two behaviors test the identical scenario with identical outcomes
9. **Edge Cases:** Happy path + critical edge cases specified (e.g., invalid input, missing required state, boundary conditions)
10. **Traceability:** Each behavior references its source requirement (or feature) for audit trail

### Gate Fail Conditions

A specification fails review when:

- **Any requirement is not covered by a behavior**
- **Any Given clause is missing context or requires implicit assumptions**
- **Any When is ambiguous or describes multiple steps**
- **Any Then is unmeasurable, vague, or untestable**
- **Two behaviors contradict (mutually exclusive outcomes)**
- **A behavior assumes implementation details instead of specifying contracts**
- **Critical edge cases are missing** (e.g., "what if permission denied?" unanswerable)

### Validation Checklist

Reviewers use this checklist:

```
□ Requirement coverage: Every requirement has ≥1 behavior
□ Atomicity: Each behavior = 1 logical assertion
□ Given complete: All context specified, no implicit setup
□ When singular: One observable action, not a sequence
□ Then observable: All outcomes measurable/testable/verifiable
□ Consistency: No contradictions between behaviors
□ Edge cases: Happy path + top 3 risks specified
□ Independence: Each behavior testable in isolation
□ Non-redundancy: No duplicate behaviors
□ Traceability: Behaviors linked to requirements
□ Contract focus: Behaviors specify what, not how
□ Ambiguity: No undefined terms, all references resolved
```
