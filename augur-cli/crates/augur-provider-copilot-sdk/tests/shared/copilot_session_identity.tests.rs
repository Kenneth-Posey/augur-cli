static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn copilot_client_name_is_stable() {
    assert_eq!(
        augur_provider_copilot_sdk::shared::copilot_session_identity::DCMK_COPILOT_CLIENT_NAME,
        "augur-cli"
    );
}

#[test]
fn isolated_config_dir_prefers_explicit_override() {
    let _guard = ENV_LOCK.lock().expect("env lock poisoned");
    let temp = tempfile::tempdir().expect("tempdir");
    let override_path = temp.path().join("copilot-config");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("DCMK_COPILOT_CONFIG_DIR", &override_path) };
    let result =
        augur_provider_copilot_sdk::shared::copilot_session_identity::isolated_config_dir();
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("DCMK_COPILOT_CONFIG_DIR") };

    assert_eq!(result.as_deref(), Some(override_path.as_path()));
}
