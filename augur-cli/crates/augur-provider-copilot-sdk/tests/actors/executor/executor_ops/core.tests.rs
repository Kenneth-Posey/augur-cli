//! Unit tests for `executor_ops::build_client_options` and
//! `executor_ops::build_session_config`.
//!
//! Tests are gated on `copilot-executor` because the SDK types are not present
//! without the feature. All tests are synchronous - no tokio runtime required.

use augur_domain::config::types::{CopilotSdkSettings, ExecutorConfig};
use augur_domain::string_newtypes::{BearerToken, FilePath, ModelName, StringNewtype};
use augur_provider_copilot_sdk::actors::executor::executor_ops::{
    build_client_options, build_session_config,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn minimal_config() -> ExecutorConfig {
    ExecutorConfig {
        sdk: CopilotSdkSettings::default(),
    }
}

// ── build_client_options ─────────────────────────────────────────────────────

/// `allow_all_tools` must always be `true` regardless of config contents.
///
/// This field guards against tool-permission regression: removing it would
/// cause the CLI to start with the default restricted toolset.
#[test]
fn build_client_options_allow_all_tools_is_true() {
    let config = minimal_config();
    let opts = build_client_options(&config);
    assert!(opts.allow_all_tools, "allow_all_tools must always be true");
}

/// `cli_args` must contain the `"--allow-all"` flag.
///
/// The flag is the CLI-level permission gate. Its absence would silently
/// restrict all tools even when `allow_all_tools` is set on the struct.
#[test]
fn build_client_options_cli_args_contains_allow_all() {
    let config = minimal_config();
    let opts = build_client_options(&config);
    let args = opts.cli_args.expect("cli_args must be Some");
    assert!(
        args.iter().any(|a| a == "--allow-all"),
        "cli_args must contain \"--allow-all\", got: {:?}",
        args
    );
}

/// `cwd` must be `Some(...)` so the CLI session starts in the correct directory.
///
/// An absent `cwd` causes the session to inherit an unpredictable working
/// directory from the spawned subprocess, breaking relative-path tool calls.
#[test]
fn build_client_options_cwd_is_some() {
    let config = minimal_config();
    let opts = build_client_options(&config);
    assert!(
        opts.cwd.is_some(),
        "cwd must be Some(current_dir), got None"
    );
}

/// When `cli_path` is `None` in config, the output `cli_path` is also `None`.
///
/// `None` signals the SDK to locate `gh` on `$PATH` rather than using a
/// hardcoded binary location.
#[test]
fn build_client_options_cli_path_none_maps_to_none() {
    let config = minimal_config();
    let opts = build_client_options(&config);
    assert!(
        opts.cli_path.is_none(),
        "cli_path should be None when config has None"
    );
}

/// When `cli_path` is `Some("path/to/gh")`, the output is `Some(PathBuf)` with
/// the same path components.
///
/// This allows operators to pin a specific `gh` binary for reproducible runs.
#[test]
fn build_client_options_cli_path_some_maps_to_pathbuf() {
    let config = ExecutorConfig {
        sdk: CopilotSdkSettings {
            cli_path: Some(FilePath::new("path/to/gh")),
            ..CopilotSdkSettings::default()
        },
    };
    let opts = build_client_options(&config);
    let path = opts.cli_path.expect("cli_path should be Some");
    assert_eq!(
        path,
        std::path::PathBuf::from("path/to/gh"),
        "cli_path PathBuf should mirror the config string"
    );
}

/// `github_token` is forwarded verbatim from `config.auth_token`.
///
/// The token must reach the SDK unchanged; any transformation would cause
/// authentication failures.
#[test]
fn build_client_options_github_token_forwarded_from_config() {
    let config = ExecutorConfig {
        sdk: CopilotSdkSettings {
            auth_token: Some(BearerToken::new("ghp_test_token")),
            ..CopilotSdkSettings::default()
        },
    };
    let opts = build_client_options(&config);
    assert_eq!(
        opts.github_token.as_deref(),
        Some("ghp_test_token"),
        "github_token must match config.auth_token"
    );
}

/// When `auth_token` is `None`, `github_token` is also `None` so the SDK
/// falls back to `$GITHUB_TOKEN` or the ambient `gh` session.
#[test]
fn build_client_options_github_token_none_when_auth_token_absent() {
    let config = minimal_config();
    let opts = build_client_options(&config);
    assert!(
        opts.github_token.is_none(),
        "github_token should be None when config.auth_token is None"
    );
}

// ── build_session_config ─────────────────────────────────────────────────────

/// `streaming` must always be `true`.
///
/// Disabling streaming causes the session to return a single blocking response
/// instead of incremental tokens, which would break the TUI's live update loop.
#[test]
fn build_session_config_streaming_is_true() {
    let config = minimal_config();
    let sc = build_session_config(&config);
    assert!(sc.streaming, "streaming must always be true");
}

/// `working_directory` must be `Some(...)` so the session resolves paths
/// relative to the current working directory.
///
/// An absent `working_directory` causes the session to inherit an
/// unpredictable directory from the spawned process, breaking tool calls
/// that rely on project-relative paths.
#[test]
fn build_session_config_working_directory_is_some() {
    let config = minimal_config();
    let sc = build_session_config(&config);
    assert!(
        sc.working_directory.is_some(),
        "working_directory must be Some(current_dir_as_string), got None"
    );
}

/// `model` is forwarded verbatim from `config.model`.
///
/// The model identifier must reach the session unchanged so the operator's
/// model selection is honoured.
#[test]
fn build_session_config_model_forwarded_from_config() {
    let config = ExecutorConfig {
        sdk: CopilotSdkSettings {
            model: Some(ModelName::new("gpt-4o")),
            ..CopilotSdkSettings::default()
        },
    };
    let sc = build_session_config(&config);
    assert_eq!(
        sc.model.as_deref(),
        Some("gpt-4o"),
        "model must match config.model"
    );
}

/// When `model` is `None`, the session `model` is also `None`, letting the
/// SDK use its default model.
#[test]
fn build_session_config_model_none_when_config_model_absent() {
    let config = minimal_config();
    let sc = build_session_config(&config);
    assert!(
        sc.model.is_none(),
        "model should be None when config.model is None"
    );
}

/// Session config must set a stable client name to separate augur-cli
/// SDK traffic from regular Copilot CLI sessions.
#[test]
fn build_session_config_sets_dcmk_client_name() {
    let config = minimal_config();
    let sc = build_session_config(&config);
    assert_eq!(
        sc.client_name.as_deref(),
        Some("augur-cli"),
        "client_name must identify this application for session isolation"
    );
}

/// Session config must provide an isolated config dir so SDK session state does
/// not mix with the default Copilot CLI session namespace.
#[test]
fn build_session_config_sets_isolated_config_dir() {
    let config = minimal_config();
    let sc = build_session_config(&config);
    assert!(
        sc.config_dir.is_some(),
        "config_dir must be set for isolated SDK session storage"
    );
}
