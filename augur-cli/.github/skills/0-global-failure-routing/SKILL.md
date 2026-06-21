---
name: 0-global-failure-routing
description: >
  Classifies pipeline failures by type, owner, recoverability, and escalation.
  Use whenever a pipeline stage, tool, compiler, test runner, or review gate
  fails and a consistent failure report is needed.
---

# 0-global-failure-routing


## Key Concepts

### Escalation Characteristic

Every failure must be classified as exactly one of these characteristics:

| Characteristic | Meaning | Diagnostic implication |
|----------------|---------|------------------------|
| **transient** | Environmental or timing-sensitive signal that may clear without artifact changes | Record the environmental evidence and note that the signal may not reflect a code or plan defect. |
| **owner-actionable** | A concrete remediation or review domain clearly owns the issue | Record the owning domain and the evidence that makes the issue locally actionable. |
| **blocking** | A defect, ambiguity, or missing prerequisite prevents confident progress | Record the blocker and the unanswered question, prerequisite, or environment constraint. |

### Failure Type

- **Root domain:** Failed subsystem (compiler, test runner, linter, API, validation gate, reviewer)
- **Symptom pattern:** Observable signal (exit code, error prefix, missing artifact)
- **Repeatability:** Whether the same retry is likely to succeed (transient vs. systematic)
- **Ownership domain:** Remediation or review domain responsible for follow-up
- **Example errors:** Concrete instances (for example, "ECONNREFUSED on first HTTP call" → transient)

## Key Files

- `README.md` - overview and usage notes

## Examples

### Example 1: Compiler Syntax Error

**Observed failure:**
```
$ build
error: undefined identifier `x`
 --> src/main:5:10
  |
5 |   print(x)
  |         ^ not found in this scope
```

**Classification:**
- Type: Syntax error
- Root domain: Compiler
- Symptom: `error:` in stderr, exit code non-zero
- Ownership domain: implementation correction
- Escalation characteristic: owner-actionable

**Diagnostic payload:** Full stderr, file path, line number.

---

### Example 2: Transient Network Timeout

**Observed failure:**
```
$ build
error: failed to fetch https://pkg-registry/api/v1/packages/...
error: operation timed out after 300s
```

**Classification:**
- Type: Dependency fetch (network timeout)
- Root domain: External service (<package-registry>)
- Symptom: `timed out` in stderr
- Repeatability: Transient (likely recovers without artifact changes)
- Ownership domain: environment / external service health
- Escalation characteristic: transient

**Diagnostic payload:** Timestamp, stderr excerpt, environmental context.

---

### Example 3: Flaky Test

**Observed failure:**
```
$ run test flaky_test
ERROR: flaky_test - shared_state mismatch; expected 42
  location: tests/lib:123
```

**First run:** Fails  
**Second run (no code changes):** Passes

**Classification:**
- Type: Flaky test (passes on rerun)
- Root domain: Test runner + code logic
- Symptom: Non-deterministic pass/fail
- Ownership domain: ambiguous test contract or concurrency behavior
- Escalation characteristic: blocking

**Diagnostic payload:** Test name, assertion detail, reproduction instructions, run history.

**Blocking notes:**
- Determine whether the test contract is incorrect, the code is racy, or the environment introduces timing instability.
- Record which evidence would disambiguate the failure on the next analysis pass.

---

### Example 4: Review Change-Request

**Observed failure:**
```
Review node: PR review submitted
Reviewer: alice@example.com
State: CHANGES_REQUESTED
Feedback: "This function needs error handling for the database connection timeout. Please add a Result return type and propagate the error."
```

**Classification:**
- Type: Review change-request
- Root domain: Human review gate
- Ownership domain: change-requested implementation or documentation area
- Escalation characteristic: owner-actionable

**Diagnostic payload:** Reviewer feedback, PR context, required changes summary.

---

### Example 5: Environment Blocker

**Observed failure:**
```
$ run test suite
build-tool: command not found
```

**Classification:**
- Type: Tool not installed
- Root domain: Environment / PATH
- Symptom: `command not found`
- Ownership domain: environment setup
- Escalation characteristic: blocking

**Diagnostic payload:** Missing tool name, expected installation method.

**Resolution context:** System setup is required; no code or plan artifact can resolve this alone.

---

## Validation Rules

### Classification Completeness

Every failure must map to exactly one escalation characteristic. A classification is **valid** if:

1. **Recoverability assessed:** The signal is marked transient, owner-actionable, or blocking.
2. **Ownership determined:** The owning remediation domain is named when the issue is actionable.
3. **Context preserved:** Failure payload includes stderr, exit code, file path, line number (if applicable).
4. **Blocking reason defined:** If blocking, the unanswered question or missing prerequisite is named explicitly.

### Taxonomy Completeness

A new failure type must be added if:
- It does not fit into one of the six domains (Compiler, Test, Lint, Tool, Validation, Review).
- Its escalation characteristic is ambiguous.
- Its ownership domain is unmapped.

### Decision Determinism

For a given failure type and context, the classification should be deterministic. If evidence conflicts or ownership is unknown, classify as `blocking` and name the unresolved question explicitly.

---

## Appendix: Quick Reference

### Escalation Cheat Sheet

| If... | Record... |
|------|-----------|
| Network or timeout signal with clear environmental evidence | `transient` + environment evidence |
| Compiler syntax error | `owner-actionable` + implementation correction |
| Test assertion mismatch with a clear contract gap | `owner-actionable` + test or behavior correction |
| Lint warning with a clear standards mapping | `owner-actionable` + code quality domain |
| Permission denied or missing toolchain | `blocking` + environment prerequisite |
| Flaky test with conflicting evidence | `blocking` + ambiguity notes |
| Review change-request | `owner-actionable` + requested change domain |
| Ownership unknown | `blocking` + unresolved ownership note |
