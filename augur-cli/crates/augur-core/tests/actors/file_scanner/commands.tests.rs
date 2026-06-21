use std::fs;

#[test]
fn file_scanner_commands_define_scan_and_shutdown() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/file_scanner/commands.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("file_scanner commands source must be readable");

    assert!(source.contains("Scan { prefix: FilePath }"));
    assert!(source.contains("Shutdown"));
}
