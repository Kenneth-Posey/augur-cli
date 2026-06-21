use std::fs;

#[test]
fn catalog_manager_actor_ops_exposes_catalog_pipeline_functions() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/catalog_manager/catalog_manager_actor_ops.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("catalog_manager_actor_ops source must be readable");

    assert!(source.contains("pub(super) async fn run_actor"));
    assert!(source.contains("pub(super) async fn generate_catalog"));
    assert!(source.contains("fn persist_provider_catalogs_in_dir("));
}
