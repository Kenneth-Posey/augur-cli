---
name: 2-plan-domain-planning
description: "Designs domain models during planning by identifying entities, aggregates, value objects, relationships, and constraints independent of implementation language. Use at the Plan stage when a feature or refactor introduces new domain concepts or modifies existing domain boundaries."
---

# Skill: 2-plan-domain-planning

Produce a **Domain Entity Specification** that guides later behavior-wiring and domain-implementation work.

---

## Scope

**When to invoke this skill:**
- Designing new bounded contexts, aggregate boundaries, or value types
- Clarifying entity lifecycle (creation, validation, state transitions, deletion)
- Specifying invariants that must hold across aggregate operations
- Decomposing complex domain logic into semantic units
- Resolving conflicts between domain semantics and storage/wire representations

**When NOT to invoke:**
- Implementing domain logic in code (use behavior-wiring or domain-implementation)
- Designing persistence schemas, APIs, or UI layouts (use infrastructure planning)
- Optimizing performance or storage (use platform-specific design)
- Styling or presentation concerns

---

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### Entity

**Characteristics:**
- **Identity**: Uniquely identifiable (often by ID, UUID, or natural key)
- **Lifecycle**: Created, modified, and eventually discarded (or archived)
- **Mutability**: State changes over time in response to domain events or operations
- **Responsibility**: Models a noun (Agent, Order, Account, etc.)

**Example domains:**
- `User` (identity: user_id, lifecycle: signup → active/inactive → deleted)
- `Order` (identity: order_id, lifecycle: created → confirmed → shipped → delivered)

### Aggregate

**Characteristics:**
- **Root Entity**: One entity designated as the aggregate root
- **Boundary**: Encapsulates related entities and value objects
- **Invariants**: Business rules that must hold after every operation on the aggregate
- **Atomicity**: Updates to an aggregate must be atomic; no partial updates
- **External References**: Only the aggregate root is referenced from outside

**Example:** 
- Aggregate root: `Order`
- Children: `OrderLineItem` (entities), `ShippingAddress` (value object)
- Invariant: "An order must have at least one line item and a valid shipping address"

### Value Object

**Characteristics:**
- **Immutability**: Cannot change after creation; new instances replace old ones
- **No Identity**: Two value objects with identical attributes are equivalent
- **No Side Effects**: Pure data; no operations with domain side effects
- **Reusable**: Can be shared across aggregates without risk

**Example:**
- `Money` (value: 100, currency: USD) ≡ `Money` (value: 100, currency: USD)
- `Address` (street, city, state, zip)
- `DateRange` (start_date, end_date)

### Relationship

**Types:**
- **One-to-One** (Entity ↔ Entity): e.g., User ↔ Profile
- **One-to-Many** (Aggregate Root → Child Entities): e.g., Order → LineItems
- **Many-to-Many** (Aggregate ↔ Aggregate): e.g., Courses ↔ Students
- **Composition** (Parent → Child): e.g., Order ⊃ LineItem (child lifecycle depends on parent)
- **Association** (Aggregate A → Aggregate B via reference): e.g., Order → Customer (by customer_id only)

**Naming Convention:** Relationships are named by role. Bidirectional relationships must name both directions explicitly or justify unidirectionality.

### Invariant

**Examples:**
- "An Order must have at least one LineItem"
- "A user's email must be unique"
- "ShippingAddress.postal_code must match ShippingAddress.country"
- "An invoice total must equal the sum of its line items"

---

## Composition & References

### Aggregate Reference Pattern

When one aggregate needs to reference another:

| Pattern | Usage | Reference Type |
|---------|-------|-----------------|
| **Direct Nesting** | Value objects, child entities (composition) | Child object embedded in parent aggregate |
| **ID Reference** | Cross-aggregate associations | Store only the ID (aggregate root identity); fetch full object on demand |
| **Eventual Consistency** | Loosely coupled aggregates | Store ID; reconcile state via events or scheduled jobs |

**Rule:** Aggregates do not embed other aggregate roots. Reference them by ID only.

### Bidirectional vs. Unidirectional

