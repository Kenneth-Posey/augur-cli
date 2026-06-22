//! Tests for `sdk_client` assistant module.
//!
//! Validates `build_client` error paths.
//! All functions are feature-gated; tests run only with `copilot-executor`.

#[cfg(test)]
mod suite {
    /// `build_client` returns an `Err` when no `cli_path` is configured and the
    /// Copilot CLI is not discoverable on PATH. Validates the CLI-not-found
    /// error message so callers can emit a useful `AgentOutput::Error`.
    #[test]
    fn build_client_no_cli_returns_error_when_cli_absent() {
        use augur_domain::config::types::{CopilotChatConfig, CopilotSdkSettings};
        use augur_provider_copilot_sdk::actors::copilot::assistant::sdk_client::build_client;

        if copilot_sdk::find_copilot_cli().is_some() {
            return; // CLI installed - skip; error path not reachable
        }

        let config = CopilotChatConfig {
            enabled: true.into(),
            sdk: CopilotSdkSettings::default(),
        };
        let result = build_client(&config);
        assert!(result.is_err(), "expected Err when Copilot CLI not on PATH");
        let msg = result.err().expect("already asserted is_err").to_string();
        assert!(
            msg.contains("not found") || msg.contains("gh extension"),
            "expected CLI-not-found message, got: {msg}"
        );
    }

    /// `build_client` succeeds (returns `Ok`) when an explicit `cli_path` is
    /// provided, even if that path does not exist yet. Client construction is
    /// lazy - it does not validate the binary until `client.start()` is called.
    #[test]
    fn build_client_with_explicit_cli_path_returns_ok() {
        use augur_domain::config::types::{CopilotChatConfig, CopilotSdkSettings};
        use augur_domain::string_newtypes::{FilePath, StringNewtype};
        use augur_provider_copilot_sdk::actors::copilot::assistant::sdk_client::build_client;

        let config = CopilotChatConfig {
            enabled: true.into(),
            sdk: CopilotSdkSettings {
                cli_path: Some(FilePath::new("/usr/bin/true")),
                ..CopilotSdkSettings::default()
            },
        };
        let result = build_client(&config);
        assert!(
            result.is_ok(),
            "expected Ok when cli_path is explicitly set: {:?}",
            result.err()
        );
    }
}
