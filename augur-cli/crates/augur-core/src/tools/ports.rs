//! Lower-tier contracts shared between tool modules and their providers.

use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::string_newtypes::{FilePath, OutputText};
use std::path::{Path, PathBuf};

/// Specifies which lines of a file to read.
///
/// Line numbers are 1-indexed and inclusive at both ends. Out-of-bounds values
/// are clamped to the actual line count by the file-read provider.
#[derive(Clone, Debug)]
pub enum ReadRange {
    /// Read every line in the file.
    Full,
    /// Read from the given 1-indexed line to the end of the file.
    From(usize),
    /// Read from the first line to the given 1-indexed line (inclusive).
    To(usize),
    /// Read the inclusive slice from the first to the second 1-indexed line.
    Between(usize, usize),
}

/// Result returned by the file-read provider contract.
pub struct FileReadResult {
    /// Text output forwarded to the tool result message.
    pub output: OutputText,
    /// True when the operation failed (access denied, I/O error, etc.).
    pub is_error: IsPredicate,
}

/// Tool-facing contract for file line counting and range reads.
#[async_trait::async_trait]
pub trait FileReadPort: Send + Sync + 'static {
    /// Count the number of lines in `path`.
    async fn line_count(&self, path: FilePath) -> FileReadResult;

    /// Read `range` from `path`.
    async fn read_range(&self, path: FilePath, range: ReadRange) -> FileReadResult;
}

/// Tool-facing contract for cache refresh and working-file selection.
#[async_trait::async_trait]
pub trait CacheToolPort: Send + Sync + 'static {
    /// Tell the cache provider which file is currently being edited.
    async fn set_working_file(&self, path: PathBuf) -> anyhow::Result<()>;

    /// Force a refresh of cached content for `path`.
    async fn refresh_file(&self, path: PathBuf) -> anyhow::Result<()>;
}

/// Return `Some(&dir)` if `canonical_path` starts with any directory in
/// `canonical_allowed`, or `None` if access should be denied.
///
/// Both arguments must be canonical (absolute, resolved) paths. Returns `None`
/// when `canonical_allowed` is empty, denying all access.
///
/// Shared by the file-read actor and the file-write tool so that both enforce
/// the same sandbox rule without a cross-layer import.
pub fn is_within_allowed_dirs<'a>(
    canonical_path: &Path,
    canonical_allowed: &'a [PathBuf],
) -> Option<&'a PathBuf> {
    canonical_allowed
        .iter()
        .find(|d| canonical_path.starts_with(d))
}

#[cfg(test)]
mod tests {
    use super::{is_within_allowed_dirs, FileReadPort, FileReadResult, ReadRange};

    #[test]
    fn read_range_type_is_reachable_in_owning_module() {
        let type_name = core::any::type_name::<ReadRange>();
        assert!(type_name.contains("ReadRange"));
    }

    #[test]
    fn file_read_result_type_is_reachable_in_owning_module() {
        let type_name = core::any::type_name::<FileReadResult>();
        assert!(type_name.contains("FileReadResult"));
    }

    #[test]
    fn allowed_dirs_function_symbol_is_reachable_in_owning_module() {
        let function_name = core::any::type_name_of_val(&is_within_allowed_dirs);
        assert!(function_name.contains("is_within_allowed_dirs"));
    }

    #[test]
    fn file_read_port_trait_bound_is_usable_in_owning_module() {
        fn accepts_file_read_port<T: FileReadPort>() {}
        let _ = accepts_file_read_port::<FileReadPortTestDouble>;
        assert_eq!(stringify!(FileReadPort), "FileReadPort");
    }

    struct FileReadPortTestDouble;

    #[async_trait::async_trait]
    impl FileReadPort for FileReadPortTestDouble {
        async fn line_count(
            &self,
            _path: augur_domain::domain::string_newtypes::FilePath,
        ) -> FileReadResult {
            unreachable!("type-check-only test double")
        }

        async fn read_range(
            &self,
            _path: augur_domain::domain::string_newtypes::FilePath,
            _range: ReadRange,
        ) -> FileReadResult {
            unreachable!("type-check-only test double")
        }
    }
}
