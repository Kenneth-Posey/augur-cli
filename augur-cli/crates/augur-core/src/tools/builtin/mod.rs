//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
pub mod approve_phase;
/// Approves the active review or execution phase.
/// Shared child-process setup with session isolation (TTY hang prevention).
pub mod child_process;
/// Appends text to the end of a target file.
pub mod file_append;
/// Writes text content to a file (create or overwrite).
pub mod file_create;
/// Inserts text before or after a unique text anchor.
pub mod file_insert;
/// Counts lines in a readable file.
pub mod file_line_count;
/// Reads the full contents of a file.
pub mod file_read;
/// Reads a file or a selected inclusive line range.
pub mod file_read_range;
/// Removes a file from the filesystem.
pub mod file_remove;
/// Replaces occurrences of old_text with new_text (with optional text anchors).
pub mod file_replace;
/// Removes content between two unique text anchors (inclusive).
pub mod file_slice;
/// Lists directory contents, optionally recursively.
pub mod list_directory;
/// Queries the rust-analyzer language server for code navigation operations.
pub mod lsp_query;
/// Asks the user a structured question and waits for a reply.
pub mod query_user;
/// Refreshes a cached file snapshot.
pub mod refresh_cache_file;
/// Requests rework with a human-readable reason.
pub mod request_rework;
/// Executes a shell command in the repo root with secret env vars stripped.
pub mod scoped_shell_exec;
/// Marks a file as the current working target.
pub mod set_working_file;
/// Executes a shell command and captures its output.
pub mod shell_exec;
/// Checks file and directory sizes with safety boundaries.
pub mod size_check;
/// Requests spawning of a named sub-agent via an mpsc channel.
pub mod spawn_agent;
/// Executes SQL against a per-session in-memory SQLite database.
pub mod sql_query;
/// Deterministically awaits background task terminal state by run_id.
pub mod task_await;
/// Lists queued, active, and terminal background task runs.
pub mod task_status;
