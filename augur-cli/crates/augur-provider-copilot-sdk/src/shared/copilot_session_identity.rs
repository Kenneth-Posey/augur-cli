//! Shared Copilot SDK session-identity helpers.
//!
//! We set an explicit SDK client name and session config directory so
//! augur-cli sessions do not mix with regular Copilot CLI sessions.

/// Stable SDK client name used for all augur-cli Copilot sessions.
pub const DCMK_COPILOT_CLIENT_NAME: &str = "augur-cli";

/// Build the dedicated Copilot SDK session config directory path.
///
/// Priority:
/// 1. `DCMK_COPILOT_CONFIG_DIR` override (when set and non-empty)
/// 2. `$HOME/.config/augur-cli/copilot-sdk`
/// 3. `/tmp/augur-cli/copilot-sdk` fallback when `HOME` is unset
///
/// Returns `None` only if directory creation fails.
pub fn isolated_config_dir() -> Option<std::path::PathBuf> {
    let explicit = std::env::var("DCMK_COPILOT_CONFIG_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from);
    let base = explicit.unwrap_or_else(default_config_dir);
    match std::fs::create_dir_all(&base) {
        Ok(()) => Some(base),
        Err(error) => {
            tracing::warn!(
                path = %base.display(),
                error = %error,
                "failed to create isolated Copilot SDK config dir; falling back to CLI default"
            );
            None
        }
    }
}

fn default_config_dir() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        .join(".config")
        .join("augur-cli")
        .join("copilot-sdk")
}
