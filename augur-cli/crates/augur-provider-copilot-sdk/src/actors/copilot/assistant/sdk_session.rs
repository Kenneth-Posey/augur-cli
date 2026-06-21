//! SDK session lifecycle helpers for `CopilotChatActor`.
//!
//! Extracted from `actor.rs` to keep the actor event loop within the 200-line
//! logic threshold. Covers session creation and session resumption logic.
//! Sessions are now established eagerly at startup; no pre-session waiting
//! phase is required.

use augur_domain::config::types::CopilotChatConfig;
use augur_domain::string_newtypes::{SdkSessionId, StringNewtype};

/// Sentinel value that effectively disables the CLI's background auto-compact.
///
/// The Copilot SDK treats `background_compaction_threshold` as a fraction of
/// the context window (0.0-1.0). A value of `1.0` is never reached in practice,
/// so it disables the CLI-side background compaction entirely. Our own
/// `check_auto_compact` at 85% is the sole compaction trigger.
/// Consumers: `create_session`, `resume_session`.
const DISABLE_AUTO_COMPACT_THRESHOLD: f64 = 1.0;

/// Arguments bundle for `create_or_resume_session`.
///
/// Groups the client reference, actor config, tool list, and optional prior
/// SDK session ID so the function signature stays within the 3-parameter limit.
/// When `sdk_session_id` is `Some`, `create_or_resume_session` attempts resume
/// before falling back to a fresh `create_session`.
/// Consumers: `actor::run_with_sdk` lazy-init phase.
#[derive(bon::Builder)]
pub struct CreateOrResumeSessionArgs<'a> {
    client: &'a copilot_sdk::Client,
    config: &'a CopilotChatConfig,
    tools: Vec<copilot_sdk::Tool>,
    sdk_session_id: Option<SdkSessionId>,
}

/// Create a new Copilot chat session with the given model config and tool list.
///
/// Sets `working_directory` to the current working directory so the Copilot
/// model has file-system context for the project. Falls back to omitting the
/// field if `current_dir` is unavailable.
///
/// `tools` is appended to `SessionConfig::tools` so the model knows which
/// external tools are available before the first message is sent.
///
/// Parameters:
/// - `client`: the connected SDK client.
/// - `config`: actor config (model name, etc.).
/// - `tools`: tool definitions to register on the new session.
///
/// Returns the session on success or a `CopilotError` on failure.
/// Consumers: `actor::run_with_sdk`, `actor::attempt_session_restart`.
#[tracing::instrument(skip_all, level = "debug")]
pub async fn create_session(
    client: &copilot_sdk::Client,
    config: &CopilotChatConfig,
    tools: Vec<copilot_sdk::Tool>,
) -> Result<std::sync::Arc<copilot_sdk::Session>, copilot_sdk::CopilotError> {
    use crate::shared::copilot_permissions::allow_all_handler;
    use copilot_sdk::SessionConfig;
    let working_directory = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(str::to_owned));
    tracing::debug!(cwd = ?working_directory, "CopilotChatActor: creating session");
    let session_config = SessionConfig {
        streaming: true,
        model: config
            .sdk
            .model
            .as_ref()
            .map(|model| model.as_str().to_owned()),
        config_dir: crate::shared::copilot_session_identity::isolated_config_dir(),
        tools,
        working_directory,
        client_name: Some(
            crate::shared::copilot_session_identity::DCMK_COPILOT_CLIENT_NAME.to_string(),
        ),
        // Enable infinite sessions for session-restore continuity, but disable
        // the CLI's background auto-compact (threshold = 1.0 is never reachable).
        // Our own check_auto_compact at 85% is the sole compaction trigger.
        infinite_sessions: Some(copilot_sdk::InfiniteSessionConfig {
            enabled: Some(true),
            background_compaction_threshold: Some(DISABLE_AUTO_COMPACT_THRESHOLD),
            buffer_exhaustion_threshold: None,
        }),
        request_permission: Some(true),
        // Register handler atomically before session is visible to dispatch loop
        // to eliminate the race window that causes PermissionRequested denial.
        permission_handler: copilot_sdk::PermissionHandlerField::some(allow_all_handler()),
        ..Default::default()
    };
    match client.create_session(session_config).await {
        Ok(s) => {
            tracing::warn!(
                session_id = %s.session_id(),
                infinite_sessions_workspace = ?s.workspace_path(),
                "CopilotChatActor: session created"
            );
            Ok(s)
        }
        Err(e) => {
            tracing::error!(error = %e, "CopilotChatActor: failed to create session");
            Err(e)
        }
    }
}

