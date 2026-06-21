//! Integration test: path/tool permissions end-to-end with a live Copilot CLI session.
//!
//! This is an **optional** live test that spins up a real GitHub Copilot CLI session in
//! headless mode and validates that path/tool permissions work end-to-end using the
//! executor's actual configuration functions.
//!
//! # Prerequisites
//!
//! - Active GitHub authentication (`gh auth status` must pass)
//! - Internet access for the Copilot API
//!
//! # How to run
//!
//! ```sh
//! cargo test --features copilot-executor -- --include-ignored executor_permissions
//! ```
//!
//! The test is `#[ignore]` by default so it is never executed during a normal
//! `cargo test` run.  It only runs when `--include-ignored` (or `--ignored`) is
//! supplied together with `--features copilot-executor`.

use std::time::Duration;

use augur_domain::config::types::CopilotSdkSettings;
use augur_domain::string_newtypes::{FilePath, StringNewtype};
use copilot_sdk::SessionEventData;

// ── constants ─────────────────────────────────────────────────────────────────

/// Path to the Copilot CLI binary. Override via COPILOT_CLI_PATH env var at build time.
const COPILOT_CLI_PATH: &str = match option_env!("COPILOT_CLI_PATH") {
    Some(p) => p,
    None => "copilot",
};

/// Upper-bound wall-clock timeout (seconds) for the entire live session probe.
const TEST_TIMEOUT_SECS: u64 = 30;

// ── tests ─────────────────────────────────────────────────────────────────────

/// Verifies that `executor_ops::build_client_options` and
/// `executor_ops::build_session_config` produce a configuration that the live
/// Copilot CLI accepts **and** that tool-execution permissions are threaded
/// through correctly via `--allow-all`.
///
/// Probe: sends a simple shell-exec request. Without `--allow-all`, the CLI
/// would emit a `SessionError` containing "permission" or "denied". With
/// `--allow-all` the model must respond with at least one
/// `AssistantMessageDelta` or `AssistantMessage` event before `SessionIdle`.
///
/// Expected outcome:
/// - Test completes within `TEST_TIMEOUT_SECS` seconds (no hard timeout).
/// - No `SessionError` whose lower-cased message contains "permission" or "denied".
/// - At least one `AssistantMessageDelta` or `AssistantMessage` event received.

#[tokio::test]
#[ignore]
async fn executor_path_permissions_allow_all_paths_end_to_end() {
    // ── Arrange ───────────────────────────────────────────────────────────────

    // Build a minimal ExecutorConfig that points at the known CLI binary and
    // uses the ambient `gh` CLI login - no hardcoded token.
    let config = augur_domain::config::types::ExecutorConfig {
        sdk: CopilotSdkSettings {
            cli_path: Some(FilePath::new(COPILOT_CLI_PATH)),
            model: None,
            auth_token: None,
            use_logged_in_user: Some(true.into()),
        },
    };

    // Use the real production configuration builders - this is the point of
    // the test: validate that these functions produce options the CLI accepts.
    let client_options =
        augur_provider_copilot_sdk::actors::executor::executor_ops::build_client_options(&config);
    let session_config =
        augur_provider_copilot_sdk::actors::executor::executor_ops::build_session_config(&config);

    let client = copilot_sdk::Client::new(client_options)
        .expect("Client::new must succeed with valid options");

    client
        .start()
        .await
        .expect("client.start() must connect to the live Copilot CLI process");

    let session = client
        .create_session(session_config)
        .await
        .expect("create_session must succeed after client is connected");

    session
        .register_permission_handler(|_req| copilot_sdk::PermissionRequestResult::approved())
        .await;

    let mut events = session.subscribe();

    // ── Act ───────────────────────────────────────────────────────────────────

    session
        .send(
            "Run the following shell command exactly as written and show me only its raw output, \
             nothing else: echo SHELL_EXEC_CONFIRMED_$(date +%s)",
        )
        .await
        .expect("session.send must enqueue the message without error");

    let mut assistant_text = String::new();
    let mut permission_error_detected = false;
    let mut permission_error_text = String::new();

    let outcome = tokio::time::timeout(Duration::from_secs(TEST_TIMEOUT_SECS), async {
        while let Ok(event) = events.recv().await {
            match &event.data {
                // Primary signal: accumulate response text from delta events.
                SessionEventData::AssistantMessageDelta(d) => {
                    assistant_text.push_str(&d.delta_content);
                }
                SessionEventData::AssistantMessage(m) => {
                    assistant_text.push_str(&m.content);
                }
                // Terminal signal: session finished normally.
                SessionEventData::SessionIdle(_) => {
                    break;
                }
                // Error signal: inspect for permission denial keywords.
                SessionEventData::SessionError(err) => {
                    let lowered = err.message.to_lowercase();
                    if lowered.contains("permission") || lowered.contains("denied") {
                        permission_error_detected = true;
                        permission_error_text = err.message.clone();
                    }
                    // A session error is still a terminal event.
                    break;
                }
                // All other event kinds are ignored for this probe.
                _ => {}
            }
        }
    })
    .await;

    // ── Cleanup (defer-style) ─────────────────────────────────────────────────

    // Best-effort: destroy the session and stop the client regardless of the
    // assertion results.  Errors here are intentionally swallowed so that the
    // real assertion failure surfaces cleanly.
    let _ = session.destroy().await;
    client.stop().await;

    // ── Assert ────────────────────────────────────────────────────────────────

    assert!(
        outcome.is_ok(),
        "Test timed out after {TEST_TIMEOUT_SECS}s - \
         no SessionIdle or SessionError received; \
         the CLI may not have started or the session stalled"
    );

    assert!(
        !permission_error_detected,
        "Received a permission/denied SessionError - \
         `--allow-all` may not have been applied correctly by \
         `build_client_options`.  Error text: {permission_error_text:?}"
    );

    assert!(
        assistant_text.contains("SHELL_EXEC_CONFIRMED_"),
        "Response does not contain SHELL_EXEC_CONFIRMED_ - bash tool may be blocked. \
         Full response: {assistant_text:?}"
    );
}
