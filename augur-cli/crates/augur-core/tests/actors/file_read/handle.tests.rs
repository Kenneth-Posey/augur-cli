use std::fs;

#[test]
fn file_read_handle_line_count_is_instrumented() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/file_read/handle.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("file_read handle source must be readable");
    assert!(
        source.contains(
            "#[tracing::instrument(skip(self), fields(path = %path))]\n    pub async fn line_count",
        ),
        "FileReadHandle::line_count must be instrumented with the path field",
    );
}

#[test]
fn file_read_handle_read_range_is_instrumented() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/file_read/handle.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("file_read handle source must be readable");
    assert!(
        source.contains(
            "#[tracing::instrument(skip(self), fields(path = %path))]\n    pub async fn read_range",
        ),
        "FileReadHandle::read_range must be instrumented with the path field",
    );
}
