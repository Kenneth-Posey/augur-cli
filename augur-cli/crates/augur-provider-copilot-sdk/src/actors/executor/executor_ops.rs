//! Pure constructor functions for SDK configuration types.
//!
//! These functions extract the `ClientOptions` and `SessionConfig` construction
//! logic from `run_with_sdk` so it can be tested independently of the actor
//! runtime. Both functions are pure: no async, no channels, no SDK I/O.
//! The only side-effect is reading the process working directory via
//! `std::env::current_dir()`.

use augur_domain::StringNewtype;
use augur_domain::config::types::ExecutorConfig;

/// Build `ClientOptions` from the executor configuration.
///
/// Sets the permission-critical fields unconditionally:
/// - `allow_all_tools` is always `true`
/// - `cli_args` always includes `"--allow-all"`
/// - `cwd` is populated from the current process working directory
///
/// The caller is responsible for forwarding the returned value directly to
/// `Client::new` without stripping or overriding these fields.
pub fn build_client_options(config: &ExecutorConfig) -> copilot_sdk::ClientOptions {
    let cwd = std::env::current_dir().ok();
    copilot_sdk::ClientOptions {
        cli_path: config
            .sdk
            .cli_path
            .as_ref()
            .map(|path| std::path::PathBuf::from(path.as_str())),
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
    }
}

/// Build `SessionConfig` from the executor configuration.
///
/// Sets the permission-critical fields unconditionally:
/// - `streaming` is always `true`
/// - `working_directory` is populated from the current process working directory
/// - `permission_handler` is pre-set to an allow-all handler to eliminate the
///   race window between session creation and handler registration
///
/// The caller is responsible for forwarding the returned value directly to
/// `Client::create_session` without stripping or overriding these fields.
pub fn build_session_config(config: &ExecutorConfig) -> copilot_sdk::SessionConfig {
    use crate::shared::copilot_permissions::allow_all_handler;
    let working_directory = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    copilot_sdk::SessionConfig {
        streaming: true,
        model: config
            .sdk
            .model
            .as_ref()
            .map(|model| model.as_str().to_owned()),
        config_dir: crate::shared::copilot_session_identity::isolated_config_dir(),
        working_directory,
        client_name: Some(
            crate::shared::copilot_session_identity::DCMK_COPILOT_CLIENT_NAME.to_string(),
        ),
        request_permission: Some(true),
        permission_handler: copilot_sdk::PermissionHandlerField::some(allow_all_handler()),
        ..Default::default()
    }
}
