use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("augur-core has parent")
        .parent()
        .expect("workspace root has parent")
        .to_path_buf()
}

#[test]
fn llm_ops_are_owned_by_provider_shared_request_context() {
    let root = repo_root();
    assert!(
        !root
            .join("crates/augur-core/src/actors/llm/llm_ops.rs")
            .exists()
    );
    assert!(
        root
            .join("crates/augur-provider-shared/src/request_context.rs")
            .exists()
    );
}
