//! Unit tests for TokenTrackerHandle.
//!
//! These tests use a real in-process actor so they exercise the full
//! message-passing path without mocking the channel.

use augur_core::actors::token_tracker::TokenTrackerHandle;
use augur_core::actors::token_tracker::token_tracker_ops::TokenTrackerCommand;
use augur_domain::domain::{
    Count, Temperature, TokenCount,
    newtypes::NumericNewtype,
    string_newtypes::{OutputText, StringNewtype},
    types::{ContextUsageStats, LlmTokenCounts, LlmUsage, ProjectTokenTotals},
};
use tokio::sync::mpsc;

fn make_usage() -> LlmUsage {
    LlmUsage {
        model: OutputText::new("test"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(10),
            tokens_out: TokenCount::new(5),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.0),
    }
}

fn make_context_stats() -> ContextUsageStats {
    ContextUsageStats {
        current_tokens: TokenCount::new(500),
        token_limit: TokenCount::new(8000),
        messages_length: Count::of(10),
    }
}

/// Verifies record_usage enqueues a RecordUsage command when the channel has capacity.
#[test]
fn test_record_usage_enqueues_command_when_channel_has_capacity() {
    let (tx, mut rx) = mpsc::channel::<TokenTrackerCommand>(1);
    let handle = TokenTrackerHandle::new(tx);
    handle.record_usage(make_usage());
    assert!(matches!(
        rx.try_recv(),
        Ok(TokenTrackerCommand::RecordUsage(_))
    ));
}

/// Verifies record_usage silently drops when the channel is full (no panic, no error).
#[test]
fn test_record_usage_silently_drops_when_channel_full() {
    let (tx, _rx) = mpsc::channel::<TokenTrackerCommand>(1);
    tx.try_send(TokenTrackerCommand::Shutdown).ok();
    let handle = TokenTrackerHandle::new(tx);
    handle.record_usage(make_usage());
}

/// Verifies record_usage silently drops when the channel receiver is closed.
#[test]
fn test_record_usage_silently_drops_when_channel_closed() {
    let (tx, rx) = mpsc::channel::<TokenTrackerCommand>(1);
    drop(rx);
    let handle = TokenTrackerHandle::new(tx);
    handle.record_usage(make_usage());
}

/// Verifies record_context enqueues a RecordContext command when the channel has capacity.
#[test]
fn test_record_context_enqueues_command_when_channel_has_capacity() {
    let (tx, mut rx) = mpsc::channel::<TokenTrackerCommand>(1);
    let handle = TokenTrackerHandle::new(tx);
    handle.record_context(make_context_stats());
    assert!(matches!(
        rx.try_recv(),
        Ok(TokenTrackerCommand::RecordContext(_))
    ));
}

/// Verifies record_context silently drops when the channel receiver is closed.
#[test]
fn test_record_context_silently_drops_when_channel_closed() {
    let (tx, rx) = mpsc::channel::<TokenTrackerCommand>(1);
    drop(rx);
    let handle = TokenTrackerHandle::new(tx);
    handle.record_context(make_context_stats());
}

/// Verifies snapshot returns ProjectTokenTotals::default() when the actor has stopped.
#[tokio::test]
async fn test_snapshot_returns_default_after_actor_shutdown() {
    let (tx, rx) = mpsc::channel::<TokenTrackerCommand>(4);
    drop(rx);
    let handle = TokenTrackerHandle::new(tx);
    let totals = handle.snapshot().await;
    assert_eq!(totals, ProjectTokenTotals::default());
}

/// Verifies reset_totals enqueues a ResetTotals command.
#[test]
fn test_reset_totals_enqueues_command_when_channel_has_capacity() {
    let (tx, mut rx) = mpsc::channel::<TokenTrackerCommand>(1);
    let handle = TokenTrackerHandle::new(tx);
    handle.reset_totals();
    assert!(matches!(
        rx.try_recv(),
        Ok(TokenTrackerCommand::ResetTotals)
    ));
}
