use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use augur_domain::domain::{CachedFileContent, FilePath, StringNewtype};

fn build_rustdoc() {
    static BUILD_ONCE: OnceLock<()> = OnceLock::new();
    BUILD_ONCE.get_or_init(|| {
        let status = Command::new("cargo")
            .args(["doc", "--no-deps", "--lib"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("failed to run `cargo doc --no-deps --lib`");
        assert!(
            status.success(),
            "`cargo doc --no-deps --lib` should succeed"
        );
    });
}

pub fn rustdoc_html(relative_path: impl Into<FilePath>) -> CachedFileContent {
    build_rustdoc();
    let relative_path = relative_path.into();
    let local_target_doc = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/doc");
    let workspace_target_doc = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/doc");

    let full_path = [local_target_doc, workspace_target_doc]
        .into_iter()
        .map(|base| base.join(relative_path.as_str()))
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target/doc")
                .join(relative_path.as_str())
        });

    CachedFileContent::from(
        fs::read_to_string(&full_path).unwrap_or_else(|err| {
            panic!("expected rustdoc output at {}: {err}", full_path.display())
        }),
    )
}
