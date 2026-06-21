//! User settings persistence: saves/restores provider and model selections across sessions.
//!
//! Settings are stored in the `user_settings:` section of
//! `~/.augur-cli/config/application.yaml`.

pub use augur_domain::config::types::UserSettings;

use crate::config::write_section_value;
use augur_domain::domain::string_newtypes::{EndpointName, ModelId, StringNewtype};
use augur_domain::domain::thinking_mode::ReasoningEffort;
use std::path::{Path, PathBuf};

/// Return the path to the installed application config file:
/// `~/.augur-cli/config/application.yaml`.
///
/// Always returns `Some`; the `Option` wrapper is kept for API compatibility
/// with callers that handle `None` as a graceful no-op.
/// Returns `None` when `$HOME` is not set.
pub fn user_settings_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join(".augur-cli")
            .join("config")
            .join("application.yaml"),
    )
}

fn load_section(path: &Path) -> UserSettings {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let value: serde_yaml::Value =
        serde_yaml::from_str(&content).unwrap_or(serde_yaml::Value::Null);
    match value.get("user_settings") {
        Some(section) => serde_yaml::from_value(section.clone()).unwrap_or_default(),
        None => UserSettings::default(),
    }
}

fn write_section(path: &Path, settings: &UserSettings) {
    let yaml_lines = serde_yaml::to_string(&settings).unwrap_or_default();
    write_section_value(path, "user_settings", &yaml_lines);
}

/// Borrowed selection values used when persisting [`UserSettings`].
pub(crate) struct UserSettingsSelection<'a> {
    pub endpoint: Option<&'a EndpointName>,
    pub model: Option<&'a ModelId>,
    pub effort: Option<&'a ReasoningEffort>,
}

impl<'a> UserSettingsSelection<'a> {
    pub(crate) fn new(
        endpoint: Option<&'a EndpointName>,
        model: Option<&'a ModelId>,
        effort: Option<&'a ReasoningEffort>,
    ) -> Self {
        Self {
            endpoint,
            model,
            effort,
        }
    }
}

fn save_to_path(path: &Path, selection: UserSettingsSelection<'_>) {
    let settings = UserSettings {
        last_endpoint: selection.endpoint.map(|e| e.as_str().to_owned()),
        last_model: selection.model.map(|m| m.as_str().to_owned()),
        last_reasoning_effort: selection.effort.map(|e| e.as_ref().to_owned()),
    };
    write_section(path, &settings);
}

/// Load user settings from the installed application config.
///
/// Returns `UserSettings::default()` when the config file is missing or
/// the `user_settings:` section is absent.
pub fn load_user_settings() -> UserSettings {
    match user_settings_path().filter(|p| p.exists()) {
        Some(path) => load_section(&path),
        None => UserSettings::default(),
    }
}

/// Save user settings to the installed application config synchronously.
///
/// Updates only the `user_settings:` section; other sections are preserved.
/// Silently ignores failures - user settings are best-effort.
/// Does nothing when `$HOME` is unset or the config file does not exist.
pub fn save_user_settings_sync(
    endpoint: Option<&EndpointName>,
    model: Option<&ModelId>,
    effort: Option<&ReasoningEffort>,
) {
    let Some(path) = user_settings_path().filter(|p| p.exists()) else {
        return;
    };
    save_to_path(&path, UserSettingsSelection::new(endpoint, model, effort));
}

