use augur_core::tools::builtin::scoped_shell_exec::ScopedShellExecTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;
use augur_domain::domain::task_types::RepoRoot;

#[tokio::test]
async fn echo_command_returns_output() {
    let root = RepoRoot::new("/tmp");
    let tool = ScopedShellExecTool::new(root);
    let result = tool
        .execute(serde_json::json!({"command": "echo hello"}))
        .await;
    assert!(
        !result.is_error,
        "unexpected error: {}",
        result.output.as_str()
    );
    assert!(
        result.output.as_str().contains("hello"),
        "output: {}",
        result.output.as_str()
    );
}

#[tokio::test]
async fn working_directory_is_repo_root() {
    let root = RepoRoot::new("/tmp");
    let tool = ScopedShellExecTool::new(root);
    let result = tool.execute(serde_json::json!({"command": "pwd"})).await;
    assert!(
        !result.is_error,
        "unexpected error: {}",
        result.output.as_str()
    );
    assert!(
        result.output.as_str().contains("/tmp"),
        "output: {}",
        result.output.as_str()
    );
}

#[tokio::test]
async fn secret_env_vars_stripped() {
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("OPENAI_API_KEY", "should-not-appear") };
    let root = RepoRoot::new("/tmp");
    let tool = ScopedShellExecTool::new(root);
    let result = tool.execute(serde_json::json!({"command": "env"})).await;
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("OPENAI_API_KEY") };
    assert!(
        !result.is_error,
        "unexpected error: {}",
        result.output.as_str()
    );
    assert!(
        !result.output.as_str().contains("should-not-appear"),
        "secret leaked into output: {}",
        result.output.as_str()
    );
}

#[tokio::test]
async fn command_timeout_returns_error() {
    let root = RepoRoot::new("/tmp");
    let tool = ScopedShellExecTool::new(root);
    let result = tool
        .execute(serde_json::json!({"command": "sleep 100", "timeout_secs": 1}))
        .await;
    assert!(result.is_error, "expected timeout error");
    assert!(
        result.output.as_str().contains("timed out")
            || result.output.as_str().contains("timeout")
            || result.output.as_str().contains("Elapsed"),
        "unexpected error message: {}",
        result.output.as_str()
    );
}

#[tokio::test]
async fn rejects_shell_dash_c_passthrough() {
    let root = RepoRoot::new("/tmp");
    let tool = ScopedShellExecTool::new(root);
    let result = tool
        .execute(serde_json::json!({"command": "sh -c 'echo hi'"}))
        .await;
    assert!(result.is_error, "expected rejection");
    assert!(
        result.output.as_str().contains("not allowed"),
        "unexpected message: {}",
        result.output.as_str()
    );
}
