# Plan Store Module

The `plan_store` module provides async disk I/O for plan trees--serialized `PlanTree` documents that the supervisor actor persists and loads during phased workflow execution. Each plan lives in a directory `{base_dir}/{plan_id}/` containing a `tree.json` for the plan structure and a `steps/` subdirectory with one `.md` file per executable step.

## Public API

**`PlanTreeStore`** is the primary struct, constructed with a configurable `base_dir` (defaulting to `"plans"` when no explicit path is given). It exposes five async methods: `save` (serializes a `PlanTree` to `tree.json`), `load` (reads and deserializes a previously saved tree), `write_step` (writes a step content file to the `steps/` subdirectory), and `read_step` (reads a step file back). The `PlanStoreError` enum covers I/O errors, serialization/deserialization failures, and not-found conditions.

## Architectural Role

The plan store is the disk backing for the supervisor's plan-driven workflow execution. When the supervisor starts a plan, it calls `save` to persist the plan tree. During execution, the supervisor uses `read_step` and `write_step` to load step content and save step artifacts. The store's lazy directory creation means it works out of the box with the default `"plans"` path; no pre-existing directory structure is required. Together with the persistence module, it forms the crate's complete durable-storage layer: persistence handles session data, and the plan store handles plan-tree data. Both modules keep blocking I/O off the async runtime by using `tokio::fs` throughout.