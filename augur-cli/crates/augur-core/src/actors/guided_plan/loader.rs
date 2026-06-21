//! Guided plan file loader: reads and parses YAML frontmatter from plan files.

use augur_domain::domain::guided_plan::GuidedPlanConfig;
use std::fmt;
use std::path::Path;

/// Sentinel string that delimits the YAML frontmatter block.
///
/// A guided plan file starts with `---\n`, a YAML block, then a second `---\n`
/// line. Everything after the second delimiter is the markdown body, which is
/// ignored by the loader.
const FRONTMATTER_DELIM: &str = "---";

/// Errors produced by `load_guided_plan`.
///
/// Consumers: `key_dispatch::handle_submit` (for the `/run-plan` command) which
/// maps each variant to a user-facing error message pushed to the output pane.
#[derive(Debug)]
pub enum LoadError {
    /// Failed to read the file from disk.
    Io(std::io::Error),
    /// The file does not have a valid `---` frontmatter block, or the `guided`
    /// key is absent or set to `false`.
    MissingFrontmatter,
    /// The YAML in the frontmatter is malformed or missing required fields.
    Parse(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "io error reading plan file: {e}"),
            LoadError::MissingFrontmatter => {
                write!(f, "plan file has no `guided: true` YAML frontmatter")
            }
            LoadError::Parse(msg) => write!(f, "plan frontmatter parse error: {msg}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// Load a `GuidedPlanConfig` from a plan file at `path`.
///
/// The file must begin with a YAML frontmatter block delimited by `---` lines and
/// contain a top-level `guided: true` key. The markdown body after the second `---`
/// is ignored. Returns `LoadError::MissingFrontmatter` when no frontmatter is
/// present or `guided` is `false`. Returns `LoadError::Parse` when the YAML is
/// malformed or required fields are absent.
///
/// Call site: `/run-plan` command handler in `key_dispatch::handle_submit` (Phase 4).
pub fn load_guided_plan(path: &Path) -> Result<GuidedPlanConfig, LoadError> {
    let content = std::fs::read_to_string(path).map_err(LoadError::Io)?;
    let yaml = extract_frontmatter(&content).ok_or(LoadError::MissingFrontmatter)?;
    validate_guided_flag(yaml)?;
    let config = serde_yaml::from_str::<GuidedPlanConfig>(yaml)
        .map_err(|e| LoadError::Parse(e.to_string()))?;
    Ok(config)
}

/// Extract the YAML block between the first and second `---` delimiters.
///
/// Returns `None` when the file does not start with `---` or has no closing
/// delimiter. The returned slice is the content between the two delimiters
/// (exclusive).
fn extract_frontmatter(content: &str) -> Option<&str> {
    let body = content
        .strip_prefix(&format!("{FRONTMATTER_DELIM}\n"))
        .or_else(|| content.strip_prefix(&format!("{FRONTMATTER_DELIM}\r\n")))?;
    let end = body
        .find(&format!("\n{FRONTMATTER_DELIM}\n"))
        .or_else(|| body.find(&format!("\n{FRONTMATTER_DELIM}\r\n")))
        .or_else(|| body.find(&format!("\n{FRONTMATTER_DELIM}")));
    let end_offset = end?;
    Some(&body[..end_offset])
}

/// Reject files where the `guided` YAML key is absent or explicitly `false`.
///
/// Parses a minimal YAML mapping to check only the `guided` key before doing
/// the full `GuidedPlanConfig` parse, so errors are attributed correctly.
fn validate_guided_flag(yaml: &str) -> Result<(), LoadError> {
    #[derive(serde::Deserialize)]
    struct GuidedFlag {
        #[serde(default)]
        guided: bool,
    }
    let flag: GuidedFlag =
        serde_yaml::from_str(yaml).map_err(|e| LoadError::Parse(e.to_string()))?;
    if flag.guided {
        Ok(())
    } else {
        Err(LoadError::MissingFrontmatter)
    }
}
