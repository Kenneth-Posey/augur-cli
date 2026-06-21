//! Integration tests verifying every SDK JSON-RPC method path string.
//!
//! Each test creates a mock `Session` that captures the method string passed
//! to the invoke function, calls the corresponding SDK method, and asserts
//! that the captured string matches the expected camelCase RPC path.
//!
//! A test failure here means the SDK is calling a wrong method name and the
//! server will return a -32601 (Method not found) error. All tests are gated
//! by the `copilot-executor` feature.

#[cfg(test)]
mod tests {
    use copilot_sdk::{InvokeFuture, Session};
    use std::sync::{Arc, Mutex};

    /// Creates a mock `Session` that captures the last invoked method name and
    /// returns canned responses appropriate for each known SDK method.
    ///
    /// The captured `Arc<Mutex<Option<String>>>` is returned alongside the session
    /// so individual tests can assert on the recorded method string.
    fn make_mock_session() -> (Session, Arc<Mutex<Option<String>>>) {
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let cap = captured.clone();
        let session = Session::new(
            "test-session".to_owned(),
            None::<String>,
            move |method, _params| {
                *cap.lock().unwrap() = Some(method.to_owned());
                let resp = match method {
                    "session.send" => serde_json::json!({"messageId": "mock-id"}),
                    "session.getMessages" => serde_json::json!({"events": []}),
                    "session.model.getCurrent" => serde_json::json!({"modelId": "mock-model"}),
                    "session.mode.get" => serde_json::json!({"mode": "interactive"}),
                    "session.log" => serde_json::json!({"eventId": "mock-event"}),
                    "session.plan.read" => serde_json::Value::Null,
                    "session.agent.list" => serde_json::json!({"agents": []}),
                    "session.agent.getCurrent" => serde_json::Value::Null,
                    "session.workspaces.listFiles" => serde_json::json!({"files": []}),
                    "session.workspaces.readFile" => serde_json::json!({"content": "mock-content"}),
                    "session.shell.exec" => serde_json::json!({"processId": "mock-pid"}),
                    _ => serde_json::json!({}),
                };
                Box::pin(async move { Ok(resp) }) as InvokeFuture
            },
        );
        (session, captured)
    }

    /// Returns the last method name captured by the mock session.
    fn captured_method(cap: &Arc<Mutex<Option<String>>>) -> String {
        cap.lock()
            .unwrap()
            .clone()
            .expect("no RPC method was captured by the mock session")
    }

    // =========================================================================
    // Core session lifecycle
    // =========================================================================

    /// `session.send` uses the correct camelCase RPC method string.
    ///
    /// A wrong path here would cause a -32601 on every user message sent.
    #[tokio::test]
    async fn send_uses_session_send() {
        let (session, cap) = make_mock_session();
        let _ = session.send("test message").await;
        assert_eq!(captured_method(&cap), "session.send");
    }

