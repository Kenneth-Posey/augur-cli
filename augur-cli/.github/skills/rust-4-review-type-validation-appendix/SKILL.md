---
name: rust-4-review-type-validation-appendix
description: >
  Appendix for the Rust type validation skill. Use it to navigate detailed
  guidance on concepts, examples, decisions, and tooling.
---

# Skill: Rust Type Validation - Appendix

---

## Overview

This appendix links to focused sub-documents:
- **Concepts**: Lifetime correctness, generic bounds, unsafe justification, semantic types, and tool integration
- **Examples**: Worked examples of correct and incorrect type patterns
- **Decisions**: Decision criteria, finding interpretation guidance, and validation rules
- **Tooling**: Guidance for `cargo check`, `cargo clippy`, and related tools

---

## Key Files

- `README.md` - overview and usage notes

## Composition & References

### Primary Review Authorities

- `plans/<feature-slug>/plan/domain-spec.md` - semantic type intent and
  invariants.
- `plans/<feature-slug>/plan/function-sig-plan.md` - ownership, lifetime, and
  generic-bound expectations.
- `plans/<feature-slug>/plan/dependency-graph.md` - boundary direction and type
  exposure rules.
- `plans/<feature-slug>/plan/implementation-plan.md` - declared unsafe or
  low-level surfaces that require review.

### Tool Integration

**cargo check** - Validates syntax and basic type correctness; it must pass before concluding the review

**cargo clippy** - Identifies lint violations; resolve them or document the rationale for ignoring them

**compiler borrow checker** - Primary signal for lifetime and reference-validity violations; map violations to the rules above

**Rustdoc** - Documents semantic intent and helps clarify invariants

## Reference Documents

### Focused Skill Documents

- **[rust-4-review-type-validation-concepts/SKILL.md](../rust-4-review-type-validation-concepts/SKILL.md)** - Core concepts for lifetimes, bounds, unsafe usage, semantic types, and tool integration
- **[rust-4-review-type-validation-examples/SKILL.md](../rust-4-review-type-validation-examples/SKILL.md)** - Worked examples of correct and incorrect patterns
- **[rust-4-review-type-validation-decisions/SKILL.md](../rust-4-review-type-validation-decisions/SKILL.md)** - Decision criteria, finding interpretation guidance, and validation rules
- **[rust-4-review-type-validation-tooling/SKILL.md](../rust-4-review-type-validation-tooling/SKILL.md)** - Tool commands, baselines, and integration patterns

---
