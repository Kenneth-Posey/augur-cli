---
name: 4-review-activation-validation
description: >
  Stage 4 activation validation contract for replacement work. Defines the
  deterministic cutover/wiring, legacy-bypass, runtime-assertion, and active-path
  evidence required for pass/fail review.
---

# Skill: 4-Review Activation Validation

## Purpose

Validate replacement-work activation without reviewer phrase matching. This skill is
read-only: report findings, do not patch artifacts.

## Key Files

- `README.md` - overview and usage notes

## What to Validate

### 1. Wiring Evidence
- A concrete user-action or entrypoint path reaches the replacement module
- The report includes file-and-line evidence for the new path
- The evidence is deterministic, not inferred from prose

### 2. Legacy Bypass Evidence
- The old path is removed, unreachable, or feature-flagged off by default
- Any remaining legacy reference is intentional and documented
- The report distinguishes bypass evidence from simple code comments

### 3. Runtime Assertion Evidence
- A test proves the legacy path is not used
- The test proves the new path is active
- The assertion is in the requested scope and maps to the replacement work

### 4. Replacement Activation State
- The implementation and tests agree on the active path
- The activation gate is satisfied through concrete artifacts, not reviewer wording
- No dependency on reviewer acknowledgment phrases remains

## Pass Conditions

- Wiring proof exists with file-line evidence
- Legacy bypass proof exists
- Runtime assertion proof exists
- Active replacement state is explicit and consistent
- No acceptance criterion depends on reviewer phrase matching

## Fail Conditions

- Missing wiring, bypass, or runtime-assertion evidence
- Activation state is ambiguous or inconsistent
- Evidence depends on prose instead of concrete artifacts
- Any reviewer phrase contract remains in the acceptance path

## Validation Signal

| Severity present | Signal |
|---|---|
| Critical or High findings | `fail` |
| Medium or Low findings only | `pass` with warnings |
| Validation timed out | `fail` |

## Report Format

- On pass, emit a short summary of the evidence categories confirmed.
- On fail, emit the failing categories, observed gaps, and exact correction needed.
- Emit the standard diagnostic block with `checker`, `signal`, and `findings[]`.

## Language Companion

Use [`../../local/language-companions.md`](../../local/language-companions.md) for any
language-specific test-layout or runtime-assertion conventions that affect proof collection.
