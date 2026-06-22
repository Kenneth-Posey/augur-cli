use std::sync::OnceLock;

/// Shared env var used by OpenRouter compaction tests.
pub const OPENROUTER_CONTEXT_BUDGET_ENV: &str =
    "AUGUR_CLI_OPENROUTER_CONTEXT_BUDGET_TOKENS";

/// Global async lock for tests that mutate process-wide environment variables.
pub fn openrouter_env_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}
