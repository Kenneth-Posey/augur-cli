# augur-provider-openrouter

Implements the OpenRouter provider integration with its own LLM actor for model routing, orchestrator actor for multi-step task management, and task actor for instruction and specification loading. This crate handles OpenRouter-specific API semantics including caching and routing.

## Documents

- [Crate Overview](crate-overview.docs.md) -- Architecture, major subsystems, and design decisions for the augur-provider-openrouter crate.
- [Message Compaction](compaction.docs.md) -- Context-window budgeting via tool-result stripping and turn dropping.
- [Model Configuration](model_config.docs.md) -- Per-model parameter resolution from provider catalog YAML files.
- [Provider Actors](actors.docs.md) -- LLM actor, orchestrator actor, and task actor wiring and lifecycle.