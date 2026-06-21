use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate dir should have workspace parent")
        .parent()
        .expect("workspace dir should have repo parent")
        .to_path_buf()
}

#[test]
fn executor_ops_integration_coverage_uses_normalized_tests_suffix() {
    let repo = repo_root();
    assert!(
        repo.join("crates/augur-provider-copilot-sdk/tests/actors/executor/executor_ops/integration.tests.rs")
            .exists()
    );
}
