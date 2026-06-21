//! Two-tier path resolution helpers for runtime resource lookup.
//!
//! When a `.github/` runtime resource is not found relative to the current
//! working directory (the developer workflow), these helpers fall back to
//! `~/.augur-cli/` (the installed config directory). This enables the CLI to
//! run from any directory after installation without silently degrading agent
//! dispatch, instruction loading, or workflow discovery.

use std::path::PathBuf;

/// Return the effective repository root directory.
///
/// Checks CWD first (for developer workflow). If CWD has no `.github/`
/// directory, falls back to the installed config directory at
/// `~/.augur-cli/`. If neither has `.github/`, returns CWD.
///
/// # Examples
///
/// ```ignore
/// let root = effective_repo_root();
/// assert!(root.join(".github").exists() || !root.join(".github").exists());
/// ```
pub fn effective_repo_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join(".github").exists() {
        return cwd;
    }
    if let Ok(home) = std::env::var("HOME") {
        let install = PathBuf::from(home).join(".augur-cli");
        if install.join(".github").exists() {
            return install;
        }
    }
    cwd
}

/// Resolve a repo-relative file path by checking CWD first, then the
/// installed config directory (`~/.augur-cli/...`).
///
/// Returns the first path that exists on disk. When neither exists,
/// returns the CWD-relative path (caller handles the missing-file case).
///
/// # Examples
///
/// ```ignore
/// let path = resolve_install_path(".github/copilot-instructions.md");
/// // Returns CWD-relative or `~/.augur-cli/.github/copilot-instructions.md`
/// ```
pub fn resolve_install_path(relative: &str) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cwd_path = cwd.join(relative);
    if cwd_path.exists() {
        return cwd_path;
    }
    if let Ok(home) = std::env::var("HOME") {
        let install_path = PathBuf::from(home).join(".augur-cli").join(relative);
        if install_path.exists() {
            return install_path;
        }
    }
    cwd_path
}