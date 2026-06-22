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
fn executor_event_mapper_lives_in_provider_bucket() {
    let repo = repo_root();
    assert!(
        repo.join("crates/augur-provider-copilot-sdk/src/actors/executor/event_mapper.rs")
            .exists()
    );
    assert!(!repo.join("crates/augur-core/src/actors/executor/event_mapper.rs").exists());
}
