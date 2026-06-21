# augur-core Crate Overview

`augur-core` is the largest crate in the workspace and houses the majority of
runtime actor implementations, services, and infrastructure that drive the
application. It sits between the thin `augur-app` bootstrap layer and the
shared `augur-domain` type layer: it imports domain types and traits from
`augur-domain`, wires them into concrete actors and handlers, and exposes
the handles and interfaces that the application entrypoint assembles into a
running system. The crate does not own the composition root - that lives in
`augur-app` - but it provides every runtime building block that the
composition root instantiates and connects.

The largest subsystem is the actor runtime, a collection of concurrent service
actors that carry out the application's core workflows. These actors fall into
three broad roles: dispatch and state management (the agent turn loop, session
handling, LLM dispatch, tool execution, message ingestion, and model selection),
filesystem and external access (sandboxed file I/O, shell command execution,
LSP integration, and caching), and observability and plan orchestration
(structured logging, token budget tracking, conversation history formatting,
phased plan execution, supervision checkpointing, and background agent
dispatch for plan-driven workflows). Together they form a cooperative
runtime where each actor owns a single responsibility and communicates with
others through message passing.

The remaining subsystems provide configuration, tooling, and persistence
infrastructure. The config and persistence modules handle YAML-based
application settings, provider endpoint catalog discovery, user preferences,
session save and load operations, and plan file storage on disk. The tool
system defines a handler trait and registry that map tool names to their
implementations, furnishing more than twenty built-in tools for file
operations, shell execution, LSP queries, agent spawning, user queries,
cache management, and approval gates. Supporting this are crate-level macros,
a token history tracker, a rustdoc parsing utility for extracting
documentation from source files, and a suite of test helpers that supply
fake actor implementations for deterministic testing across all major actor
roles.