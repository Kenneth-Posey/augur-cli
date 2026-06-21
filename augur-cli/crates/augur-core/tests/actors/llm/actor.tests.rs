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
fn llm_actor_is_not_in_augur_core_and_is_owned_by_provider_openrouter() {
    let root = repo_root();
    assert!(
        !root
            .join("crates/augur-core/src/actors/llm/llm_actor.rs")
            .exists()
    );
    assert!(
        root
            .join("crates/augur-provider-openrouter/src/actors/llm/llm_actor.rs")
            .exists()
    );
}
