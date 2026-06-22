---
description: "Use when user asks: add tool, create tool, new tool, implement tool handler"
name: "Add Tool"
argument-hint: "tool name and handler responsibility"
agent: "agent"
---
Add a new tool to the project's tool registry. Follow the file placement and
TDD rules in this prompt.

## Inputs

- Tool name and handler responsibility (required).
- Tool schema description (required): input parameters and expected output.
- Plan phase spec or behavioral description if this work is plan-driven.

## Task Guidance

1. Use `0-utility-codebase-survey`, `0-global-critical-rules`, and the
   applicable implementation-language companions before editing.

2. Confirm that the new tool belongs in the project's tool surface and does not
   introduce wrong-direction imports or cycles.

3. Plan the file set. A new tool normally includes:
   - the tool handler module for `<tool_name>` - input validation, dispatch to
     domain logic, and result shaping. No business logic lives here.
   - the tool registry module - update it to register the new tool.
   - `src/domain/<tool_name>_ops.rs` (if new domain logic is needed) - pure
     business logic for the tool's domain concern. No I/O, no async runtime,
     no channel types.
   - `tests/tools/<tool_name>.tests.rs` - handler tests covering happy path,
     invalid input, and error cases.
   - `tests/domain/<tool_name>_ops.tests.rs` (if `_ops.rs` was created) -
     pure unit tests for domain logic.

4. **Red** - write failing tests first for the tool schema, expected outputs,
   and error cases.

5. **Green** - implement the tool to satisfy those tests. The implementation
   must:
   - keep the handler thin (validate input, call domain logic, shape result),
   - keep domain logic pure (no I/O, no runtime handles, no channel types),
   - use semantic newtypes for domain values (not bare primitives),
   - register the tool in the tool registry module.

6. **Refactor** - improve clarity without changing behavior. Re-run the
   relevant validation commands after refactoring.

7. Before reporting completion, confirm thin handler boundaries, pure domain
   logic placement, registration, and error-path coverage.

## Validation

Run after implementation is complete:
```
cargo check
cargo test
```
Confirm:
- Handler file contains only input validation, dispatch, and result shaping.
- Domain logic in `_ops.rs` or equivalent has no async/channel/I/O imports.
- Tool is registered in the tool registry module.
- Tests cover happy path, invalid input, and error cases.

## Output

1. File list created or modified
2. Test summary (failing Red, passing Green)
3. Validation results and any unresolved blockers
