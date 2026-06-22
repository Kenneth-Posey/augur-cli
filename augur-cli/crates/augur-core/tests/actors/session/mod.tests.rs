use std::fs;

#[test]
fn session_mod_has_inner_doc_comment() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/session/mod.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("session mod source must be readable");
    let first_non_empty = source
        .lines()
        .find(|line| !line.trim().is_empty())
        .expect("session mod must not be empty");
    assert!(first_non_empty.trim_start().starts_with("//!"));
}

#[test]
fn session_mod_declares_expected_public_modules() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/session/mod.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("session mod source must be readable");
    for expected in [
        "pub mod handle;",
        "pub mod session_actor;",
        "pub mod session_ops;",
    ] {
        assert!(source.contains(expected), "missing declaration: {expected}");
    }
}
