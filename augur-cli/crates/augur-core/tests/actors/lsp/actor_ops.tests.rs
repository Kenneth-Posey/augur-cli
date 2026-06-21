use std::fs;

#[test]
fn lsp_actor_ops_contains_request_io_and_failure_drain_helpers() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/lsp/lsp_actor_ops.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("lsp_actor_ops source must be readable");
    assert!(source.contains("const MAX_LSP_RESPONSE_BYTES: usize = 64 * 1024 * 1024;"));
    assert!(source.contains("pub(super) async fn send_request"));
    assert!(source.contains("pub(super) async fn read_response"));
    assert!(source.contains("pub(super) async fn ensure_document_open"));
    assert!(source.contains("pub(super) fn notify_all_pending"));
}
