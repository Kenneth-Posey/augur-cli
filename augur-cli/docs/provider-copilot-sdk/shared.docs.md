# shared Module

The `shared` module contains two small utility sub-modules that provide consistent SDK configuration across all Copilot SDK sessions in the augur system: the chat actor, background agents, executor actor, and guided-plan hook runners.

## Copilot Permissions

`copilot_permissions` exports a single `allow_all_handler()` function that builds a `copilot_sdk::PermissionHandler` approving every permission request the SDK subprocess makes. This avoids interactive permission prompts during automated sessions---tool execution, file access, and command execution are implicitly trusted within the augur system's controlled environment. The handler is used by the chat actor, background agent dispatch, executor actor, and hook runners.

## Copilot Session Identity

`copilot_session_identity` defines a stable client name (`DCMK_COPILOT_CLIENT_NAME = "augur-cli"`) and an `isolated_config_dir()` function that ensures all Copilot SDK sessions spawned by augur-cli use a dedicated configuration directory rather than the user's default Copilot CLI config. The isolation strategy uses, in order of priority: the `DCMK_COPILOT_CONFIG_DIR` environment variable, `$HOME/.config/augur-cli/copilot-sdk`, or a fallback under `/tmp`. This prevents cross-session contamination between augur-cli and other Copilot CLI usage on the same machine.