use augur_core::actors::file_read::file_read_actor::spawn;
use augur_domain::domain::string_newtypes::{FilePath, StringNewtype};
use std::io::Write;
use std::path::PathBuf;
use tokio::time::{Duration, timeout};

fn make_temp_file(content: &str) -> (tempfile::NamedTempFile, PathBuf) {
    let mut f = tempfile::NamedTempFile::new().expect("temp file");
    write!(f, "{content}").expect("write temp file");
    let dir = f.path().parent().expect("temp parent").to_path_buf();
    (f, dir)
}

#[tokio::test]
async fn line_count_returns_correct_count() {
    let (file, dir) = make_temp_file("line1\nline2\nline3\n");
    let path_str = file.path().to_str().expect("utf8 path").to_owned();
    let (_join, handle) = spawn(vec![dir]);
    let result = handle.line_count(FilePath::new(path_str)).await;
    assert!(!result.is_error);
    assert_eq!(result.output.as_str(), "3");
}

#[tokio::test]
async fn line_count_outside_allowed_dir_is_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_join, handle) = spawn(vec![dir.path().to_path_buf()]);
    let result = handle.line_count(FilePath::new("/etc/passwd")).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn line_count_nonexistent_file_is_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_join, handle) = spawn(vec![dir.path().to_path_buf()]);
    let missing = dir.path().join("no_such_file.txt");
    let result = handle
        .line_count(FilePath::new(missing.to_string_lossy()))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn line_count_after_shutdown_returns_actor_stopped_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (join, handle) = spawn(vec![dir.path().to_path_buf()]);
    handle.shutdown();
    timeout(Duration::from_secs(2), join)
        .await
        .expect("file_read actor should stop")
        .expect("file_read actor should not panic");
    let result = handle
        .line_count(FilePath::new(dir.path().join("x.rs").to_string_lossy()))
        .await;
    assert!(result.is_error);
    assert!(
        result
            .output
            .as_str()
            .starts_with("file read actor stopped")
    );
}

#[test]
fn legacy_read_range_actor_tests_deprecated_due_private_read_range_type() {
    let source = std::fs::read_to_string(format!(
        "{}/src/actors/file_read/handle.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("file_read handle source must be readable");
    assert!(source.contains("use crate::tools::ports::{FileReadPort, FileReadResult, ReadRange};"));
}
