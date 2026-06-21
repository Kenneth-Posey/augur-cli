//! Private helper operations for the ask actor.

use crate::actors::file_read::FileReadHandle;
use crate::tools::builtin::{
    file_line_count::FileLineCountTool, file_read::FileReadTool,
    file_read_range::FileReadRangeTool, list_directory::ListDirectoryTool,
    size_check::SizeCheckTool,
};
use crate::tools::registry::ToolRegistry;
use augur_domain::config::types::AgentConfig;
use std::path::PathBuf;

/// Build a [`ToolRegistry`] restricted to read-only operations.
pub(super) fn build_ask_registry(
    file_read: FileReadHandle,
    allowed_dirs: Vec<PathBuf>,
    excluded_dirs: Vec<PathBuf>,
) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(FileReadTool::new(file_read.clone()));
    registry.register(FileReadRangeTool::new(file_read.clone()));
    registry.register(FileLineCountTool::new(file_read));
    registry.register(SizeCheckTool::new(
        allowed_dirs.clone(),
        excluded_dirs.clone(),
    ));
    registry.register(ListDirectoryTool::new(allowed_dirs, excluded_dirs));
    registry
}

/// Convert configured allowed directory newtypes into concrete `PathBuf` values.
pub(super) fn allowed_dirs_from_config(config: &AgentConfig) -> Vec<PathBuf> {
    config
        .allowed_dirs
        .iter()
        .map(|p| PathBuf::from(&**p))
        .collect()
}
