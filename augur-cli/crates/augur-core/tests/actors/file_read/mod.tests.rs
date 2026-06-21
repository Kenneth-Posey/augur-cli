use std::fs;

#[test]
fn file_read_mod_has_inner_doc_comment() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/file_read/mod.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("file_read mod source must be readable");
    let first_non_empty = source
        .lines()
        .find(|line| !line.trim().is_empty())
        .expect("file_read mod must not be empty");
    assert!(
        first_non_empty.trim_start().starts_with("//!"),
        "src/actors/file_read/mod.rs must begin with a //! module doc comment",
    );
}
