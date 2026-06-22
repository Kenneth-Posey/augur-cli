# Module: ollama

Provides the streaming completion entry point for local Ollama instances.

Ollama exposes an OpenAI-compatible Chat Completions API at
`/v1/chat/completions`. The `ollama` module is a thin delegation layer: it
calls `stream_openai_compat(ctx, None)` -- the same core streaming loop used
by the OpenAI provider -- but passes no bearer token, because a local Ollama
instance requires no authentication.

No body construction, SSE parsing, or retry logic lives here. All of those
responsibilities belong to the `openai` module's `stream_openai_compat`
function, which the Ollama module reuses without modification. The Ollama
entry point (`stream_ollama_complete`) is re-exported from the crate root
and consumed by the `augur-provider-ollama` crate. Because the module is
essentially a one-line routing function, most of its behavioral contract
(error handling, rate-limit retry, stream chunk dispatch) is defined and
tested through the shared OpenAI-compatible path.