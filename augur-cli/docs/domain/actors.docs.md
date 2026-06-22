# Actors

The `actors` module defines the actor-handle and trait abstractions that decouple runtime actors from their domain contracts. It contains four submodules providing concrete handle types, conversation data structures, and executor trait implementations that are used by agent actors, the TUI, and wiring code to communicate with running actor tasks without depending on their concrete types.

## Key Components

- **`active_model`** provides `ActiveModelHandle`, a fire-and-forget handle for setting and querying the active LLM model. It wraps a command channel (`mpsc::Sender`) and a watch channel (`watch::Receiver`) so callers can both push model-change commands and poll the current model synchronously without awaiting. The `ActiveModelCommand` enum carries the `Set(ModelId)` variant used by the `/model` slash command flow.

- **`agent`** provides `ConversationHistory`, the in-memory conversation buffer used by every agent actor. It manages three parallel message collections: the full conversation history, the OpenRouter context window (which may be compacted independently), and an offset-tracked "live" window for incremental request building. Methods like `push`, `set_messages`, and `live_messages_for_request` support compaction, context-window management, and turn-by-turn message assembly.

- **`token_tracker`** re-exports `TokenTrackerHandle` from `domain::actor_contracts`, providing the shared handle type used to submit token usage data and request snapshots from the running token-tracker actor task.

- **`tool`** provides `InlineToolExecutor`, a concrete `ToolExecutor` implementation that wraps a `ToolRegistry` and executes tool calls synchronously (inline) within the agent actor's task. It resolves tool calls by name through the registry and returns results or error messages without spawning separate tasks.

## Role in the Ecosystem

These types form the actor-facing contract layer between the domain crate's shared abstractions and the concrete runtime actors in `augur-core` and `augur-provider-openrouter`. By defining handles as plain structs with `Clone` derives rather than trait objects, they allow wiring code to construct actor handles at composition time without boxing or dynamic dispatch. The `ConversationHistory` type in particular is the single source of truth for conversation state across all agent backends, ensuring that every provider (OpenAI, Anthropic, Ollama, OpenRouter, Copilot SDK) builds requests from the same data structure.