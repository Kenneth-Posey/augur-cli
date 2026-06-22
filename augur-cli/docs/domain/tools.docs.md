# Tools

The `tools` module defines the tool system contracts: how tools are defined, how they are executed, how results are returned, and how tools are registered and discovered at runtime. It contains four submodules plus a `builtin` directory with concrete tool implementations.

## Key Components

- **`definition`**: Re-exports `ToolDefinition` from `domain::tool_types`. `ToolDefinition` is the canonical schema for a tool's interface - it carries a unique `ToolName`, a human-readable `ToolDescription` sent to the LLM explaining when to call the tool, and a JSON Schema `parameters` object describing the expected arguments. Every tool in the system has one `ToolDefinition` that is registered at startup and sent to LLM API requests in the `tools`/`functions` array.

- **`handler`**: Defines the `ToolHandler` trait, the async contract that every tool implementation must satisfy. Implementors provide a `definition()` method returning their `ToolDefinition` and an `async execute(args)` method returning a `ToolCallResult`. The trait is `Send + Sync + 'static` so handlers can be boxed and stored in the registry for concurrent access.

- **`registry`**: Provides `ToolRegistry`, the central tool lookup table. Tools are registered via `register(impl ToolHandler)` which stores both the handler box and its definition. `definitions()` returns all registered schemas for LLM API requests, and `find(name)` resolves a tool name to its handler for execution. The registry is wrapped in `Arc` by `InlineToolExecutor` in the `actors` module.

- **`execution`**: Provides utility functions for normalizing tool execution results. `normalize_tool_execution_result` converts fallible `anyhow::Result<ToolCallResult>` values into a well-formed `ToolCallResult` with the error flag set, ensuring that transport-level failures (network timeouts, deserialization errors) produce a valid tool result rather than panicking in the agent loop. `tool_result_message` builds a `Message::Tool` from a `ToolCall` and its result. The module also includes email redaction logic (`redact_email_addresses`) applied to tool outputs before they are returned to the LLM.

- **`builtin`**: Contains concrete tool implementations distributed with the application. `query_user` implements the tool that pauses agent execution to ask the user a question and wait for a response. `spawn_agent` implements the tool that launches a background agent subtask. Both are registered in the composition root at startup.

## Role in the Ecosystem

The tool system is the primary extension point for adding new capabilities to the agent. Provider crates register tool definitions in their `ToolRegistry` at composition time, and the agent actor invokes them by name as the LLM requests them. The separation between `ToolDefinition` (what the LLM sees) and `ToolHandler` (the execution logic) allows the two to evolve independently - a tool's schema can change without altering its implementation, and vice versa. The built-in tools (`query_user`, `spawn_agent`) are used by both the direct agent actor and the Copilot SDK executor, providing consistent user-interaction and agent-spawning behavior across all backends.