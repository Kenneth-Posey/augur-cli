# Domain

The `domain` module is the semantic core of the crate. It defines all shared domain types, traits, semantic newtypes, event protocols, plan tree and state types, tool definitions, context management data structures, background event classification, scheduling, DAG validation, effort levels, stream state, thinking mode, channel constants, feeds, and reply events. With over 25 submodules, it is the largest and most diverse module in the crate - the single source of truth for what domain concepts exist and how they relate.

## Key Components

### Type System Infrastructure

- **`string_newtypes`** defines the `StringNewtype` trait and the `newtype_string!` macro, which generates semantic string wrappers for every domain-significant string value. Over 60 types (e.g., `ModelName`, `EndpointUrl`, `ToolName`, `SessionId`, `FilePath`, `PlanNodeId`, `ApiKey`, `BearerToken`, `ConversationId`, `ModelId`, `ToolCallId`) are defined here, each wrapping a `String` with transparent serde serialization so they round-trip cleanly through JSON and YAML while preventing type confusion at every call site.

- **`newtypes`** defines the `NumericNewtype` trait and two generator macros: `newtype_uint!` (for `u64`, `u32`, `usize` wrappers) and `newtype_f64!` (for `f64` wrappers). Generated types include `TokenCount`, `ByteCount`, `TimestampMs`, `Count`, `LineCount`, `Temperature`, `UsdCost`, `CostPerMtok`, `WaitSecs`, and many more. Each carries arithmetic operator overloads, serde support, and `Deref` to the inner type. This submodule also contains semantic boolean wrappers (`IsPredicate`, `IsActive`, `IsVisible`, `IsEnabled`, etc.) and string-backed semantic types (`ErrorMessage`, `AccumulatedContent`, `PanelModeLabel`, `BufferThreshold`).

### Core Message and Stream Types

- **`types`** defines the foundational data types used across every actor: `Message` (role, content, timestamp, optional tool call ID and tool calls), `Role` (User/Assistant/System/Tool), `ToolCall`, `LlmUsage` / `LlmTokenCounts` (per-turn token and cost accounting), `StreamChunk` (the per-request streaming event enum: Token, ToolCall, Done, Usage, Error, RateLimitRetry), `ProjectTokenTotals` (accumulated session totals), `ContextUsageStats`, and `MessageRecord` (a `Message` paired with a `MessageType` tag for persistence). This submodule also defines the high-level event enums `AgentOutput`, `SupervisorEvent`, `CommandOutcome`, `AgentFeedOutput`, and the `FeedId`/`FeedEntry`/`RouteResult` types used for feed routing.

### Event Protocol System

- **`events`** defines 11 semantic domain event types (`SessionInfo`, `SessionStarted`, `SessionResumed`, `SnapshotRewind`, `Reasoning`, `ToolRequested`, `ExternalToolRequest`, `PermissionRequest`, `HookStarted`, `HookCompleted`, `SkillInvoked`) that represent distinct Copilot SDK session events. Each type carries structured metadata that cannot be represented by existing generic types (Message, ToolCall, AgentOutput). This module also provides the complete event inventory mapping (`inventory`) categorizing all 41 `SessionEventData` variants, and the protocol definitions in `protocols`.

### Plan Tree and State

- **`plan_tree`**, **`plan_state`**, and **`guided_plan`** define the hierarchical plan execution model. `PlanTree` and `PlanNodeId` represent the tree structure of guided plans. `PlanState` tracks execution progress through the tree. `GuidedPlan` types support the phase/hook model for step-by-step guided execution. These types are consumed by the supervisor actor, executor actor, and TUI plan panel.

### Tool System

- **`tool_types`** defines `ToolDefinition` (name, description, JSON Schema parameters) and `ToolCallResult` (output, error flag, session log) - the fundamental types that describe what tools are available and what their execution produces.
- **`tool_call_formatting`** handles formatting and normalization of tool call data.
- **`traits`** defines `ToolExecutor`, the async trait that all tool execution backends implement.

### Agent Specification and Task Types

- **`agent_spec_parser`** handles parsing agent specifications from configuration.
- **`task_types`** and **`task_types_step_artifact`** define the task execution model and step-level artifact tracking.

### Data Flow and Lifecycle Infrastructure

- **`background_events`** provides the priority-based event classification system (`BackgroundEventPriority::Critical/Informational/Debug`, `BackgroundPanelMode`), the `DeltaAccumulator` for streaming token buffering, `ToolExecutionMetadata`/`ToolExecutionResult` for tool lifecycle tracking, `ToolExecutionContext` for context management, and the deterministic `classify_event_priority` function.
- **`context_management`** defines `CompactionConfig`, `CompactionPipelineContext`, and `SessionSnapshot` for context window management and message compaction.
- **`feeds`** defines typed feed channel message enums (`LlmFeedMessage`, `UserFeedMessage`, `HistoryFeedMessage`) with semantic tags for routing.
- **`channels`** provides channel capacity constants used by actor channel creation.
- **`scheduler`** and **`stream_state`** define scheduling types and stream processing state.
- **`reply_events`** defines reply/response event types for the turn lifecycle.
- **`thinking_mode`** defines `ReasoningEffort` and related types for LLM thinking/reasoning configuration.
- **`effort_level`** defines effort tier enums.
- **`dag_validation`** provides types for validating directed acyclic graph structures.
- **`endpoint_model_catalog`** defines endpoint-to-model catalog relationships.
- **`lsp`** contains LSP-related types.
- **`actor_contracts`** defines shared actor handle and command contracts (`TokenTrackerHandle`, `LoggerHandle`, `HistoryAdapterHandle`) with their command enums.

## Role in the Ecosystem

The `domain` module is the crate's center of gravity and the architectural keystone of the entire workspace. It defines every data model, every trait contract, and every semantic wrapper that other crates rely on. Because it has no runtime dependencies on actors, networking, or Tokio, any consumer - from provider adapters to the TUI to test harnesses - can depend on it without pulling in heavyweight infrastructure. The semantic newtype system enforced here ensures that primitive types cannot be accidentally interchanged across call sites throughout the entire codebase.