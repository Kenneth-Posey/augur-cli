use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use augur_domain::domain::{CachedFileContent, FilePath, StringNewtype};

fn build_rustdoc() {
    static BUILD_ONCE: OnceLock<()> = OnceLock::new();
    BUILD_ONCE.get_or_init(|| {
        // Build docs from the workspace root so all workspace crate docs are generated.
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root two levels above CARGO_MANIFEST_DIR");
        let status = Command::new("cargo")
            .args(["doc", "--no-deps", "--lib", "-p", "augur-domain"])
            .current_dir(workspace_root)
            .status()
            .expect("failed to run `cargo doc`");
        assert!(
            status.success(),
            "`cargo doc --no-deps --lib -p augur-domain` should succeed"
        );
    });
}

pub fn rustdoc_html(relative_path: impl Into<FilePath>) -> CachedFileContent {
    build_rustdoc();
    let relative_path = relative_path.into();
    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let full_path = target_dir.join("doc").join(relative_path.as_str());
    CachedFileContent::from(
        fs::read_to_string(&full_path).unwrap_or_else(|err| {
            panic!("expected rustdoc output at {}: {err}", full_path.display())
        }),
    )
}
