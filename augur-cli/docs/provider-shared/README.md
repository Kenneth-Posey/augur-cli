# augur-provider-shared

Provides shared utilities consumed by multiple provider crates: Anthropic body construction, generic retry logic with backoff, SSE stream parsing for server-sent events, request context types, and shared wire-protocol helpers for Ollama and OpenAI provider implementations.

## Documents

- [Crate Overview](crate-overview.docs.md) -- Architecture, major subsystems, and design decisions for the augur-provider-shared crate.
- [anthropic](anthropic.docs.md) -- Anthropic Messages API body construction, SSE event processing, and retry loop.
- [ollama](ollama.docs.md) -- Ollama streaming completion via the OpenAI-compatible path.
- [openai](openai.docs.md) -- OpenAI-compatible Chat Completions request construction, SSE stream processing, and retry logic.
- [request_context](request_context.docs.md) -- LLM actor command protocol, validated request context, and API key resolution.
- [retry](retry.docs.md) -- Shared HTTP rate-limit detection and exponential backoff computation.
- [streaming](streaming.docs.md) -- Shared SSE line parsing with carry-buffer handling for split HTTP chunks.