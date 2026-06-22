use std::fs;

#[test]
fn command_mod_has_inner_doc_comment() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/command/mod.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("command mod source must be readable");

    let first_non_empty = source
        .lines()
        .find(|line| !line.trim().is_empty())
        .expect("command mod must not be empty");
    assert!(first_non_empty.trim_start().starts_with("//!"));
}

#[test]
fn command_mod_exports_expected_surfaces() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/command/mod.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("command mod source must be readable");

    for expected in [
        "pub mod command_actor;",
        "pub mod handle;",
        "pub mod registry;",
        "pub mod types;",
        "pub use handle::CommandHandle;",
    ] {
        assert!(
            source.contains(expected),
            "expected declaration missing from command mod: {expected}"
        );
    }
}
