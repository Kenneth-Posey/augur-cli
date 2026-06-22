# OpenRouter Provider Actors

## Scope

Documents the three actor subsystems exposed by `crates/augur-provider-openrouter/src/actors/`. These actors implement OpenRouter-specific runtime behavior: an LLM actor for dispatching completion requests through the OpenRouter gateway, an orchestrator actor for managing multi-step agent task lifecycle, and a task actor for executing individual task runs. The wiring layer in `src/wiring.rs` owns construction of these actors from configuration.

## Key Components

The `llm` submodule provides the `LlmActor` (spawned via `spawn()`), which receives `LlmCommand` messages over an mpsc channel and dispatches each completion request as an independent tokio task. It injects OpenRouter-specific HTTP headers (cache control, `X-OpenRouter-Title`, `HTTP-Referer`) and session metadata into the request context, then routes the actual streaming call to the provider-specific backend (OpenRouter's own OpenAI-compatible adapter, or the standard OpenAI/Anthropic/Ollama adapters for routed models). A cloneable `LlmHandle` is returned to callers.

The `openrouter_orchestrator` submodule owns the `OpenRouterOrchestratorActor`, which manages a queue of pending task runs subject to a configurable parallel limit. Its command loop accepts `EnqueueSpawn`, `TransitionToActive`, `TerminalResult`, `AwaitRun`, `AwaitAny`, `QueryStatus`, `ResetSession`, and `Shutdown` commands. The orchestrator maintains a `RunLifecycleLedger` tracking pending, active, terminal, and consumed run states, and uses a session generation counter to invalidate stale work on session reset.

The `openrouter_task` submodule provides the `OpenRouterTaskActor` that executes individual task runs. It loads instruction files from disk via `instruction_loader` and reads agent specification files from the agent directory via `spec_loader`. Each spawned task receives its configuration (allowed directories, instruction prefix, repository root, agent spec base path) from the orchestrator's shared config.

## Execution Flow

1. An external request arrives at the orchestrator as an `EnqueueSpawn` command with a `SpawnAgentRequest` payload.
2. The orchestrator records the run as pending, sends an acknowledgement with the dispatch status, and enqueues the spawn request.
3. As capacity allows, the orchestrator dequeues spawns, transitions them to active, and spawns a `OpenRouterTaskActor` for each.
4. The task actor loads the agent spec and instruction files, runs the agent loop against the LLM actor via `LlmHandle`, and reports results back through the correlation channels.
5. On terminal result, the orchestrator records the outcome, removes the join handle, satisfies any awaiting waiters, and dispatches the next queued run.

## Contracts and Invariants

- The LLM actor never blocks its run loop on network I/O -- each completion request is dispatched as an independent tokio task.
- The orchestrator's session generation counter is monotonic and saturating; a `ResetSession` command aborts all active joins and clears all pending/active/terminal state.
- The orchestrator accepts lifecycle events (`TransitionToActive`, `TerminalResult`) only for runs it knows about (pending or active). Stale events from previous sessions are silently ignored.
- `AwaitRun` and `AwaitAny` are one-shot: the first matching terminal result is consumed and sent. If the run is still pending or active, the waiter is deferred until the run completes.

## Validation

Actor behavior is validated through integration tests that exercise the full spawn -- dispatch -- await -- result lifecycle. The LLM actor's header injection and routing logic is covered by unit tests in the provider-specific submodules. The orchestrator's lifecycle transitions and queue management are tested via synthetic command sequences that verify ledger state after each transition.

## References

- Source: `crates/augur-provider-openrouter/src/actors/` (mod.rs, `llm/`, `openrouter_orchestrator/`, `openrouter_task/`)
- Wiring and construction: `src/wiring.rs`
- Shared types for task lifecycle: `augur_domain::task_types`
- Compaction consumed by task actor: [compaction.docs.md](compaction.docs.md)
- Per-model configuration consumed by LLM dispatch: [model_config.docs.md](model_config.docs.md)