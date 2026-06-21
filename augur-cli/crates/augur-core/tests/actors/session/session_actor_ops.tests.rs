use augur_core::actors::session::handle::SessionHandle;
use augur_core::actors::session::session_ops::SessionCommand;
use std::fs;

#[test]
fn mirrored_surface_smoke_actor_ops() {
    assert!(core::module_path!().contains("actor_ops"));
}

fn source_lines() -> Vec<String> {
    fs::read_to_string(format!(
        "{}/src/actors/session/mod.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("session mod source must be readable")
    .lines()
    .map(str::to_owned)
    .collect()
}

fn assert_pub_mod_is_documented(lines: &[String], decl: &str) {
    let idx = lines
        .iter()
        .position(|line| line.trim() == decl)
        .unwrap_or_else(|| panic!("missing declaration: {decl}"));
    let previous = lines[..idx]
        .iter()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| panic!("missing doc comment before: {decl}"));
    assert!(
        previous.trim_start().starts_with("///"),
        "{decl} must be preceded by a /// doc comment, got: {previous}",
    );
}

/// Verifies the integration surface reaches `SessionHandle`.
#[test]
fn mirrored_surface_smoke_handle() {
    let type_name = core::any::type_name::<SessionHandle>();
    assert!(type_name.contains("SessionHandle"));
}

/// Verifies the integration surface reaches `SessionCommand`.
#[test]
fn mirrored_surface_smoke_ops() {
    let type_name = core::any::type_name::<SessionCommand>();
    assert!(type_name.contains("SessionCommand"));
}

/// Verifies that the session module starts with a `//!` banner.
#[test]
fn session_mod_has_inner_doc_comment() {
    let lines = source_lines();
    let first_non_empty = lines
        .iter()
        .find(|line| !line.trim().is_empty())
        .expect("session mod must not be empty");
    assert!(
        first_non_empty.trim_start().starts_with("//!"),
        "src/actors/session/mod.rs must begin with a //! module doc comment",
    );
}

/// Verifies that every public session submodule declaration is documented.
#[test]
fn session_mod_public_submodules_are_documented() {
    let lines = source_lines();
    for decl in [
        "pub mod session_actor;",
        "pub mod handle;",
        "pub mod session_ops;",
    ] {
        assert_pub_mod_is_documented(&lines, decl);
    }
}
