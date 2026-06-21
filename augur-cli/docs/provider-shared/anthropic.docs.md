# Module: anthropic

Provides Anthropic Messages API body construction and SSE event stream
processing for the Claude streaming completion path.

## Request Construction

The body builder in the `body` submodule converts the domain's `Message` and
`ToolDefinition` types into the Anthropic wire format. System messages are
extracted from the message list and placed in the top-level `"system"` field.
When cache tiers are present (from `CacheSnapshot`), the system field is
rendered as a content-block array with per-tier `cache_control` markers,
enabling Anthropic's prompt caching. Tool definitions use the `"input_schema"`
key required by the Anthropic API rather than the `"parameters"` key used in
the OpenAI-compatible format. The constructor omits the `"tools"` field
entirely when the tools list is empty, because Anthropic rejects
`"tools": []`.

## SSE Event Processing

The streaming loop reads the byte stream from the HTTP response and dispatches
Anthropic-specific SSE events: `message_start` (capturing model name and
cache-read tokens), `content_block_start` (initiating a new tool call slot),
`content_block_delta` (forwarding text deltas as `Token` chunks and
accumulating JSON argument fragments), `content_block_stop` (emitting a
completed `ToolCall` chunk), `message_delta` (capturing prompt and completion
token counts), and `message_stop` (emitting `Usage` then `Done`). The event
type is tracked across consecutive lines because Anthropic sends `event:`
before `data:` in each SSE block.

Tool call arguments are accumulated across multiple content-block deltas,
similar to the OpenAI path. The `EventParseState` struct bundles usage
accumulation (model, token counts) with tool-call state (pending id, name,
and arguments buffer) so the per-event handler stays within the 3-parameter
function limit. Usage is reported once at `message_stop` via
`StreamChunk::Usage` and includes cached-token breakdowns.

## Retry

The Anthropic retry loop mirrors the OpenAI pattern. It sends the API key via
the `x-api-key` header and includes the `anthropic-version: 2023-06-01` header
required by the Anthropic API. On HTTP 429 rate-limit responses, it reads the
`Retry-After` header; when the error body signals a "requests exceeded" quota
error, it switches to exponential backoff. The retry loop emits
`StreamChunk::RateLimitRetry` events on each attempt so the TUI can surface
the wait status to the user.