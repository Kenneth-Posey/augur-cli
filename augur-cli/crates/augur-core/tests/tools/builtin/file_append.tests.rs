use augur_core::tools::builtin::file_append::FileAppendTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;

fn make_tool(dir: &tempfile::TempDir) -> FileAppendTool {
    FileAppendTool::new(vec![dir.path().to_path_buf()])
}

#[tokio::test]
async fn execute_appends_to_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("append_output.txt");
    let path_str = path.to_str().unwrap().to_owned();
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path_str, "content": "first line\n"}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "first line\n");
}

#[tokio::test]
async fn execute_appends_to_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("append_existing.txt");
    std::fs::write(&path, "existing content\n").unwrap();
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path.to_str().unwrap(), "content": "appended line"}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "existing content\nappended line");
}

#[tokio::test]
async fn execute_multiple_appends() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("append_multiple.txt");
    let tool = make_tool(&dir);
    tool.execute(serde_json::json!({"path": path.to_str().unwrap(), "content": "line1\n"}))
        .await;
    tool.execute(serde_json::json!({"path": path.to_str().unwrap(), "content": "line2\n"}))
        .await;
    tool.execute(serde_json::json!({"path": path.to_str().unwrap(), "content": "line3"}))
        .await;
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "line1\nline2\nline3");
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
async fn symlink_append_is_denied() {
    let allowed = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("secret.txt");
    std::fs::write(&outside_file, "original").unwrap();
    let symlink_path = allowed.path().join("link.txt");
    std::os::unix::fs::symlink(&outside_file, &symlink_path).unwrap();
    let tool = make_tool(&allowed);
    let result = tool
        .execute(serde_json::json!({"path": symlink_path.to_str().unwrap(), "content": "attached"}))
        .await;
    assert!(
        result.is_error,
        "append through symlink must be denied, got: {}",
        result.output.as_str()
    );
    assert!(
        result.output.as_str().contains("permission denied"),
        "error must contain 'permission denied', got: {}",
        result.output.as_str()
    );
    assert_eq!(std::fs::read_to_string(&outside_file).unwrap(), "original");
}
