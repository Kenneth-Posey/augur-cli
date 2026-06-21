use augur_core::tools::builtin::file_replace::FileReplaceTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;

fn make_tool(dir: &tempfile::TempDir) -> FileReplaceTool {
    FileReplaceTool::new(vec![dir.path().to_path_buf()])
}

fn write_file(dir: &tempfile::TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path.to_str().unwrap().to_owned()
}

#[tokio::test]
async fn execute_replaces_globally() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "hello world\nhello everyone\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "old_text": "hello", "new_text": "hi"}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "hi world\nhi everyone\n");
    assert!(result.output.as_str().contains("2 occurrence(s)"));
}

#[tokio::test]
async fn execute_replaces_in_text_anchor_range() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(
        &dir,
        "test.txt",
        "START_MARKER\naaa\nbbb\nEND_MARKER\naaa\n",
    );
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "old_text": "aaa", "new_text": "xxx", "start_text": "START_MARKER", "end_text": "END_MARKER"}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "START_MARKER\nxxx\nbbb\nEND_MARKER\naaa\n");
    assert!(result.output.as_str().contains("1 occurrence(s)"));
}

#[tokio::test]
async fn execute_no_change_when_old_text_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "aaa\nbbb\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "old_text": "zzz", "new_text": "yyy"}))
        .await;
    assert!(!result.is_error, "should not be an error");
    assert!(
        result.output.as_str().contains("not found"),
        "should report not found: {}",
        result.output.as_str()
    );
    let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "aaa\nbbb\n");
}

#[tokio::test]
async fn execute_start_text_not_found_reports_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "aaa\nbbb\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "old_text": "aaa", "new_text": "xxx", "start_text": "NOT_THERE", "end_text": "bbb"}))
        .await;
    assert!(!result.is_error);
    assert!(result.output.as_str().contains("not found"));
}

#[tokio::test]
async fn execute_start_text_not_unique_reports_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "DUPE\nmiddle\nDUPE\nend\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "old_text": "middle", "new_text": "x", "start_text": "DUPE", "end_text": "end"}))
        .await;
    assert!(!result.is_error);
    assert!(result.output.as_str().contains("not unique"));
}

#[tokio::test]
async fn execute_missing_path_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"old_text": "a", "new_text": "b"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_missing_old_text_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "aaa\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "new_text": "b"}))
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
        .execute(serde_json::json!({"path": path, "old_text": "a", "new_text": "b"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_reports_single_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "foo\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "old_text": "foo", "new_text": "bar"}))
        .await;
    assert!(!result.is_error);
    assert!(result.output.as_str().contains("1 occurrence(s)"));
}

#[tokio::test]
async fn execute_start_after_end_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "AAA\nmiddle\nBBB\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "old_text": "middle", "new_text": "x", "start_text": "BBB", "end_text": "AAA"}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_replaces_empty_new_text() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "test.txt", "hello world\n");
    let tool = make_tool(&dir);
    let result = tool
        .execute(serde_json::json!({"path": path, "old_text": "world", "new_text": ""}))
        .await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
    assert_eq!(content, "hello \n");
}
