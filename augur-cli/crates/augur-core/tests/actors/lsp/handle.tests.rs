use std::fs;

#[test]
fn lsp_handle_exposes_client_request_surface() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/lsp/handle.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("lsp handle source must be readable");
    assert!(source.contains("pub struct LspHandle"));
    assert!(source.contains("impl LspClient for LspHandle"));
    assert!(source.contains("async fn request("));
    assert!(source.contains("pub async fn send(&self, request: LspRequest) -> Result<(), LspError>"));
}
