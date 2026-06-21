---
name: rust-3-implement-domain-implementation
description: >
  Rust-specific patterns for implementing domain types and value objects. Teaches struct
  and enum design, newtype wrappers, impl block structure, domain invariant enforcement,
  and pure _ops.rs companion modules. Use when building the domain layer from a domain
  model.
---

# Rust 3 Implement Domain Implementation

## Use This Skill When

- the domain model, invariants, and core rules are already defined
- you need Rust `struct` and `enum` types for entities and value objects
- you need newtypes for domain primitives
- you need clear `impl` structure and constructor-based invariant checks
- you need pure domain logic organized in `_ops.rs` companion modules

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### 1. Struct and Enum for Domain Types

**What it is**: Rust `struct` types represent entities and value objects.
`enum` types represent domain choices and state variants.

**Entities** (mutable, identity, lifecycle):
```rust
pub struct User {
    pub id: UserId,
    pub email: Email,
    pub status: UserStatus,
    created_at: Timestamp,
}

pub enum UserStatus {
    Active,
    Inactive,
    Suspended { reason: String },
}
```

**Value Objects** (immutable, no identity, defined by value):
```rust
pub struct Money {
    amount: i64,  // Cents, not dollars
    currency: Currency,
}

pub struct Email {
    address: String,  // Validated
}

pub enum Currency {
    Usd,
    Eur,
    Gbp,
}
```

**Key discipline**:
- Entities have identity (e.g., `UserId`); value objects don't
- Value objects are immutable; their fields are private and validated
- Use `enum` for closed domain variants (not open strings)
- Use `struct` when the domain concept is a "thing" with properties

### 2. Newtype Wrapper Pattern for Primitives

**What it is**: Newtype wrapping creates semantic types from primitives,
preventing accidental misuse. No bare `f64`, `String`, or `u32` at domain boundaries.
For single-field wrappers that cross serialization boundaries, use
`#[serde(transparent)]` so the wrapper preserves the inner wire format.

**Pattern**:
```rust
pub struct UserId(u64);
pub struct Email(String);
pub struct Price(Money);
pub struct Percentage(f64);
```

**Why it matters**:
- Type safety: `fn process_user(id: UserId)` cannot accept `Price` by mistake
- Semantic clarity: `Price` vs `f64` immediately signals intent
- Invalid states made impossible: `Email` constructor validates format

**Constructor pattern**:
```rust
impl Email {
    pub fn new(address: String) -> Result<Self, EmailError> {
        // Validate format
        if !address.contains('@') {
            return Err(EmailError::InvalidFormat);
        }
        Ok(Email(address))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Usage
let email = Email::new("user@example.com".to_string())?;
// email.0 is private; must use as_str() getter
```

**Guideline**: For every primitive that has domain meaning, create a newtype.
Common domain newtypes:
- IDs: `UserId`, `OrderId`, `ItemId`
- Money: `Price`, `Balance`, `Amount`
- Strings with constraints: `Email`, `PhoneNumber`, `Username`
- Decimals with constraints: `Percentage`, `Probability`, `Score`

### 3. Impl Block Structure

**What it is**: `impl` blocks organize associated functions and methods.
Structure them by responsibility for clarity.

**Organization pattern**:
```rust
impl User {
    // Constructors and factories
    pub fn new(email: Email) -> Result<Self, UserError> { /* */ }
    pub fn from_signup(request: SignupRequest) -> Result<Self, UserError> { /* */ }
    pub fn deleted() -> Self { /* */ }

    // Getters (minimal, only when needed)
    pub fn id(&self) -> UserId { self.id }
    pub fn email(&self) -> &Email { &self.email }

    // Domain operations (core logic)
    pub fn change_email(&mut self, new_email: Email) -> Result<(), UserError> { /* */ }
    pub fn suspend(&mut self, reason: String) { /* */ }
    pub fn reactivate(&mut self) -> Result<(), UserError> { /* */ }

    // Queries (derived properties, read-only)
    pub fn is_active(&self) -> bool { self.status == UserStatus::Active }
    pub fn is_suspended(&self) -> bool { matches!(self.status, UserStatus::Suspended { .. }) }

    // Conversions (to other types)
    pub fn to_dto(&self) -> UserDto { /* */ }
}
```

