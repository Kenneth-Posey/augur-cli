---
name: 0-global-dependency-adoption
description: >
  Rules for adopting observability, units, test mocking, and struct builder
  crates. Use when adding or reviewing crate dependencies.
---

# Dependency Adoption Standards

## Goal

- Keep dependency choices intentional, minimal, and consistent.
- Prefer existing project patterns before introducing a crate.
- When adding a crate, define where and why it is used, and keep its usage
  scoped.

## Crate Selection Rules

### `tracing` and supporting tracing crates

- Use `tracing` for all runtime observability in new code.
- Use `tracing-subscriber` only at application entry points and test harnesses
  that need subscriber setup.
- Add supporting crates only for concrete needs:
  - `tracing-appender` for file or rolling sink requirements.
  - `tracing-error` for richer error span context integration.
  - exporter crates only when the deployment target requires them.
- Do not add alternate logging frameworks for new runtime code.

### `uom`

- Use `uom` when new functionality needs multi-dimensional unit algebra and
  manual cross-type impls would grow quickly.
- Prefer existing domain newtypes for simple business-domain numeric semantics.
- Keep `uom` usage localized to modules that benefit from dimensional safety.
- Define explicit conversion boundaries between `uom` types and domain wrappers.

### `mockall`

- Use `mockall` for trait or interface mocking in unit tests.
- Prefer `mockall` over hand-written fake structs when multiple tests need the
  same trait behavior.
- Keep mocks close to test modules and focused on observable behavior.

### `mockito`

- Use `mockito` for HTTP boundary tests where external endpoints are otherwise
  required.
- Use deterministic request matching (method, path, headers, body) and explicit
  response setup.
- Do not use `mockito` for non-HTTP boundaries.

### Builder

- **bon** (`bon = "3"`) - preferred crate for struct builders.
  Place in `[dependencies]` (builders are part of the production type API).
  Use `#[derive(bon::Builder)]` on structs only; do not use bon's
  function-builder feature.

## Dependency Placement

- Runtime crates belong in `[dependencies]`.
- Test-only crates belong in `[dev-dependencies]`.
- Add dependencies to each crate manifest that actually needs them.
- Avoid adding unused workspace-wide dependencies.

## Validation Requirements

- After adding behavior that depends on a crate, add or update tests that prove
  it.
- Ensure local checks pass (`cargo check` and relevant test targets).
- Document dependency-driven changes in changelog entries when they affect
  users or operations.

## Review Heuristics

- If new runtime output uses anything other than `tracing`, migrate to
  `tracing`.
- If dimensional logic is becoming a large matrix of manual cross-type impls,
  evaluate `uom`.
- If tests rely on ad hoc trait doubles repeatedly, replace with `mockall`.
- If HTTP tests depend on live services, replace with `mockito`.
- If a struct that should expose a builder has a hand-written builder struct,
  replace it with `#[derive(bon::Builder)]`.
