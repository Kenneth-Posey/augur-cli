//! Program settings persistence: project-owned defaults that shape runtime behavior.
//!
//! Settings are stored in the `program_settings:` section of
//! `~/.augur-cli/config/application.yaml`.

pub use augur_domain::config::types::ProgramSettings;

use crate::config::write_section_value;
use std::path::{Path, PathBuf};

/// Return the path to the installed application config file:
/// `~/.augur-cli/config/application.yaml`.
///
/// Returns `None` when `$HOME` is not set.
pub fn program_settings_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join(".augur-cli")
            .join("config")
            .join("application.yaml"),
    )
}

fn load_section(path: &Path) -> ProgramSettings {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let value: serde_yaml::Value =
        serde_yaml::from_str(&content).unwrap_or(serde_yaml::Value::Null);
    match value.get("program_settings") {
        Some(section) => serde_yaml::from_value(section.clone()).unwrap_or_default(),
        None => ProgramSettings::default(),
    }
}

fn write_section(path: &Path, settings: &ProgramSettings) {
    let yaml_lines = serde_yaml::to_string(&settings).unwrap_or_default();
    write_section_value(path, "program_settings", &yaml_lines);
}

/// Load program settings from the installed application config.
///
/// Returns `ProgramSettings::default()` when the config file is missing or
/// the `program_settings:` section is absent.
pub fn load_program_settings() -> ProgramSettings {
    match program_settings_path().filter(|p| p.exists()) {
        Some(path) => load_section(&path),
        None => ProgramSettings::default(),
    }
}

/// Save program settings to the installed application config synchronously.
///
/// Updates only the `program_settings:` section; other sections are preserved.
/// Silently ignores failures - program settings are best-effort.
/// Does nothing when `$HOME` is unset or the config file does not exist.
pub fn save_program_settings_sync(settings: &ProgramSettings) {
    let Some(path) = program_settings_path().filter(|p| p.exists()) else {
        return;
    };
    write_section(&path, settings);
}

/// Save program settings to the installed application config.
///
/// Updates only the `program_settings:` section; other sections are preserved.
/// Silently ignores failures - program settings are best-effort.
/// Does nothing when `$HOME` is unset or the config file does not exist.
pub fn save_program_settings(settings: &ProgramSettings) {
    let Some(path) = program_settings_path().filter(|p| p.exists()) else {
        return;
    };
    write_section(&path, settings);
}

#[cfg(test)]
mod tests {
    use super::*;
    use augur_domain::domain::string_newtypes::{FilePath, StringNewtype};

    #[test]
    fn load_section_returns_default_for_missing_file() {
        let path = std::path::Path::new("/no/such/settings/file.yaml");
        let settings = load_section(path);
        assert_eq!(settings.excluded_directories.len(), 3);
    }

    #[test]
    fn write_then_load_section_roundtrip() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("application.yaml");

        std::fs::write(&path, "endpoints: []\ndefault_endpoint: openrouter\n")
            .expect("write initial yaml");

        let settings = ProgramSettings {
            excluded_directories: vec![
                FilePath::new(".git"),
                FilePath::new("target"),
                FilePath::new("node_modules"),
            ],
        };
        write_section(&path, &settings);

        let loaded = load_section(&path);
        let paths: Vec<_> = loaded
            .excluded_directories
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        assert_eq!(paths, vec![".git", "target", "node_modules"]);
    }

    #[test]
    fn write_section_preserves_comments_outside_program_settings() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("application.yaml");

        let initial = "\
# ── Endpoints ─────────────────────────────────────────────────────────
endpoints:
  - name: openrouter

# ── Persistence ───────────────────────────────────────────────────────
persistence:
  log_dir: /some/dir

# ── Program settings ──────────────────────────────────────────────────
# Some comment about excluded dirs
program_settings:
  excluded_directories:
    - .git
    - target

# ── Footer ────────────────────────────────────────────────────────────
# Final comment
";
        std::fs::write(&path, initial).expect("write initial yaml");

        let settings = ProgramSettings {
            excluded_directories: vec![
                FilePath::new(".git"),
                FilePath::new("target"),
                FilePath::new("changelogs"),
            ],
        };
        write_section(&path, &settings);

        let after = std::fs::read_to_string(&path).expect("read result");

        // Comments before and after must survive
        assert!(after.contains("# ── Endpoints"), "endpoints header lost");
        assert!(
            after.contains("# ── Persistence"),
            "persistence header lost"
        );
        assert!(after.contains("# ── Footer"), "footer header lost");
        assert!(after.contains("# Final comment"), "footer comment lost");
        assert!(
            after.contains("# Some comment about excluded dirs"),
            "section header comment lost"
        );

        // New values must be present
        assert!(after.contains("changelogs"), "changelogs entry missing");
        assert!(after.contains("excluded_directories:"));

        let loaded = load_section(&path);
        let paths: Vec<_> = loaded
            .excluded_directories
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        assert_eq!(paths, vec![".git", "target", "changelogs"]);
    }
}
