use augur_domain::config::types::ProgramSettings;
use augur_domain::domain::StringNewtype;

#[test]
fn default_program_settings_exclude_git_target_and_changelogs() {
    let settings = ProgramSettings::default();
    let values: Vec<_> = settings
        .excluded_directories
        .iter()
        .map(|p| p.as_str().to_owned())
        .collect();
    assert_eq!(values, vec![".git", "target", "changelogs"]);
}

#[test]
fn roundtrip_yaml_preserves_excluded_directories() {
    let original = ProgramSettings::default();
    let yaml = serde_yaml::to_string(&original).expect("serialize");
    let restored: ProgramSettings = serde_yaml::from_str(&yaml).expect("deserialize");
    assert_eq!(
        restored.excluded_directory_paths(),
        original.excluded_directory_paths()
    );
}
