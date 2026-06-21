//! Subprocess hook runner: executes a shell command and checks exit code.

use super::MAX_HOOK_OUTPUT_LINES;
use augur_domain::domain::guided_plan::HookOutcome;
use augur_domain::domain::string_newtypes::ShellCommand;
use augur_domain::domain::FailureReason;
use augur_domain::domain::StringNewtype;
use std::process::ExitStatus;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::process::Command;

/// Reads lines from an async buffered reader into `captured` up to `max_lines` total.
///
/// Reads line by line until EOF, an error, or the `captured` length reaches `max_lines`.
/// Lines are trimmed of trailing newlines before appending.
///
/// Parameters:
/// - `reader`: async line reader (stdout or stderr from the child process).
/// - `max_lines`: upper bound on total captured lines (shared across multiple calls).
/// - `captured`: mutable buffer that receives trimmed lines.
async fn read_stream_lines<R>(reader: &mut R, max_lines: usize, captured: &mut Vec<String>)
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    let mut line = String::new();
    while captured.len() < max_lines {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => captured.push(line.trim_end_matches('\n').to_string()),
            Err(_) => break,
        }
    }
}

/// Run a shell command as a subprocess hook and return the outcome.
///
/// Splits `command` on whitespace to extract the program name and arguments.
/// Spawns the process with piped stdout and stderr, reads both streams line by
/// line (interleaved), and captures up to `MAX_HOOK_OUTPUT_LINES` total lines.
/// Exit code `0` returns `HookOutcome::Passed`. Any non-zero exit or spawn
/// failure returns `HookOutcome::Failed` with the captured output or error message.
///
/// This function is async-safe: it spawns a child process, not an OS thread.
/// Output is capped at `MAX_HOOK_OUTPUT_LINES` to prevent unbounded memory use.
/// Consumers: `actors::guided_plan::actor::run_hooks`.
#[tracing::instrument(skip(command), level = "info")]
pub(crate) async fn run_subprocess_hook(command: &ShellCommand) -> HookOutcome {
    run_subprocess_hook_outcome(command)
        .await
        .unwrap_or_else(HookOutcome::Failed)
}

async fn run_subprocess_hook_outcome(command: &ShellCommand) -> Result<HookOutcome, FailureReason> {
    let (program, args) = parse_command_parts(command)?;
    let mut child = spawn_subprocess(program, args.as_slice())?;
    let captured = collect_subprocess_output(&mut child).await;
    let status = wait_for_subprocess(&mut child).await?;
    Ok(hook_outcome_from_status(status, captured))
}

fn hook_outcome_from_status(status: ExitStatus, captured: Vec<String>) -> HookOutcome {
    if status.success() {
        HookOutcome::Passed
    } else {
        HookOutcome::Failed(FailureReason::from(captured.join("\n")))
    }
}

fn parse_command_parts(command: &ShellCommand) -> Result<(&str, Vec<&str>), FailureReason> {
    let mut parts = command.as_str().split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| FailureReason::from("empty command string"))?;
    let args = parts.collect();
    Ok((program, args))
}

fn spawn_subprocess(program: &str, args: &[&str]) -> Result<Child, FailureReason> {
    Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| FailureReason::from(format!("failed to spawn process: {error}")))
}

async fn collect_subprocess_output(child: &mut Child) -> Vec<String> {
    let stdout = child.stdout.take().map(BufReader::new);
    let stderr = child.stderr.take().map(BufReader::new);
    let mut captured: Vec<String> = Vec::new();

    if let Some(mut reader) = stdout {
        read_stream_lines(&mut reader, MAX_HOOK_OUTPUT_LINES, &mut captured).await;
    }
    if let Some(mut reader) = stderr {
        read_stream_lines(&mut reader, MAX_HOOK_OUTPUT_LINES, &mut captured).await;
    }
    captured
}

async fn wait_for_subprocess(child: &mut Child) -> Result<ExitStatus, FailureReason> {
    child
        .wait()
        .await
        .map_err(|error| FailureReason::from(format!("failed to wait for process: {error}")))
}
