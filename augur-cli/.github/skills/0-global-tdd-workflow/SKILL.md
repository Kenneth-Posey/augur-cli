---
name: 0-global-tdd-workflow
description: >
  Red/Green/Refactor discipline for implementation work: test-first
  specification, done criteria, and regression protection. Use at the start of
  implementation work and before accepting a code change.
---

# 0-global-tdd-workflow

## Key Concepts

### 1. Red Phase: Test-First Specification

- Write test case(s) that fail before code exists.
- Test assertions map 1:1 to acceptance criteria or behavioral specification.
- Each test is **independent** and **isolated** - no test should depend on side effects of another test.
- Test names clearly express the condition being verified: `test_<action>_<input_condition>_<expected_result>()`.
- For every Happy Path test, include at least one Sad Path (error case) test.
- Assertion messages are **explicit** and enable quick root-cause diagnosis.

**Example**:
```
Test: `test_parse_json_valid_object_returns_parsed_map`
- Input: valid JSON object string `{"key": "value"}`
- Expected: Result contains parsed map with matching key-value pair
- Assertion: `assert result.get("key") == "value"`

Test: `test_parse_json_invalid_syntax_returns_parse_error`
- Input: malformed JSON string `{invalid}`
- Expected: Result is an Err with ParseError variant
- Assertion: `assert result is Err(ParseError::Syntax)`
```

## Key Files

- `README.md` - overview and usage notes

## Examples

### Example 1: Feature Implementation (Red/Green/Refactor)

**Acceptance Criteria**:
- Parse YAML configuration file into Config struct.
- Return error if file is missing or syntax is invalid.

**Red Phase**:
```text
test: test_load_config_valid_yaml_returns_config
  given: yaml = "port: 8080\nhost: localhost"
  when:  result = load_config(yaml)
  then:  result is Ok
         result.port == 8080
         result.host == "localhost"

test: test_load_config_invalid_yaml_returns_error
  given: yaml = "port: [invalid yaml"
  when:  result = load_config(yaml)
  then:  result is Err(ConfigError::SyntaxError)
```

**Green Phase**:
```text
fn load_config(yaml):
  return parse_yaml(yaml)
    on_error: wrap as ConfigError::SyntaxError
```

**Refactor Phase**:
```text
// Load and parse configuration from YAML string.
// Arguments:
//   yaml - YAML-formatted configuration string
// Returns:
//   Ok(Config) if parsing succeeds
//   Err(ConfigError) on invalid syntax
fn load_config(yaml):
  trimmed = yaml.trim()
  return parse_yaml(trimmed)
    on_error: wrap as ConfigError::SyntaxError("Invalid YAML: {error}")
```

**Done Checklist**:
- ✅ Red tests fail before implementation.
- ✅ Green implementation passes tests.
- ✅ Refactor adds documentation without changing behavior.
- ✅ All tests pass after refactoring.
- ✅ No new test failures.

---

### Example 2: Bug Fix (Red/Green/Refactor)

**Bug Report**: "Login fails silently when session cache is corrupted."

**Acceptance Criteria**:
- Detect corrupted session cache.
- Log error and clear cache.
- Return specific error to client.

**Red Phase**:
```text
test: test_login_corrupted_cache_clears_and_returns_error
  given: session = setup_corrupted_session()
  when:  result = authenticate(session)
  then:  result is Err(AuthError::CacheCorrupted)
         session_cache_exists() == false
```

**Green Phase**:
```text
fn authenticate(session):
  match validate_cache(session):
    Ok(user)                   => return Ok(user)
    Err(CacheError::Corrupted) =>
      clear_session_cache()
      return Err(AuthError::CacheCorrupted)
    Err(other)                 => return Err(AuthError::Unexpected(other))
```

**Refactor Phase**:
```text
// Authenticate user from session cache.
// Detects and recovers from corrupted cache by clearing and returning error.
fn authenticate(session):
  return validate_cache(session)
    on_error(err):
      if err is CacheError::Corrupted:
        clear_session_cache()
        return Err(AuthError::CacheCorrupted)
      else:
        return Err(AuthError::Unexpected(err))
```

---

### Example 3: Refactoring Session (Refactor Phase Only)

**Goal**: Extract repeated logging logic without changing behavior.

**Before**:
```text
fn process_payment(order):
  log("DEBUG: Processing order {order.id}")
  charge_card(order.method)  // may fail
  log("DEBUG: Payment succeeded for order {order.id}")
  return Ok

fn refund_payment(order):
  log("DEBUG: Refunding order {order.id}")
  reverse_charge(order.method)  // may fail
  log("DEBUG: Refund succeeded for order {order.id}")
  return Ok
```

**After** (Red tests still pass; behavior identical):
```text
fn log_event(order_id, message):
  log("DEBUG: {message} for order {order_id}")

fn process_payment(order):
  log_event(order.id, "Processing order")
  charge_card(order.method)  // may fail
  log_event(order.id, "Payment succeeded")
  return Ok

fn refund_payment(order):
  log_event(order.id, "Refunding order")
  reverse_charge(order.method)  // may fail
  log_event(order.id, "Refund succeeded")
  return Ok
```

