---
name: rust-3-implement-function-sig-implementation
description: >
  Rust-specific patterns for implementing public interfaces and adapters. Teaches trait
  definitions, impl blocks, adapter patterns, boundary conversions, and pub visibility
  discipline. Use when implementing public APIs that match declared function signature
  contracts.
---

# Rust 3 Implement Function Sig Implementation

## Prerequisites

Use this skill after the contract is defined:

- Function signatures, error cases, and preconditions exist
- Traits are designed
- Boundary types such as adapters or DTOs are specified
- Public surface rules are known

It focuses on:

- Implementing traits from contracts
- Structuring `impl` blocks
- Adapting external boundary types
- Using `From`/`Into` at module boundaries
- Keeping `pub` visibility narrow

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### 1. Trait Definition and Implementation

Traits define contracts. Implementations provide the behavior behind them.
Use traits for both public interfaces and internal abstractions.

**Contract pattern**:
```rust
// Public interface trait
pub trait UserRepository {
    fn find_by_id(&self, id: UserId) -> Result<User, RepositoryError>;
    fn save(&mut self, user: &User) -> Result<(), RepositoryError>;
    fn delete(&mut self, id: UserId) -> Result<(), RepositoryError>;
}

// Implementation
pub struct InMemoryUserRepository {
    users: HashMap<UserId, User>,
}

impl UserRepository for InMemoryUserRepository {
    fn find_by_id(&self, id: UserId) -> Result<User, RepositoryError> {
        self.users.get(&id)
            .cloned()
            .ok_or(RepositoryError::NotFound)
    }

    fn save(&mut self, user: &User) -> Result<(), RepositoryError> {
        self.users.insert(user.id(), user.clone());
        Ok(())
    }

    fn delete(&mut self, id: UserId) -> Result<(), RepositoryError> {
        self.users.remove(&id)
            .ok_or(RepositoryError::NotFound)?;
        Ok(())
    }
}
```

**Contract principles**:
- Trait signatures are the public contract; implementations honor them exactly
- Error types are part of the contract (what can fail and how)
- Lifetime and generic parameters are part of the contract
- Preconditions and postconditions are documented in trait docs, not impl

### 2. Impl Block Structure for Clarity

Use multiple `impl` blocks to separate inherent methods from trait
implementations and to keep each trait implementation distinct.

**Organization pattern**:
```rust
// 1. Inherent methods (constructors, queries)
impl UserRepository {
    pub fn new() -> Self {
        UserRepository { users: HashMap::new() }
    }

    pub fn count(&self) -> usize {
        self.users.len()
    }
}

// 2. Trait implementations (UserRepository contract)
impl UserRepository for InMemoryUserRepository {
    fn find_by_id(&self, id: UserId) -> Result<User, RepositoryError> { /* */ }
    fn save(&mut self, user: &User) -> Result<(), RepositoryError> { /* */ }
    fn delete(&mut self, id: UserId) -> Result<(), RepositoryError> { /* */ }
}

// 3. Additional trait implementations (Debug, Clone, etc.)
impl Clone for InMemoryUserRepository {
    fn clone(&self) -> Self {
        InMemoryUserRepository {
            users: self.users.clone(),
        }
    }
}

impl Debug for InMemoryUserRepository {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("InMemoryUserRepository")
            .field("count", &self.users.len())
            .finish()
    }
}
```

**Key guidelines**:
- Inherent impl comes first (constructors, helpers)
- Trait impls follow, one per trait
- Group all trait impls or keep them separated by concern
- Consistent ordering aids navigation

### 3. Adapter Pattern for External Boundary Types

Adapters convert external types such as API, database, or message formats into
internal domain types. Keep them thin, usually through `From`, `Into`, or
`TryFrom`.

**Pattern**:
```rust
// External type (from HTTP library)
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub body: Option<String>,
}

// Internal command
pub struct CreateUserCommand {
    pub email: Email,
    pub password: String,
}

// Adapter: HttpRequest -> CreateUserCommand
impl TryFrom<HttpRequest> for CreateUserCommand {
    type Error = AdapterError;

    fn try_from(req: HttpRequest) -> Result<Self, Self::Error> {
        // Parse and validate the external representation
        let body = req.body.ok_or(AdapterError::MissingBody)?;
        let parsed: serde_json::Value = serde_json::from_str(&body)
            .map_err(AdapterError::InvalidJson)?;

        let email = Email::new(
            parsed["email"]
                .as_str()
                .ok_or(AdapterError::MissingField("email"))?
                .to_string()
        ).map_err(AdapterError::InvalidEmail)?;

        let password = parsed["password"]
            .as_str()
            .ok_or(AdapterError::MissingField("password"))?
            .to_string();

        Ok(CreateUserCommand { email, password })
    }
}

// Usage
let command = CreateUserCommand::try_from(http_req)?;
// Now command is validated and internal to domain
```

**Adapter placement**:
- Put external adapters in the interface or adapter layer
- Keep domain-to-domain adapters in the domain layer
- Make adapters one-way unless both directions are required

