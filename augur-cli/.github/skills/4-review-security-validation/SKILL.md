---
name: 4-review-security-validation
description: >
  Stage 4 security review checklist for unsafe or low-level code justification,
  input validation, injection prevention, integer safety, secret handling, and
  cryptographic correctness across languages. Use before integration testing.
---

# Skill: 4-Review Security Validation

## Purpose

Validate that the implementation avoids common security flaws: unsafe or
low-level operations are justified, external inputs are validated, injection
vectors are absent, integer arithmetic is safe, secrets are handled correctly,
and cryptographic operations use approved algorithms.

## Key Files

- `README.md` - overview and usage notes

## What to Validate

> **N/A Sections:** Omit entire validation categories that do not apply to the
> current feature (e.g., "Cryptographic Correctness" when there is no crypto
> code, "Path Handling" when there is no filesystem access). Do not write
> "N/A" placeholder sections. A missing section implies the category was not
> applicable.

### 1. Unsafe Operation Justification
- Every region of code that bypasses language safety guarantees
  (raw pointers, unsafe blocks, FFI calls, manual memory management)
  has an inline comment documenting the safety preconditions
- Safety preconditions are specific and verifiable, not generic placeholders
- Safer alternatives have been ruled out

### 2. Input Validation
- Public functions accepting external (untrusted) input validate bounds, length,
  encoding, and shape before use
- Buffer and collection operations check bounds before indexing
- String encoding is validated where it matters

### 3. Injection Prevention
- Query construction does not use raw string concatenation with user input
  (SQL injection, command injection, LDAP injection risk)
- Shell/process execution does not accept unsanitized user input
- File and path operations validate against directory traversal attacks

### 4. Integer Safety
- Arithmetic on values derived from external input uses checked or saturating
  operations where overflow is possible
- No silent integer overflow in code that affects security boundaries
- No unbounded allocation sizes derived from untrusted input (denial-of-service risk)

### 5. Secret Handling
- No hardcoded credentials, API keys, tokens, or cryptographic secrets in source
- Error messages do not expose secrets, internal file paths, or connection strings
- Secrets are not written to logs or standard output/error
- Sensitive values are cleared from memory after use where the runtime permits

### 6. Cryptographic Correctness
- Only approved algorithms are used (SHA-256 or stronger; no MD5 or SHA-1 for
  security purposes; minimum 256-bit keys for symmetric encryption)
- No custom cryptographic implementations
- Random number generation uses a cryptographically secure source where
  security properties depend on unpredictability

### 7. Panic Safety in Library Code
- Production library code does not contain unconditional panic patterns that
  could be triggered by untrusted input
- Test code and binary entry points are exempt

## Pass Conditions

- All unsafe/low-level operations are documented with verified preconditions
- All external inputs are validated before use
- No injection vulnerabilities
- Integer arithmetic is safe for security-relevant paths
- No hardcoded or logged secrets
- Approved cryptographic algorithms and key sizes used

## Fail Conditions

- **Critical:** Hardcoded credential, API key, or secret in source
- **Critical:** SQL/command/path injection vulnerability
- **Critical:** Unsafe operation with no justification comment
- **Critical:** Integer overflow in a security boundary without checked arithmetic
- **Critical:** Incorrect cryptographic algorithm (e.g., MD5 for integrity)
- **High:** Missing input validation on a public function accepting external input
- **High:** Unsafe operation where a safe alternative exists
- **High:** Error message exposes internal path, connection string, or secret
- **Medium:** Secret present in a log statement
- **Low:** Magic numeric literal in a cryptographic constant

## Validation Signal

| Severity present | Signal |
|---|---|
| Critical or High findings | `fail` |
| Medium or Low findings only | `pass` with warnings |
| Validation timed out | `fail` |

## Report Format

**On pass (signal = pass):**
- Emit one summary line per validation category in the form:
  `Category Name: ✓ (brief note, e.g., "12 modules verified")`
- Emit the JSON diagnostic block with `findings: []` (or `findings` with only
  Medium/Low entries if present)
- Omit: detailed row-by-row verification tables, per-item bullet lists,
  validation checklists, and any duplicate `## Signal` section at the bottom
  - the signal is already stated in the report header

**On fail (signal = fail):**
- Emit full detail (table/bullets/evidence) only for the failing categories
- Emit the summary line format for all passing categories
- Emit the JSON diagnostic block with all findings fully populated

## Language Companion

See `4-review-security-validation` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for
language-specific unsafe syntax, validation patterns, injection-risk
constructs, integer-safety APIs, and checker logic.
