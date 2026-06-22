# Module: retry

Provides shared HTTP rate-limit detection and backoff computation for the
Anthropic and OpenAI provider retry loops.

Both providers follow the same retry strategy: attempt the POST up to 5 times
(`MAX_RETRY_ATTEMPTS`), detecting HTTP 429 rate-limit responses. When the 429
error body contains the substring "requests exceeded" (matched
case-insensitively by `is_requests_exceeded`), the retry uses exponential
backoff starting at 60 seconds (`BACKOFF_INITIAL_SECS`) and doubling each
attempt via `compute_backoff_wait`. For other 429 responses (for example,
per-minute rate limits), the module reads the server-supplied `Retry-After`
header via `parse_retry_after`, falling back to a 60-second default
(`DEFAULT_RETRY_WAIT_SECS`) and capping at 120 seconds
(`MAX_RETRY_WAIT_SECS`) to prevent a misbehaving server from blocking the
agent indefinitely.

All functions are pure computation -- they do not perform HTTP calls or
manage sleep timers. The provider-specific `send_with_retry` functions (in
the `anthropic::retry` and `openai` modules) orchestrate the actual retry
loop: they call the shared functions to determine wait durations, emit
`StreamChunk::RateLimitRetry` events so the TUI can surface wait status,
sleep via `tokio::time::sleep`, and retry the request. This separation
keeps the backoff logic independently testable while the provider modules
own HTTP dispatch and stream lifecycle.