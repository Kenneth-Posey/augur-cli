use std::path::Path;
use std::{env, fs};

fn main() {
    // Walk up from CARGO_MANIFEST_DIR to find the workspace root (parent of crates/).
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = Path::new(&manifest_dir);

    // This crate is at <workspace_root>/crates/augur-provider-copilot-sdk/
    let workspace_root = manifest_path
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above manifest dir");

    let workspace_root_str = workspace_root.to_str().unwrap();
    println!("cargo:rustc-env=WORKSPACE_ROOT={}/", workspace_root_str);

    // Verify the workspace root contains a Cargo.toml with [workspace]
    let workspace_toml = workspace_root.join("Cargo.toml");
    let content = fs::read_to_string(&workspace_toml).unwrap_or_default();
    assert!(
        content.contains("[workspace]"),
        "WORKSPACE_ROOT must contain a workspace Cargo.toml, got {}",
        workspace_toml.display()
    );
}
