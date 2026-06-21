//! Shared status-bar field refresh helpers used by both the TUI actor and UI logic.

use crate::domain::tui_state::StatusBarData;
use augur_domain::domain::string_newtypes::{GitBranch, StringNewtype, WorkingDir};

/// Refresh the cwd and git branch fields that back the left side of the status bar.
///
/// Used during initial status-bar construction and after agent turn completion so
/// the rendered branch/cwd always reflect the live repository state.
pub fn refresh_status_bar_base_fields(status: &mut StatusBarData) {
    status.git_branch = read_git_branch();
    status.cwd = WorkingDir::new(
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| String::from("?")),
    );
}

/// Run `git branch --show-current` and return the current branch name.
///
/// Returns `None` if the command fails, the output is not valid UTF-8, or the
/// working tree is in detached-HEAD state (empty branch name). Appends `'*'`
/// when [`read_git_is_dirty`] returns `true`.
pub(crate) fn read_git_branch() -> Option<GitBranch> {
    let output = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if branch.is_empty() {
        return None;
    }
    if read_git_is_dirty() {
        branch.push('*');
    }
    Some(GitBranch::new(branch))
}

/// Return `true` when the working tree has uncommitted changes.
///
/// Runs `git status --porcelain`. Returns `false` if the command fails or
/// produces no output. Used by [`read_git_branch`] to append a `'*'` marker.
fn read_git_is_dirty() -> bool {
    let output = match std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };
    output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty()
}
