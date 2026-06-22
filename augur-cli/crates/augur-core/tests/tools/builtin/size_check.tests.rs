use augur_core::tools::builtin::size_check::{
    ExclusionConfig, RecommendationType, SizeCheckError, SizeCheckRequest, SizeCheckTool,
    check_size_with_scope,
};
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::newtypes::NumericNewtype;
use augur_domain::domain::string_newtypes::FilePath;
use augur_domain::domain::string_newtypes::StringNewtype;
use std::ffi::OsString;
use std::path::Path;

fn request(path: &Path) -> SizeCheckRequest {
    SizeCheckRequest {
        path: FilePath::new(path.to_string_lossy().to_string()),
        command_type: None,
        filter_pattern: None,
        max_depth: None,
    }
}

fn no_exclusions() -> ExclusionConfig<'static> {
    ExclusionConfig::new(&[], &[])
}

fn exclusions(dirs: &[std::path::PathBuf]) -> ExclusionConfig<'_> {
    ExclusionConfig::new(dirs, &[])
}

fn name_exclusions(names: &[OsString]) -> ExclusionConfig<'_> {
    ExclusionConfig::new(&[], names)
}

#[test]
fn check_size_counts_text_file_bytes_and_lines() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("sample.txt");
    std::fs::write(&file, "a\nb\nc\n").expect("write file");
    let response =
        check_size_with_scope(request(&file), &[dir.path().to_path_buf()], no_exclusions())
            .expect("size check");
    assert_eq!(response.byte_count.inner(), 6);
    assert_eq!(response.counts.line_count, Some(3));
    assert_eq!(response.counts.file_count, None);
}

#[test]
fn check_size_rejects_path_outside_allowed_scope() {
    let allowed = tempfile::tempdir().expect("allowed");
    let outside = tempfile::tempdir().expect("outside");
    let outside_file = outside.path().join("outside.txt");
    std::fs::write(&outside_file, "x").expect("write file");
    let error = check_size_with_scope(
        request(&outside_file),
        &[allowed.path().to_path_buf()],
        no_exclusions(),
    )
    .expect_err("outside path must be rejected");
    match error {
        SizeCheckError::InvalidPath(msg) => {
            assert!(
                msg.contains("escapes allowed scope"),
                "unexpected msg: {msg}"
            );
        }
        other => panic!("expected InvalidPath, got {other:?}"),
    }
}

#[test]
fn check_size_rejects_traversal_escape_after_canonicalization() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let outside = tempfile::tempdir().expect("outside");
    let outside_file = outside.path().join("secrets.txt");
    std::fs::write(&outside_file, "secret").expect("write file");
    let traversal = sandbox
        .path()
        .join("..")
        .join(outside.path().file_name().expect("outside file_name"))
        .join("secrets.txt");
    let error = check_size_with_scope(
        request(&traversal),
        &[sandbox.path().canonicalize().expect("canonical sandbox")],
        no_exclusions(),
    )
    .expect_err("traversal must be rejected");
    assert!(
        matches!(error, SizeCheckError::InvalidPath(_)),
        "expected InvalidPath, got {error:?}"
    );
}

#[test]
fn check_size_rejects_shell_injection_pattern() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("sample.txt");
    std::fs::write(&file, "line").expect("write file");
    let mut req = request(&file);
    req.command_type = Some("grep".to_owned());
    req.filter_pattern = Some("value$HOME".to_owned());
    let error = check_size_with_scope(req, &[dir.path().to_path_buf()], no_exclusions())
        .expect_err("must reject");
    assert!(
        matches!(error, SizeCheckError::InvalidCommand(_)),
        "expected InvalidCommand, got {error:?}"
    );
}

#[test]
fn check_size_grep_on_directory_uses_recursive_behavior() {
    let dir = tempfile::tempdir().expect("tempdir");
    let nested = dir.path().join("nested");
    std::fs::create_dir_all(&nested).expect("create nested");
    let nested_file = nested.join("sample.txt");
    std::fs::write(&nested_file, "alpha\nneedle\nomega\n").expect("write file");
    let mut req = request(dir.path());
    req.command_type = Some("grep".to_owned());
    req.filter_pattern = Some("needle".to_owned());
    let response = check_size_with_scope(req, &[dir.path().to_path_buf()], no_exclusions())
        .expect("directory grep should recurse instead of failing");
    assert!(
        response.byte_count.inner() > 0,
        "recursive grep should produce output bytes"
    );
    assert_eq!(response.counts.line_count, Some(1));
    assert_eq!(
        response.estimated_tokens.inner(),
        response.byte_count.inner() / 4
    );
    assert_eq!(response.recommendation, RecommendationType::Proceed);
}

