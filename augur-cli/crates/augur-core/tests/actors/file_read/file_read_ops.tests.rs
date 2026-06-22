#[test]
fn legacy_ops_unit_tests_deprecated_due_private_visibility() {
    let source = std::fs::read_to_string(format!(
        "{}/src/actors/file_read/file_read_ops.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("file_read ops source must be readable");
    assert!(source.contains(
        "pub(super) fn apply_range(content: &OutputText, range: &ReadRange) -> OutputText"
    ),);
}
