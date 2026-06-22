---
name: 0-global-functional-pseudocode
description: >
  Pseudocode notation standard for .github/skills/ files.
  Use when writing, reviewing, or converting pseudocode examples in any skill
  specification: function signatures, let-bindings, error propagation, match
  expressions, pipeline operators, and side-effect annotations.
---

# 0-global-functional-pseudocode

The notation is language-agnostic - it must never read as Rust, Python, or any other concrete language.

## When to Use This Skill

Invoke this skill when:

- Writing new pseudocode examples in any `.github/skills/` file.
- Reviewing existing pseudocode blocks to check they conform to this notation.
- Converting pseudo-Rust (`fn foo():` with Python colons, `HttpResponse::`,
  Rust-specific syntax, etc.) into proper pseudocode.

Do **not** use this skill for:

- Actual Rust implementation code - use `rust-3-implement-behavior-wiring` and
  companion skills.
- Diagrams or tables - use standard Markdown.

## Key Files

- `README.md` - overview and usage notes

## How This Skill Relates to Other Skills and Instructions

| Artifact | Relationship |
|---|---|
| `rust-3-implement-behavior-wiring` | Governs real Rust code; pseudocode here is illustration only |
| `0-global-documentation-standards` | Governs prose structure; this skill governs code-block notation |
| `0-global-behavioral-specification` | Uses pseudocode blocks to specify behavior; this skill defines their form |
| `0-global-tdd-workflow` | Red/Green examples use informal notation; examples in skill files use this standard |

## Core Notation Rules

### 1. Function Signatures - always explicit

Every function declaration must include parameter names, types, and a return
type. Pure functions have no annotation. Effectful functions use
`[effect: description]`.

```pseudocode
// Pure - no annotation
fn validate_query(q: String) -> Result<ValidQuery, QueryError>

// Side-effectful - annotated
fn save_user(user: User) -> Result<UserId, DbError>  [effect: db.write]
fn send_email(addr: Email, body: String) -> Result<(), MailError>  [effect: smtp.send]
fn log_event(event: Event) -> ()  [effect: log.write]
```

### 2. Let Bindings - immutable, no reassignment

Bind values once with `let`. Never use imperative reassignment (`x = x + 1`,
`mut x`, etc.).

```pseudocode
let query   = validate_query(req.q)?
let results = SearchService.query(query)?
let view    = format_results(results)
return Ok(view)
```

### 3. Error Propagation - `?` suffix

`?` means: if the expression returns `Err`, return that error immediately.

```pseudocode
let user   = AuthService.verify(req.user_id, req.token)?
let method = PaymentMethodService.get(req.user_id)?
```

### 4. Match Expressions - exhaustive, explicit arms

List all arms. Use a wildcard arm only when the variant set is genuinely open,
and note that with a comment. Arms are comma-terminated.

```pseudocode
match payment_method {
  CreditCard(cc)    => process_credit_card(cc, req.amount)?,
  BankAccount(acct) => process_ach(acct, req.amount)?,
}
```

### 5. Pipeline Operator `|>`

Use `|>` for sequential single-argument transformations. The left side becomes
the sole argument to the next function.

```pseudocode
let result = raw_input |> parse |> validate |> normalize
```

### 6. Type Declarations

```pseudocode
type UserId        = String
type Email         = String
type Result<T, E>  = Ok(T) | Err(E)
type Option<T>     = Some(T) | None
```

Algebraic variants use `TypeName(payload)` syntax. Sum types use `|`. Product
types use `{ field: Type }` record syntax.

### 7. Distinguishing Pure vs Effectful Functions

| Category | Definition | Annotation |
|---|---|---|
| Pure | Deterministic, no I/O, no state mutation | None |
| Effectful | Any I/O, network, db, timer, randomness, logging | `[effect: <category>]` |

A function that calls an effectful function is also effectful and must carry the
annotation. Separate multiple effects with commas:
`[effect: db.read, db.write, http.call]`.

