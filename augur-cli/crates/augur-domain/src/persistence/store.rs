//! Session file I/O: save, load, and list session records.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::string_newtypes::{FilePath, SessionId, StringNewtype};
use crate::persistence::types::{SessionRecord, SessionSummary, summarize};

const MAX_SESSION_LIST_SIZE: usize = 20;

/// Detect the Git repository name by reading `origin` remote from a git config
/// file rooted at `cwd`, or by reading the basename of the `.git` worktree path.
///
/// Returns `None` when the directory is not inside a Git repository or the
/// repository name cannot be determined.
pub fn detect_git_repo_name(cwd: &Path) -> Option<String> {
    // Walk up from cwd looking for a .git directory (worktree) or .git file (submodule)
    let git_path = find_git_dir(cwd)?;

    // Resolve the actual git directory: submodules use a .git file containing
    // "gitdir: <path>" pointing to the real git directory in the parent repo.
    let git_dir = if git_path.is_dir() {
        git_path.clone()
    } else if git_path.is_file() {
        // Read the .git file to find the actual gitdir path
        let content = std::fs::read_to_string(&git_path).ok()?;
        let gitdir_line = content.lines().next()?;
        let path_str = gitdir_line.strip_prefix("gitdir: ")?.trim();
        let resolved = git_path.parent()?.join(path_str);
        if resolved.is_dir() {
            resolved
        } else {
            return None;
        }
    } else {
        return None;
    };

    // Read the `remote "origin".url` from .git/config if available.
    let config_path = git_dir.join("config");
    if let Ok(content) = std::fs::read_to_string(&config_path)
        && let Some(name) = extract_repo_name_from_git_config(&content)
    {
        return Some(name);
    }

    // Fallback: use the parent directory's basename
    let parent = git_path.parent()?;
    let name = parent.file_name()?.to_str()?.to_owned();
    // Ignore bare `.git` as a repo name
    if name != ".git" && !name.is_empty() {
        return Some(name);
    }

    None
}

/// Walk up from `cwd` looking for a `.git` directory or file.
fn find_git_dir(cwd: &Path) -> Option<PathBuf> {
    let mut current = Some(cwd);
    while let Some(dir) = current {
        let candidate = dir.join(".git");
        if candidate.exists() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

/// Extract the repository name from a `remote "origin".url` line in git config content.
fn extract_repo_name_from_git_config(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(url_val) = trimmed.strip_prefix("url = ") {
            let url = url_val.trim_matches('"');
            // Handle common URL formats:
            //   https://github.com/owner/repo.git
            //   git@github.com:owner/repo.git
            //   /absolute/path/repo (local path)
            let repo_part = if let Some(pos) = url.rfind('/') {
                &url[pos + 1..]
            } else {
                url
            };
            let name = repo_part.strip_suffix(".git").unwrap_or(repo_part);
            // Reject `.` and `..` -- these would resolve to unexpected
            // parent/self paths when used with `PathBuf::join`.
            if !name.is_empty() && name != "." && name != ".." {
                return Some(name.to_owned());
            }
        }
    }
    None
}

/// Apply a repo-name subdirectory nesting to a base path, if a git repo name
/// can be detected from `cwd`.
///
/// Returns `base / repo_name` when a repo name is detected, or `base` unchanged
/// when no git repository context is found.
pub fn apply_repo_subdir(base: PathBuf, cwd: &Path) -> PathBuf {
    match detect_git_repo_name(cwd) {
        Some(repo_name) => base.join(repo_name),
        None => base,
    }
}

pub fn resolve_sessions_dir(configured: Option<&FilePath>) -> PathBuf {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .expect("HOME environment variable must be set");

    match configured.map(|path| path.as_str()) {
        Some(path) if path.starts_with("~/") => home.join(&path[2..]),
        Some("~") => home.clone(),
        Some(path) => PathBuf::from(path),
        None => home.join(".augur-cli/sessions"),
    }
}

#[tracing::instrument(level = "debug", skip(record))]
pub fn save_session(record: &SessionRecord, dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(record)?;
    let id_str = &*record.meta.id;
    let target = dir.join(format!("{id_str}.json"));
    let tmp = dir.join(format!("{id_str}.tmp"));
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &target)?;
    Ok(())
}

#[tracing::instrument(level = "debug")]
pub fn load_session(dir: &Path, id: &SessionId) -> anyhow::Result<SessionRecord> {
    let id_str = &**id;
    let path = dir.join(format!("{id_str}.json"));
    let json = fs::read_to_string(&path)?;
    let record = serde_json::from_str(&json)?;
    Ok(record)
}

#[tracing::instrument(level = "debug")]
pub fn delete_session(dir: &Path, id: &SessionId) -> anyhow::Result<()> {
    let id_str = &**id;
    let path = dir.join(format!("{id_str}.json"));
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[tracing::instrument(level = "debug")]
pub fn list_sessions(dir: &Path) -> anyhow::Result<Vec<SessionSummary>> {
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut summaries = collect_summaries(dir);
    summaries.sort_by(|a, b| b.identity.last_updated_at.cmp(&a.identity.last_updated_at));
    summaries.truncate(MAX_SESSION_LIST_SIZE);
    Ok(summaries)
}

fn collect_summaries(dir: &Path) -> Vec<SessionSummary> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|e| load_record(&e.path()))
        .filter(|r| !r.meta.flags.ask_session.0)
        .map(|r| summarize(&r))
        .collect()
}

fn load_record(path: &Path) -> Option<SessionRecord> {
    let json = fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}
