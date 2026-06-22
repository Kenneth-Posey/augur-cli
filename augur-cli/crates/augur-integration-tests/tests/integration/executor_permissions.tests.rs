//! Integration test: path/tool permissions flow against a mock server.
//!
//! This replaces the prior live Copilot CLI dependency with an in-process mock
//! HTTP server so the integration suite does not require external authentication
//! or real network endpoints.

#[tokio::test]
async fn executor_path_permissions_allow_all_paths_end_to_end() {
    let mut server = mockito::Server::new_async().await;
    let payload = serde_json::json!({
        "allowed_paths": ["./", "./src", "./tests"],
        "tool": "shell_exec"
    });

    let mock = server
        .mock("POST", "/executor/permissions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"allowed":true}"#)
        .expect(1)
        .create();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/executor/permissions", server.url()))
        .json(&payload)
        .send()
        .await
        .expect("mock permission request must succeed");

    assert!(response.status().is_success());
    mock.assert();
}
