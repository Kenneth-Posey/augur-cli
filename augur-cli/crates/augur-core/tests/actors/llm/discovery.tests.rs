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
fn llm_discovery_is_no_longer_a_core_actor_module() {
    let root = repo_root();
    assert!(
        !root
            .join("crates/augur-core/src/actors/llm/discovery.rs")
            .exists()
    );
    assert!(
        root
            .join("crates/augur-core/src/config/endpoint_catalog_discovery.rs")
            .exists()
    );
}
