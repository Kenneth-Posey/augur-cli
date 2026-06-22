# Domain Module

The `domain` module within `augur-core` houses core-owned domain contracts that are specific to the crate's runtime orchestration layer. It does **not** re-export types from `augur-domain`; instead, it contains the deterministic orchestrator's phase 1 contracts: workflow document parsing, step execution modes, dispatch specifications, failure routing, and runtime event types.

## Contents

The module exposes two public sub-modules mapped to source files: `deterministic_orchestrator` (the `deterministic_orchestrator.rs` source) and `deterministic_orchestrator_ops` (the `deterministic_orchestrator_ops.rs` source). The former defines the `WorkflowDocument`, `WorkflowStage`, `WorkflowStep`, and related types that model a parsed workflow YAML document along with its step kinds (`WorkerWithGate`, `SinglePass`, `ParallelGroup`, `GroupMember`), dispatch metadata, execution artifacts, transition logic, and failure decisions (`RerunCurrentStep`, `BacktrackTo`, `Halt`, `DelegateFix`). It also defines runtime signals (`NormalizedSignal`), execution records (`StepExecutionRecord`, `GroupMemberResult`), and events (`DeterministicOrchestratorEvent`).

## Architectural Role

This module is the bridge between the semantic workflow model (defined in `augur-domain`) and the orchestration actor that drives multi-step pipeline execution. By keeping these contracts in `augur-core` rather than `augur-domain`, the crate maintains ownership of the lowering logic that converts parsed YAML into executable step types with validation rules (for example, `WorkerWithGate` steps require both `model` and `gate_agent`). The `deterministic_orchestrator_ops` companion source provides the operational logic that consumes these contracts during workflow execution.