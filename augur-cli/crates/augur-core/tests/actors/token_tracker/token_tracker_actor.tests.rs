//! Integration tests for the token-tracker actor.
//!
//! # Test coverage
//! - spawn / shutdown lifecycle
//! - record_usage → snapshot accumulation
//! - startup initialization and async persistence

use augur_core::actors::token_tracker;
use augur_core::token_history::ProjectSettings;
use augur_domain::domain::{
    newtypes::NumericNewtype,
    string_newtypes::{OutputText, StringNewtype},
    types::{ContextUsageStats, LlmTokenCounts, LlmUsage, ProjectTokenTotals},
    Count, Temperature, TokenCount,
};
use tempfile::TempDir;

fn make_usage(tokens_in: u64) -> LlmUsage {
    LlmUsage {
        model: OutputText::new("test"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(tokens_in),
            tokens_out: TokenCount::new(0),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.0),
    }
}

fn tmp_settings_path(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("settings.json")
}

fn spawn_with_settings_file(
    settings: ProjectSettings,
    path: &std::path::Path,
) -> (
    tokio::task::JoinHandle<()>,
    augur_core::actors::token_tracker::TokenTrackerHandle,
) {
    token_tracker::spawn_with_settings(settings, Some(path.to_path_buf()))
}

/// Verifies spawn initializes the actor and record_usage accumulates tokens.
#[tokio::test]
async fn test_actor_record_usage_accumulates_tokens() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (_join, handle) = token_tracker::spawn();

    handle.record_usage(make_usage(100));
    let totals = handle.snapshot().await;
    assert_eq!(totals.tokens_in, TokenCount::new(100));
}

/// Verifies record_context does not change the token totals snapshot.
#[tokio::test]
async fn test_actor_record_context_does_not_change_totals() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (_join, handle) = token_tracker::spawn();

    let initial = handle.snapshot().await;
    assert_eq!(initial, ProjectTokenTotals::default());

    let stats = ContextUsageStats {
        current_tokens: TokenCount::new(500),
        token_limit: TokenCount::new(8000),
        messages_length: Count::of(10),
    };
    handle.record_context(stats);
    let totals = handle.snapshot().await;
    assert_eq!(totals, ProjectTokenTotals::default());
}

/// Verifies snapshot returns the current accumulated totals.
#[tokio::test]
async fn test_actor_snapshot_returns_current_totals() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (_join, handle) = token_tracker::spawn();

    let u = LlmUsage {
        model: OutputText::new("m"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::ZERO,
            tokens_out: TokenCount::new(300),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.0),
    };
    handle.record_usage(u);
    let totals = handle.snapshot().await;
    assert_eq!(totals.tokens_out, TokenCount::new(300));
}

/// Verifies shutdown causes the actor's join handle to complete cleanly.
#[tokio::test]
async fn test_actor_shutdown_exits_run_loop_cleanly() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (join, handle) = token_tracker::spawn();

    handle.record_usage(make_usage(10));
    handle.record_usage(make_usage(10));
    handle.shutdown();
    join.await.expect("actor task must complete without panic");
}

/// Verifies dropping all handle clones exits the run loop naturally.
#[tokio::test]
async fn test_actor_channel_closed_exits_run_loop() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (join, handle) = token_tracker::spawn();

    drop(handle);
    join.await
        .expect("actor task must complete without panic when channel closes");
}

/// Verifies snapshot returns current totals when the actor is running.
#[tokio::test]
async fn test_snapshot_returns_current_totals_when_actor_running() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (_join, handle) = token_tracker::spawn();

    let u = LlmUsage {
        model: OutputText::new("m"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::ZERO,
            tokens_out: TokenCount::new(300),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.0),
    };
    handle.record_usage(u);
    let totals = handle.snapshot().await;
    assert_eq!(totals.tokens_out, TokenCount::new(300));
}

/// Verifies snapshot returns default totals when the actor has stopped.
#[tokio::test]
async fn test_snapshot_returns_default_when_actor_stopped() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (join, handle) = token_tracker::spawn();

    handle.shutdown();
    join.await.expect("actor must stop cleanly");

    let totals = handle.snapshot().await;
    assert_eq!(totals, ProjectTokenTotals::default());
}

