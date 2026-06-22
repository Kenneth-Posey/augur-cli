use crate::config::types::{
    AgentConfig, AppConfig, CopilotConfig, CopilotSdkSettings, EndpointConfig, EndpointCredentials,
    PersistenceConfig, Provider,
};
use crate::domain::newtypes::{NumericNewtype, Temperature, TokenCount};
use crate::domain::string_newtypes::{
    EndpointName, EndpointUrl, FilePath, ModelName, OutputText, StringNewtype,
};
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;

fn empty_config() -> AppConfig {
    AppConfig {
        endpoints: vec![],
        default_endpoint: EndpointName::new("none"),
        agent: AgentConfig {
            system_prompt: OutputText::new(""),
            max_tokens: TokenCount::new(4096),
            temperature: Temperature::new(0.7),
            allowed_dirs: vec![],
        },
        copilot: CopilotConfig::default(),
        persistence: PersistenceConfig {
            log_dir: FilePath::new("./logs"),
            sessions_dir: None,
        },
            program_settings: Default::default(),
            user_settings: Default::default(),
    }
}

/// Helper to run git commands with error checking.
fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {:?} failed: stdout={:?} stderr={:?}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Helper to initialize a temporary git repository with a given branch name.
fn init_git_repo(branch: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    git(dir.path(), &["init", "-b", branch]);
    git(dir.path(), &["config", "user.name", "Test User"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    std::fs::write(dir.path().join("tracked.txt"), "tracked\n").expect("seed tracked file");
    git(dir.path(), &["add", "tracked.txt"]);
    git(dir.path(), &["commit", "-m", "initial"]);
    dir
}

/// Helper to acquire a global lock for changing the current working directory.
/// Ensures tests don't interfere with each other when modifying process-global cwd.
fn cwd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Guard struct that changes the current working directory on creation and
/// restores it on drop. Holds a lock to prevent concurrent cwd changes.
struct CurrentDirGuard {
    _lock: MutexGuard<'static, ()>,
    previous: std::path::PathBuf,
}

impl CurrentDirGuard {
    /// Change the current working directory to `path`, returning a guard that
    /// will restore the previous cwd when dropped.
    fn enter(path: &Path) -> Self {
        let lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::current_dir().expect("current dir should be readable");
        std::env::set_current_dir(path).expect("set current dir should succeed");
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).expect("restore current dir should succeed");
    }
}

/// Verifies that format_model_display falls back to the endpoint name when no
/// matching endpoint exists in the config, appending the effort level label.
#[test]
fn format_model_display_fallback_uses_endpoint_name() {
    let config = empty_config();
    let ep = EndpointName::new("my-ep");
    let display = super::format_model_display(&config, &ep);
    assert!(
        display.contains("my-ep"),
        "display must include endpoint name, got: {display:?}"
    );
}

/// Verifies that format_model_display prefers the configured endpoint model and
/// appends the derived effort label when the endpoint exists.
#[test]
fn format_model_display_uses_endpoint_model_with_effort_suffix() {
    let mut config = empty_config();
    config.endpoints = vec![EndpointConfig {
        name: EndpointName::new("my-ep"),
        provider: Provider::OpenAi,
        base_url: EndpointUrl::new("https://example.invalid"),
        model: ModelName::new("gpt-5"),
        credentials: EndpointCredentials::default(),
    }];

    let display = super::format_model_display(&config, &EndpointName::new("my-ep"));

    assert_eq!(display.as_str(), "gpt-5 (high)");
}

/// Verifies that format_model_display uses the Copilot SDK model label without
/// appending an effort suffix when Copilot chat is enabled.
#[test]
fn format_model_display_uses_copilot_model_without_effort_suffix() {
    let mut config = empty_config();
    config.copilot.copilot_chat.enabled = true.into();
    config.copilot.copilot_chat.sdk = CopilotSdkSettings {
        model: Some(ModelName::new("claude-sonnet-4-6")),
        ..Default::default()
    };

    let display = super::format_model_display(&config, &EndpointName::new("ignored"));

    assert_eq!(display.as_str(), "claude-sonnet-4-6");
}

#[test]
fn format_model_display_prefers_endpoint_model_even_when_copilot_enabled() {
    let mut config = empty_config();
    config.copilot.copilot_chat.enabled = true.into();
    config.endpoints = vec![EndpointConfig {
        name: EndpointName::new("openrouter"),
        provider: Provider::OpenRouter,
        base_url: EndpointUrl::new("https://openrouter.ai/api/v1"),
        model: ModelName::new("anthropic/claude-sonnet-4-5"),
        credentials: EndpointCredentials::default(),
    }];

    let display = super::format_model_display(&config, &EndpointName::new("openrouter"));
    assert_eq!(display.as_str(), "anthropic/claude-sonnet-4-5 (high)");
}

// ── build_status_bar() tests ─────────────────────────────────────────────────

/// Verifies that build_status_bar initializes all StatusBarData fields from
/// the provided config and endpoint name.
#[test]
fn build_status_bar_initializes_all_fields() {
    let config = empty_config();
    let ep_name = EndpointName::new("test-ep");

    let status = super::build_status_bar(&config, &ep_name);

    // Verify all fields are populated
    assert!(!status.model_display.as_str().is_empty());
    assert!(!status.cwd.as_str().is_empty());
}

/// Verifies that build_status_bar sets the model_display field using the
/// format_model_display function with the provided config and endpoint name.
#[test]
fn build_status_bar_sets_model_display_from_format_model_display() {
    let mut config = empty_config();
    config.endpoints = vec![EndpointConfig {
        name: EndpointName::new("claude"),
        provider: Provider::Anthropic,
        base_url: EndpointUrl::new("https://api.anthropic.com"),
        model: ModelName::new("claude-sonnet-4-6"),
        credentials: EndpointCredentials::default(),
    }];
    let ep_name = EndpointName::new("claude");

    let status = super::build_status_bar(&config, &ep_name);

    // temperature is 0.7, which maps to "high" effort level
    assert_eq!(status.model_display.as_str(), "claude-sonnet-4-6 (high)");
}

/// Verifies that build_status_bar populates the cwd field from the current
/// working directory.
#[test]
fn build_status_bar_sets_cwd_from_current_dir() {
    let config = empty_config();
    let ep_name = EndpointName::new("test-ep");

    let status = super::build_status_bar(&config, &ep_name);

    // cwd should not be empty and should be the current working directory
    assert!(!status.cwd.as_str().is_empty());
    assert_ne!(status.cwd.as_str(), "");
}

/// Verifies that build_status_bar populates git_branch from the current git
/// repository state.
#[test]
fn build_status_bar_sets_git_branch_from_current_repo() {
    let repo = init_git_repo("develop");
    let _guard = CurrentDirGuard::enter(repo.path());

    let config = empty_config();
    let ep_name = EndpointName::new("test-ep");

    let status = super::build_status_bar(&config, &ep_name);

    // git_branch should be populated with the current branch name
    assert_eq!(
        status.git_branch.as_ref().map(|b| b.as_str()),
        Some("develop")
    );
}

/// Verifies that build_status_bar sets git_branch to None when executed
/// outside a git repository.
#[test]
fn build_status_bar_sets_git_branch_none_outside_git_repo() {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let _guard = CurrentDirGuard::enter(dir.path());

    let config = empty_config();
    let ep_name = EndpointName::new("test-ep");

    let status = super::build_status_bar(&config, &ep_name);

    // git_branch should be None when not in a git repo
    assert_eq!(status.git_branch, None);
}

/// Verifies that build_status_bar marks the git_branch with '*' suffix when
/// the repository is dirty.
#[test]
fn build_status_bar_marks_dirty_git_repo_with_asterisk() {
    let repo = init_git_repo("develop");
    // Create an untracked file to make repo dirty
    std::fs::write(repo.path().join("untracked.txt"), "content\n").expect("create untracked file");
    let _guard = CurrentDirGuard::enter(repo.path());

    let config = empty_config();
    let ep_name = EndpointName::new("test-ep");

    let status = super::build_status_bar(&config, &ep_name);

    // git_branch should have '*' suffix indicating dirty state
    assert_eq!(
        status.git_branch.as_ref().map(|b| b.as_str()),
        Some("develop*")
    );
}

/// Verifies that build_status_bar uses the Copilot model label when Copilot
/// chat is enabled.
#[test]
fn build_status_bar_uses_copilot_model_display_when_enabled() {
    let mut config = empty_config();
    config.copilot.copilot_chat.enabled = true.into();
    config.copilot.copilot_chat.sdk = CopilotSdkSettings {
        model: Some(ModelName::new("gpt-4")),
        ..Default::default()
    };

    let ep_name = EndpointName::new("ignored");

    let status = super::build_status_bar(&config, &ep_name);

    // model_display should be the Copilot model without effort suffix
    assert_eq!(status.model_display.as_str(), "gpt-4");
}
