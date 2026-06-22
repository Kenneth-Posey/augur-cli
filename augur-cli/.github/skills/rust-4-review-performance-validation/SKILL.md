---
name: rust-4-review-performance-validation
description: >
  Rust-specific performance pattern validation for algorithmic complexity, data
  structure selection, allocation efficiency, and loop correctness. Use when
  verifying that implementation avoids common Rust performance anti-patterns.
---

# Rust 4 Review Performance Validation

**Authority boundary**: Performance patterns and allocation efficiency only. Do
not use this skill for behavioral correctness, type safety, naming conventions,
style, or security review.

## Review Role

Review changed Rust code for performance patterns. Use repo-local authorities
and any benchmark or static-analysis evidence in the handoff, then emit
`pass|fail`.

## Key Files

- `README.md` - overview and usage notes

## Scope

### What to Validate

1. **Algorithmic Complexity**
    - Function complexity matches the expectations recorded in
      `plans/<feature-slug>/plan/implementation-plan.md`,
      `plans/<feature-slug>/plan/domain-spec.md`, or benchmark/profiler output
      supplied in the review handoff
   - No naive recursive algorithms that should be iterative (e.g., unbounded Fibonacci)
   - Recursion is bounded with a clear base case
   - No redundant computations or repeated I/O inside loops

2. **Data Structure Selection**
   - `Vec` used for sequential access patterns
   - `HashMap` used for key-value lookup
   - `HashSet` used for membership tests
   - No data structure mismatches that degrade algorithmic complexity

3. **Clone and Allocation Patterns**
   - No unnecessary `.clone()` calls
   - No `Vec::new()` inside hot loops without pre-allocation
   - No string concatenation with `+` inside loops (use `String::push_str` or `write!`)
   - No excessive heap allocations in tight loops
   - No large stack arrays (e.g., `[u8; 1_000_000]`)

4. **Loop Correctness**
   - Loop termination conditions are clear and bounds are reasonable
   - No repeated regex compilation inside loops (use `lazy_static` or `once_cell`)
   - No repeated I/O operations that could be batched

### Assumptions

This skill assumes:
- The codebase compiles without errors
- Performance expectations are documented in
  `plans/<feature-slug>/plan/implementation-plan.md`,
  `plans/<feature-slug>/plan/domain-spec.md`, or deterministic benchmark/profiler
  output supplied with the review handoff
- Benchmarks are not required to exist (static analysis only)

## Validation Inputs

- Changed Rust files in the hot path under review
- `plans/<feature-slug>/plan/implementation-plan.md` for declared runtime and
  allocation constraints
- `plans/<feature-slug>/plan/domain-spec.md` for data-shape assumptions that
  affect complexity and memory use
- `plans/<feature-slug>/design/behaviors.md` when behavior sequencing implies
  batching, caching, or repeated work expectations
- Deterministic benchmark or profiler output when it is the actual handoff input

## Review Output

- Findings tied to the exact loop, allocation site, or data-structure choice
- Each finding linked back to the governing handoff file or deterministic tool output
- Static-analysis conclusion that states whether follow-up benchmarking is required

## Validation Signal

Use `pass` or `fail` based on the code and supporting evidence.

| Severity | Signal |
|----------|--------|
| Critical or High findings present | `fail` |
| Medium or Low findings only | `pass` with warnings |
| Validation timed out | `fail` |
