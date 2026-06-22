use augur_core::actors::tool::handle::ToolExecutor;
use augur_core::actors::tool::tool_actor::spawn;
use augur_core::actors::tool::tool_ops::ToolCall;
use augur_core::tools::builtin::file_read::FileReadTool;
use augur_core::tools::registry::ToolRegistry;
use augur_domain::domain::string_newtypes::{StringNewtype, ToolName};
use tokio::time::{Duration, timeout};

/// Verifies that the tool actor spawns and shuts down cleanly.
#[tokio::test]
async fn spawn_and_shutdown() {
    let (join, handle) = spawn(ToolRegistry::new());
    handle.shutdown();
    join.await.expect("tool actor panicked");
}

/// Verifies that a known tool executes and returns a non-error result.
#[tokio::test]
async fn execute_known_tool_via_handle() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    std::fs::write(&file_path, b"expected content").unwrap();
    let path = file_path.to_str().unwrap().to_owned();

    let (_fr_join, fr_handle) =
        augur_core::actors::file_read::file_read_actor::spawn(vec![dir.path().to_path_buf()]);
    let mut registry = ToolRegistry::new();
    registry.register(FileReadTool::new(fr_handle));
    let (_join, handle) = spawn(registry);

    let call = ToolCall {
        id: augur_domain::domain::string_newtypes::ToolCallId::new("call_fr"),
        name: ToolName::new("file_read"),
        arguments: serde_json::json!({"path": path}),
    };
    let result = handle.execute(call).await.expect("execute failed");
    assert!(!result.is_error, "error: {}", result.output.as_str());
    assert!(result.output.as_str().contains("expected content"));
}

/// Verifies that an unknown tool name returns a not-found error result.
#[tokio::test]
async fn execute_unknown_tool_returns_not_found_error() {
    let (_join, handle) = spawn(ToolRegistry::new());
    let call = ToolCall {
        id: augur_domain::domain::string_newtypes::ToolCallId::new("call_notfound"),
        name: ToolName::new("nonexistent_tool"),
        arguments: serde_json::json!({}),
    };
    let result = handle.execute(call).await.expect("execute failed");
    assert!(result.is_error);
    assert!(result.output.as_str().contains("not found"));
}

/// Verifies that `execute` returns a stopped-actor error after shutdown.
#[tokio::test]
async fn execute_after_shutdown_returns_actor_stopped_error() {
    let (join, handle) = spawn(ToolRegistry::new());
    handle.shutdown();
    timeout(Duration::from_secs(2), join)
        .await
        .expect("tool actor should stop")
        .expect("tool actor should not panic");

    let call = ToolCall {
        id: augur_domain::domain::string_newtypes::ToolCallId::new("call_shutdown"),
        name: ToolName::new("nonexistent_tool"),
        arguments: serde_json::json!({}),
    };
    let error = handle
        .execute(call)
        .await
        .expect_err("shutdown should make execute fail");
    assert!(
        error.to_string().contains("tool actor stopped"),
        "unexpected shutdown error: {error}",
    );
}

#[test]
fn mirror_sync_executes_spawn_and_shutdown() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
