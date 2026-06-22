//! Async loader that reads an agent specification file from disk.
//!
//! Delegates all parsing to [`augur_domain::parse_agent_spec`]; this module
//! is responsible only for file IO and error mapping. No parsing logic lives
//! here.

use augur_domain::{AgentSpec, AgentSpecName, AgentSpecParseError, parse_agent_spec};
use std::fmt;
use std::path::{Path, PathBuf};

/// Error returned when loading an agent specification file fails.
#[derive(Debug)]
pub enum SpecLoadError {
    /// The file could not be read from disk.
    Io(std::io::Error),
    /// The file content could not be parsed as a valid agent specification.
    Parse(AgentSpecParseError),
}

impl fmt::Display for SpecLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecLoadError::Io(e) => write!(f, "IO error loading agent spec: {e}"),
            SpecLoadError::Parse(e) => write!(f, "parse error loading agent spec: {e}"),
        }
    }
}

impl std::error::Error for SpecLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SpecLoadError::Io(e) => Some(e),
            SpecLoadError::Parse(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for SpecLoadError {
    fn from(e: std::io::Error) -> Self {
        SpecLoadError::Io(e)
    }
}

impl From<AgentSpecParseError> for SpecLoadError {
    fn from(e: AgentSpecParseError) -> Self {
        SpecLoadError::Parse(e)
    }
}

/// Load an agent specification from a file on disk.
///
/// Reads the file at `path` asynchronously using `tokio::fs` and delegates
/// to [`parse_agent_spec`] to extract YAML frontmatter and instruction body.
///
/// # Parameters
///
/// - `path`: filesystem path to the `.md` agent spec file.
/// - `name`: logical name forwarded to the parser as the fallback description.
///
/// # Errors
///
/// Returns [`SpecLoadError::Io`] if the file cannot be read, or
/// [`SpecLoadError::Parse`] if the file content contains malformed YAML.
pub async fn load_agent_spec(
    path: &std::path::Path,
    name: AgentSpecName,
) -> Result<AgentSpec, SpecLoadError> {
    let content = tokio::fs::read_to_string(path).await?;
    let spec = parse_agent_spec(&content, name)?;
    Ok(spec)
}

/// Strip the numeric-prefix portion from an agent file stem.
///
/// Agent spec files follow the naming convention
/// `{stage}-{category}-{seq}-{logical-name}`, e.g.
/// `0-global-06-git-operator`. This function returns the logical name tail
/// (`git-operator`), leaving the stem unchanged when it does not match the
/// convention (e.g. a plain `git-operator` stem is returned as-is).
///
/// # Examples
///
/// ```text
/// strip_agent_name_prefix("0-global-06-git-operator") == "git-operator"
/// strip_agent_name_prefix("1-design-01-requirements-builder") == "requirements-builder"
/// strip_agent_name_prefix("git-operator") == "git-operator"
/// ```
pub fn strip_agent_name_prefix(stem: &AgentSpecName) -> AgentSpecName {
    let stem_text = stem.as_ref();
    let parts: Vec<&str> = stem_text.splitn(4, '-').collect();
    if parts.len() == 4
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[2].chars().all(|c| c.is_ascii_digit())
    {
        AgentSpecName::new(parts[3])
    } else {
        stem.clone()
    }
}

/// Locate the filesystem path for an agent spec by its logical name.
///
/// First tries an exact match: `{base}/{name}.agent.md`. If that file does
/// not exist, scans `base` for a file whose name ends with
/// `-{name}.agent.md` - this handles the
/// `{stage}-{category}-{seq}-{logical-name}.agent.md` prefix pattern used
/// by the `.github/agents/` directory.
///
/// When not found in `base`, falls back to the installed config directory at
/// `~/.augur-cli/.github/agents/` so agent specs placed there are also
/// discoverable at spawn time.
///
/// Returns `None` when neither directory can be read or no match is found.
pub fn find_agent_spec_path(base: &Path, name: &AgentSpecName) -> Option<PathBuf> {
    // Try primary search path first.
    if let Some(found) = find_in_dir(base, name) {
        return Some(found);
    }
    // Fall back to installed config directory.
    if let Ok(home) = std::env::var("HOME") {
        let fallback = PathBuf::from(home).join(".augur-cli/.github/agents");
        if fallback.exists() && fallback != base {
            return find_in_dir(&fallback, name);
        }
    }
    None
}

/// Search a single directory for an agent spec file matching `name`.
fn find_in_dir(base: &Path, name: &AgentSpecName) -> Option<PathBuf> {
    let exact = base.join(format!("{}.agent.md", name.as_ref()));
    if exact.exists() {
        return Some(exact);
    }
    let Ok(entries) = std::fs::read_dir(base) else {
        return None;
    };
    let suffix = format!("-{}.agent.md", name.as_ref());
    entries.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(&suffix))
            .unwrap_or(false)
    })
}