    /// `session.abort` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn abort_uses_session_abort() {
        let (session, cap) = make_mock_session();
        let _ = session.abort().await;
        assert_eq!(captured_method(&cap), "session.abort");
    }

    /// `session.getMessages` uses the correct camelCase RPC method string.
    ///
    /// This is also the method used by `keepalive_session`; a wrong path here
    /// would cause keepalive to fail on every tick.
    #[tokio::test]
    async fn get_messages_uses_session_get_messages() {
        let (session, cap) = make_mock_session();
        let _ = session.get_messages().await;
        assert_eq!(captured_method(&cap), "session.getMessages");
    }

    /// `session.destroy` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn destroy_uses_session_destroy() {
        let (session, cap) = make_mock_session();
        let _ = session.destroy().await;
        assert_eq!(captured_method(&cap), "session.destroy");
    }

    // =========================================================================
    // Model management
    // =========================================================================

    /// `session.model.getCurrent` uses the correct camelCase RPC method string.
    ///
    /// Previously was `session.model.get_current` (snake_case - wrong).
    #[tokio::test]
    async fn get_model_uses_session_model_get_current() {
        let (session, cap) = make_mock_session();
        let _ = session.get_model().await;
        assert_eq!(captured_method(&cap), "session.model.getCurrent");
    }

    /// `session.model.switchTo` uses the correct camelCase RPC method string.
    ///
    /// Previously was `session.model.switch_to` (snake_case - wrong).
    #[tokio::test]
    async fn set_model_uses_session_model_switch_to() {
        let (session, cap) = make_mock_session();
        let _ = session.set_model("mock-model", None).await;
        assert_eq!(captured_method(&cap), "session.model.switchTo");
    }

    // =========================================================================
    // Mode management
    // =========================================================================

    /// `session.mode.get` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn get_mode_uses_session_mode_get() {
        let (session, cap) = make_mock_session();
        let _ = session.get_mode().await;
        assert_eq!(captured_method(&cap), "session.mode.get");
    }

    /// `session.mode.set` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn set_mode_uses_session_mode_set() {
        use copilot_sdk::SessionMode;
        let (session, cap) = make_mock_session();
        let _ = session.set_mode(SessionMode::Interactive).await;
        assert_eq!(captured_method(&cap), "session.mode.set");
    }

    // =========================================================================
    // Logging
    // =========================================================================

    /// `session.log` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn log_uses_session_log() {
        let (session, cap) = make_mock_session();
        let _ = session.log("test message", None).await;
        assert_eq!(captured_method(&cap), "session.log");
    }

    // =========================================================================
    // Plan management
    // =========================================================================

    /// `session.plan.read` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn read_plan_uses_session_plan_read() {
        let (session, cap) = make_mock_session();
        let _ = session.read_plan().await;
        assert_eq!(captured_method(&cap), "session.plan.read");
    }

    /// `session.plan.update` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn update_plan_uses_session_plan_update() {
        use copilot_sdk::PlanData;
        let (session, cap) = make_mock_session();
        let plan = PlanData {
            content: Some("test plan".to_owned()),
            title: None,
        };
        let _ = session.update_plan(&plan).await;
        assert_eq!(captured_method(&cap), "session.plan.update");
    }

    /// `session.plan.delete` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn delete_plan_uses_session_plan_delete() {
        let (session, cap) = make_mock_session();
        let _ = session.delete_plan().await;
        assert_eq!(captured_method(&cap), "session.plan.delete");
    }

    // =========================================================================
    // Agent management
    // =========================================================================

    /// `session.agent.list` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn list_agents_uses_session_agent_list() {
        let (session, cap) = make_mock_session();
        let _ = session.list_agents().await;
        assert_eq!(captured_method(&cap), "session.agent.list");
    }

    /// `session.agent.getCurrent` uses the correct camelCase RPC method string.
    ///
    /// Previously was `session.agent.get_current` (snake_case - wrong).
    #[tokio::test]
    async fn get_current_agent_uses_session_agent_get_current() {
        let (session, cap) = make_mock_session();
        let _ = session.get_current_agent().await;
        assert_eq!(captured_method(&cap), "session.agent.getCurrent");
    }

    /// `session.agent.select` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn select_agent_uses_session_agent_select() {
        let (session, cap) = make_mock_session();
        let _ = session.select_agent("mock-agent").await;
        assert_eq!(captured_method(&cap), "session.agent.select");
    }

    /// `session.agent.deselect` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn deselect_agent_uses_session_agent_deselect() {
        let (session, cap) = make_mock_session();
        let _ = session.deselect_agent().await;
        assert_eq!(captured_method(&cap), "session.agent.deselect");
    }

    // =========================================================================
    // Compaction
    // =========================================================================

    /// `session.history.compact` uses the correct camelCase RPC method string.
    ///
    /// This is the `/compact` slash-command path; a wrong string here would
    /// cause compaction to silently fail with -32601.
    #[tokio::test]
    async fn compact_uses_session_history_compact() {
        let (session, cap) = make_mock_session();
        let _ = session.compact().await;
        assert_eq!(captured_method(&cap), "session.history.compact");
    }

    // =========================================================================
    // Workspace operations
    // =========================================================================

    /// `session.workspaces.listFiles` uses the correct camelCase RPC method string.
    ///
    /// Previously was `session.workspace.list_files` (singular + snake_case - wrong).
    #[tokio::test]
    async fn workspace_list_files_uses_session_workspaces_list_files() {
        let (session, cap) = make_mock_session();
        let _ = session.workspace_list_files().await;
        assert_eq!(captured_method(&cap), "session.workspaces.listFiles");
    }

    /// `session.workspaces.readFile` uses the correct camelCase RPC method string.
    ///
    /// Previously was `session.workspace.read_file` (singular + snake_case - wrong).
    #[tokio::test]
    async fn workspace_read_file_uses_session_workspaces_read_file() {
        let (session, cap) = make_mock_session();
        let _ = session.workspace_read_file("src/main.rs").await;
        assert_eq!(captured_method(&cap), "session.workspaces.readFile");
    }

    /// `session.workspaces.createFile` uses the correct camelCase RPC method string.
    ///
    /// Previously was `session.workspace.create_file` (singular + snake_case - wrong).
    #[tokio::test]
    async fn workspace_create_file_uses_session_workspaces_create_file() {
        let (session, cap) = make_mock_session();
        let _ = session
            .workspace_create_file("src/new.rs", "fn main() {}")
            .await;
        assert_eq!(captured_method(&cap), "session.workspaces.createFile");
    }

    // =========================================================================
    // Shell operations
    // =========================================================================

    /// `session.shell.exec` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn shell_exec_uses_session_shell_exec() {
        use copilot_sdk::ShellExecOptions;
        let (session, cap) = make_mock_session();
        let opts = ShellExecOptions {
            command: "echo test".to_owned(),
            cwd: None,
            env: None,
        };
        let _ = session.shell_exec(opts).await;
        assert_eq!(captured_method(&cap), "session.shell.exec");
    }

    /// `session.shell.kill` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn shell_kill_uses_session_shell_kill() {
        use copilot_sdk::ShellSignal;
        let (session, cap) = make_mock_session();
        let _ = session.shell_kill("mock-pid", ShellSignal::SIGTERM).await;
        assert_eq!(captured_method(&cap), "session.shell.kill");
    }

    // =========================================================================
    // Fleet management
    // =========================================================================

    /// `session.fleet.start` uses the correct camelCase RPC method string.
    #[tokio::test]
    async fn start_fleet_uses_session_fleet_start() {
        let (session, cap) = make_mock_session();
        let _ = session.start_fleet(None).await;
        assert_eq!(captured_method(&cap), "session.fleet.start");
    }
}