/// Verifies cloned handles share the same actor channel.
#[tokio::test]
async fn test_cloned_handles_share_same_actor_channel() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (_join, handle_a) = token_tracker::spawn();
    let handle_b = handle_a.clone();

    handle_a.record_usage(make_usage(50));
    handle_b.record_usage(make_usage(50));
    let totals = handle_a.snapshot().await;
    assert_eq!(totals.tokens_in, TokenCount::new(100));
}

/// Verifies concurrent clone mutations all serialize without lost updates.
#[tokio::test(flavor = "multi_thread")]
async fn test_concurrent_clones_serialize_all_mutations() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (_join, handle) = token_tracker::spawn();

    let tasks: Vec<_> = (0..4)
        .map(|_| {
            let h = handle.clone();
            tokio::spawn(async move {
                h.record_usage(make_usage(100));
            })
        })
        .collect();

    for t in tasks {
        t.await.unwrap();
    }
    let totals = handle.snapshot().await;
    assert_eq!(totals.tokens_in, TokenCount::new(400));
}

/// Verifies spawn initializes totals from the provided ProjectSettings input.
#[tokio::test]
async fn test_spawn_initializes_state_from_input_settings() {
    let dir = TempDir::new().unwrap();
    let path = tmp_settings_path(&dir);
    let mut settings = ProjectSettings::default();
    settings.token_totals.tokens_in = TokenCount::new(77);
    let (_join, handle) = spawn_with_settings_file(settings, &path);
    let totals = handle.snapshot().await;
    assert_eq!(totals.tokens_in, TokenCount::new(77));
}

/// Verifies spawn returns a non-finished join handle and a usable handle.
#[tokio::test]
async fn test_spawn_returns_non_completed_join_handle() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (join, handle) = token_tracker::spawn();

    assert!(
        !join.is_finished(),
        "actor must not be finished immediately after spawn"
    );
    let totals = handle.snapshot().await;
    assert_eq!(totals, ProjectTokenTotals::default());
    handle.shutdown();
}

/// Verifies record_usage asynchronously persists updated totals to project settings.
#[tokio::test]
async fn test_record_usage_persists_totals_to_settings_file() {
    use augur_core::token_history::load_or_create;
    use tokio::time::{sleep, Duration};

    let dir = TempDir::new().unwrap();
    let path = tmp_settings_path(&dir);
    let (_join, handle) = spawn_with_settings_file(ProjectSettings::default(), &path);

    handle.record_usage(make_usage(1));
    let _ = handle.snapshot().await;

    for _ in 0..40 {
        if path.exists() {
            let settings = load_or_create(path.as_path()).expect("load persisted settings");
            if settings.token_totals.tokens_in == TokenCount::new(1) {
                return;
            }
        }
        sleep(Duration::from_millis(25)).await;
    }

    panic!("token tracker must persist updated totals asynchronously");
}

/// Verifies ResetTotals clears running totals for the next session.
#[tokio::test]
async fn test_reset_totals_clears_running_totals() {
    let dir = TempDir::new().unwrap();
    let _path = tmp_settings_path(&dir);
    let (_join, handle) = token_tracker::spawn();

    handle.record_usage(make_usage(42));
    let before = handle.snapshot().await;
    assert_eq!(before.tokens_in, TokenCount::new(42));

    handle.reset_totals();
    let after = handle.snapshot().await;
    assert_eq!(after, ProjectTokenTotals::default());
}

/// Verifies startup can load persisted totals and initialize actor state from them.
#[tokio::test]
async fn test_spawn_with_loaded_settings_initializes_persisted_totals() {
    let dir = TempDir::new().unwrap();
    let path = tmp_settings_path(&dir);
    let mut persisted = ProjectSettings::default();
    persisted.token_totals.tokens_in = TokenCount::new(13);
    augur_core::token_history::save(&persisted, path.as_path()).expect("save settings");
    let loaded = augur_core::token_history::load_or_create(path.as_path()).expect("load settings");
    let (_join, handle) = spawn_with_settings_file(loaded, &path);
    let totals = handle.snapshot().await;
    assert_eq!(totals.tokens_in, TokenCount::new(13));
}

/// Verifies all four module files exist in the token_tracker directory.
#[test]
fn test_token_tracker_module_files_exist() {
    let base = std::path::Path::new("src/actors/token_tracker");
    for file in &[
        "mod.rs",
        "token_tracker_actor.rs",
        "handle.rs",
        "token_tracker_ops.rs",
    ] {
        assert!(
            base.join(file).exists(),
            "expected src/actors/token_tracker/{file} to exist"
        );
    }
}
