//! Local deterministic-workflow seeding and loading compile-targets.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::deterministic_orchestrator::WorkflowDocument;
use crate::domain::deterministic_orchestrator_ops::{
    decide_local_workflow_source_action, LocalWorkflowPresence, LocalWorkflowSourceAction,
};

/// Canonical workflow seed source copied only when the local file is missing.
pub const CANONICAL_PLAN_EXECUTION_PATH: &str = ".github/plan_execution.yml";
/// Local workflow runtime source used after seeding.
pub const LOCAL_PLAN_EXECUTION_PATH: &str = ".github/local/plan_execution.yml";

/// Errors produced by deterministic workflow loading adapters.
#[derive(Debug)]
pub(crate) enum WorkflowLoaderError {
    /// A filesystem read or write failed.
    Io(std::io::Error),
    /// YAML parsing failed.
    Parse(serde_yaml::Error),
    /// A workflow path attempted to escape the repository root.
    InvalidWorkflowPath,
}

impl fmt::Display for WorkflowLoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "workflow loader I/O error: {error}"),
            Self::Parse(error) => write!(f, "workflow loader parse error: {error}"),
            Self::InvalidWorkflowPath => {
                write!(f, "workflow loader path error: workflow path must stay within the repository root")
            }
        }
    }
}

impl std::error::Error for WorkflowLoaderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::InvalidWorkflowPath => None,
        }
    }
}

/// Returns the local workflow path anchored to the provided repository root.
pub(crate) fn local_workflow_path(repo_root: &Path) -> PathBuf {
    repo_root.join(LOCAL_PLAN_EXECUTION_PATH)
}

/// Ensures the deterministic runtime has a local workflow file to use.
pub(crate) fn ensure_local_workflow_file(repo_root: &Path) -> Result<PathBuf, WorkflowLoaderError> {
    let local_path = contained_workflow_path(repo_root, LOCAL_PLAN_EXECUTION_PATH)?;
    let source_action = decide_local_workflow_source_action(local_workflow_presence(&local_path));

    match source_action {
        LocalWorkflowSourceAction::UseExistingLocalWorkflow => Ok(local_path),
        LocalWorkflowSourceAction::SeedLocalWorkflowFromCanonical => {
            seed_local_workflow_from_canonical(repo_root, &local_path)?;
            Ok(local_path)
        }
    }
}

/// Loads the deterministic workflow contract from the local workflow file.
pub(crate) fn load_workflow_document(
    repo_root: &Path,
) -> Result<WorkflowDocument, WorkflowLoaderError> {
    let local_path = contained_workflow_path(repo_root, LOCAL_PLAN_EXECUTION_PATH)?;
    let content = fs::read_to_string(&local_path).map_err(WorkflowLoaderError::Io)?;
    serde_yaml::from_str(&content).map_err(WorkflowLoaderError::Parse)
}

/// Returns the semantic presence of the local workflow file.
fn local_workflow_presence(local_path: &Path) -> LocalWorkflowPresence {
    if local_path.exists() {
        LocalWorkflowPresence::Present
    } else {
        LocalWorkflowPresence::Absent
    }
}

/// Seeds the runtime-local workflow file from the canonical project workflow.
fn seed_local_workflow_from_canonical(
    repo_root: &Path,
    local_path: &Path,
) -> Result<(), WorkflowLoaderError> {
    let canonical_path = contained_workflow_path(repo_root, CANONICAL_PLAN_EXECUTION_PATH)?;
    let local_parent = local_path.parent().ok_or_else(|| {
        WorkflowLoaderError::Io(std::io::Error::other(
            "local workflow path must have a parent directory",
        ))
    })?;

    fs::create_dir_all(local_parent).map_err(WorkflowLoaderError::Io)?;
    fs::copy(canonical_path, local_path)
        .map(|_| ())
        .map_err(WorkflowLoaderError::Io)
}

fn contained_workflow_path(
    repo_root: &Path,
    workflow_relative_path: &str,
) -> Result<PathBuf, WorkflowLoaderError> {
    let canonical_repo_root = fs::canonicalize(repo_root).map_err(WorkflowLoaderError::Io)?;
    let candidate_path = canonical_repo_root.join(workflow_relative_path);
    ensure_path_is_contained(&canonical_repo_root, &candidate_path)?;
    Ok(candidate_path)
}

fn ensure_path_is_contained(
    repo_root: &Path,
    candidate_path: &Path,
) -> Result<(), WorkflowLoaderError> {
    let existing_ancestor = nearest_existing_ancestor(candidate_path)
        .ok_or(WorkflowLoaderError::InvalidWorkflowPath)?;
    let canonical_ancestor =
        fs::canonicalize(existing_ancestor).map_err(WorkflowLoaderError::Io)?;

    if canonical_ancestor.starts_with(repo_root) {
        Ok(())
    } else {
        Err(WorkflowLoaderError::InvalidWorkflowPath)
    }
}

fn nearest_existing_ancestor(candidate_path: &Path) -> Option<&Path> {
    let mut current = Some(candidate_path);

    while let Some(path) = current {
        if path.exists() {
            return Some(path);
        }

        current = path.parent();
    }

    None
}