- **Unidirectional** (preferred): A → B only. Simpler, fewer invariants. Query reverse direction if needed.
- **Bidirectional** (necessary when): Frequent navigation in both directions, or domain rules require mutual awareness.

**When bidirectional, both directions must be synchronized in code and tests.**

---

## Examples

### Example 1: E-Commerce Order Domain

```
Aggregate: Order
├── Root Entity: Order (identity: order_id)
│   ├── Fields: customer_id, created_at, status, total_amount
│   └── Invariants:
│       • Must have at least 1 LineItem
│       • Status transitions: PENDING → CONFIRMED → SHIPPED → DELIVERED
│       • total_amount = sum(line_items.amount)
│
├── Child Entity: LineItem
│   ├── Fields: line_id, product_id, quantity, unit_price
│   ├── Identity: scoped to Order (local identity only)
│   └── Invariant: quantity > 0, unit_price >= 0
│
├── Value Object: ShippingAddress
│   ├── Immutable fields: street, city, state, postal_code, country
│   └── Invariant: postal_code matches country format
│
└── Value Object: Money
    ├── Immutable fields: amount (decimal), currency (enum)
    └── Invariant: amount >= 0

Relationships:
• Order → Customer (reference by customer_id; Customer is separate aggregate)
• Order ⊃ LineItem (composition; LineItem has no independent identity)
• Order ⊃ ShippingAddress (composition; address value object)
```

### Example 2: User Account Domain

```
Aggregate: UserAccount
├── Root Entity: User (identity: user_id / email)
│   ├── Fields: email, username, password_hash, created_at, status
│   ├── Relationships: 1:1 → Profile
│   └── Invariants:
│       • Email is unique and valid format
│       • Username is unique and 3–32 chars
│       • Cannot delete user with active subscriptions
│
├── Value Object: EmailAddress
│   ├── Fields: address (string), verified (bool)
│   └── Invariant: Must match RFC 5322 pattern
│
├── Value Object: Credentials
│   ├── Fields: password_hash (bcrypt), updated_at, login_attempts
│   └── Invariant: login_attempts reset after successful login
│
└── Value Object: Profile
    ├── Fields: first_name, last_name, avatar_url, bio
    └── No invariants (optional decoration)

Associations:
• User → Subscription (many-to-many via join; managed separately)
• User → AuditLog (one-to-many; append-only)
```

---

## Decision Criteria

### Entity vs. Value Object

| Question | Entity | Value Object |
|----------|--------|--------------|
| Does it have persistent identity? | Yes (ID or natural key) | No (identified by attributes) |
| Does it change over time? | Yes | No (replaced, not updated) |
| Is it shared across aggregates? | Only by reference (ID) | Can be embedded freely |
| Does equality mean same object? | Yes (by ID) | No (by attributes) |

**Decision Rule:** Start with value objects. Promote to entity only if identity persistence is essential.

### Aggregate Boundary

Draw aggregate boundaries by asking:

1. **Consistency**: What data must be consistent together?
2. **Atomicity**: What must update together in a single transaction?
3. **Invariants**: What business rules bind these objects?
4. **Lifespan**: Do the objects' lifecycles depend on each other?

**Boundaries are too broad if:**
- Different teams own different parts
- Parts have independent read/update patterns
- Invariants only apply to subsets

**Boundaries are too narrow if:**
- Invariants span across multiple aggregates frequently
- Every operation requires multi-aggregate coordination

### One-to-One Relationships

| Case | Pattern | Reason |
|------|---------|--------|
| Value object in entity | Composition | Same lifecycle, immutable descriptor |
| Child entity in aggregate | Composition | Child cannot exist independently |
| Two separate aggregates | ID reference | Independent lifecycles, separate consistency boundaries |

---

## Validation Rules

### Structural Validation

1. **Every aggregate has exactly one root entity** - verified by design.
2. **No circular references between aggregates** - aggregates may reference by ID; no bidirectional nesting.
3. **All value objects are immutable** - no mutable fields; replacement, not mutation.
4. **Child entities are only referenced from parent aggregate** - no external references to child identities.
5. **Relationships are named from both directions (or justified)** - unidirectional relationships must document why reverse is unnecessary.

### Semantic Validation

