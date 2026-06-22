use augur_core::actors::file_scanner::file_scanner_actor::scan_directory;
use augur_domain::domain::string_newtypes::FilePath;

#[test]
fn scan_directory_returns_empty_for_missing_prefix() {
    let results = scan_directory(&FilePath::new("definitely_not_a_real_prefix_kenny"));
    assert!(results.is_empty());
}
