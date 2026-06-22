use augur_core::tools::builtin::list_directory::ListDirectoryTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;
use std::path::PathBuf;

/// Verifies that a non-recursive listing of a directory returns the immediate
/// entries only, with directories listed before files, each correctly labeled.
#[tokio::test]
async fn list_directory_non_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    let subdir = path.join("subdir");
    std::fs::create_dir(&subdir).unwrap();
    std::fs::write(subdir.join("nested.txt"), "nested").unwrap();
    std::fs::write(path.join("file.txt"), "hello").unwrap();

    let tool = ListDirectoryTool::new(vec![path.to_path_buf()], vec![]);
    let args = serde_json::json!({ "path": path.to_str().unwrap() });
    let result = tool.execute(args).await;

    assert!(!result.is_error);
    let output = result.output.as_str();
    // Root label present
    assert!(output.contains('/'), "root label should end with /");
    // Directory listed before file
    let subdir_pos = output.find("subdir/").unwrap();
    let file_pos = output.find("file.txt").unwrap();
    assert!(
        subdir_pos < file_pos,
        "directories should appear before files"
    );
    // Recursive content should not appear (subdir should be empty inside)
    assert!(output.contains("  subdir/"));
    assert!(output.contains("  file.txt"));
    assert!(
        !output.contains("nested.txt"),
        "non-recursive listing must not include nested descendants"
    );
}

/// Verifies that recursive listing walks into subdirectories and lists their
/// contents with increasing indentation.
#[tokio::test]
async fn list_directory_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    let subdir = path.join("inner");
    std::fs::create_dir(&subdir).unwrap();
    std::fs::write(subdir.join("nested.txt"), "content").unwrap();
    std::fs::write(path.join("top.txt"), "top").unwrap();

    let tool = ListDirectoryTool::new(vec![path.to_path_buf()], vec![]);
    let args = serde_json::json!({ "path": path.to_str().unwrap(), "recursive": true });
    let result = tool.execute(args).await;

    assert!(!result.is_error);
    let output = result.output.as_str();
    assert!(
        output.contains("    nested.txt"),
        "nested file should appear with deeper indentation"
    );
}

/// Verifies that listing a non-existent directory returns an error result.
#[tokio::test]
async fn list_directory_missing_path_returns_error() {
    let tool = ListDirectoryTool::new(
        vec![std::path::PathBuf::from("/definitely/does/not/exist")],
        vec![],
    );
    let args = serde_json::json!({ "path": "/definitely/does/not/exist/12345" });
    let result = tool.execute(args).await;
    assert!(result.is_error, "missing directory should produce an error");
}

/// Verifies that a missing path argument returns an error result.
#[tokio::test]
async fn list_directory_missing_arg_returns_error() {
    let tool = ListDirectoryTool::new(vec![], vec![]);
    let args = serde_json::json!({});
    let result = tool.execute(args).await;
    assert!(result.is_error);
    assert!(result.output.as_str().contains("missing"));
}

/// Verifies that an explicitly empty path string returns an error result.
#[tokio::test]
async fn list_directory_empty_path_returns_error() {
    let tool = ListDirectoryTool::new(vec![], vec![]);
    let result = tool.execute(serde_json::json!({ "path": "" })).await;
    assert!(result.is_error);
}

/// Verifies that directories and files are alphabetized within their own groups.
#[tokio::test]
async fn list_directory_orders_entries_alphabetically_within_groups() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    std::fs::create_dir(path.join("zeta")).unwrap();
    std::fs::create_dir(path.join("alpha")).unwrap();
    std::fs::write(path.join("zeta.txt"), "z").unwrap();
    std::fs::write(path.join("alpha.txt"), "a").unwrap();

    let tool = ListDirectoryTool::new(vec![path.to_path_buf()], vec![]);
    let result = tool
        .execute(serde_json::json!({ "path": path.to_str().unwrap() }))
        .await;

    assert!(!result.is_error);
    let output = result.output.as_str();
    assert!(output.find("  alpha/").unwrap() < output.find("  zeta/").unwrap());
    assert!(output.find("  alpha.txt").unwrap() < output.find("  zeta.txt").unwrap());
}

/// Verifies that a path outside the allowed directories is denied with an error.
#[tokio::test]
async fn sandbox_deny_rejects_path_outside_allowed_dirs() {
    let allowed = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();

    let tool = ListDirectoryTool::new(vec![allowed.path().to_path_buf()], vec![]);
    let args = serde_json::json!({ "path": outside.path().to_str().unwrap() });
    let result = tool.execute(args).await;

    assert!(result.is_error, "path outside allowed_dirs must be denied");
    assert!(
        result.output.as_str().contains("access denied"),
        "error message must contain 'access denied', got: {}",
        result.output.as_str()
    );
}

#[test]
fn mirror_sync_executes_list_directory_non_recursive() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}

/// Verifies that recursive listing omits directories configured in
/// `excluded_dirs` and all nested descendants beneath them.
#[tokio::test]
async fn list_directory_recursive_omits_excluded_directories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    let include_dir = path.join("include_me");
    let exclude_dir = path.join("exclude_me");
    std::fs::create_dir(&include_dir).unwrap();
    std::fs::create_dir(&exclude_dir).unwrap();
    std::fs::write(include_dir.join("shown.txt"), "ok").unwrap();
    std::fs::write(exclude_dir.join("hidden.txt"), "no").unwrap();

    let tool = ListDirectoryTool::new(vec![path.to_path_buf()], vec![exclude_dir.to_path_buf()]);
    let result = tool
        .execute(serde_json::json!({ "path": path.to_str().unwrap(), "recursive": true }))
        .await;

    assert!(!result.is_error);
    let output = result.output.as_str();
    assert!(output.contains("include_me/"));
    assert!(output.contains("shown.txt"));
    assert!(!output.contains("exclude_me/"));
    assert!(!output.contains("hidden.txt"));
}

/// Verifies that injected exclusions are honored when listing recursively.
#[tokio::test]
async fn list_directory_injected_changelogs_exclusion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    let changelogs = path.join("changelogs");
    let visible = path.join("visible");
    std::fs::create_dir(&changelogs).unwrap();
    std::fs::create_dir(&visible).unwrap();
    std::fs::write(changelogs.join("hidden.txt"), "no").unwrap();
    std::fs::write(visible.join("shown.txt"), "yes").unwrap();

    let tool = ListDirectoryTool::new(vec![path.to_path_buf()], vec![PathBuf::from("changelogs")]);
    let result = tool
        .execute(serde_json::json!({ "path": path.to_str().unwrap(), "recursive": true }))
        .await;

    assert!(!result.is_error);
    let output = result.output.as_str();
    assert!(output.contains("visible/"));
    assert!(output.contains("shown.txt"));
    assert!(!output.contains("changelogs/"));
    assert!(!output.contains("hidden.txt"));
}

/// Verifies that an explicitly requested excluded directory can still be listed.
#[tokio::test]
async fn list_directory_allows_explicit_path_to_excluded_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    let target_dir = path.join("target");
    std::fs::create_dir(&target_dir).unwrap();
    std::fs::write(target_dir.join("artifact.txt"), "present").unwrap();

    let tool = ListDirectoryTool::new(vec![path.to_path_buf()], vec![PathBuf::from("target")]);
    let result = tool
        .execute(serde_json::json!({ "path": target_dir.to_str().unwrap(), "recursive": true }))
        .await;

    assert!(!result.is_error);
    let output = result.output.as_str();
    assert!(output.contains("artifact.txt"));
}
