//! Artifact storage helpers for the deterministic orchestrator.
//!
//! The background agent is solely responsible for reading and writing artifact
//! files via file tool calls (`file_read`, `file_create`, `file_replace`, etc.)
//! in its own LLM conversation. The orchestrator only tracks whether declared
//! artifact paths exist on disk -- it never reads or writes file content itself.

use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::domain::deterministic_orchestrator::WorkflowArtifactRef;
use crate::domain::deterministic_orchestrator::WorkflowStep;
use crate::domain::deterministic_orchestrator_ops::StepIndex;
use augur_domain::domain::WorkflowStepId;
use augur_domain::domain::string_newtypes::StringNewtype;

/// Existence marker for a workflow artifact.
///
/// Carried through the pipeline command channel solely to confirm that the
/// background agent created the file. No file content is loaded into pipeline
/// state -- the evaluator/reviewer agent reads content itself via `file_read`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArtifactExistence {
    /// Typed workflow artifact reference that was confirmed to exist.
    pub(crate) artifact: WorkflowArtifactRef,
}

/// Errors produced by the deterministic orchestrator artifact store.
#[derive(Debug)]
pub(crate) enum ArtifactStoreError {
    /// A filesystem read or write failed.
    Io(std::io::Error),
    /// An artifact path attempted to escape the repository root.
    InvalidArtifactPath,
}

impl fmt::Display for ArtifactStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "artifact store I/O error: {error}"),
            Self::InvalidArtifactPath => {
                write!(
                    f,
                    "artifact store path error: artifact path must stay within the repository root"
                )
            }
        }
    }
}

impl std::error::Error for ArtifactStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidArtifactPath => None,
        }
    }
}

/// Returns `true` when a path string looks like a prose description rather than
/// a real file path.
///
/// A path is prose if it contains at least one ASCII space character, if it
/// contains both `<` and `>` (an unresolved `<…>` placeholder), or if it
/// contains no `/` at all (config-key references such as
/// `"changelog_path_pattern"` or `"no-output"` never carry a directory
/// separator). Real file paths always contain at least one `/`. Prose paths are
/// skipped silently by methods that check artifact existence.
fn is_prose_path(path: &str) -> bool {
    path.contains(' ') || (path.contains('<') && path.contains('>')) || !path.contains('/')
}

/// Returns `true` when an artifact path cannot be used for existence checking.
///
/// Extends `is_prose_path` to also catch timestamp-placeholder paths whose
/// `MM-DD-YYYY-HHMM` segment is never substituted during slug expansion, leaving
/// the literal string in the path so the file can never be found by exact match.
fn is_unverifiable_artifact_path(path: &str) -> bool {
    is_prose_path(path) || path.contains("MM-DD-YYYY")
}

/// Boundary type that resolves step-scoped artifacts against a repository root.
#[derive(Clone, Debug)]
pub(crate) struct StepArtifactResolver {
    repo_root: PathBuf,
}

impl StepArtifactResolver {
    /// Creates a new resolver anchored to a repository root.
    pub(crate) fn new(repo_root: impl Into<PathBuf>) -> Self {
        let repo_root = repo_root.into();
        Self {
            repo_root: fs::canonicalize(&repo_root).unwrap_or(repo_root),
        }
    }

    /// Returns the absolute path for a workflow artifact reference.
    pub(crate) fn artifact_path(
        &self,
        artifact: &WorkflowArtifactRef,
    ) -> Result<PathBuf, ArtifactStoreError> {
        let relative_path = self.normalized_relative_path(artifact)?;
        let absolute_path = self.repo_root.join(relative_path);
        self.ensure_path_is_contained(&absolute_path)?;
        Ok(absolute_path)
    }