**Effect categories:**

| Tag | Meaning |
|---|---|
| `db.read` | Database read |
| `db.write` | Database write |
| `http.call` | Outbound HTTP request |
| `smtp.send` | Email transmission |
| `log.write` | Append to log |
| `time.now` | Read current time |
| `rand` | Random number generation |
| `fs.read` | File system read |
| `fs.write` | File system write |

### 8. Code Fence Language Tag

Always use ` ```pseudocode ` as the fence language tag. Never use ` ```rust `,
` ```text `, ` ```python `, or any other language tag for pseudocode examples.

---

## Complete Examples

### Example 1 - Search Request Handler

```pseudocode
// Types
type SearchRequest  = { q: String, user_id: UserId }
type SearchResponse = { items: List<Item> }

// Pure
fn validate_query(q: String) -> Result<ValidQuery, QueryError>
fn format_results(items: List<Item>) -> SearchResponse

// Effectful
fn search_handler(req: SearchRequest) -> Result<SearchResponse, HandlerError>  [effect: db.read]
  let query    = validate_query(req.q)?
  let items    = SearchService.query(query)?
  let response = format_results(items)
  return Ok(response)
```

### Example 2 - Multi-Branch Payment Handler

```pseudocode
// Types
type PaymentRequest = { user_id: UserId, token: Token, invoice_id: InvoiceId, amount: Money }
type PaymentResult  = Success(Transaction) | Unauthorized(AuthError) | Error(PaymentError)
type PaymentMethod  = CreditCard(CreditCardData) | BankAccount(AccountData)

// Pure
fn process_credit_card(cc: CreditCardData, amount: Money) -> Result<Transaction, PaymentError>
fn process_ach(acct: AccountData, amount: Money) -> Result<Transaction, PaymentError>

// Effectful
fn payment_handler(req: PaymentRequest) -> Result<PaymentResult, HandlerError>
    [effect: db.read, db.write, http.call, smtp.send]
  let user   = AuthService.verify(req.user_id, req.token)?
  let method = PaymentMethodService.get(req.user_id)?

  let tx = match method {
    CreditCard(cc)    => process_credit_card(cc, req.amount)?,
    BankAccount(acct) => process_ach(acct, req.amount)?,
  }

  InvoiceService.update_status(req.invoice_id, Status::Paid)?
  EmailService.send_confirmation(req.user_id, tx)?

  return Ok(PaymentResult::Success(tx))
```

### Example 3 - Event Dispatcher (Pipeline)

```pseudocode
// Types
type Event = Order(OrderData) | Payment(PaymentData) | Notification(NotificationData)
type DispatchResult = OrderResult(OrderData, Status)
                    | PaymentResult(PaymentData, Status)
                    | NotificationResult(String, Status)

// Effectful
fn dispatch_event(event: Event) -> Result<DispatchResult, DispatchError>
    [effect: db.read, db.write, smtp.send]
  match event {
    Order(data)        => OrderService.process(data)        |> wrap_order_result,
    Payment(data)      => PaymentService.process(data)      |> wrap_payment_result,
    Notification(data) => NotificationService.send(data)    |> wrap_notification_result,
  }

fn feed_dispatcher(events: Stream<Event>) -> Stream<DispatchResult>
    [effect: db.read, db.write, smtp.send]
  events |> map(dispatch_event) |> collect_results
```

---

## Notation Quick Reference

| Construct | Notation |
|---|---|
| Pure function signature | `fn name(param: Type) -> ReturnType` |
| Effectful signature | `fn name(param: Type) -> ReturnType  [effect: category]` |
| Let binding | `let x = expr` |
| Error propagation | `expr?` |
| Match arm | `Pattern(inner) => expr,` |
| Pipeline | `value \|> fn1 \|> fn2` |
| Sum type | `Type = Variant1(T) \| Variant2(U)` |
| Record type | `type Foo = { field: Type }` |
| Code fence tag | ` ```pseudocode ` |
