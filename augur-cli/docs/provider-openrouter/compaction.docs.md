# OpenRouter Message Compaction

## Scope

Documents the compaction utilities in `crates/augur-provider-openrouter/src/compaction.rs`. These functions manage OpenRouter-specific context-window budgeting by stripping tool-result bodies and dropping oldest conversation turns to keep requests within the provider's token limit. This module does not cover general message formatting or serialization -- those concerns belong to the shared provider layer.

## Key Components

The module exposes a family of `compact_messages_*` functions, all following the same two-phase strategy: first strip the body of the oldest tool-result messages (a pre-compaction pass that reclaims bulk context while preserving conversation structure), then drop entire turns (user/assistant pairs) from oldest to newest until the estimated token count falls under the compaction threshold. Only when no more turns remain does it fall back to dropping leading instruction-prefix messages.

The `build_openrouter_message_compactor()` function creates a `MessageCompactor` closure that resolves per-model compaction target and strip fraction from the provider catalog at call time, then always applies compaction regardless of the current budget (no early-exit). This is used by the `/compact` command. Token estimation uses a heuristic combining word-count and character-count estimates, lower-bounded to one token.

## Data Flow

1. A set of `Message` values enters a compaction function along with a threshold and a strip fraction.
2. The messages are parsed into a `MessagePlan`: leading prefix messages (instruction blocks), an optional system prompt, and conversation turns.
3. The pre-compaction pass iterates over the oldest `fraction` of `Role::Tool` messages and empties their content body.
4. The main loop builds a candidate message list from the remaining plan, estimates its total tokens, and either returns it (under budget) or drops the next oldest turn or prefix.
5. When turns or prefixes are dropped, a system note is injected explaining what was omitted.

## Contracts and Invariants

- The leading system prompt (the last `Role::System` message before the first non-system message) is always preserved. Only instruction-prefix messages before it may be dropped, and only after all turns have been exhausted.
- `compact_messages_for_openrouter` has an early exit: if the raw message list is already under budget, it returns unchanged. `compact_messages_for_openrouter_forced` bypasses this check so `/compact` always reclaims space.
- The default context budget is 400,000 tokens, overridable via the `AUGUR_CLI_OPENROUTER_CONTEXT_BUDGET_TOKENS` environment variable.
- The default tool-result strip fraction is 0.9 (90% of oldest tool-result bodies are stripped).

## Validation

Unit tests exercise the compaction logic indirectly via the `model_config` module's integration with per-model parameters. The primary validation comes from integration tests that verify the `/compact` command and automatic compaction during multi-turn agent conversations. Token estimation heuristics are validated against expected OpenRouter behavior in end-to-end test scenarios.

## References

- Source: `crates/augur-provider-openrouter/src/compaction.rs`
- Model config resolution (provides per-model thresholds): [model_config.docs.md](model_config.docs.md)
- The `MessageCompactor` trait consumed by the agent actor is defined in `augur_domain::domain::task_types`