use augur_core::tools::builtin::file_create::FileCreateTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;

fn make_tool(dir: &tempfile::TempDir) -> FileCreateTool {
    FileCreateTool::new(vec![dir.path().to_path_buf()])
}

#[tokio::test]
async fn execute_creates_file_with_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test_output.txt");
    let path_str = path.to_str().unwrap().to_owned();
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path_str, "content": "test content"}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "test content");
}

#[tokio::test]
async fn execute_missing_args_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = make_tool(&dir);
    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_empty_path_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": "", "content": "x"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_missing_content_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.txt");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path.to_str().unwrap()}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_io_error_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing").join("out.txt");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path.to_str().unwrap(), "content": "x"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_refuses_to_overwrite_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("existing.txt");
    std::fs::write(&path, "old content").unwrap();
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path.to_str().unwrap(), "content": "new content"}))
        .await;
    assert!(!result.is_error, "overwrite refusal should not be an error");
    assert!(
        result.output.as_str().contains("already exists"),
        "should warn about existing file: {}",
        result.output.as_str()
    );
    // Original content should be unchanged
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old content");
}

#[tokio::test]
async fn execute_empty_content_writes_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.txt");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path.to_str().unwrap(), "content": ""}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
}

#[tokio::test]
async fn symlink_create_is_denied_with_permission_error() {
    let allowed = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("secret.txt");
    std::fs::write(&outside_file, "original").unwrap();
    let symlink_path = allowed.path().join("link.txt");
    std::os::unix::fs::symlink(&outside_file, &symlink_path).unwrap();
    let tool = make_tool(&allowed);
    let result = tool
        .execute(serde_json::json!({"path": symlink_path.to_str().unwrap(), "content": "attacked"}))
        .await;
    // Symlink targets are rejected by path validation before the exists check
    assert!(
        result.is_error,
        "create through symlink must be denied, got: {}",
        result.output.as_str()
    );
    assert_eq!(std::fs::read_to_string(&outside_file).unwrap(), "original");
}

#[tokio::test]
async fn execute_creates_new_file_even_when_dir_has_other_files() {
    let dir = tempfile::tempdir().unwrap();
    let existing_path = dir.path().join("existing.txt");
    std::fs::write(&existing_path, "content").unwrap();
    let path = dir.path().join("new_file.txt");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path.to_str().unwrap(), "content": "new"}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}