**Key guidelines**:
- Group by responsibility (constructors, operations, queries, conversions)
- Keep pure domain logic in methods (no I/O)
- Use `&self` for queries, `&mut self` for operations (mutable signals mutation)
- Avoid getters for all fields; only expose what domain logic needs

### 4. Domain Invariant Enforcement in Constructors

**What it is**: Constructors validate invariants upfront, making invalid states
unrepresentable. Once constructed, a domain object is guaranteed valid.

**Pattern**:
```rust
pub struct Order {
    id: OrderId,
    items: Vec<LineItem>,  // Must be non-empty
    status: OrderStatus,
    total: Money,
}

impl Order {
    pub fn new(id: OrderId, items: Vec<LineItem>) -> Result<Self, OrderError> {
        // Invariant: items must not be empty
        if items.is_empty() {
            return Err(OrderError::NoItems);
        }

        // Invariant: all items must have positive quantity
        if items.iter().any(|item| item.quantity <= 0) {
            return Err(OrderError::InvalidQuantity);
        }

        // Invariant: total must match sum of line items
        let total = items.iter()
            .map(|item| item.line_total())
            .fold(Money::zero(), |acc, amt| acc + amt);

        Ok(Order {
            id,
            items,
            status: OrderStatus::Draft,
            total,
        })
    }

    // Invariant maintained: items list never modified externally
    pub fn items(&self) -> &[LineItem] {
        &self.items
    }
}
```

**Invariant categories**:
- **Structural**: Items non-empty, lists sorted
- **Value constraints**: Price non-negative, email valid format
- **Relationship**: Total equals sum of line items
- **State machine**: Only valid status transitions

**Enforcement strategy**:
- Constructor validates all upfront invariants
- Private fields prevent external mutation
- Operations document and maintain invariants
- Type system encodes what's possible (invalid states = compile error)

### 5. Pure Domain Logic in `_ops.rs` Companion Modules

**What it is**: Complex domain logic (calculations, rule engines, algorithms)
is organized in `_ops.rs` modules alongside the main type. This separates
pure logic from the type definition, making logic testable and reusable.

**Structure**:
```rust
// lib/domain/order.rs
pub struct Order { /* */ }

impl Order {
    pub fn new(id: OrderId, items: Vec<LineItem>) -> Result<Self, OrderError> { /* */ }
    pub fn apply_discount(&mut self, discount: Money) { /* */ }
    // ... other methods
}

// lib/domain/order_ops.rs
pub fn calculate_tax(subtotal: Money, region: &Region) -> Money {
    // Pure logic, no I/O, no type mutation
    let rate = region.tax_rate();
    subtotal * rate
}

pub fn determine_shipping_cost(weight: Weight, destination: &Address) -> Result<Money, ShippingError> {
    // Pure function: same inputs always produce same output
}

pub fn should_offer_loyalty_discount(order: &Order) -> bool {
    // Query logic isolated
    order.items().len() >= 5 && order.total > Money::from(100_00)
}
```

**Usage in operations**:
```rust
impl Order {
    pub fn finalize(&mut self, region: &Region) -> Result<(), OrderError> {
        let tax = order_ops::calculate_tax(self.total, region);
        self.tax_amount = tax;

        let shipping = order_ops::determine_shipping_cost(self.weight, &self.destination)?;
        self.shipping = shipping;

        if order_ops::should_offer_loyalty_discount(self) {
            // Apply discount
        }

        self.status = OrderStatus::Finalized;
        Ok(())
    }
}
```

**Why separate into `_ops.rs`**:
- Pure logic is easier to test (no state setup needed)
- Calculations can be reused by multiple types
- Clear separation: type structure vs. domain algorithms
- Supports business rule engines without coupling to the type

## Examples

### Example 1: Simple Value Object with Validation

**Scenario**: Implement `Email` value object with format validation

**Domain Model**:
```
Value Object: Email
- Address: string, must contain '@' and be max 254 chars
- Immutable
- No identity (two emails with same address are equal)
```

