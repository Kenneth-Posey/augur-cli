#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::process::Command;
#[cfg(test)]
use std::sync::OnceLock;

#[cfg(test)]
use augur_domain::domain::{CachedFileContent, FilePath, StringNewtype};

#[cfg(test)]
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

/// Build crate rustdoc once, then load a generated HTML page by relative path.
#[cfg(test)]
pub fn rustdoc_html(relative_path: impl Into<FilePath>) -> CachedFileContent {
    build_rustdoc();
    let relative_path = relative_path.into();
    let full_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/doc")
        .join(relative_path.as_str());
    CachedFileContent::from(
        fs::read_to_string(&full_path)
            .unwrap_or_else(|err| panic!("expected rustdoc output at {}: {err}", full_path.display())),
    )
}
