//! Project-level token history: persistent state across all sessions.
//!
//! `ProjectSettings` is saved to `state/token-history.json` in the working
//! directory. Uses an atomic temp-file rename on save to avoid partial-write corruption.

use augur_domain::domain::types::ProjectTokenTotals;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Root settings object stored in `state/token-history.json`.
///
/// Extend with additional project-level fields here; existing serde data will
/// continue to round-trip cleanly via `#[serde(default)]` on any new optional fields.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProjectSettings {
    #[serde(default)]
    pub token_totals: ProjectTokenTotals,
}

impl ProjectSettings {}

/// Return the canonical path for the token history file.
///
/// Resolves to `./state/token-history.json` in the current working directory.
/// This is the single source of truth for the settings file location; do not
/// hardcode the filename anywhere else.
pub fn token_history_path() -> PathBuf {
    PathBuf::from("./state/token-history.json")
}

/// Load project settings from `path`, or return defaults when the file is absent.
///
/// Returns `ProjectSettings::default()` when the file does not exist.
/// Returns `Err` on malformed JSON or permission errors.
/// Does **not** create the file - creation happens on the first `save` call.
pub fn load_or_create(path: &Path) -> anyhow::Result<ProjectSettings> {
    if !path.exists() {
        return Ok(ProjectSettings::default());
    }
    let json = std::fs::read_to_string(path)?;
    let settings = serde_json::from_str(&json)?;
    Ok(settings)
}

/// Ensure the token-history file exists on disk, creating a default file when missing.
pub fn ensure_initialized(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        return Ok(());
    }
    save(&ProjectSettings::default(), path)
}

/// Serialize `settings` to `path` using an atomic temp-file rename.
///
/// Writes to `<path>.tmp`, then renames to `path` so partial writes do not
/// corrupt the settings file. Creates parent directories when absent.
/// Returns `Err` on serialization or I/O failure.
pub fn save(settings: &ProjectSettings, path: &Path) -> anyhow::Result<()> {
    create_parent_dirs(path)?;
    let json = serde_json::to_string_pretty(settings)?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, &json)?;
    std::fs::rename(&temp_path, path)?;
    Ok(())
}

/// Create all parent directories for `path` when they do not already exist.
fn create_parent_dirs(path: &Path) -> anyhow::Result<()> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            std::fs::create_dir_all(parent)?;
            Ok(())
        }
        _ => Ok(()),
    }
}
