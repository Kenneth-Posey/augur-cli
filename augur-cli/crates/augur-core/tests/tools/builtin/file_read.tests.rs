use augur_core::actors::file_read::file_read_actor;
use augur_core::tools::builtin::file_read::FileReadTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;
use std::io::Write;
use std::path::PathBuf;

fn spawn_tool(extra_dirs: Vec<PathBuf>) -> (FileReadTool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut dirs = vec![dir.path().to_path_buf()];
    dirs.extend(extra_dirs);
    let (_join, handle) = file_read_actor::spawn(dirs);
    (FileReadTool::new(handle), dir)
}

fn write_temp_file(dir: &tempfile::TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    write!(f, "{}", content).unwrap();
    path.to_str().unwrap().to_owned()
}

#[tokio::test]
async fn execute_reads_existing_file() {
    let (tool, dir) = spawn_tool(vec![]);
    let path = write_temp_file(&dir, "test.txt", "hello from file");
    let result = tool.execute(serde_json::json!({"path": path})).await;
    assert!(!result.is_error, "error: {}", result.output.as_str());
    assert!(result.output.as_str().contains("hello from file"));
}

#[tokio::test]
async fn execute_missing_file_is_error() {
    let (tool, dir) = spawn_tool(vec![]);
    let path = dir.path().join("definitely_does_not_exist.txt");
    let result = tool
        .execute(serde_json::json!({"path": path.to_str().unwrap()}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_missing_path_key_is_error() {
    let (tool, _dir) = spawn_tool(vec![]);
    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_empty_path_is_error() {
    let (tool, _dir) = spawn_tool(vec![]);
    let result = tool.execute(serde_json::json!({"path": ""})).await;
    assert!(result.is_error);
}
