use std::fs;

#[test]
fn lsp_actor_spawn_contract_is_present() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/lsp/lsp_actor.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("lsp_actor source must be readable");
    assert!(source.contains("pub fn spawn(config: LspActorConfig) -> (JoinHandle<()>, LspHandle)"));
    assert!(source.contains("const LSP_EXECUTABLE: &str = \"rust-analyzer\";"));
    assert!(source.contains("pub(crate) fn spawn_with_io"));
}
