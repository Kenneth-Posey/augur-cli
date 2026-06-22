//! Shared HTTP retry helpers for provider crates.

use augur_domain::domain::newtypes::{Count, NumericNewtype, WaitSecs};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};

/// Maximum number of retry attempts on an HTTP 429 rate-limit response.
///
/// Both Anthropic and OpenAI providers loop up to this many times before
/// giving up and sending `StreamChunk::Error` to the caller.
pub const MAX_RETRY_ATTEMPTS: usize = 5;

/// HTTP status code indicating the client has been rate-limited by the API.
///
/// Used in `anthropic::send_with_retry` and `openai::send_with_retry` to
/// detect a rate-limit response and trigger either a server-supplied wait
/// or exponential backoff depending on the error body.
pub const HTTP_RATE_LIMIT_STATUS: u16 = 429;

/// Default wait duration when a 429 response lacks a `Retry-After` header.
///
/// Units: whole seconds. Consumed by `parse_retry_after` when the header is
/// absent and the body does not contain a "requests exceeded" error.
const DEFAULT_RETRY_WAIT_SECS: WaitSecs = WaitSecs::of(60);

/// Hard ceiling on the wait duration extracted from `Retry-After`.
///
/// Prevents an unexpectedly large server-supplied value from blocking the
/// agent for longer than this cap. Consumed by `parse_retry_after` for
/// non-requests-exceeded 429 responses.
const MAX_RETRY_WAIT_SECS: WaitSecs = WaitSecs::of(120);

/// Initial backoff duration for "requests exceeded" exponential backoff.
///
/// The first retry attempt waits this long; each subsequent attempt doubles
/// via `BACKOFF_FACTOR`. Starting at 60 seconds gives the API time to recover
/// from quota exhaustion. Units: whole seconds.
/// Consumed by `compute_backoff_wait`.
pub const BACKOFF_INITIAL_SECS: WaitSecs = WaitSecs::of(60);

/// Multiplier applied to the backoff wait on each successive "requests exceeded" retry.
///
/// A factor of 2 produces the sequence 60s → 120s → 240s → 480s → 960s
/// across five attempts. Dimensionless. Consumed by `compute_backoff_wait`.
pub const BACKOFF_FACTOR: u32 = 2;

/// Extract the retry wait duration in seconds from a 429 response.
///
/// Reads the `Retry-After` header and parses it as an integer number of
/// seconds. Falls back to `DEFAULT_RETRY_WAIT_SECS` when the header is
/// absent or unparseable, and caps the result at `MAX_RETRY_WAIT_SECS`.
/// Called by both providers when the 429 body does NOT contain a
/// "requests exceeded" error (those use `compute_backoff_wait` instead).
pub fn parse_retry_after(response: &reqwest::Response) -> WaitSecs {
    let secs = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| DEFAULT_RETRY_WAIT_SECS.inner())
        .min(MAX_RETRY_WAIT_SECS.inner());
    WaitSecs::new(secs)
}

/// Compute the exponential backoff delay for a given retry attempt.
///
/// Returns `BACKOFF_INITIAL_SECS * BACKOFF_FACTOR^attempt`. Attempt 0 returns
/// the initial 60-second wait; each subsequent attempt doubles the duration:
/// 60s → 120s → 240s → 480s → 960s across five attempts.
/// Called by both provider `send_with_retry` functions when the 429 response
/// body is identified as a "requests exceeded" error by `is_requests_exceeded`.
pub fn compute_backoff_wait(attempt: Count) -> WaitSecs {
    let factor = BACKOFF_FACTOR.pow(attempt.inner() as u32) as u64;
    WaitSecs::new(BACKOFF_INITIAL_SECS.inner() * factor)
}

/// Returns `true` when a 429 response body signals a model requests quota error.
///
/// Matches the substring "requests exceeded" case-insensitively. Called by
/// both provider `send_with_retry` functions to distinguish quota-exhaustion
/// retries (which use `compute_backoff_wait`) from other 429 responses (which
/// use the server-supplied `Retry-After` header via `parse_retry_after`).
pub fn is_requests_exceeded(body: &OutputText) -> bool {
    body.as_str().to_lowercase().contains("requests exceeded")
}
