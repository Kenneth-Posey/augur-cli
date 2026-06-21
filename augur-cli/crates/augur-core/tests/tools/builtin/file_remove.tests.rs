use augur_core::tools::builtin::file_remove::FileRemoveTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;

fn make_tool(dir: &tempfile::TempDir) -> FileRemoveTool {
    FileRemoveTool::new(vec![dir.path().to_path_buf()])
}

fn write_file(dir: &tempfile::TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path.to_str().unwrap().to_owned()
}

#[tokio::test]
async fn execute_removes_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "to_remove.txt", "content");
    let tool = make_tool(&dir);
    let result = tool.execute(serde_json::json!({"path": path})).await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    assert!(!dir.path().join("to_remove.txt").exists());
}

#[tokio::test]
async fn execute_file_not_found_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonexistent.txt");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path.to_str().unwrap()}))
        .await;
    assert!(result.is_error);
    assert!(result.output.as_str().contains("not found"));
}

#[tokio::test]
async fn execute_missing_path_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = make_tool(&dir);
    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_empty_path_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = make_tool(&dir);
    let result = tool.execute(serde_json::json!({"path": ""})).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_path_not_in_allowed_dirs_is_error() {
    let allowed = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let path = write_file(&outside, "secret.txt", "data");
    let tool = make_tool(&allowed);
    let result = tool.execute(serde_json::json!({"path": path})).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn symlink_remove_is_denied() {
    let allowed = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("target.txt");
    std::fs::write(&outside_file, "content").unwrap();
    let symlink_path = allowed.path().join("link.txt");
    std::os::unix::fs::symlink(&outside_file, &symlink_path).unwrap();
    let tool = make_tool(&allowed);
    let result = tool
        .execute(serde_json::json!({"path": symlink_path.to_str().unwrap()}))
        .await;
    assert!(result.is_error, "remove through symlink must be denied");
    assert!(outside_file.exists(), "outside file must be untouched");
}