    /// Returns the resolver root.
    pub(crate) fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Returns the id of the first step in `step_index` whose declared output
    /// artifacts are not all present on disk.
    ///
    /// A step is directly complete when every verifiable entry in its
    /// `created_artifacts` list resolves to an existing file and at least one
    /// such entry exists.  Steps whose artifacts are all unverifiable
    /// (prose descriptions or timestamp-placeholder paths) are considered
    /// complete when any later step in the pipeline is directly complete -
    /// a later step having run proves the pipeline advanced past this point
    /// in a prior run, so checkpoint and commit steps are skipped on resume
    /// without re-creating their write artifacts.
    ///
    /// Returns `None` when all steps are complete (or the index is empty).
    pub(crate) fn find_resume_step_id(&self, step_index: &StepIndex) -> Option<WorkflowStepId> {
        let ids = &step_index.ordered_executable_step_ids;
        let directly_complete = self.directly_complete_steps(ids, step_index);

        // Second pass: find the first incomplete step.
        for (i, step_id) in ids.iter().enumerate() {
            if directly_complete[i] {
                continue;
            }
            let Some(step) = step_index.workflow_step(step_id) else {
                return Some(step_id.clone());
            };
            if self.can_skip_unverifiable_step(step, i, &directly_complete) {
                continue;
            }
            return Some(step_id.clone());
        }
        None
    }

    fn directly_complete_steps(&self, ids: &[WorkflowStepId], step_index: &StepIndex) -> Vec<bool> {
        ids.iter()
            .map(|step_id| {
                let Some(step) = step_index.workflow_step(step_id) else {
                    return false;
                };
                self.is_step_directly_complete(step)
            })
            .collect()
    }

    fn is_step_directly_complete(&self, step: &WorkflowStep) -> bool {
        let verifiable: Vec<_> = step
            .execution
            .created_artifacts
            .iter()
            .filter(|artifact| !is_unverifiable_artifact_path(artifact.path.as_str()))
            .collect();
        !verifiable.is_empty()
            && verifiable.iter().all(|artifact| {
                self.artifact_path(artifact)
                    .map(|path| path.exists())
                    .unwrap_or(false)
            })
    }

    fn can_skip_unverifiable_step(
        &self,
        step: &WorkflowStep,
        index: usize,
        directly_complete: &[bool],
    ) -> bool {
        if self.has_verifiable_artifact(step) {
            return false;
        }
        directly_complete[index + 1..]
            .iter()
            .any(|&complete| complete)
    }

    fn has_verifiable_artifact(&self, step: &WorkflowStep) -> bool {
        step.execution
            .created_artifacts
            .iter()
            .any(|artifact| !is_unverifiable_artifact_path(artifact.path.as_str()))
    }

    /// Pre-creates parent directories for all step output artifacts.
    ///
    /// For each non-prose artifact in `step.execution.created_artifacts`, resolves
    /// the absolute path and calls `std::fs::create_dir_all` on its parent directory.
    /// Prose paths (containing spaces, unresolved `<…>` placeholders, or no `/`) are
    /// skipped. Directory creation failures are logged as warnings and do not halt the
    /// step.
    pub(crate) fn pre_create_output_dirs(&self, step: &WorkflowStep) {
        for artifact in &step.execution.created_artifacts {
            self.pre_create_output_dir(artifact);
        }
    }

