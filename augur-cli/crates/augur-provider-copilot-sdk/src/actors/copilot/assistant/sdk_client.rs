//! SDK client construction and authentication helpers for `CopilotChatActor`.
//!
//! Extracted from `actor.rs` to keep the actor event loop within the 200-line
//! logic threshold. Functions here are pure factory / I/O operations with no
//! state ownership.

use augur_domain::config::types::CopilotChatConfig;
use augur_domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::types::AgentOutput;

/// Build a `copilot_sdk::Client` from the actor configuration.
///
/// Resolves the CLI binary from `config.cli_path` or PATH via
/// `copilot_sdk::find_copilot_cli()`. Returns `CopilotError::InvalidConfig`
/// when the binary is absent so the actor can emit a helpful
/// `AgentOutput::Error` before exiting. `allow_all_tools: true` and
/// `cli_args: ["--allow-all"]` ensure all tool and path permissions are
/// granted without interactive prompts.
///
/// Parameters:
/// - `config`: actor runtime configuration including optional `cli_path`,
///   `auth_token`, and `use_logged_in_user`.
///
/// Returns the constructed client or a `CopilotError::InvalidConfig` when
/// the CLI binary cannot be found.
///
/// Consumers: `actor::run_with_sdk` startup sequence.
pub fn build_client(
    config: &CopilotChatConfig,
) -> Result<copilot_sdk::Client, copilot_sdk::CopilotError> {
    use copilot_sdk::ClientOptions;

    let cli_path = resolve_cli_path(config.sdk.cli_path.as_ref())?;
    let cwd = std::env::current_dir().ok();
    let options = ClientOptions {
        cli_path,
        github_token: config
            .sdk
            .auth_token
            .as_ref()
            .map(|token| token.as_str().to_owned()),
        use_logged_in_user: config.sdk.use_logged_in_user.map(|value| value.0),
        allow_all_tools: true,
        cli_args: Some(vec!["--allow-all".to_string()]),
        cwd,
        ..Default::default()
    };
    copilot_sdk::Client::new(options)
}

/// Resolve the Copilot CLI binary path for subprocess mode.
///
/// Uses `explicit_path` when provided; otherwise calls
/// `copilot_sdk::find_copilot_cli()` to search PATH. Returns
/// `CopilotError::InvalidConfig` when the binary cannot be located.
fn resolve_cli_path(
    explicit_path: Option<&augur_domain::string_newtypes::FilePath>,
) -> Result<Option<std::path::PathBuf>, copilot_sdk::CopilotError> {
    if let Some(p) = explicit_path {
        let path = std::path::PathBuf::from(p.as_str());
        tracing::warn!(cli_path = %path.display(), "CopilotChatActor: using configured CLI path");
        Ok(Some(path))
    } else {
        match copilot_sdk::find_copilot_cli() {
            Some(p) => {
                tracing::warn!(cli_path = %p.display(), "CopilotChatActor: resolved CLI path from PATH");
                Ok(Some(p))
            }
            None => Err(copilot_sdk::CopilotError::InvalidConfig(
                "GitHub Copilot CLI not found in PATH. \
                 Install it with `npm install -g @github/copilot` \
                 or set `cli_path` in config."
                    .to_owned(),
            )),
        }
    }
}

/// Check authentication status after `client.start()` has been called.
///
/// Returns `Some(AgentOutput::Error(...))` if the SDK reports the user is not
/// authenticated, or `None` if auth is confirmed. SDK errors checking status
/// are logged as warnings and treated as non-fatal (returns `None`) so the
/// actor can attempt to continue even when the auth check itself fails.
///
/// Parameters:
/// - `client`: the started SDK client.
///
/// Returns:
/// - `Some(AgentOutput::Error)` when `!status.is_authenticated`.
/// - `None` when authenticated or when the auth check returns an SDK error.
///
/// Consumers: `actor::run_with_sdk` startup sequence, `actor::attempt_session_restart`.
#[tracing::instrument(skip(client), level = "debug")]
pub async fn check_auth_status(client: &copilot_sdk::Client) -> Option<AgentOutput> {
    match client.get_auth_status().await {
        Ok(status) if !status.is_authenticated => {
            let msg = format!(
                "GitHub Copilot is not authenticated. Run `gh auth login` to authenticate. \
                 Login: {:?}",
                status.login
            );
            tracing::error!("{}", msg);
            Some(AgentOutput::Error(OutputText::new(msg)))
        }
        Ok(_) => None,
        Err(e) => {
            tracing::warn!(error = %e, "CopilotChatActor: could not verify auth status, continuing");
            None
        }
    }
}
