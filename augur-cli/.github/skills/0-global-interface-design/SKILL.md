---
name: 0-global-interface-design
description: >
  Function and method interface design: contract clarity, type boundary discipline,
  parameter bundling, temporal coupling elimination, and input validation contracts. Use
  when designing or reviewing function signatures, trait methods, or API entry points
  to ensure explicit contracts and clean encapsulation.
---

# 0-global-interface-design

## Key Concepts

### 1. Interface Contract

**Characteristics**:
- Defines what callers may assume, not how the code works
- Written in terms of inputs, outputs, preconditions, postconditions, and invariants
- Stable across implementation changes that preserve the contract
- Complete enough that callers need not read the implementation to use it correctly

**Contract Incompleteness Anti-patterns**:
- "Call function A before function B" (temporal coupling; should be one function or explicit ordering in the signature)
- "Global state must be initialized first" (should be passed as parameter or owned by a struct)
- "This works only if you're on the main thread" (should be enforced by type system or explicit parameter)
- "The result is valid only if you check this flag first" (should return wrapped type that enforces checking)

### 2. Type Boundary

**Characteristics**:
- Public boundary types are **stable** and change only when the interface changes
- Internal types are **private** and may change without affecting callers
- Internal implementation types should not leak into return values or error types
- Domain model types (entities, value objects) are typically public; data transfer objects and internal caches are private

**Type Boundary Violations**:
- Returning an internal struct that callers can inspect (breaks encapsulation; should return only what contract promises)
- Accepting a public type but silently converting it to an internal representation (accept the internal type or declare the conversion in the contract)
- Exception/error types that expose internal implementation details (should map internal errors to public error enum)

**Example** (language-agnostic):
```
✓ CORRECT: Interface accepts (UserId, Email) → returns Result<User, UserError>
  - Callers may create UserId and Email (public types)
  - Result and UserError are specified in contract
  - Callers need not know about internal _PasswordHash type
  
✗ WRONG: Interface accepts (UserId, Email) → returns (User, _PasswordHash)
  - _PasswordHash is internal; callers shouldn't see or construct it
  - Violates type boundary
```

### 3. Method Contract Specification

**Components**:

1. **Precondition**: State or value constraints that must be true before the method is called
   ```
   Example: "receiver must be in [Idle, Waiting] state"
   Example: "count parameter must be > 0"
   ```

2. **Postcondition**: State or value changes that will be true after successful execution
   ```
   Example: "receiver transitions to [Ready] state"
   Example: "return value is in range [0, 1000)"
   ```

3. **Invariant**: Constraints that remain true before, during, and after method execution
   ```
   Example: "balance is always >= 0"
   Example: "item list is always sorted by timestamp"
   ```

4. **Side Effects**: Observable external actions (I/O, state mutations, network calls)
   ```
   Example: "writes to log file"
   Example: "mutates receiver.cache"
   Example: "sends message to external service"
   ```

5. **Failure Modes**: Explicit conditions under which the method fails and how failure is reported
   ```
   Example: "returns error if count < 0"
   Example: "returns empty list if no items found (not an error)"
   Example: "panics if file I/O fails"
   ```

**Contract Completeness Checklist**:
- [ ] All parameters have documented meaning (not vague like "config object")
- [ ] All parameter constraints are explicit (what values are valid? what combinations?)
- [ ] Return type is fully specified (what does it contain? what does null/empty mean?)
- [ ] All side effects are declared (what external systems are touched? what state changes?)
- [ ] Failure modes are explicit (when can this method fail? how is failure reported?)
- [ ] Preconditions are either enforceable by type system or listed (not hidden)
- [ ] Postconditions are observable (caller can verify they happened)

### 4. Parameter Bundling and Composition

**Principle**: Functions should have **≤3 explicit parameters**. When more parameters are needed, bundle related parameters into a struct/record with a meaningful name.

**Anti-pattern (too many parameters)**:
```
// Discouraged
create_user(String name, String email, Date birthDate, String country, 
            String timezone, Role role, bool emailVerified, bool active)
```

