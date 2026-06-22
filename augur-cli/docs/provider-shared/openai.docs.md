# Module: openai

Provides OpenAI-compatible Chat Completions request construction, SSE stream
processing, and retry logic shared by the OpenAI and Ollama provider paths.

## Request Construction

The body builder converts domain `Message` and `ToolDefinition` types into the
OpenAI Chat Completions wire format. Tool messages carry a `"tool_call_id"`
field, and assistant messages with tool calls emit `"content": null` alongside
the `"tool_calls"` array -- both conventions required by the OpenAI API. The
request body includes `"stream_options": {"include_usage": true}` so the
provider sends a final usage delta that carries prompt, completion, and cached
token counts. When a session ID is present, it is forwarded as the `"user"`
field (or `"session_id"` for OpenRouter endpoints) so requests are attributable
in the provider's activity log. Extra HTTP headers are injected for OpenRouter
response caching; they are empty for all other providers.

## SSE Stream Processing

The streaming loop reads the SSE byte stream via `drain_complete_sse_lines`
from the `streaming` module. Text deltas in `choices[0].delta.content` are
emitted immediately as `StreamChunk::Token`. Tool call arguments are
accumulated across multiple deltas using an index-based slot system: each
`tool_calls[N]` entry carries an `"index"` field that identifies which
parallel tool call the fragment belongs to, and slots are grown on demand so
out-of-order or sparse indices are handled safely. When `finish_reason` is
`"tool_calls"`, all accumulated slots are drained and emitted as
`StreamChunk::ToolCall` in index order.

The model name is captured from the first chunk that includes a `"model"`
field, and token counts (prompt, completion, cached from
`prompt_tokens_details.cached_tokens`, and cache-write tokens) are updated
from the final usage object. The `finish_stream` function emits
`StreamChunk::Usage` followed by `StreamChunk::Done`, logging the structured
response summary to the `llm_raw` target.

## Retry

The retry loop sends the POST request via `reqwest`, authenticating with a
bearer token when present. On HTTP 429 rate-limit responses, it reads the
`Retry-After` header via `parse_retry_after`; if the error body contains
"requests exceeded", it switches to exponential backoff via
`compute_backoff_wait`. After `MAX_RETRY_ATTEMPTS` (5) exhausted retries, it
emits an error chunk. Non-2xx responses are reported as `StreamChunk::Error`
with the HTTP status code and body text.