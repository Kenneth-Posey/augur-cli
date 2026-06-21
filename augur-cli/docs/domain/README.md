# augur-domain

This crate defines shared domain types, traits, and contracts consumed by all other crates. It contains no runtime actors. It provides semantic newtypes, event protocols, plan tree and state types, tool definitions and execution contracts, context management data structures, background event types, scheduling, DAG validation, effort levels, stream state, thinking mode, and channel constants.## Documents

- [Crate Overview](crate-overview.docs.md) -- Architecture, major subsystems, and design decisions for the augur-domain crate.
- [Actors](actors.docs.md) -- Actor-handle contracts, conversation history, and inline tool executor.
- [Config](config.docs.md) -- Application configuration schema, provider catalog types, and YAML loaders.
- [Domain](domain.docs.md) -- Semantic newtypes, core message and stream types, event protocols, plan tree/state, background events, feeds, and data flow infrastructure.
- [Persistence](persistence.docs.md) -- Session storage model, async persistence handle, and filesystem I/O.
- [Tools](tools.docs.md) -- Tool definitions, execution contracts, handler trait, registry, and builtin tools.