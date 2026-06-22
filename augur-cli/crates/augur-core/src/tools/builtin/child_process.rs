//! Shared child-process setup used by all subprocess-spawning tools.
//!
//! The primary protection is **session isolation**: every subprocess is detached
//! from the controlling terminal via `setsid()` in a `pre_exec` closure. This
//! prevents interactive TUI commands (like `gh copilot config`) from hanging
//! indefinitely - they fail fast with `ENXIO` when trying to open `/dev/tty`
//! because the child no longer has a controlling terminal.
//!
//! All subprocess spawn points in this crate route through this module so that
//! the protection applies uniformly to the entire class of problem.

use std::ffi::OsStr;
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use tokio::process::Command;

/// Apply session isolation to a [`tokio::process::Command`].
///
/// This attaches a `pre_exec` closure that calls `libc::setsid()` in the child
/// process immediately after `fork()`. The child is placed in a new session
/// with no controlling terminal.
///
/// The caller is still responsible for setting the program, arguments,
/// environment, working directory, and stdio handles.
pub fn isolate_session(cmd: &mut Command) -> &mut Command {
    // SAFETY: `pre_exec` closures run in the child after fork, before exec.
    // Only async-signal-safe functions are permitted. `setsid()` is safe here.
    // The `pre_exec` method on `tokio::process::Command` is unsafe because it
    // gives access to the raw child process; we only call `setsid()` which is
    // async-signal-safe.
    unsafe {
        cmd.pre_exec(|| {
            let ret = libc::setsid();
            let _ = ret; // discard; failure is non-fatal
            Ok(())
        })
    }
}

/// Apply session isolation to a [`std::process::Command`] (synchronous variant).
pub fn isolate_session_sync(cmd: &mut std::process::Command) -> &mut std::process::Command {
    // SAFETY: Same rationale as `isolate_session` - `setsid()` is async-signal-safe.
    unsafe {
        cmd.pre_exec(|| {
            let ret = libc::setsid();
            let _ = ret;
            Ok(())
        })
    }
}

/// Wraps a `tokio::process::Command` with piped stdout/stderr and session isolation.
///
/// This is the standard setup used by async shell-execution tools in this crate:
/// - stdout and stderr are piped for capture
/// - stdin is null (no interactive input)
/// - session isolation is applied via `setsid()` pre_exec
pub fn piped_command<S: AsRef<OsStr>>(program: S) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    isolate_session(&mut cmd);
    cmd
}

/// Wraps a synchronous `std::process::Command` with piped stdout/stderr and session isolation.
///
/// Like [`piped_command`] but returns `std::process::Command` for use with
/// synchronous `.output()` calls (e.g. `size_check`).
pub fn piped_command_sync<S: AsRef<OsStr>>(program: S) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    isolate_session_sync(&mut cmd);
    cmd
}