### 4. From/Into Conversions at Boundaries

`From` and `Into` provide standard conversions between types. Use them at
module boundaries to keep APIs ergonomic without leaking boundary details.

**Pattern**:
```rust
// From implementation: allows .into() or From::from()
impl From<Email> for UserDto {
    fn from(email: Email) -> Self {
        UserDto {
            email: email.as_str().to_string(),
        }
    }
}

// Usage
let email = Email::new("user@example.com".into()).unwrap();
let dto: UserDto = email.into();  // Automatic conversion via From

// Or explicitly
let dto2 = UserDto::from(email);

// In function signatures, Into trait allows flexibility
pub fn send_email<T: Into<Email>>(recipient: T) -> Result<(), Error> {
    let email = recipient.into();
    // ...
}

// Can be called with Email or String
send_email(Email::new("user@example.com".into())?)?;
send_email("other@example.com".into())?;  // Automatic into Email
```

**Boundary conversion rules**:
- Implement `From` for deterministic conversions (always succeed)
- Implement `TryFrom` for fallible conversions (might fail)
- Place conversions at layer boundaries (interface → domain, domain → persistence)
- Use `Into` in function parameters to accept multiple types ergonomically

### 5. Public (`pub`) Surface Discipline

Make only necessary types and functions `pub`; leave everything else private.
This keeps the API small and avoids accidental coupling.

**Discipline pattern**:
```rust
// lib/interface/user_api.rs

// PUBLIC: Main API
pub struct UserApi { /* */ }

// PUBLIC: Error type users need to handle
pub enum UserApiError {
    NotFound,
    ValidationFailed(String),
}

// PUBLIC: Data transfer object for response
pub struct UserResponse {
    pub id: String,
    pub email: String,
}

impl UserApi {
    // PUBLIC: Constructor
    pub fn new(repo: Arc<dyn UserRepository>) -> Self {
        // ...
    }

    // PUBLIC: Main operation
    pub async fn get_user(&self, id: &str) -> Result<UserResponse, UserApiError> {
        // ...
    }
}

// PRIVATE: Helper function not part of the API
fn parse_user_id(id_str: &str) -> Result<UserId, UserApiError> {
    // ...
}

// PRIVATE: Internal type used by API
struct UserQuery {
    id: UserId,
}

// PRIVATE: Internal result
struct InternalUserResult {
    user: User,
}

impl UserApi {
    // PRIVATE: Helper method
    fn query_user(&self, query: UserQuery) -> Result<InternalUserResult, UserApiError> {
        // ...
    }
}
```

**Visibility guidelines**:
- `pub`: Main API, types callers need, error types
- `pub(crate)`: Internal to crate, used by other modules
- `pub(in path)`: Specific module visibility
- Private (default): Implementation details, not part of API contract

## Examples

### Example 1: Simple Trait Implementation

**Scenario**: Implement `UserRepository` trait for in-memory storage

**Contract**:
```
Trait: UserRepository
- find_by_id(id: UserId) -> Result<User, RepositoryError>
  - Returns: User if found
  - Error: NotFound if id not in repository
- save(user: &User) -> Result<(), RepositoryError>
  - Effect: Stores or updates user
  - Error: DuplicateEmail if email already exists
- delete(id: UserId) -> Result<(), RepositoryError>
  - Effect: Removes user from repository
  - Error: NotFound if id not in repository
```

**Implementation**:
```rust
pub struct InMemoryUserRepository {
    users: HashMap<UserId, User>,
}

impl InMemoryUserRepository {
    pub fn new() -> Self {
        InMemoryUserRepository {
            users: HashMap::new(),
        }
    }
}

impl UserRepository for InMemoryUserRepository {
    fn find_by_id(&self, id: UserId) -> Result<User, RepositoryError> {
        self.users.get(&id)
            .cloned()
            .ok_or(RepositoryError::NotFound)
    }

    fn save(&mut self, user: &User) -> Result<(), RepositoryError> {
        // Check for duplicate email
        if self.users.values()
            .any(|u| u.email() == user.email() && u.id() != user.id())
        {
            return Err(RepositoryError::DuplicateEmail);
        }

        self.users.insert(user.id(), user.clone());
        Ok(())
    }

    fn delete(&mut self, id: UserId) -> Result<(), RepositoryError> {
        self.users.remove(&id)
            .ok_or(RepositoryError::NotFound)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_returns_saved_user() {
        let mut repo = InMemoryUserRepository::new();
        let user = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
        repo.save(&user).unwrap();

        let found = repo.find_by_id(UserId::new(1)).unwrap();
        assert_eq!(found.id(), UserId::new(1));
    }

    #[test]
    fn test_find_not_found() {
        let repo = InMemoryUserRepository::new();
        let result = repo.find_by_id(UserId::new(999));
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[test]
    fn test_save_duplicate_email_fails() {
        let mut repo = InMemoryUserRepository::new();
        let user1 = User::new(UserId::new(1), Email::new("test@example.com".into()).unwrap());
        let user2 = User::new(UserId::new(2), Email::new("test@example.com".into()).unwrap());

        repo.save(&user1).unwrap();
        let result = repo.save(&user2);
        assert!(matches!(result, Err(RepositoryError::DuplicateEmail)));
    }
}
```

