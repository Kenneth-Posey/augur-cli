use augur_core::tools::builtin::file_insert::FileInsertTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;

fn make_tool(dir: &tempfile::TempDir) -> FileInsertTool {
    FileInsertTool::new(vec![dir.path().to_path_buf()])
}

fn write_file(dir: &tempfile::TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path.to_str().unwrap().to_owned()
}

#[tokio::test]
async fn execute_inserts_before_anchor() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "line2\nline3\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "anchor_text": "line2", "content": "line1\n", "position": "before"}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "line1\nline2\nline3\n");
}

#[tokio::test]
async fn execute_inserts_after_anchor() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "line1\nline3\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "anchor_text": "line1", "content": "\nline2", "position": "after"}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "line1\nline2\nline3\n");
}

#[tokio::test]
async fn execute_anchor_not_found_reports_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "line1\nline2\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "anchor_text": "NOT_THERE", "content": "x", "position": "before"}))
        .await;
    assert!(!result.is_error);
    assert!(result.output.as_str().contains("not found"));
}

#[tokio::test]
async fn execute_non_unique_anchor_reports_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "DUPE\nmiddle\nDUPE\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "anchor_text": "DUPE", "content": "x", "position": "before"}))
        .await;
    assert!(!result.is_error);
    assert!(result.output.as_str().contains("not unique"));
    assert!(result.output.as_str().contains("2"));
}

#[tokio::test]
async fn execute_missing_path_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"anchor_text": "a", "content": "x", "position": "before"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_missing_anchor_text_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "a\nb\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "content": "x", "position": "before"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_missing_position_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "a\nb\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "anchor_text": "a", "content": "x"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_invalid_position_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "a\nb\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "anchor_text": "a", "content": "x", "position": "invalid"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_path_not_in_allowed_dirs_is_error() {
    let allowed = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let path = write_file(&outside, "secret.txt", "data\n");
    let tool = make_tool(&allowed);
    let result = tool
        .execute(serde_json::json!({"path": path, "anchor_text": "da", "content": "x", "position": "before"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_inserts_after_anchor_at_end() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "line1\nline2");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "anchor_text": "line2", "content": "\nline3", "position": "after"}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "line1\nline2\nline3");
}
