use std::fs;

#[test]
fn history_adapter_handle_is_reexported_from_domain() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/history_adapter/handle.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("history_adapter handle source must be readable");
    assert!(source.contains("pub use augur_domain::HistoryAdapterHandle;"));
}
