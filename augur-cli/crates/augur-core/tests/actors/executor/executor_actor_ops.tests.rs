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
fn executor_actor_ops_coverage_is_consolidated_under_executor_ops() {
    let repo = repo_root();
    assert!(
        repo.join("crates/augur-provider-copilot-sdk/tests/actors/executor/executor_ops.tests.rs")
            .exists()
    );
    assert!(
        repo.join("crates/augur-provider-copilot-sdk/tests/actors/executor/executor_ops/core.tests.rs")
            .exists()
    );
}
