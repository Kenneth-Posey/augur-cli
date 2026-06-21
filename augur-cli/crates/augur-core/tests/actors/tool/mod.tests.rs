use std::fs;

#[test]
fn tool_mod_has_inner_doc_comment() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/tool/mod.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("tool mod source must be readable");
    let first_non_empty = source
        .lines()
        .find(|line| !line.trim().is_empty())
        .expect("tool mod must not be empty");
    assert!(first_non_empty.trim_start().starts_with("//!"));
}

#[test]
fn tool_mod_exports_inline_executor_surface() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/tool/mod.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("tool mod source must be readable");

    for expected in [
        "pub mod handle;",
        "pub mod inline_executor;",
        "pub mod tool_actor;",
        "pub mod tool_ops;",
        "pub use inline_executor::InlineToolExecutor;",
    ] {
        assert!(source.contains(expected), "missing declaration: {expected}");
    }
}