**Implementation**:
```rust
// domain/email.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Email {
    address: String,
}

#[derive(Debug)]
pub enum EmailError {
    Missing,
    InvalidFormat,
    TooLong,
}

impl Email {
    pub fn new(address: String) -> Result<Self, EmailError> {
        // Invariant: not empty
        if address.is_empty() {
            return Err(EmailError::Missing);
        }

        // Invariant: max 254 chars
        if address.len() > 254 {
            return Err(EmailError::TooLong);
        }

        // Invariant: contains @
        if !address.contains('@') {
            return Err(EmailError::InvalidFormat);
        }

        // Basic format check
        let parts: Vec<&str> = address.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(EmailError::InvalidFormat);
        }

        Ok(Email { address })
    }

    pub fn as_str(&self) -> &str {
        &self.address
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_email() {
        let email = Email::new("user@example.com".to_string()).unwrap();
        assert_eq!(email.as_str(), "user@example.com");
    }

    #[test]
    fn test_invalid_missing_at() {
        let result = Email::new("userexample.com".to_string());
        assert!(matches!(result, Err(EmailError::InvalidFormat)));
    }

    #[test]
    fn test_empty_address() {
        let result = Email::new(String::new());
        assert!(matches!(result, Err(EmailError::Missing)));
    }
}
```

**Valid pattern**: Newtype wrapper with private field, constructor validates
all invariants, immutable after construction, getter provides controlled access.

### Example 2: Entity with State Machine

**Scenario**: Implement `User` entity with status lifecycle

**Domain Model**:
```
Entity: User
- ID: UserId (newtype on u64)
- Email: Email (value object)
- Status: Active, Inactive, Suspended
- Lifecycle: Active -> Inactive or Suspended only, Inactive <-> Active,
  Suspended -> Active requires reason cleared
```

**Implementation**:
```rust
pub struct User {
    id: UserId,
    email: Email,
    status: UserStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
}

#[derive(Debug)]
pub enum UserError {
    InvalidTransition,
}

impl User {
    pub fn new(id: UserId, email: Email) -> Self {
        User {
            id,
            email,
            status: UserStatus::Active,
        }
    }

    // Domain operations: enforce state machine
    pub fn deactivate(&mut self) {
        match self.status {
            UserStatus::Active => self.status = UserStatus::Inactive,
            UserStatus::Inactive => {} // Already inactive
            UserStatus::Suspended => self.status = UserStatus::Inactive,
        }
    }

    pub fn reactivate(&mut self) -> Result<(), UserError> {
        match self.status {
            UserStatus::Active => Ok(()),  // Already active
            UserStatus::Inactive => {
                self.status = UserStatus::Active;
                Ok(())
            }
            UserStatus::Suspended => Err(UserError::InvalidTransition),
        }
    }

    pub fn suspend(&mut self) -> Result<(), UserError> {
        match self.status {
            UserStatus::Active => {
                self.status = UserStatus::Suspended;
                Ok(())
            }
            _ => Err(UserError::InvalidTransition),
        }
    }

    // Query methods
    pub fn is_active(&self) -> bool {
        self.status == UserStatus::Active
    }

    pub fn id(&self) -> UserId {
        self.id
    }

    pub fn email(&self) -> &Email {
        &self.email
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_user_is_active() {
        let user = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
        assert!(user.is_active());
    }

    #[test]
    fn test_deactivate_then_reactivate() {
        let mut user = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
        user.deactivate();
        assert!(!user.is_active());

        user.reactivate().unwrap();
        assert!(user.is_active());
    }

    #[test]
    fn test_cannot_reactivate_suspended() {
        let mut user = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
        user.suspend().unwrap();
        let result = user.reactivate();
        assert!(matches!(result, Err(UserError::InvalidTransition)));
    }
}
```

**Valid pattern**: State machine enforced by operations, invariants maintained,
invalid transitions caught at runtime with error, test coverage for all paths.

### Example 3: Domain Logic in `_ops.rs`

**Scenario**: Order pricing with tax and discount calculations

