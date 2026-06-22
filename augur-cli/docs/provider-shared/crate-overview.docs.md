# Crate Overview: augur-provider-shared

The `augur-provider-shared` crate houses the cross-cutting protocol and
retry machinery that the per-provider crates (Anthropic, OpenAI, Ollama,
OpenRouter) would otherwise duplicate. Rather than each provider
reimplementing SSE line parsing, rate-limit backoff, or JSON body
construction, they depend on this crate for a shared implementation.
The three single-endpoint provider crates (`augur-provider-anthropic`,
`augur-provider-openai`, `augur-provider-ollama`) re-export their
`stream_complete` entry point directly from this crate, making it the
de facto implementation surface for their core streaming loop.

The shared protocol utilities form the largest responsibility group. The
`openai` module builds the Chat Completions request body from the domain's
`Message` and `ToolDefinition` types, drives the SSE streaming response
with `drain_complete_sse_lines` from the `streaming` module, accumulates
tool call arguments across multiple deltas, and emits typed
`StreamChunk` events for text tokens, tool calls, usage metadata, and
stream termination. The `retry` module provides a uniform rate-limit
handling strategy: it parses the `Retry-After` header, detects
"requests exceeded" error bodies for exponential backoff, and caps wait
durations so a misbehaving server never blocks the agent indefinitely.
Both the Anthropic and OpenAI retry loops consume these same shared
functions, ensuring consistent behavior across providers.

Anthropic-specific helpers live in the `anthropic` submodule. The body
constructor builds system message blocks with per-tier `cache_control`
markers, extracting the system text from the message list and
converting tool definitions and conversation messages into the
Anthropic wire format. The Anthropic retry loop mirrors the OpenAI
pattern but sends the API key as the `x-api-key` header and uses the
`anthropic-version` header required by the Anthropic API. The `ollama`
module is the smallest piece: it delegates directly to the OpenAI-
compatible path, passing no bearer token since a local Ollama instance
requires no authentication. The `request_context` module ties these
pieces together by defining the shared `RequestContext` struct,
`LlmCommand` enum for the actor dispatch protocol, and the
`build_request_context` function that validates endpoint configuration
against `AppConfig` and resolves API keys before a request reaches any
provider code.