//! Built-in shell_exec tool: runs a shell command and returns combined output.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use augur_domain::domain::string_newtypes::{OutputText, ShellCommand, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;

const TOOL_NAME: &str = "shell_exec";

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

/// Executes a command directly and returns stdout+stderr.
pub struct ShellExecTool;

fn shell_exec_result(output: impl Into<String>, is_error: bool) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(output.into()))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(is_error))
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
            Err(shell_exec_result(
                "missing or empty 'command' argument",
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

fn combine_process_output(out: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if stderr.is_empty() {
        stdout
    } else {
        format!("{stdout}\nstderr: {stderr}")
    }
}

async fn run_command(argv: &[String]) -> ToolCallResult {
    let mut child = tokio::process::Command::new(&argv[0]);
    child.args(&argv[1..]);
    match child.output().await {
        Err(error) => shell_exec_result(error.to_string(), true),
        Ok(out) => shell_exec_result(combine_process_output(&out), !out.status.success()),
    }
}

#[async_trait::async_trait]
impl ToolHandler for ShellExecTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Execute a shell command and return stdout and stderr.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to run"
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
        tracing::Span::current().record("command", tracing::field::display(command.as_str()));
        let argv = match parse_command(command.as_str()) {
            Ok(argv) => argv,
            Err(message) => return shell_exec_result(message, true),
        };
        run_command(&argv).await
    }
}
