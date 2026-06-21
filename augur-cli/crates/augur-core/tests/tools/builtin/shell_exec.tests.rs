use augur_core::tools::builtin::shell_exec::ShellExecTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;

#[tokio::test]
async fn execute_echo_returns_stdout() {
    let tool = ShellExecTool;
    let result = tool
        .execute(serde_json::json!({"command": "echo hello"}))
        .await;
    assert!(!result.is_error);
    assert!(
        result.output.as_str().contains("hello"),
        "output: {}",
        result.output.as_str()
    );
}

#[tokio::test]
async fn execute_missing_command_arg_returns_error() {
    let tool = ShellExecTool;
    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_empty_command_arg_returns_error() {
    let tool = ShellExecTool;
    let result = tool.execute(serde_json::json!({"command": ""})).await;
    assert!(result.is_error);
    assert_eq!(
        result.output.as_str(),
        "missing or empty 'command' argument"
    );
}

#[tokio::test]
async fn execute_failing_command_is_error() {
    let tool = ShellExecTool;
    let result = tool.execute(serde_json::json!({"command": "false"})).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn execute_combines_stdout_and_stderr() {
    let tool = ShellExecTool;
    let result = tool
        .execute(serde_json::json!({"command": "python3 -c 'import sys; print(\"out\", end=\"\"); print(\"err\", end=\"\", file=sys.stderr); sys.exit(1)'"}))
        .await;
    assert!(result.is_error);
    assert_eq!(result.output.as_str(), "out\nstderr: err");
}

#[tokio::test]
async fn execute_rejects_shell_dash_c() {
    let tool = ShellExecTool;
    let result = tool
        .execute(serde_json::json!({"command": "sh -c 'echo hi'"}))
        .await;
    assert!(result.is_error);
    assert!(
        result.output.as_str().contains("not allowed"),
        "output: {}",
        result.output.as_str()
    );
}
