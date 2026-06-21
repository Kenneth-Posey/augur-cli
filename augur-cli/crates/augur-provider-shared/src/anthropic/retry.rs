use crate::retry::{
    compute_backoff_wait, is_requests_exceeded, parse_retry_after, HTTP_RATE_LIMIT_STATUS,
    MAX_RETRY_ATTEMPTS,
};
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use augur_domain::domain::string_newtypes::{ApiKeyValue, OutputText, StringNewtype};
use augur_domain::domain::types::StreamChunk;
use tokio::sync::mpsc;

/// Request bundle for a retrying Anthropic POST.
#[derive(bon::Builder)]
pub(super) struct AnthropicRetryRequest<'a> {
    /// Reply channel used for streamed status and error chunks.
    pub(super) reply_tx: &'a mpsc::Sender<StreamChunk>,
    /// Target provider URL.
    pub(super) url: &'a str,
    /// API key sent via `x-api-key`.
    pub(super) api_key: &'a ApiKeyValue,
    /// Serialized JSON request body.
    pub(super) body_str: &'a str,
}

/// Send an Anthropic request with automatic 429 retry.
///
/// Attempts the POST up to `MAX_RETRY_ATTEMPTS` times. On HTTP 429, reads the
/// `Retry-After` header via `parse_retry_after`, sends `StreamChunk::RateLimitRetry`
/// to notify the TUI, sleeps, then retries. On other non-2xx responses, sends
/// `StreamChunk::Error` and returns `None`. Returns `Some(response)` on the first
/// successful response.
pub(super) async fn send_with_retry(
    request: AnthropicRetryRequest<'_>,
) -> Option<reqwest::Response> {
    let client = reqwest::Client::new();
    for attempt in 0..MAX_RETRY_ATTEMPTS {
        let response = send_anthropic_request(&client, &request).await?;
        let Some(response) = handle_anthropic_rate_limit(attempt, response, request.reply_tx).await
        else {
            continue;
        };
        if response.status().is_success() {
            return Some(response);
        }
        if emit_anthropic_http_error(response, request.reply_tx).await {
            return None;
        }
    }
    let _ = request
        .reply_tx
        .send(StreamChunk::Error(OutputText::new(format!(
            "rate limit: exhausted {} retries",
            MAX_RETRY_ATTEMPTS
        ))))
        .await;
    None
}

async fn send_anthropic_request(
    client: &reqwest::Client,
    request: &AnthropicRetryRequest<'_>,
) -> Option<reqwest::Response> {
    match client
        .post(request.url)
        .header("x-api-key", request.api_key.as_str())
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(request.body_str.to_owned())
        .send()
        .await
    {
        Ok(response) => Some(response),
        Err(error) => {
            let _ = request
                .reply_tx
                .send(StreamChunk::Error(OutputText::new(error.to_string())))
                .await;
            None
        }
    }
}

async fn handle_anthropic_rate_limit(
    attempt: usize,
    response: reqwest::Response,
    reply_tx: &mpsc::Sender<StreamChunk>,
) -> Option<reqwest::Response> {
    if response.status().as_u16() != HTTP_RATE_LIMIT_STATUS {
        return Some(response);
    }
    let header_wait = parse_retry_after(&response);
    let body = response.text().await.unwrap_or_default();
    let wait = if is_requests_exceeded(&OutputText::from(body.as_str())) {
        compute_backoff_wait(Count::new(attempt))
    } else {
        header_wait
    };
    tracing::warn!(
        attempt,
        wait_secs = wait.inner(),
        "Anthropic rate limit - retrying"
    );
    let _ = reply_tx.send(StreamChunk::RateLimitRetry(wait)).await;
    tokio::time::sleep(std::time::Duration::from_secs(wait.inner())).await;
    None
}

async fn emit_anthropic_http_error(
    response: reqwest::Response,
    reply_tx: &mpsc::Sender<StreamChunk>,
) -> bool {
    if response.status().is_success() {
        return false;
    }
    let status = response.status().as_u16();
    let body_text = response.text().await.unwrap_or_default();
    let _ = reply_tx
        .send(StreamChunk::Error(OutputText::new(format!(
            "HTTP {status}: {body_text}"
        ))))
        .await;
    true
}
