use augur_domain::StringNewtype;
use augur_domain::domain::newtypes::{Count, NumericNewtype, WaitSecs};
use augur_domain::domain::string_newtypes::OutputText;
use augur_provider_shared::{
    BACKOFF_FACTOR, BACKOFF_INITIAL_SECS, HTTP_RATE_LIMIT_STATUS, MAX_RETRY_ATTEMPTS,
    compute_backoff_wait, is_requests_exceeded, parse_retry_after,
};

#[test]
fn compute_backoff_wait_grows_exponentially() {
    assert_eq!(compute_backoff_wait(Count::new(0)), BACKOFF_INITIAL_SECS);
    assert_eq!(
        compute_backoff_wait(Count::new(2)),
        WaitSecs::new(BACKOFF_INITIAL_SECS.inner() * BACKOFF_FACTOR.pow(2) as u64)
    );
}

#[test]
fn is_requests_exceeded_matches_case_insensitively() {
    assert!(is_requests_exceeded(&OutputText::new("Requests Exceeded")));
    assert!(!is_requests_exceeded(&OutputText::new("different error")));
}

#[tokio::test]
async fn parse_retry_after_uses_header_and_cap() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let addr = listener.local_addr().expect("local addr");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buf = [0; 1024];
        let _ = std::io::Read::read(&mut stream, &mut buf);
        use std::io::Write;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-length: 0\r\nretry-after: 135\r\n\r\n"
        )
        .expect("write response");
    });

    let response = reqwest::get(format!("http://{addr}"))
        .await
        .expect("response");
    let wait = parse_retry_after(&response);

    server.join().expect("server");
    assert_eq!(wait, WaitSecs::new(120));
}

#[test]
fn exports_remain_stable() {
    assert_eq!(HTTP_RATE_LIMIT_STATUS, 429);
    assert_eq!(MAX_RETRY_ATTEMPTS, 5);
}
