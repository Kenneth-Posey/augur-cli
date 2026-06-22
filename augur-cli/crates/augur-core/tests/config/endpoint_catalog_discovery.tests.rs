use augur_core::config::endpoint_catalog_discovery::discover_endpoints;
use augur_core::config::loader::load_config;

#[test]
fn discover_endpoints_returns_entries_when_config_loaded() {
    // Load the real app config (present in the dev environment).
    let config = match load_config(None) {
        Ok(c) => c,
        Err(_) => return, // Skip if config is not available in this environment.
    };
    let options = discover_endpoints(&config);
    // At minimum the configured endpoints are listed.
    assert_eq!(options.len(), config.endpoints.len());
}
