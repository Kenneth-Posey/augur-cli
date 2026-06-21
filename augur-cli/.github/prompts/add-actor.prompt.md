---
description: "Use when user asks: add actor, create actor, new actor, implement actor"
name: "Add Actor"
argument-hint: "actor name and domain responsibility"
agent: "agent"
---
Add a new actor using the thin-shell/functional-core pattern, required file
layout, and local TDD discipline defined here.

## Inputs

- Actor name and domain responsibility (required).
- Parent domain directory under `src/actors/` (required).
- Plan phase spec or behavioral description if this work is plan-driven.

## Task Guidance

1. **Review required guidance** - before editing, use
   `0-utility-codebase-survey` to map existing symbols, `0-global-critical-rules`
   for TDD and quality rules, and the
   `3-implement-behavior-wiring` / `3-implement-domain-implementation`
   language companions.

2. **Confirm placement** - verify that the actor belongs in the intended domain
   and does not introduce wrong-direction imports or cycles.

3. **Create this file set**:
    - `src/actors/<domain>/<actor_name>.rs` - thin async shell: event loop,
      command handling, state ownership, feed publication, logging.
   - `src/actors/<domain>/<actor_name>_ops.rs` - functional core: pure
     calculations, decision logic, state-transition helpers. No I/O, no async
     runtime imports, no channel types in public contracts.
   - `src/actors/<domain>/mod.rs` - update to re-export the new actor.
   - `src/wiring.rs` - add construction and handle wiring for the new actor.
   - `tests/actors/<domain>/<actor_name>.tests.rs` - async coordination and
     publication tests using public handles, feeds, and snapshots only.
   - `tests/actors/<domain>/<actor_name>_ops.tests.rs` - pure unit tests for
     `_ops.rs` functions.

4. **Red** - write failing tests first. Cover the actor name,
   responsibility, handle interface, command types, and feed types. Test only
   through the public handle, feeds, and snapshots - never through internals.

5. **Green** - implement the actor to satisfy those failing tests. The
   implementation must:
    - keep the shell thin (no dense business logic in the event loop),
    - keep `_ops.rs` pure (no I/O, no runtime handles, no channel types),
    - expose all consumable outputs through the actor handle only,
    - use semantic newtypes for domain values (not bare primitives).

6. **Refactor** - improve clarity without changing behavior. Re-run the
   relevant validation commands after refactoring.

7. **Local quality bar** - before reporting completion, confirm:
   - dependency direction still holds,
   - shell and `_ops.rs` remain separate,
   - command handling, state transitions, and feed publication are covered.

## Validation

Run after implementation:
```
cargo check
cargo test
```
Confirm:
- Shell file contains only async execution, command handling, state, and feeds.
- `_ops.rs` contains only pure functions with no async/channel/I/O imports.
- All public actor outputs are accessed through the actor handle.
- Tests use only public handles, feeds, and snapshots.

## Output

1. File list created or modified
2. Test summary from Red and Green
3. Validation results and any unresolved blockers
