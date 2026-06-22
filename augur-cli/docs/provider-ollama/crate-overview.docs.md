# augur-provider-ollama Crate Overview

The `augur-provider-ollama` crate is a thin adapter that connects the
application's LLM request pipeline to a locally-running Ollama instance.
Ollama serves open-weight models - such as Llama, Mistral, and Gemma - via
a REST API that mirrors the OpenAI chat completions format at the
`/v1/chat/completions` endpoint. This crate translates the application's
streaming completion requests into that wire format and returns model token
output as a stream of typed chunks, exactly as the other provider crates do.
Because the integration surface is nearly identical to the OpenAI-compatible
protocol, the crate delegates all request construction, HTTP transport, and
SSE parsing to the shared provider infrastructure, keeping its own codebase to
a minimal re-export layer.

The crate relies entirely on `augur-provider-shared` for runtime behavior.
Specifically, it re-exports `stream_ollama_complete`, a function defined in
the shared crate's `ollama` module that calls `stream_openai_compat` without
a bearer token. This means the Ollama adapter inherits the same streaming
semantics, error handling, and retry logic used by the OpenAI and OpenRouter
provider crates, differing only in the absence of authentication credentials.
For a developer tracing the request path, the flow moves from the LLM actor
through the provider dispatch to `augur-provider-shared::ollama::stream_complete`,
which builds an OpenAI-format JSON body, sends it to the local Ollama server,
and parses the server-sent event stream into `StreamChunk::Token`,
`StreamChunk::Usage`, and `StreamChunk::Done` chunks that the consumer
processes uniformly.

Because Ollama runs as a local process and does not require API keys or bearer
tokens, the crate never configures credentials or attaches authorization
headers to requests. This distinguishes it from the Anthropic, OpenAI, and
OpenRouter provider crates, which each carry endpoint-specific authentication
logic. The only configuration needed at the application level is the base URL
pointing to the local Ollama server - typically `http://localhost:11434` -
which is supplied through the shared `EndpointConfig` type alongside the model
name. From the perspective of the application's runtime, the Ollama provider
is interchangeable with any other backend: the same `RequestContext`,
`GenerationParams`, and reply channel types drive the call, and the same
`StreamChunk` types carry the response back to the consumer.