**Pattern (bundled)**:
```
// Encouraged
create_user(CreateUserCommand cmd)
  where CreateUserCommand contains: 
    name, email, birthDate, country, timezone, role, emailVerified, active
```

**Exception**: When all parameters are of different, semantically distinct types and few (< 3), bundling may be premature. Clarity takes precedence over rule-following.

### 5. Public vs. Implementation Detail

**Rule**: If something is **not written in the contract**, callers **must not depend on it**.

**Public Interface (contract)** includes:
- Function name and visibility level
- Parameter list (names, types, constraints)
- Return type and meaning
- Declared side effects (e.g., "writes to log")
- Declared failure modes and how they're reported
- Invariants and postconditions

**Implementation Detail** includes:
- How the return value is computed (algorithm)
- Internal data structures used
- Order of operations (unless order is part of contract)
- Performance characteristics (unless guaranteed by contract)
- Internal error types or stack traces (these are private; public errors must map internal to public)
- Temporary files, caches, or internal state

**Anti-pattern (leaking implementation detail)**:
```
✗ "Returns a Vec with internal allocation strategy details exposed"
✗ "May throw SQLException (database-specific error type)"
✗ "Caches result internally; returns different object on repeated call"
✗ "Order of elements depends on hash table iteration (internal detail)"
```

**Pattern (pure contract)**:
```
✓ "Returns a list of items matching the filter"
✓ "May return Error::NotFound if item doesn't exist"
✓ "Always returns consistent order: sorted by creation date"
```

### 6. Temporal Coupling

**Anti-pattern (temporal coupling)**:
```
reservoir = new Reservoir()
reservoir.set_config(config)     // Must call before fill()
reservoir.fill_water(amount)     // Depends on config being set
reservoir.open_drain()

// If caller forgets set_config(), fill_water() fails silently or crashes
// Type system doesn't prevent this; it's a hidden contract
```

**Pattern (eliminate coupling via constructor)**:
```
reservoir = new Reservoir(config)  // Config required at creation
reservoir.fill_water(amount)       // Config is guaranteed present
reservoir.open_drain()

// Type system enforces that config must be provided; no surprise
```

**Detection Checklist**:
- [ ] Does any function assume another function was called before it?
- [ ] Does any function assume global or instance state was initialized?
- [ ] Is there an undocumented "call order" that callers must remember?
- [ ] Could a caller accidentally call functions in the wrong order and get a confusing error?

If yes to any, eliminate the coupling by:
1. **Composition**: Pass the required state as a parameter instead of assuming it was set up
2. **Constructor enforcement**: Make dependencies required parameters of a struct/class constructor
3. **Type system gating**: Use types to make invalid states unrepresentable (e.g., `Result<T, E>` prevents use of T without checking)

### 7. Input Parameter Validation

**Validation Timing** (in order of preference):
1. **Compile-time**: Type system prevents invalid values (e.g., `NonNegativeInteger` instead of `i32`)
2. **Entry point**: Check before any internal work; fail fast
3. **Layer boundary**: Check when data crosses domain boundaries (e.g., REST endpoint → domain logic)
4. **Never implicit**: Don't silently coerce invalid input (e.g., if you need an email, don't accept any `String`)

**Validation Contract** includes:
- What ranges/formats are valid (e.g., "email must match RFC 5322", "count must be 1-1000")
- What happens if invalid (e.g., "returns ValidationError", "panics", "defaults to X")
- Who is responsible for validation (caller or callee?)

**Anti-pattern (no validation contract)**:
```
function set_timeout(value: number)
// What if value is negative? Null? Float?
// Caller has no idea; assumes anything goes or has to read implementation
```

**Pattern (explicit validation)**:
```
function set_timeout(value: PositiveInteger)
// Type system ensures value is positive; no runtime check needed
// Caller knows invalid values cannot be constructed
```

Or if validation is runtime:
```
function set_timeout(value: number) -> Result<(), TimeoutError>
// Returns error if value is invalid
// Caller knows to check Result; error type is explicit
```

## Key Files

- `README.md` - overview and usage notes

## Examples

