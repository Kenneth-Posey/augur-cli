//! Built-in scoped_shell_exec tool: runs a shell command in the repo root,
//! stripping secret environment variables before spawning the child process.

use crate::tools::builtin::child_process;
use crate::tools::handler::{ToolCallResult, ToolHandler};
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::string_newtypes::{OutputText, ShellCommand, StringNewtype, ToolName};
use augur_domain::domain::task_types::RepoRoot;
use augur_domain::tools::definition::ToolDefinition;
use std::time::Duration;
use tokio::time::timeout;

const TOOL_NAME: &str = "execute";
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Deny-listed environment variable names that are always stripped.
const DENY_LIST: &[&str] = &[
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "GITHUB_TOKEN",
    "COPILOT_AGENT_TOKEN",
];

/// Returns `true` when a key should be stripped from the child environment.
///
/// Matches the fixed deny list and any key ending with `_SECRET` or `_KEY`.
fn is_secret_key(key: &str) -> bool {
    if DENY_LIST.contains(&key) {
        return true;
    }
    key.ends_with("_SECRET") || key.ends_with("_KEY")
}

/// Collects environment variables, omitting any that match the secret key predicate.
///
/// Returns a `Vec<(String, String)>` ready for `Command::envs`.
fn filtered_env() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(k, _)| !is_secret_key(k))
        .collect()
}

fn parse_command(command: &str) -> Result<Vec<String>, String> {
    let parts = shell_words::split(command).map_err(|_| "invalid command syntax".to_string())?;
    if parts.is_empty() {
        return Err("missing or empty 'command' argument".to_string());
    }
    if matches!(
        parts.first().map(String::as_str),
        Some("sh" | "bash" | "zsh" | "dash")
    ) && matches!(parts.get(1).map(String::as_str), Some("-c"))
    {
        return Err("shell pass-through via '*sh -c' is not allowed".to_string());
    }
    Ok(parts)
}

fn result(output: OutputText, is_error: bool) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(output)
        .is_error(IsPredicate::from(is_error))
        .build()
}

fn parse_command_arg(args: &serde_json::Value) -> Result<ShellCommand, ToolCallResult> {
    match args["command"].as_str() {
        Some(s) if !s.is_empty() => Ok(ShellCommand::new(s.to_owned())),
        _ => {
            tracing::warn!(
                event = "tool_command_missing",
                tool_name = TOOL_NAME,
                args_kind = json_value_kind(args),
                has_command_key = args.get("command").is_some(),
            );
            Err(result(
                OutputText::new("missing or empty 'command' argument"),
                true,
            ))
        }
    }
}

fn json_value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn build_child_command(repo_root: &RepoRoot, argv: &[String]) -> tokio::process::Command {
    let mut child_cmd = child_process::piped_command(&argv[0]);
    child_cmd
        .args(&argv[1..])
        .current_dir(repo_root.as_ref())
        .env_clear()
        .envs(filtered_env());
    child_cmd
}

fn output_from_command_result(
    execution: Result<Result<std::process::Output, std::io::Error>, tokio::time::error::Elapsed>,
    timeout_secs: u64,
) -> ToolCallResult {
    match execution {
        Err(_elapsed) => result(
            OutputText::new(format!("command timed out after {timeout_secs}s")),
            true,
        ),
        Ok(Err(e)) => result(OutputText::new(e.to_string()), true),
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let combined = if stderr.is_empty() {
                stdout
            } else {
                format!("{stdout}\nstderr: {stderr}")
            };
            result(OutputText::new(combined), !out.status.success())
        }
    }
}

/// Executes a shell command in the repository root directory.
///
/// Strips all secret environment variables (API keys, tokens, and keys ending
/// in `_SECRET` or `_KEY`) from the child process environment before spawning.
/// The working directory is always set to the injected [`RepoRoot`].
pub struct ScopedShellExecTool {
    repo_root: RepoRoot,
}

impl ScopedShellExecTool {
    /// Create a new `ScopedShellExecTool` bound to the given repository root.
    ///
    /// All commands executed by this tool will run with `repo_root` as the
    /// current working directory.
    pub fn new(repo_root: RepoRoot) -> Self {
        Self { repo_root }
    }
}

#[async_trait::async_trait]
impl ToolHandler for ScopedShellExecTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Execute a single command directly in the repository root. This is NOT a shell - shell operators (&&, ||, |, ;, >, >>) and shell builtins (cd, export) are NOT supported. Run each command independently as a separate call. Working directory is always the repository root.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Single command to execute with its arguments. No shell operators or builtins."
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Optional timeout in seconds (default 30)"
                    }
                },
                "required": ["command"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args), fields(command))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let command = match parse_command_arg(&args) {
            Ok(command) => command,
            Err(result) => return result,
        };
        let timeout_secs = args["timeout_secs"]
            .as_u64()
            .unwrap_or(DEFAULT_TIMEOUT_SECS);
        tracing::Span::current().record("command", tracing::field::display(command.as_str()));
        let argv = match parse_command(command.as_str()) {
            Ok(argv) => argv,
            Err(message) => return result(OutputText::new(message), true),
        };

        let mut child_cmd = build_child_command(&self.repo_root, &argv);
        let execution = timeout(Duration::from_secs(timeout_secs), child_cmd.output()).await;
        output_from_command_result(execution, timeout_secs)
    }
}