    fn pre_create_output_dir(&self, artifact: &WorkflowArtifactRef) {
        if is_prose_path(artifact.path.as_str()) {
            return;
        }
        let path = match self.artifact_path(artifact) {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(
                    artifact = %artifact.path.as_str(),
                    %error,
                    "failed to resolve artifact path for directory pre-creation"
                );
                return;
            }
        };
        let Some(parent) = path.parent() else {
            return;
        };
        if let Err(error) = fs::create_dir_all(parent) {
            tracing::warn!(
                path = %parent.display(),
                %error,
                "failed to pre-create output artifact directory"
            );
        }
    }

    /// Checks that expected-input artifacts for a workflow step exist on disk.
    ///
    /// Inputs that look like prose descriptions -- strings containing a space, an
    /// unresolved `<…>` placeholder, or no `/` at all (e.g. bare config-key
    /// references like `"changelog_path_pattern"`) -- are silently skipped. Only
    /// entries that look like real file paths are checked. Returns `Err` only
    /// when a real-looking path fails to resolve (path traversal).
    ///
    /// The background agent reads the actual content via its own `file_read`
    /// tool calls. The orchestrator only verifies the files exist.
    pub(crate) fn verify_step_inputs_exist(
        &self,
        step: &WorkflowStep,
    ) -> Result<(), ArtifactStoreError> {
        for artifact in &step.execution.expected_inputs {
            if is_prose_path(artifact.path.as_str()) {
                continue;
            }
            let path = self.artifact_path(artifact)?;
            if !path.exists() {
                tracing::warn!(
                    step_id = %step.id,
                    artifact = %artifact.path.as_str(),
                    "declared input artifact does not exist on disk"
                );
            }
        }
        Ok(())
    }

    /// Existence check for the created-artifact set.
    ///
    /// Returns an `ArtifactExistence` marker for each declared artifact that
    /// exists on disk. The background agent writes artifacts via tool calls;
    /// capture only confirms the file was created -- it never reads file content
    /// into pipeline context. The reviewer/evaluator step reads content itself
    /// via `file_read`. Absent files produce `None` entries which are filtered
    /// out of the returned list.
    pub(crate) fn capture_artifact_existence(
        &self,
        created_artifacts: &[WorkflowArtifactRef],
    ) -> Vec<ArtifactExistence> {
        created_artifacts
            .iter()
            .filter_map(|artifact| match self.check_artifact_existence(artifact) {
                Ok(Some(exists)) => Some(exists),
                Ok(None) => None,
                Err(error) => {
                    tracing::warn!(error = %error, "failed to check artifact existence");
                    None
                }
            })
            .collect()
    }

    fn check_artifact_existence(
        &self,
        artifact: &WorkflowArtifactRef,
    ) -> Result<Option<ArtifactExistence>, ArtifactStoreError> {
        let artifact_path = self.artifact_path(artifact)?;

        match artifact_path.try_exists() {
            Ok(true) => Ok(Some(ArtifactExistence {
                artifact: artifact.clone(),
            })),
            Ok(false) => Ok(None),
            Err(error) => Err(ArtifactStoreError::Io(error)),
        }
    }

    fn normalized_relative_path(
        &self,
        artifact: &WorkflowArtifactRef,
    ) -> Result<PathBuf, ArtifactStoreError> {
        let artifact_path = Path::new(&*artifact.path);
        if artifact_path.is_absolute() {
            return Err(ArtifactStoreError::InvalidArtifactPath);
        }
        let normalized = artifact_path
            .components()
            .try_fold(PathBuf::new(), fold_normalized_component)?;
        if normalized.as_os_str().is_empty() {
            return Err(ArtifactStoreError::InvalidArtifactPath);
        }
        Ok(normalized)
    }

    fn ensure_path_is_contained(&self, candidate_path: &Path) -> Result<(), ArtifactStoreError> {
        let existing_ancestor = self
            .nearest_existing_ancestor(candidate_path)
            .ok_or(ArtifactStoreError::InvalidArtifactPath)?;
        let canonical_ancestor =
            fs::canonicalize(existing_ancestor).map_err(ArtifactStoreError::Io)?;

        if canonical_ancestor.starts_with(&self.repo_root) {
            Ok(())
        } else {
            Err(ArtifactStoreError::InvalidArtifactPath)
        }
    }

    fn nearest_existing_ancestor<'a>(&self, candidate_path: &'a Path) -> Option<&'a Path> {
        let mut current = Some(candidate_path);

        while let Some(path) = current {
            if path.exists() {
                return Some(path);
            }
            current = path.parent();
        }
        None
    }
}

fn fold_normalized_component(
    mut path: PathBuf,
    component: Component<'_>,
) -> Result<PathBuf, ArtifactStoreError> {
    match component {
        Component::CurDir => Ok(path),
        Component::Normal(segment) => {
            path.push(segment);
            Ok(path)
        }
        Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
            Err(ArtifactStoreError::InvalidArtifactPath)
        }
    }
}