### Example 1: Poorly Designed Function Signature

```
// Anti-pattern: Hidden coupling, no contract, missing validation
function process(obj, flag, cb)
  // obj: could be anything
  // flag: boolean for what?
  // cb: callback for what? When is it called? What happens if it throws?
  // Are there preconditions? Side effects? Failure modes?
```

**Problems**:
- No parameter meaning
- No documentation of what is valid
- Temporal coupling: caller may not know if obj must be pre-initialized
- Callback contract unknown: when is it called, and what happens on failure?
- Type system cannot help

### Example 1: Improved

```
// Pattern: Clear contract, type-based validation, composition
function process(request: ProcessRequest) -> Result<ProcessResult, ProcessError>
where ProcessRequest = {
  input: ValidatedInput,  // Type ensures input meets constraints
  retryPolicy: RetryPolicy,  // Explicit control, not hidden boolean
  onProgress: ProgressCallback  // Named callback with signature
}

// Precondition: None (struct constructor enforces valid state)
// Postcondition: Either returns ProcessResult or ProcessError (not null/exception)
// Side effects: Calls onProgress callback during processing
// Failure modes: Returns ProcessError::InvalidInput, ProcessError::Timeout, etc.
```

**Improvements**:
- Each parameter has a meaningful name
- Request struct can evolve without breaking call sites
- Types enforce validity (ValidatedInput, RetryPolicy)
- Return type forces caller to handle both success and error
- Callback is explicit and named

---

### Example 2: Type Boundary Violation

```
// Anti-pattern: Internal types leak into public interface
type User = {
  id: UserId,
  email: Email,
  _passwordHash: PasswordHash,  // Private detail exposed
  _internalState: InternalState  // Caller should never see this
}

function get_user(id: UserId) -> User
  // Caller receives User with private fields
  // Temptation to inspect _passwordHash or _internalState
  // If internal representation changes, all code breaks
```

### Example 2: Improved

```
// Pattern: Public API type hides implementation
type User = {
  id: UserId,
  email: Email,
  created_at: Timestamp
  // Private fields NOT included in public type
}

function get_user(id: UserId) -> Result<User, UserError>
  // Returns only what contract promises
  // Internal _passwordHash and _internalState are hidden
  // Internal implementation can change without affecting callers
```

---

### Example 3: Temporal Coupling

```
// Anti-pattern: Hidden initialization order requirement
struct Connection {
  hostname: string
}

function open_connection(conn: Connection) -> void
  // Must set conn.hostname before calling this
  // Precondition is undocumented; caller must guess

function send_message(conn: Connection, msg: string) -> void
  // Assumes open_connection() was called
  // Will fail confusingly if not

// Caller's code:
conn = new Connection()
// Oops! Forgot to set hostname
send_message(conn, "hello")  // Fails with cryptic error
```

### Example 3: Improved

```
// Pattern: Dependencies expressed in constructor
struct Connection {
  hostname: string  // Required, not optional
}

function Connection::new(hostname: string) -> Connection {
  // Hostname must be provided; cannot construct without it
  return Connection { hostname }
}

function open_connection(conn: &Connection) -> Result<(), ConnectionError> {
  // Works with Connection that has hostname
}

// Caller's code:
conn = Connection::new("example.com")  // Hostname required at creation
open_connection(&conn)  // Type ensures conn is valid
send_message(&conn, "hello")  // No surprise; conn is ready
```

---

## Decision Criteria

### When to Bundle Parameters

**Bundle into a struct when**:
- Function has > 3 explicit parameters
- Parameters form a semantic unit (e.g., all related to "user creation" or "retry logic")
- Bundle reduces caller cognitive load (named structure is more understandable than parameter list)
- New features are likely to add fields to the bundle

**Don't bundle when**:
- Parameters are truly independent and small count (≤ 3 of different types)
- Bundling would create a one-off struct used nowhere else
- Clarity actually decreases (force-bundling a single int parameter is not better)

### When to Expose Internal Type vs. Hide It

**Expose (public interface type) when**:
- Type represents a domain concept that callers care about (e.g., User, Order, Account)
- Type is stable (contracts with callers depend on its structure)
- Callers may construct, store, or pass the type around

