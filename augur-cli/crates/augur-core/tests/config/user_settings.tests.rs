use augur_core::config::user_settings::UserSettings;

#[test]
fn default_settings_have_expected_values() {
    let settings = UserSettings::default();
    assert_eq!(settings.last_endpoint.as_deref(), Some("openrouter"));
    assert!(settings.last_model.is_some());
    assert!(settings.last_reasoning_effort.is_some());
}

#[test]
fn user_settings_clone_is_equal() {
    let s = UserSettings::default();
    let s2 = s.clone();
    assert_eq!(s.last_endpoint, s2.last_endpoint);
    assert_eq!(s.last_model, s2.last_model);
}
