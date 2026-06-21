---
name: Language-Specific Skill Routing
description: >
  Maps capability keys to their Rust companion skills and the repo-local Rust
  execution conventions they rely on.
---

# Language-Specific Skill Routing

This repository is **Rust-first**. Verified evidence: `Cargo.toml`,
`Cargo.lock`, `src/**/*.rs`, and `tests/**/*.rs`.

When working in this repo's language context, use this file as the
authoritative Rust routing bridge instead of inferring companion names.

## Capability Key → Rust Companion Map

| Capability Key | Outcome | Companion / Notes |
|---|---|---|
| `1-design-feature-decomposition` | `universal only` | Use the universal skill directly. |
| `1-design-requirements-engineering` | `language companion exists` | `rust-1-design-requirements-engineering` |
| `2-plan-architecture-planning` | `no companion exists yet / placeholder needed` | No dedicated Rust planning companion exists here today. Use the universal skill and repo-local Rust layout rules from `directories.md`; if review tooling is needed later, use the Stage 4 Rust architecture companions. |
| `2-plan-domain-planning` | `no companion exists yet / placeholder needed` | No dedicated Rust domain-planning companion exists here today. Use the universal skill plus Rust domain constraints from `rust.instructions.md` and the Stage 3/4 Rust companions. |
| `2-plan-function-sig-planning` | `language companion exists` | `rust-2-plan-function-sig-planning` |
| `2-plan-behavior-planning` | `language companion exists` | `rust-2-plan-behavior-planning` |
| `2-plan-behavior-reviewing` | `language companion exists` | `rust-2-plan-behavior-reviewing` |
| `2-plan-test-planning` | `language companion exists` | `rust-2-plan-test-planning` |
| `3-implement-domain-implementation` | `language companion exists` | `rust-3-implement-domain-implementation` |
| `3-implement-function-sig-implementation` | `language companion exists` | `rust-3-implement-function-sig-implementation` |
| `3-implement-test-suite-completion` | `language companion exists` | `rust-3-implement-test-suite-completion` (with Rust appendices: `...-unit-tests`, `...-integration`, `...-property-tests`, `...-async-tests`, `...-validation`, `...-examples`) |
| `3-implement-behavior-wiring` | `language companion exists` | `rust-3-implement-behavior-wiring` |
| `4-review-architecture-tools` | `language companion exists` | `rust-4-review-architecture-tools` |
| `4-review-architecture-validation` | `language companion exists` | `rust-4-review-architecture-validation` |
| `4-review-behavior-tools` | `language companion exists` | `rust-4-review-behavior-tools` |
| `4-review-behavior-validation` | `language companion exists` | `rust-4-review-behavior-validation` |
| `4-review-completeness-tools` | `language companion exists` | `rust-4-review-completeness-tools` |
| `4-review-completeness-validation` | `language companion exists` | `rust-4-review-completeness-validation` |
| `4-review-consistency-tools` | `language companion exists` | `rust-4-review-consistency-tools` |
| `4-review-consistency-validation` | `language companion exists` | `rust-4-review-consistency-validation` |
| `4-review-function-sig-tools` | `language companion exists` | `rust-4-review-function-sig-tools` |
| `4-review-function-sig-validation` | `language companion exists` | `rust-4-review-function-sig-validation` |
| `4-review-performance-tools` | `language companion exists` | `rust-4-review-performance-tools` |
| `4-review-performance-validation` | `language companion exists` | `rust-4-review-performance-validation` |
| `4-review-security-tools` | `language companion exists` | `rust-4-review-security-tools` |
| `4-review-security-validation` | `language companion exists` | `rust-4-review-security-validation` |
| `4-review-type-validation` | `language companion exists` | `rust-4-review-type-validation` |
| `4-review-type-validation-tooling` | `language companion exists` | `rust-4-review-type-validation-tooling` |
| `0-global-tdd-workflow` | `universal only` | Use the universal skill directly. |
| `0-global-critical-rules` | `universal only` | Use the universal skill directly. |
| `0-global-interface-design` | `universal only` | Use the universal skill directly. |
| `0-global-documentation-standards` | `universal only` | Use the universal skill directly. |
| `0-global-dependency-adoption` | `universal only` | Use the universal skill directly. |
| `0-global-line-count-check` | `universal only` | Use the universal skill directly. |

## Rust Path and Test Conventions

- Production Rust code lives under `src/`.
- The composition root is `src/wiring.rs`.
- Tests primarily live under `tests/`.
- The dominant mirrored-test convention is `tests/<src-path>.tests.rs`.
- Standalone harness files also exist (for example `tests/integration_full_turn.rs`
  and `tests/debug_test.rs`), so verify the nearby pattern before creating a new
  test file.
- Test helpers and data live under `tests/helpers/`, `tests/fixtures/`, and
  `tests/snapshots/`.

## Required Rust Commands

Use these repo-local commands unless a narrower scoped command is explicitly
required by the mapped Rust companion skill:

| Purpose | Command |
|---|---|
| Build gate | `cargo build --workspace` |
| Test gate | `cargo test` |
| Lint gate | `cargo clippy --all-targets -- -D warnings` |
| Check gate | `cargo check --all-targets` |
| Red compile-only gate | `cargo test --no-run` |

## Temporary Stub Policy

- Temporary compile-target stubs are allowed only to get a Red test to compile.
- By Green, remove all production `todo!()`, `unimplemented!()`, and placeholder
  panic branches.
- Treat `unreachable!()` as exceptional: only keep it for documented impossible
  states with clear justification.

## Usage

Agents must always consult this table rather than hardcoding Rust companion
skill names.

- **Capabilities with a universal skill counterpart**: invoke the universal skill
  first, then look up the capability key here and invoke the listed Rust
  companion when one exists.
- **Capabilities with only a Rust companion**: use the listed Rust companion
  directly.
- **Capabilities marked placeholder**: do not invent a missing Rust skill name;
  use the universal skill plus the repo-local Rust rules noted in this file.
- **Always reference this file** for the authoritative mapping. Do not infer or
  hardcode companion skill names from naming conventions alone.