#[test]
fn check_size_grep_on_file_preserves_single_file_behavior() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("single.txt");
    std::fs::write(&file, "needle\nother\n").expect("write file");
    let mut req = request(&file);
    req.command_type = Some("grep".to_owned());
    req.filter_pattern = Some("needle".to_owned());
    let response = check_size_with_scope(req, &[dir.path().to_path_buf()], no_exclusions())
        .expect("single-file grep works");
    assert!(
        response.byte_count.inner() > 0,
        "single-file grep should produce output bytes"
    );
    assert_eq!(response.counts.line_count, Some(1));
    assert_eq!(
        response.estimated_tokens.inner(),
        response.byte_count.inner() / 4
    );
    assert_eq!(response.recommendation, RecommendationType::Proceed);
}

#[tokio::test]
async fn size_check_tool_executes_and_returns_recommendation_json() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("sample.txt");
    std::fs::write(&file, "hello\nworld\n").expect("write file");
    let tool = SizeCheckTool::new(vec![dir.path().to_path_buf()], vec![]);
    let args = serde_json::json!({ "path": file.to_string_lossy() });
    let result = tool.execute(args).await;
    assert!(!bool::from(result.is_error), "tool should succeed");
    let payload: serde_json::Value =
        serde_json::from_str(result.output.as_str()).expect("json output");
    assert_eq!(payload["recommendation"], "proceed");
    assert_eq!(payload["line_count"], 2);
}

#[tokio::test]
async fn size_check_tool_definition_exposes_expected_name() {
    let tool = SizeCheckTool::new(vec![], vec![]);
    assert_eq!(tool.definition().name.as_str(), "size_check");
}

/// Verifies that excluded directories are skipped during recursive directory
/// size checking.
#[test]
fn check_size_dir_omits_excluded_directories() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path();

    let include_dir = path.join("include_me");
    let exclude_dir = path.join("exclude_me");
    std::fs::create_dir(&include_dir).unwrap();
    std::fs::create_dir(&exclude_dir).unwrap();
    // Write a nontrivial file in the excluded dir
    std::fs::write(
        exclude_dir.join("hidden.txt"),
        "this should be excluded content",
    )
    .unwrap();
    // Write a small file in the included dir
    std::fs::write(include_dir.join("shown.txt"), "ok").unwrap();

    let response = check_size_with_scope(
        request(path),
        &[path.to_path_buf()],
        exclusions(&[exclude_dir.to_path_buf()]),
    )
    .expect("size check with exclusions");
    // Should count the included file, not the excluded one
    assert_eq!(
        response.counts.file_count,
        Some(1),
        "only include_me/shown.txt should be counted"
    );
    let shown_size = std::fs::metadata(include_dir.join("shown.txt"))
        .expect("metadata")
        .len();
    assert_eq!(
        response.byte_count.inner(),
        shown_size,
        "byte count should exclude the excluded directory content"
    );
}

/// Verifies that excluded directories are skipped by name match during
/// recursive directory size checking.
#[test]
fn check_size_dir_omits_excluded_dir_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path();

    let visible = path.join("visible");
    let logs = path.join("logs");
    let target = path.join("target");
    std::fs::create_dir_all(&visible).unwrap();
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(visible.join("a.rs"), "pub fn a() {}").unwrap();
    std::fs::write(logs.join("session.log"), "log data").unwrap();
    std::fs::write(target.join("artifact.bin"), b"binary data").unwrap();

    let response = check_size_with_scope(
        request(path),
        &[path.to_path_buf()],
        name_exclusions(&[OsString::from("logs"), OsString::from("target")]),
    )
    .expect("size check with excluded name patterns");
    assert_eq!(
        response.counts.file_count,
        Some(1),
        "only visible/a.rs should be counted"
    );
    let visible_size = std::fs::metadata(visible.join("a.rs"))
        .expect("metadata")
        .len();
    assert_eq!(
        response.byte_count.inner(),
        visible_size,
        "byte count should exclude logs and target"
    );
}