/// Build a [`copilot_sdk::ResumeSessionConfig`] for session resumption.
///
/// Mirrors the config constructed by `create_session`: streaming enabled,
/// infinite sessions on with auto-compact disabled, and `allow_all_handler`
/// registered atomically. `working_directory` is passed in so the caller
/// can resolve it once and log it before calling this helper.
/// Consumers: [`resume_session`].
fn build_resume_config(
    tools: Vec<copilot_sdk::Tool>,
    working_directory: Option<String>,
) -> copilot_sdk::ResumeSessionConfig {
    use crate::shared::copilot_permissions::allow_all_handler;
    copilot_sdk::ResumeSessionConfig {
        streaming: true,
        tools,
        working_directory,
        client_name: Some(
            crate::shared::copilot_session_identity::DCMK_COPILOT_CLIENT_NAME.to_string(),
        ),
        // Same as create_session: keep infinite sessions enabled for restore
        // continuity, but set background_compaction_threshold = 1.0 so the
        // CLI never auto-compacts independently. Our check_auto_compact is the
        // sole trigger.
        infinite_sessions: Some(copilot_sdk::InfiniteSessionConfig {
            enabled: Some(true),
            background_compaction_threshold: Some(DISABLE_AUTO_COMPACT_THRESHOLD),
            buffer_exhaustion_threshold: None,
        }),
        request_permission: Some(true),
        // Register handler atomically before session is visible to dispatch loop.
        permission_handler: copilot_sdk::PermissionHandlerField::some(allow_all_handler()),
        ..Default::default()
    }
}

/// Resume an existing SDK session by its stored session ID.
///
/// Mirrors `create_session` - sets `streaming: true`, registers the supplied
/// `tools`, and sets `working_directory` to the current process CWD.
///
/// Parameters:
/// - `client`: the connected SDK client.
/// - `sdk_session_id`: the SDK session ID to resume.
/// - `tools`: tool definitions to register on the resumed session.
///
/// Returns the resumed session or a `CopilotError` on failure.
/// Consumers: `create_or_resume_session`.
#[tracing::instrument(skip(client, tools), fields(sdk_session_id = %sdk_session_id), level = "debug")]
pub async fn resume_session(
    client: &copilot_sdk::Client,
    sdk_session_id: &SdkSessionId,
    tools: Vec<copilot_sdk::Tool>,
) -> Result<std::sync::Arc<copilot_sdk::Session>, copilot_sdk::CopilotError> {
    let working_directory = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(str::to_owned));
    tracing::debug!(
        sdk_session_id = %sdk_session_id,
        cwd = ?working_directory,
        "CopilotChatActor: resuming session"
    );
    let resume_config = build_resume_config(tools, working_directory);
    match client
        .resume_session(sdk_session_id.as_str(), resume_config)
        .await
    {
        Ok(s) => {
            tracing::warn!(
                session_id = %s.session_id(),
                infinite_sessions_workspace = ?s.workspace_path(),
                "CopilotChatActor: session resumed"
            );
            Ok(s)
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                sdk_session_id = %sdk_session_id,
                "CopilotChatActor: failed to resume session"
            );
            Err(e)
        }
    }
}

/// Create a new SDK session or resume an existing one.
///
/// When `sdk_session_id` is `Some`, calls `resume_session`. Falls back to
/// `create_session` if resume fails, emitting a WARN log before the fallback.
/// When `sdk_session_id` is `None`, calls `create_session` directly.
///
/// Parameters:
/// Returns the established session or an error if both resume and fallback fail.
/// Consumers: `actor::run_with_sdk` lazy-init phase.
#[tracing::instrument(skip(args), level = "debug")]
pub async fn create_or_resume_session(
    args: CreateOrResumeSessionArgs<'_>,
) -> Result<std::sync::Arc<copilot_sdk::Session>, copilot_sdk::CopilotError> {
    let CreateOrResumeSessionArgs {
        client,
        config,
        tools,
        sdk_session_id,
    } = args;
    let Some(id) = sdk_session_id else {
        return create_session(client, config, tools).await;
    };
    match resume_session(client, &id, tools.clone()).await {
        Ok(s) => {
            tracing::info!(sdk_session_id = %id, "CopilotChatActor: resumed prior SDK session");
            Ok(s)
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                sdk_session_id = %id,
                "CopilotChatActor: resume failed, creating new session"
            );
            create_session(client, config, tools).await
        }
    }
}