**Verification**: All existing tests pass (same output, different code path).

---

## Decision Criteria

### When to Start Red Phase

- When a new feature is requested (in plan or issue).
- When a bug is reported and reproduced.
- When a refactoring scope is defined (if tests are lacking).
- When acceptance criteria are explicit (mapped from plan).

### When Red Phase is Complete

- At least one Happy Path test and one Sad Path test exist.
- Tests fail before implementation (verified by running).
- Test names match the behavioral contract.
- Each test is independent (no shared state between tests).

### When to Move from Red to Green

- All Red tests are written and fail.
- Confirm Red tests match acceptance criteria.
- No further Red tests need to be added before implementation.

### When Green Phase is Complete

- All Red tests pass.
- No existing tests broken.
- Code compiles without errors.
- Implementation is minimal (no speculative logic).
- When the work replaces existing behavior, the activation gate is satisfied
  and `review-activation-checker` reports pass.

### When to Move from Green to Refactor

- All Red tests pass consistently.
- Code review (if required) approves the implementation logic.
- No new behavior needs to be added.

### When Refactor Phase is Complete

- All Red tests still pass.
- Code is clearer, better documented, or more efficient.
- Linting and formatting rules applied.
- Cross-cutting concerns (dependency direction, trait boundaries) verified.

### When to Skip Refactor Phase

- Only if Green-phase code is already optimal (rare).
- Refactor is deferred to a later, dedicated refactoring sprint (document in plan).

---

## Validation Rules

### Mandatory Validations Before Acceptance

1. **Red Phase Validation**:
   - [ ] Test file exists and is correctly located.
   - [ ] Run unit test suite; confirm tests fail before implementation.
   - [ ] Each test asserts exactly one behavioral outcome (no multi-assertion tests without sub-contexts).
   - [ ] Test names are unambiguous and self-documenting.

2. **Green Phase Validation**:
   - [ ] Run unit test suite; confirm all Red tests pass.
   - [ ] Run full test suite; confirm no regressions.
   - [ ] Run compiler/type-checker and linter; confirm no new errors.
   - [ ] Implementation logic matches minimum viable scope (not speculative).

3. **Refactor Phase Validation**:
   - [ ] Run unit test suite; confirm all Red tests still pass.
   - [ ] Run full test suite; confirm no regressions.
   - [ ] Code coverage did not decrease (verify via coverage tool if required).
   - [ ] Linting and formatting rules applied: run linter and apply formatter.

4. **Integration Validation**:
   - [ ] Cross-module tests pass (if phase introduces new public APIs).
   - [ ] Dependency direction unchanged or approved by dependency-plan-evaluator.
   - [ ] Documentation updated (public API changes, new modules).
   - [ ] Git commit message references acceptance criteria and test names.

5. **Activation Gate Validation** (required for new feature cutover or replacement work):
   - [ ] Wiring proof from user action to the new module exists with file+line evidence.
   - [ ] Legacy bypass proof exists: old path removed, unreachable, or feature-flagged off by default.
   - [ ] Runtime assertion test proves the legacy path is not used and the new path is active.
   - [ ] `review-activation-checker` emits pass for wiring proof, legacy bypass proof,
     runtime assertion test, and active replacement state.
   - [ ] No phase-complete state exists for deferred wiring unless the phase is scaffold-only.

### Failure Conditions

- ❌ Red tests pass before implementation → Red phase is invalid; rewrite tests.
- ❌ Green implementation doesn't pass Red tests → Green phase is incomplete; continue implementation.
- ❌ Refactor phase breaks any Red tests → Change is not a refactor; revert or move to Green phase.
- ❌ New tests added during Green phase → Should have been added during Red; add to Red phase instead.
- ❌ Refactoring introduces new functions not required by Red tests → Not a refactor; may belong in Green or a new feature cycle.
- ❌ Replacement work advances without `review-activation-checker` pass → Implementation is incomplete; finish the activation gate first.

---


## Appendix: Quick Reference

### Red Phase Checklist
```
[ ] Test file created with failing test(s)
[ ] Test names follow: test_<action>_<condition>_<expected_result>
[ ] At least 1 Happy Path test
[ ] At least 1 Sad Path test
[ ] All assertions include diagnostic messages
[ ] Tests fail before implementation (verified)
```

### Green Phase Checklist
```
[ ] Implementation added to pass Red tests
[ ] All Red tests pass (run unit test suite)
[ ] No regressions (run full test suite)
[ ] Compiler/type-checker passes (no errors)
[ ] Linter clean (or warnings approved)
[ ] Implementation is minimum viable (no speculative features)
```

### Refactor Phase Checklist
```
[ ] Code clarity improved (naming, organization, comments)
[ ] All Red tests still pass
[ ] No regressions
[ ] Linting rules applied (run linter, apply formatter)
 [ ] Cross-cutting concerns verified (dependency-plan-evaluator if needed)
[ ] No new behavior introduced
```