**Files**:
```
domain/
  order.rs          (Order struct, impl, constructors)
  order_ops.rs      (Pure pricing logic)
```

**Implementation**:
```rust
// domain/order.rs
pub struct Order {
    id: OrderId,
    items: Vec<LineItem>,
    subtotal: Money,
    tax: Money,
    discount: Money,
    total: Money,
}

impl Order {
    pub fn new(id: OrderId, items: Vec<LineItem>) -> Result<Self, OrderError> {
        if items.is_empty() {
            return Err(OrderError::NoItems);
        }

        let subtotal = items.iter()
            .map(|item| item.line_total())
            .fold(Money::zero(), |acc, amt| acc + amt);

        Ok(Order {
            id,
            items,
            subtotal,
            tax: Money::zero(),
            discount: Money::zero(),
            total: subtotal,
        })
    }

    pub fn apply_tax_and_shipping(&mut self, tax_rate: f64, shipping: Money) -> Result<(), OrderError> {
        self.tax = order_ops::calculate_tax(self.subtotal, tax_rate);
        self.total = self.subtotal + self.tax + shipping;
        Ok(())
    }

    pub fn apply_discount(&mut self, percentage: u32) -> Result<(), OrderError> {
        if percentage > 100 {
            return Err(OrderError::InvalidDiscount);
        }
        self.discount = order_ops::calculate_discount_amount(self.subtotal, percentage);
        self.total = self.subtotal + self.tax - self.discount;
        Ok(())
    }

    pub fn total(&self) -> Money {
        self.total
    }
}

// domain/order_ops.rs
pub fn calculate_tax(subtotal: Money, tax_rate: f64) -> Money {
    (subtotal.as_cents() as f64 * tax_rate / 100.0).round() as i64
        |> Money::from_cents
}

pub fn calculate_discount_amount(subtotal: Money, percentage: u32) -> Money {
    (subtotal.as_cents() as f64 * percentage as f64 / 100.0).round() as i64
        |> Money::from_cents
}

pub fn qualifies_for_bulk_discount(item_count: usize) -> bool {
    item_count >= 10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_tax() {
        let subtotal = Money::from(100_00);
        let tax = calculate_tax(subtotal, 10.0);
        assert_eq!(tax.as_cents(), 10_00);
    }

    #[test]
    fn test_calculate_discount() {
        let subtotal = Money::from(100_00);
        let discount = calculate_discount_amount(subtotal, 20);
        assert_eq!(discount.as_cents(), 20_00);
    }
}
```

**Valid pattern**: Pure logic isolated in `_ops.rs`, easily testable without
Order state, reusable by other types if needed.

## Tool Integration

### 1. Testing Domain Types

Test for invariant enforcement:
```sh
cargo test --lib domain::  # Run all domain tests
```

### 2. Clippy for Type Correctness

Check for common domain modeling mistakes:
```sh
cargo clippy --lib -- -W clippy::all
```

Watch for:
- Unimplemented traits (missing Debug, Clone, etc.)
- Public fields that should be private
- Unnecessary cloning in impl methods

### 3. Code Coverage for Domain Logic

Run with coverage to ensure domain operations are tested:
```sh
cargo tarpaulin --lib --out Html --output-dir reports
```

## Decision Criteria

### Implementation

Use these criteria when implementing domain types:

1. **Newtype Coverage**: Every domain primitive has a newtype (no bare `u64`, `String`)
2. **Serde Transparency**: Single-field serialized wrappers preserve wire format
3. **Invariant Validation**: All invariants checked in constructor
4. **Private Fields**: Domain types prevent external mutation
5. **Operation Safety**: Methods maintain invariants or return errors for invalid transitions
6. **Logic Organization**: Complex pure logic in `_ops.rs` modules

### Review

Use these criteria when reviewing domain implementations:

1. **Semantic Type Correctness**: Newtypes prevent accidental misuse
2. **Invariant Enforcement**: Constructors and operations enforce all documented rules
3. **Encapsulation**: Fields are private except when design requires public (rare)
4. **Operation Completeness**: All domain operations from model are implemented
5. **Test Coverage**: Unit tests cover happy paths and error paths for invariant violations