/// Save user settings to the installed application config.
///
/// Updates only the `user_settings:` section; other sections are preserved.
/// Silently ignores failures - user settings are best-effort.
/// Does nothing when `$HOME` is unset or the config file does not exist.
pub fn save_user_settings(
    endpoint: Option<&EndpointName>,
    model: Option<&ModelId>,
    effort: Option<&ReasoningEffort>,
) {
    let Some(path) = user_settings_path().filter(|p| p.exists()) else {
        return;
    };
    save_to_path(&path, UserSettingsSelection::new(endpoint, model, effort));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_user_settings_has_expected_values() {
        let s = UserSettings::default();
        assert_eq!(s.last_endpoint.as_deref(), Some("openrouter"));
        assert_eq!(s.last_model.as_deref(), Some("deepseek/deepseek-v4-flash"));
        assert_eq!(s.last_reasoning_effort.as_deref(), Some("high"));
    }

    #[test]
    fn user_settings_clone_equality() {
        let s = UserSettings::default();
        assert_eq!(s.clone(), s);
    }

    #[test]
    fn load_section_returns_default_for_missing_file() {
        let path = std::path::Path::new("/no/such/settings/file.yaml");
        let settings = load_section(path);
        assert_eq!(settings, UserSettings::default());
    }

    #[test]
    fn write_then_load_section_roundtrip() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("application.yaml");

        // Write some initial YAML with unrelated content
        std::fs::write(&path, "endpoints: []\ndefault_endpoint: openrouter\n")
            .expect("write initial yaml");

        // Write a user_settings section
        let settings = UserSettings {
            last_endpoint: Some("copilot".to_owned()),
            last_model: Some("claude-3-5-sonnet".to_owned()),
            last_reasoning_effort: None,
        };
        write_section(&path, &settings);

        // Load it back - should preserve user_settings section
        let loaded = load_section(&path);
        assert_eq!(loaded.last_endpoint.as_deref(), Some("copilot"));
        assert_eq!(loaded.last_model.as_deref(), Some("claude-3-5-sonnet"));
        assert_eq!(loaded.last_reasoning_effort, None);
    }

    #[test]
    fn write_section_preserves_unrelated_content_and_comments() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("application.yaml");

        let initial = "\
# ── Comments at top ───────────────────────────────────────────────────
endpoints:
  - name: openrouter
    provider: OpenRouter

# ── Persistence paths ─────────────────────────────────────────────────
persistence:
  log_dir: /home/user/.augur-cli/logs

# ── User settings ─────────────────────────────────────────────────────
# This comment should be preserved too.
user_settings:
  last_endpoint: old_endpoint
  last_model: old_model
  last_reasoning_effort: low

# ── Footer comments ───────────────────────────────────────────────────
# These must survive as well.
";
        std::fs::write(&path, initial).expect("write initial yaml");

        let settings = UserSettings {
            last_endpoint: Some("openrouter".to_owned()),
            last_model: Some("deepseek/deepseek-v4-flash".to_owned()),
            last_reasoning_effort: Some("high".to_owned()),
        };
        write_section(&path, &settings);

        let after = std::fs::read_to_string(&path).expect("read result");

        // Comments before the section must survive
        assert!(
            after.contains("# ── Comments at top"),
            "pre-section comments were stripped:\n{}",
            after
        );
        assert!(
            after.contains("# ── Persistence paths"),
            "persistence comments were stripped:\n{}",
            after
        );
        assert!(
            after.contains("# ── Footer comments"),
            "footer comments were stripped:\n{}",
            after
        );
        assert!(
            after.contains("# These must survive as well."),
            "footer comment lines were stripped:\n{}",
            after
        );

        // The user_settings section boundary must survive
        assert!(
            after.contains("# ── User settings"),
            "user_settings header comment was stripped:\n{}",
            after
        );

        // New values must be present
        assert!(
            after.contains("last_endpoint: openrouter"),
            "new endpoint not found:\n{}",
            after
        );
        assert!(
            after.contains("last_model: deepseek/deepseek-v4-flash"),
            "new model not found:\n{}",
            after
        );
        assert!(
            after.contains("last_reasoning_effort: high"),
            "new effort not found:\n{}",
            after
        );

        let loaded = load_section(&path);
        assert_eq!(loaded.last_endpoint.as_deref(), Some("openrouter"));
        assert_eq!(
            loaded.last_model.as_deref(),
            Some("deepseek/deepseek-v4-flash")
        );
        assert_eq!(loaded.last_reasoning_effort.as_deref(), Some("high"));
    }

    #[test]
    fn write_section_handles_missing_section_by_appending() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("application.yaml");

        let initial = "\
# Header comment
endpoints: []
default_endpoint: openrouter
";
        std::fs::write(&path, initial).expect("write initial yaml");

        let settings = UserSettings {
            last_endpoint: Some("openrouter".to_owned()),
            last_model: Some("deepseek/deepseek-v4-flash".to_owned()),
            last_reasoning_effort: None,
        };
        write_section(&path, &settings);

        let after = std::fs::read_to_string(&path).expect("read result");
        assert!(after.contains("# Header comment"), "header comment lost");
        assert!(after.contains("user_settings:"), "section not appended");
        assert!(
            after.contains("last_endpoint: openrouter"),
            "endpoint missing"
        );

        let loaded = load_section(&path);
        assert_eq!(loaded.last_endpoint.as_deref(), Some("openrouter"));
    }
}
