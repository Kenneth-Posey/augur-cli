use augur_core::actors::file_read::file_read_actor;
use augur_core::tools::builtin::file_line_count::FileLineCountTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;
use std::io::Write;
use std::path::PathBuf;

fn spawn_tool(allowed_dirs: Vec<PathBuf>) -> (FileLineCountTool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut dirs = vec![dir.path().to_path_buf()];
    dirs.extend(allowed_dirs);
    let (_join, handle) = file_read_actor::spawn(dirs);
    (FileLineCountTool::new(handle), dir)
}

fn write_temp_file(dir: &tempfile::TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    write!(f, "{}", content).unwrap();
    path.to_str().unwrap().to_owned()
}

#[tokio::test]
async fn execute_returns_correct_line_count() {
    let (tool, dir) = spawn_tool(vec![]);
    let path = write_temp_file(&dir, "test.txt", "line1\nline2\nline3\nline4");
    let result = tool.execute(serde_json::json!({"path": path})).await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    assert_eq!(result.output.as_str(), "4");
}

#[tokio::test]
async fn execute_returns_zero_for_empty_file() {
    let (tool, dir) = spawn_tool(vec![]);
    let path = write_temp_file(&dir, "empty.txt", "");
    let result = tool.execute(serde_json::json!({"path": path})).await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    assert_eq!(result.output.as_str(), "0");
}

#[tokio::test]
async fn execute_returns_error_for_missing_path() {
    let (tool, _dir) = spawn_tool(vec![]);
    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_error);
    assert!(result.output.as_str().contains("path"));
}

#[tokio::test]
async fn execute_returns_error_for_empty_path() {
    let (tool, _dir) = spawn_tool(vec![]);
    let result = tool.execute(serde_json::json!({"path": ""})).await;
    assert!(result.is_error);
    assert!(result.output.as_str().contains("path"));
}

#[tokio::test]
async fn execute_access_denied_outside_allowed_dirs() {
    let (tool, _dir) = spawn_tool(vec![]);
    let result = tool
        .execute(serde_json::json!({"path": "/etc/passwd"}))
        .await;
    assert!(result.is_error, "expected access denied error");
}

#[tokio::test]
async fn execute_missing_file_in_allowed_dir_is_error() {
    let (tool, dir) = spawn_tool(vec![]);
    let path = dir.path().join("missing.txt");
    let result = tool
        .execute(serde_json::json!({"path": path.to_str().unwrap()}))
        .await;
    assert!(result.is_error);
}