6. **Every invariant is tied to an aggregate** - invariants protect consistency within boundaries.
7. **Every invariant is testable** - stated as verifiable conditions (not vague intent).
8. **Lifecycle stages are explicit** - entities must document their state transitions (e.g., DRAFT → PUBLISHED → ARCHIVED).
9. **Identity is immutable** - no entity can change its ID during its lifetime.
10. **Composition preserves atomicity** - child entities are updated atomically with parent.

### Completeness Validation

11. **Every field has a clear business meaning** - no pure infrastructure fields in domain model.
12. **Value object fields match their purpose** - Money has amount + currency; Address has all required postal components.
13. **Relationship cardinality is explicit** - one-to-one, one-to-many, many-to-many, or composition.
14. **Deletion/archival is specified** - how are entities removed (hard delete, soft delete, archive)?
15. **Cross-aggregate invariants are documented** - if invariants span aggregates, why aren't they grouped?

---

## Document Metadata

**Format**: Markdown with ASCII diagrams for structure, tables for relationships, and code fences for examples.

**Sections** (required for each domain model):
1. **Domain Overview** - High-level purpose; key business events or use cases
2. **Aggregates** - List each aggregate with root entity, children, and invariants
3. **Entities** - Identity definition, lifecycle stages, mutable fields
4. **Value Objects** - Immutable fields, validation rules, construction
5. **Relationships** - Cardinality, reference types, bidirectionality justification
6. **Invariants** - Business rules with plain-language descriptions and formal conditions
7. **Bounded Contexts** - If domain spans multiple contexts, define boundaries and integration points
8. **Glossary** - Define domain terms (e.g., "Order," "LineItem," "ShippingAddress")
9. **Open Questions** - Unresolved semantic issues for domain-builder or plan-domain-reviewer to clarify

> **Do not write illustrative walkthrough sections.** Example flows belong in the test suite, not the domain specification.

---

## Usage Example: Domain Planning Output

**File**: `plans/<feature-slug>/plan/domain-spec.md`

```markdown
# Domain Specification: Order Service

## Domain Overview
The order service manages customer orders from creation through fulfillment. 
Core events: OrderCreated, OrderConfirmed, OrderShipped, OrderDelivered, OrderCanceled.

## Aggregates

### Aggregate: Order
Root entity: `Order` (identity: order_id, UUID)

**Invariants:**
- Status transitions follow: PENDING → CONFIRMED → SHIPPED → DELIVERED
- Must have ≥ 1 LineItem
- total_amount = sum(line_items.unit_price × line_items.quantity)

**Children:**
- `LineItem` (entity, scoped to Order)
- `ShippingAddress` (value object)
- `Money` (value object for total_amount)

### Aggregate: Customer
Root entity: `Customer` (identity: customer_id, UUID)

**Fields:**
- name: string
- email: EmailAddress (value object)
- created_at: timestamp
- status: enum [ACTIVE, SUSPENDED, DELETED]

---

## Relationships

| From | To | Type | Cardinality | Reference |
|------|----|----|---|---|
| Order | Customer | Association | Many-to-One | customer_id |
| Order | LineItem | Composition | One-to-Many | (embedded) |
| Order | ShippingAddress | Composition | One-to-One | (embedded) |
| Customer | Order | (Reverse) | One-to-Many | Query by customer_id |

---

## Example: Creating an Order

**Input**: customer_id, list of {product_id, quantity}

**Steps**:
1. Validate customer_id exists (external aggregate check)
2. Create Order aggregate:
   - Set order_id = UUID
   - Set status = PENDING
   - For each {product_id, quantity}:
     - Fetch product (external aggregate)
     - Create LineItem with product_id, quantity, unit_price
     - Add to order.line_items
3. Calculate total_amount from line_items
4. Validate invariant: line_items.len ≥ 1
5. Store Order aggregate (atomic save)

**Result**: Order in PENDING state, ready for confirmation.

---
```

---

## Decision Log

- **Language Agnostic**: Use pseudocode or diagrams, not implementation syntax
- **Specification Only**: Define the domain model here; implement it later
- **Invariant Enforcement**: Document invariants here; enforce them in implementation
