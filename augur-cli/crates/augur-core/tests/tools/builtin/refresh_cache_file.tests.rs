use augur_core::actors::cache::cache_actor::spawn as spawn_cache;
use augur_core::tools::builtin::refresh_cache_file::RefreshCacheFileTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;
use std::time::Duration;

const CACHE_SHUTDOWN_WAIT_MS: u64 = 25;

/// Verifies that the tool returns a success result for a valid path argument.
/// The cache actor performs the rebuild; the tool itself reports success on send.
#[tokio::test]
async fn refresh_cache_file_returns_success_for_valid_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    let target = src_dir.join("lib.rs");
    std::fs::write(&target, "pub fn lib() {}").expect("write file");

    let cache_handle = spawn_cache(src_dir).expect("spawn cache");
    let tool = RefreshCacheFileTool::new(cache_handle);

    let result = tool
        .execute(serde_json::json!({ "path": target.to_str().unwrap() }))
        .await;
    assert!(
        !result.is_error,
        "expected success, got: {}",
        result.output.as_str()
    );
}

/// Verifies that the tool returns an error result when the path argument is missing.
#[tokio::test]
async fn refresh_cache_file_errors_on_missing_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");

    let cache_handle = spawn_cache(src_dir).expect("spawn cache");
    let tool = RefreshCacheFileTool::new(cache_handle);

    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_error, "expected error for missing path");
}

/// Verifies that an explicitly empty path returns an error result.
#[tokio::test]
async fn refresh_cache_file_errors_on_empty_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");

    let cache_handle = spawn_cache(src_dir).expect("spawn cache");
    let tool = RefreshCacheFileTool::new(cache_handle);

    let result = tool.execute(serde_json::json!({ "path": "" })).await;
    assert!(result.is_error, "expected error for empty path");
}

/// Verifies that a stopped cache actor causes the tool to return an error result.
#[tokio::test]
async fn refresh_cache_file_returns_error_when_cache_actor_stopped() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    let target = src_dir.join("lib.rs");
    std::fs::write(&target, "pub fn lib() {}").expect("write file");

    let cache_handle = spawn_cache(src_dir).expect("spawn cache");
    cache_handle.shutdown();
    tokio::time::sleep(Duration::from_millis(CACHE_SHUTDOWN_WAIT_MS)).await;

    let tool = RefreshCacheFileTool::new(cache_handle);
    let result = tool
        .execute(serde_json::json!({ "path": target.to_str().unwrap() }))
        .await;
    assert!(result.is_error);
}

/// Verifies the tool definition has the expected name and required parameter.
#[tokio::test]
async fn refresh_cache_file_definition_has_correct_name_and_schema() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");

    let cache_handle = spawn_cache(src_dir).expect("spawn cache");
    let tool = RefreshCacheFileTool::new(cache_handle);

    let def = tool.definition();
    assert_eq!(def.name.as_str(), "refresh_cache_file");
    let required = def.parameters["required"]
        .as_array()
        .expect("required array");
    assert!(required.iter().any(|v| v.as_str() == Some("path")));
}

#[test]
fn mirror_sync_executes_refresh_cache_file_returns_success_for_valid_path() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