**Hide (implementation detail) when**:
- Type is internal infrastructure (e.g., database connection, cache, buffer)
- Type is temporary (e.g., intermediate computation result)
- Type must change freely as implementation evolves
- Callers should not construct or inspect it

### When to Require vs. Optional Parameters

**Required parameters** (no default):
- Caller MUST provide (type system or validation enforces)
- Contract is clearer: if the parameter matters, make it required

**Optional parameters** (has default):
- Caller MAY provide; if not, default is used
- Document what the default means ("uses system timeout", "disabled by default")
- Prefer required + type system over optional parameters

### When to Express Error via Result vs. Exception vs. Null

**Result/Error type** (preferred when possible):
- Caller must explicitly handle (forced pattern matching or explicit unwrap)
- Error is part of contract (type checker tracks it)
- Multiple error cases can be distinguished

**Exception** (use when):
- Exceptional, truly unexpected condition (not "normal" failure mode)
- Catching and recovering is not typical
- Language/ecosystem strongly favors exceptions

**Null/None** (use when):
- Absence is a valid, expected outcome (not an error)
- Contract states "returns empty/none if not found"
- Caller is expected to check for null before use

---

## Validation Rules

### Contract Clarity Criteria

An interface design passes validation when:

1. **Parameter Clarity**: ✓
   - Each parameter has a documented name and meaning
   - Parameter constraints are explicit (type system or documented)
   - No parameters are vague (e.g., "config object" without specifying what it contains)

2. **Return Type Clarity**: ✓
   - Return type is fully specified (not "object" or "value")
   - Null/empty/error cases are explicit (not implicit or surprising)
   - Caller knows what to do with return value without reading implementation

3. **Side Effects Declaration**: ✓
   - All I/O operations are documented (reads file, writes to database, sends network message)
   - All state mutations are documented (modifies receiver, updates cache)
   - No hidden side effects (function must not do undocumented I/O or state changes)

4. **Preconditions**: ✓
   - All preconditions are either:
     - Encoded in types (type system prevents invalid states), OR
     - Listed in documentation (caller knows what to check)
   - No undocumented "secret setup" required before calling

5. **Type Boundaries**: ✓
   - Public types are stable (won't break frequently)
   - Private/internal types don't leak into public interfaces
   - Domain model boundaries are respected

6. **Temporal Coupling**: ✓
   - No hidden "must call A before B" requirements
   - All dependencies are passed as parameters or established in constructor

### Contract Completeness Checklist

Before a function signature is accepted, verify:

- [ ] Function/method name clearly indicates what it does
- [ ] Purpose is one clear responsibility (not combined concerns)
- [ ] Parameter count ≤ 3 (or bundled into struct if > 3)
- [ ] Each parameter has explicit type and semantic meaning
- [ ] Return type is specific (not "object" or "any")
- [ ] Failure modes are explicit (error type, null, exception, documented)
- [ ] Side effects are documented (or function has none)
- [ ] Preconditions are enforced by type or documented
- [ ] Postconditions are observable and documented
- [ ] Invariants are listed (if any)
- [ ] No temporal coupling (hidden call order dependencies)
- [ ] Public vs. private boundary is clear
- [ ] Type boundary violations audited and eliminated

### Review Gate Failure Conditions

A proposed interface **fails gate** if:

- ❌ Parameters are untyped or vaguely typed (e.g., "object", "config", "data")
- ❌ Parameter constraints are missing or implicit ("must be positive" not written, caller discovers via crash)
- ❌ Return type is ambiguous ("may return value or object or null" without distinguishing cases)
- ❌ Failure modes are not specified (what exception types? what error codes?)
- ❌ Preconditions are undocumented and only discoverable by reading code
- ❌ Function has > 3 parameters and is not bundled into named structure
- ❌ Internal/private types leak into public interface
- ❌ Temporal coupling exists (hidden call order requirement)
- ❌ Side effects are undocumented
- ❌ Function does more than one thing (mixed concerns in contract)

---
