use augur_domain::domain::newtypes::{Count, WaitSecs};
use augur_domain::domain::string_newtypes::{AccumulatedText, OutputText};
use augur_domain::{NumericNewtype, StringNewtype};
use augur_provider_shared::{
    BACKOFF_INITIAL_SECS, SseChunk, compute_backoff_wait, drain_complete_sse_lines,
    is_requests_exceeded, parse_retry_after,
};

#[test]
fn compute_backoff_wait_returns_initial_on_attempt_zero() {
    let wait = compute_backoff_wait(Count::new(0));
    assert_eq!(
        wait.inner(),
        BACKOFF_INITIAL_SECS.inner(),
        "attempt 0 must return the initial backoff duration"
    );
}

#[test]
fn compute_backoff_wait_doubles_each_attempt() {
    let w0 = compute_backoff_wait(Count::new(0)).inner();
    let w1 = compute_backoff_wait(Count::new(1)).inner();
    let w2 = compute_backoff_wait(Count::new(2)).inner();
    assert_eq!(w1, w0 * 2, "attempt 1 must be 2x attempt 0");
    assert_eq!(w2, w0 * 4, "attempt 2 must be 4x attempt 0");
}

#[test]
fn is_requests_exceeded_checks_expected_phrases() {
    assert!(is_requests_exceeded(&OutputText::from(
        r#"{"error":"requests exceeded"}"#,
    )));
    assert!(is_requests_exceeded(&OutputText::from(
        r#"{"error":{"message":"Number of model requests exceeded your limit"}}"#,
    )));
    assert!(is_requests_exceeded(&OutputText::from("REQUESTS EXCEEDED")));
    assert!(!is_requests_exceeded(&OutputText::from(
        r#"{"error":"rate limited"}"#,
    )));
}

#[test]
fn drain_complete_sse_lines_carries_partial_lines_between_chunks() {
    let mut carry = AccumulatedText::from("");
    let lines = drain_complete_sse_lines(&mut carry, SseChunk::from(b"data: hel".as_ref()));
    assert!(lines.is_empty());
    assert_eq!(carry, "data: hel");

    let lines = drain_complete_sse_lines(&mut carry, SseChunk::from(b"lo\ndata: world\n".as_ref()));
    assert_eq!(
        lines,
        vec![
            OutputText::from("data: hello"),
            OutputText::from("data: world")
        ]
    );
    assert!(carry.as_str().is_empty());
}

#[tokio::test]
async fn parse_retry_after_parses_numeric_header() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(429)
        .with_header("retry-after", "17")
        .create();
    let response = reqwest::get(server.url()).await.expect("response");
    assert_eq!(parse_retry_after(&response), WaitSecs::new(17));
}

#[tokio::test]
async fn parse_retry_after_defaults_and_clamps() {
    let mut server = mockito::Server::new_async().await;
    let _m1 = server.mock("GET", "/a").with_status(429).create();
    let _m2 = server
        .mock("GET", "/b")
        .with_status(429)
        .with_header("retry-after", "999")
        .create();
    let response_default = reqwest::get(format!("{}/a", server.url()))
        .await
        .expect("response");
    assert_eq!(parse_retry_after(&response_default), WaitSecs::new(60));
    let response_clamped = reqwest::get(format!("{}/b", server.url()))
        .await
        .expect("response");
    assert_eq!(parse_retry_after(&response_clamped), WaitSecs::new(120));
}