**Valid pattern**: All trait methods implemented, contracts honored exactly,
tests verify contract behavior.

### Example 2: Adapter for External Type

**Scenario**: Adapt JSON request to domain command

**Implementation**:
```rust
// External representation (from HTTP)
#[derive(serde::Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
}

// Internal domain command
pub struct CreateUserCommand {
    pub email: Email,
    pub password: String,
}

// Adapter
impl TryFrom<CreateUserRequest> for CreateUserCommand {
    type Error = AdapterError;

    fn try_from(req: CreateUserRequest) -> Result<Self, Self::Error> {
        let email = Email::new(req.email)
            .map_err(|e| AdapterError::InvalidEmail(format!("{:?}", e)))?;

        // Validate password
        if req.password.len() < 8 {
            return Err(AdapterError::WeakPassword);
        }

        Ok(CreateUserCommand {
            email,
            password: req.password,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_request_converts() {
        let req = CreateUserRequest {
            email: "user@example.com".to_string(),
            password: "securepass123".to_string(),
        };

        let cmd = CreateUserCommand::try_from(req).unwrap();
        assert_eq!(cmd.email.as_str(), "user@example.com");
    }

    #[test]
    fn test_weak_password_rejected() {
        let req = CreateUserRequest {
            email: "user@example.com".to_string(),
            password: "weak".to_string(),
        };

        let result = CreateUserCommand::try_from(req);
        assert!(matches!(result, Err(AdapterError::WeakPassword)));
    }
}
```

**Valid pattern**: Adapter validates external input, converts to domain types,
error handling at boundary.

### Example 3: Visibility Discipline

**Scenario**: Public API with private implementation details

**Implementation**:
```rust
// lib/interface/mod.rs

// PUBLIC: Main API
pub struct ItemService {
    repo: Arc<dyn ItemRepository>,
}

// PUBLIC: Error type
pub enum ItemServiceError {
    NotFound,
    ValidationFailed(String),
    RepositoryError(String),
}

// PUBLIC: Response DTO
pub struct ItemDto {
    pub id: String,
    pub name: String,
    pub price: String,
}

impl ItemService {
    // PUBLIC: Constructor
    pub fn new(repo: Arc<dyn ItemRepository>) -> Self {
        ItemService { repo }
    }

    // PUBLIC: Main operation
    pub fn get_item(&self, id: &str) -> Result<ItemDto, ItemServiceError> {
        let item_id = parse_item_id(id)?;
        let item = self.repo.find(&item_id)?;
        Ok(item_to_dto(&item))
    }
}

// PRIVATE: Not part of public API
fn parse_item_id(id_str: &str) -> Result<ItemId, ItemServiceError> {
    ItemId::try_from(id_str)
        .map_err(|_| ItemServiceError::ValidationFailed("Invalid item ID".into()))
}

// PRIVATE: Adapter function
fn item_to_dto(item: &Item) -> ItemDto {
    ItemDto {
        id: item.id().to_string(),
        name: item.name().to_string(),
        price: item.price().as_string(),
    }
}

// PRIVATE: Internal result type
struct ItemQueryResult {
    item: Item,
    metadata: QueryMetadata,
}
```

**Valid pattern**: Public surface is minimal and clear (ItemService, ItemDto,
ItemServiceError); implementation details are private.

## Tool Integration

### 1. Cargo Build and Check

Verify trait implementations compile:
```sh
cargo check --lib
cargo build --lib
```

### 2. Clippy for API Design

Check for public API issues:
```sh
cargo clippy --lib -- -W clippy::all
```

Watch for:
- Missing trait derives that should be public (Clone, Debug)
- Unnecessary pub on internal types
- Function complexity in public API

### 3. Cargo Doc for Public API Review

Generate and review public documentation:
```sh
cargo doc --lib --no-deps --open
```

Review:
- All public types have doc comments
- Error types are documented
- Trait methods have example usage

### 4. Sig Report for Interface Compliance

Use sig-report to verify public signature matches contract:
```sh
.github/skills/0-external-sig-report/run.sh --snapshot provided:<rustdoc-json> --function-signatures --output-format json
```

## Decision Criteria

### Implementation checks

Use these criteria when implementing interfaces:

1. **Contract Compliance**: All trait methods implement exactly as signed
2. **Error Handling**: Error types match contract; failures return correct variants
3. **Adapter Coverage**: All external types have adapters at boundaries
4. **Conversion Correctness**: `From`/`Into` work correctly for all conversions
5. **Visibility Correctness**: Only public what's part of the contract surface

### Review checks

Use these criteria when reviewing implementations:

1. **Signature Match**: Impl methods match trait signatures exactly
2. **Contract Behavior**: Methods produce documented behavior
3. **Error Coverage**: All error cases handled and returned correctly
4. **Adapter Validation**: External types converted correctly
5. **Surface Cleanliness**: No implementation details leaked to public API
