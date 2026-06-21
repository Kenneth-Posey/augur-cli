use augur_core::tools::builtin::file_slice::FileSliceTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;

fn make_tool(dir: &tempfile::TempDir) -> FileSliceTool {
    FileSliceTool::new(vec![dir.path().to_path_buf()])
}

fn write_file(dir: &tempfile::TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path.to_str().unwrap().to_owned()
}

#[tokio::test]
async fn execute_removes_content_between_anchors() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(
        &dir,
        "test.txt",
        "line1\nREMOVE_START\nline2\nline3\nREMOVE_END\nline4\n",
    );
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "start_text": "REMOVE_START", "end_text": "REMOVE_END"}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "line1\nline4\n");
}

#[tokio::test]
async fn execute_anchor_not_found_reports_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "line1\nline2\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "start_text": "NOT_THERE", "end_text": "line2"}))
        .await;
    assert!(
        !result.is_error,
        "should not be an error, just informational"
    );
    assert!(
        result.output.as_str().contains("not found"),
        "should report not found"
    );
}

#[tokio::test]
async fn execute_end_anchor_not_found_reports_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "line1\nline2\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "start_text": "line1", "end_text": "NOT_THERE"}))
        .await;
    assert!(!result.is_error);
    assert!(result.output.as_str().contains("not found"));
}

#[tokio::test]
async fn execute_non_unique_start_anchor_reports_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "DUPE\nmiddle\nDUPE\nend\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "start_text": "DUPE", "end_text": "end"}))
        .await;
    assert!(!result.is_error);
    assert!(
        result.output.as_str().contains("not unique"),
        "should report not unique"
    );
    assert!(result.output.as_str().contains("2"), "should mention count");
}

#[tokio::test]
async fn execute_non_unique_end_anchor_reports_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "begin\nDUPE\nmiddle\nDUPE\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "start_text": "begin", "end_text": "DUPE"}))
        .await;
    assert!(!result.is_error);
    assert!(result.output.as_str().contains("not unique"));
}

#[tokio::test]
async fn execute_start_after_end_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "AAA\nmiddle\nBBB\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "start_text": "BBB", "end_text": "AAA"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_missing_path_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"start_text": "a", "end_text": "b"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_missing_start_text_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "a\nb\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "end_text": "b"}))
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
        .execute(serde_json::json!({"path": path, "start_text": "da", "end_text": "ta"}))
        .await;
    assert!(result.is_error);
}
