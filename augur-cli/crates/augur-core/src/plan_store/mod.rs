//! Plan tree disk store.
//!
//! Provides async read/write access to plan trees persisted under a configurable
//! base directory. Each plan lives at `{base}/{plan_id}/tree.json`; step files
//! live at `{base}/{plan_id}/steps/{filename}`.
//!
//! This module is analogous to `src/persistence/` - it performs async I/O and
//! therefore does not belong in `src/domain/`.

use std::path::PathBuf;

use tokio::fs;

use augur_domain::domain::plan_tree::{PlanTree, PlanTreeId};
use augur_domain::domain::string_newtypes::{StepContent, StepFileName, StringNewtype};

/// Errors produced by `PlanTreeStore` operations.
#[derive(Debug)]
pub enum PlanStoreError {
    /// An underlying I/O error occurred.
    Io(std::io::Error),
    /// The plan tree could not be serialized to JSON.
    Serialize(serde_json::Error),
    /// The plan tree JSON could not be deserialized.
    Deserialize(serde_json::Error),
    /// The requested plan or step file does not exist on disk.
    NotFound(String),
}

impl std::fmt::Display for PlanStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "plan store I/O error: {e}"),
            Self::Serialize(e) => write!(f, "plan serialize error: {e}"),
            Self::Deserialize(e) => write!(f, "plan deserialize error: {e}"),
            Self::NotFound(msg) => write!(f, "plan not found: {msg}"),
        }
    }
}

impl std::error::Error for PlanStoreError {}

/// Async disk store for plan trees.
///
/// Each plan occupies a directory `{base_dir}/{plan_id}/` containing:
/// - `tree.json` - the serialized `PlanTree`.
/// - `steps/` - one `.md` file per executable step.
///
/// Consumers: `SupervisorActor` (Phase 4), `PlanTreeStore` integration tests.
pub struct PlanTreeStore {
    /// Root directory under which all plan subdirectories are created.
    base_dir: PathBuf,
}

impl PlanTreeStore {
    /// Creates a new store rooted at `base_dir`.
    ///
    /// The directory is created lazily on the first `save` or `write_step` call.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }
}

impl Default for PlanTreeStore {
    /// Returns a store rooted at the project-conventional `"plans"` directory.
    ///
    /// Used by `wiring.rs` when no explicit base directory is configured.
    /// The directory is created lazily - it does not need to exist at construction.
    fn default() -> Self {
        Self::new("plans")
    }
}

impl PlanTreeStore {
    fn plan_dir(&self, id: &PlanTreeId) -> PathBuf {
        self.base_dir.join(id.as_str())
    }

    fn tree_json_path(&self, id: &PlanTreeId) -> PathBuf {
        self.plan_dir(id).join("tree.json")
    }

    fn step_path(&self, plan_id: &PlanTreeId, step_file: &StepFileName) -> PathBuf {
        self.plan_dir(plan_id)
            .join("steps")
            .join(step_file.as_str())
    }

    /// Serializes `tree` to `{base_dir}/{tree.id}/tree.json`.
    ///
    /// Creates the plan directory if it does not exist. Overwrites any
    /// existing `tree.json` for the same plan id.
    ///
    /// Consumers: `SupervisorActor::StartPlan` handler.
    pub async fn save(&self, tree: &PlanTree) -> Result<(), PlanStoreError> {
        let plan_dir = self.plan_dir(&tree.id);
        fs::create_dir_all(&plan_dir)
            .await
            .map_err(PlanStoreError::Io)?;
        let json = serde_json::to_string_pretty(tree).map_err(PlanStoreError::Serialize)?;
        let path = self.tree_json_path(&tree.id);
        fs::write(&path, json).await.map_err(PlanStoreError::Io)
    }

    /// Loads and deserializes `{base_dir}/{id}/tree.json`.
    ///
    /// Returns `PlanStoreError::NotFound` if the file does not exist.
    ///
    /// Consumers: `SupervisorActor` resume-from-disk path (Phase 5).
    pub async fn load(&self, id: &PlanTreeId) -> Result<PlanTree, PlanStoreError> {
        let path = self.tree_json_path(id);
        if !path.exists() {
            return Err(PlanStoreError::NotFound(format!(
                "tree.json not found for plan '{id}'"
            )));
        }
        let bytes = fs::read(&path).await.map_err(PlanStoreError::Io)?;
        serde_json::from_slice(&bytes).map_err(PlanStoreError::Deserialize)
    }

    /// Writes `content` to `{base_dir}/{plan_id}/steps/{step_file}`.
    ///
    /// Creates the `steps/` subdirectory if it does not exist.
    ///
    /// Consumers: `run_meta_plan` in `meta_planner.rs` (Phase 4).
    pub async fn write_step(
        &self,
        plan_id: &PlanTreeId,
        step_file: &StepFileName,
        content: &StepContent,
    ) -> Result<(), PlanStoreError> {
        let path = self.step_path(plan_id, step_file);
        let parent = path.parent().ok_or_else(|| {
            PlanStoreError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "step path has no parent",
            ))
        })?;
        fs::create_dir_all(parent)
            .await
            .map_err(PlanStoreError::Io)?;
        fs::write(&path, content.as_str())
            .await
            .map_err(PlanStoreError::Io)
    }

    /// Reads and returns the content of `{base_dir}/{plan_id}/steps/{step_file}`.
    ///
    /// Returns `PlanStoreError::NotFound` if the file does not exist.
    ///
    /// Consumers: `SupervisorActor::begin_execution` (Phase 4).
    pub async fn read_step(
        &self,
        plan_id: &PlanTreeId,
        step_file: &StepFileName,
    ) -> Result<StepContent, PlanStoreError> {
        let path = self.step_path(plan_id, step_file);
        if !path.exists() {
            return Err(PlanStoreError::NotFound(format!(
                "step file '{step_file}' not found for plan '{plan_id}'"
            )));
        }
        fs::read_to_string(&path)
            .await
            .map(StepContent::new)
            .map_err(PlanStoreError::Io)
    }
}
