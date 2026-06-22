//! Unit tests for the pure `accumulate` function in token_tracker ops.

use augur_core::actors::token_tracker::token_tracker_ops::accumulate;
use augur_domain::domain::{
    Temperature, TokenCount,
    newtypes::NumericNewtype,
    string_newtypes::{OutputText, StringNewtype},
    types::{LlmTokenCounts, LlmUsage, ProjectTokenTotals},
};

fn usage(tokens_in: u64, tokens_out: u64) -> LlmUsage {
    LlmUsage {
        model: OutputText::new("test-model"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(tokens_in),
            tokens_out: TokenCount::new(tokens_out),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.7),
    }
}

#[test]
fn test_accumulate_adds_tokens() {
    let mut totals = ProjectTokenTotals::default();
    let u = usage(10, 20);
    accumulate(&mut totals, &u);
    assert_eq!(totals.tokens_in, TokenCount::new(10));
    assert_eq!(totals.tokens_out, TokenCount::new(20));
}

#[test]
fn test_accumulate_is_additive() {
    let mut totals = ProjectTokenTotals::default();
    accumulate(&mut totals, &usage(10, 20));
    accumulate(&mut totals, &usage(5, 3));
    assert_eq!(totals.tokens_in, TokenCount::new(15));
    assert_eq!(totals.tokens_out, TokenCount::new(23));
}

/// Verifies all five fields are accumulated correctly across two calls.
#[test]
fn test_accumulate_five_fields_adds_correctly() {
    let mut totals = ProjectTokenTotals::default();
    let usage_a = LlmUsage {
        model: OutputText::new("m"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(10),
            tokens_out: TokenCount::new(5),
            tokens_cached: TokenCount::new(2),
            cache_write_tokens: TokenCount::new(1),
            cost_usd: 0.05.into(),
        },
        temperature: Temperature::new(0.7),
    };
    let usage_b = LlmUsage {
        model: OutputText::new("m"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(20),
            tokens_out: TokenCount::new(10),
            tokens_cached: TokenCount::new(4),
            cache_write_tokens: TokenCount::new(3),
            cost_usd: 0.10.into(),
        },
        temperature: Temperature::new(0.7),
    };
    accumulate(&mut totals, &usage_a);
    accumulate(&mut totals, &usage_b);
    assert_eq!(totals.tokens_in, TokenCount::new(30));
    assert_eq!(totals.tokens_out, TokenCount::new(15));
    assert_eq!(totals.tokens_cached, TokenCount::new(6));
    assert_eq!(totals.cache_write_tokens, TokenCount::new(4));
    assert!((totals.cost_usd - 0.15).abs() < f64::EPSILON * 4.0);
}

/// Verifies that zero-valued usage leaves totals unchanged.
#[test]
fn test_accumulate_zero_usage_leaves_totals_unchanged() {
    let mut totals = ProjectTokenTotals {
        tokens_in: TokenCount::new(100),
        tokens_out: TokenCount::new(50),
        tokens_cached: TokenCount::new(10),
        cache_write_tokens: TokenCount::new(5),
        cost_usd: 1.0.into(),
    };
    let zero_usage = LlmUsage {
        model: OutputText::new("m"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::ZERO,
            tokens_out: TokenCount::ZERO,
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.0),
    };
    accumulate(&mut totals, &zero_usage);
    assert_eq!(totals.tokens_in, TokenCount::new(100));
    assert_eq!(totals.tokens_out, TokenCount::new(50));
    assert_eq!(totals.tokens_cached, TokenCount::new(10));
    assert_eq!(totals.cache_write_tokens, TokenCount::new(5));
    assert!((totals.cost_usd - 1.0).abs() < f64::EPSILON);
}

/// Verifies accumulate performs no I/O (pure function - compiles and runs without side effects).
#[test]
fn test_accumulate_is_pure_no_io() {
    let mut totals = ProjectTokenTotals::default();
    let u = LlmUsage {
        model: OutputText::new("m"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(1),
            tokens_out: TokenCount::new(1),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.0),
    };
    accumulate(&mut totals, &u);
    assert_eq!(totals.tokens_in, TokenCount::new(1));
}

use proptest::prelude::*;

proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]

    /// PBT-003: accumulate result equals the exact pre + delta sum for all five fields.
    #[test]
    fn prop_accumulate_exact_field_sums(
        pre_in     in 0u64..5_000,
        pre_out    in 0u64..5_000,
        pre_cached in 0u64..5_000,
        pre_write  in 0u64..5_000,
        pre_cost   in 0.0f64..100.0,
        delta_in   in 0u64..5_000,
        delta_out  in 0u64..5_000,
        delta_cached in 0u64..5_000,
        delta_write  in 0u64..5_000,
        delta_cost   in 0.0f64..1.0,
    ) {
        let mut totals = ProjectTokenTotals {
            tokens_in: TokenCount::new(pre_in),
            tokens_out: TokenCount::new(pre_out),
            tokens_cached: TokenCount::new(pre_cached),
            cache_write_tokens: TokenCount::new(pre_write),
            cost_usd: pre_cost.into(),
        };
        let u = LlmUsage {
            model: OutputText::new("m"),
            token_counts: LlmTokenCounts {
                tokens_in: TokenCount::new(delta_in),
                tokens_out: TokenCount::new(delta_out),
                tokens_cached: TokenCount::new(delta_cached),
                cache_write_tokens: TokenCount::new(delta_write),
                cost_usd: delta_cost.into(),
            },
            temperature: Temperature::new(0.0),
        };
        accumulate(&mut totals, &u);
        prop_assert_eq!(totals.tokens_in, TokenCount::new(pre_in + delta_in));
        prop_assert_eq!(totals.tokens_out, TokenCount::new(pre_out + delta_out));
        prop_assert_eq!(totals.tokens_cached, TokenCount::new(pre_cached + delta_cached));
        prop_assert_eq!(totals.cache_write_tokens, TokenCount::new(pre_write + delta_write));
        prop_assert!((totals.cost_usd - (pre_cost + delta_cost)).abs() < 1e-9_f64);
    }

    /// Property: accumulate monotonically increases all five token fields.
    #[test]
    fn prop_accumulate_monotonically_increases_all_fields(
        pre_in   in 0u64..10_000,
        pre_out  in 0u64..10_000,
        pre_cached in 0u64..10_000,
        pre_write  in 0u64..10_000,
        pre_cost   in 0.0f64..1000.0,
        delta_in   in 0u64..10_000,
        delta_out  in 0u64..10_000,
        delta_cached in 0u64..10_000,
        delta_write  in 0u64..10_000,
        delta_cost   in 0.0f64..1.0,
    ) {
        let mut totals = ProjectTokenTotals {
            tokens_in: TokenCount::new(pre_in),
            tokens_out: TokenCount::new(pre_out),
            tokens_cached: TokenCount::new(pre_cached),
            cache_write_tokens: TokenCount::new(pre_write),
            cost_usd: pre_cost.into(),
        };
        let u = LlmUsage {
            model: OutputText::new("m"),
            token_counts: LlmTokenCounts {
                tokens_in: TokenCount::new(delta_in),
                tokens_out: TokenCount::new(delta_out),
                tokens_cached: TokenCount::new(delta_cached),
                cache_write_tokens: TokenCount::new(delta_write),
                cost_usd: delta_cost.into(),
            },
            temperature: Temperature::new(0.0),
        };
        accumulate(&mut totals, &u);
        prop_assert!(totals.tokens_in >= TokenCount::new(pre_in));
        prop_assert!(totals.tokens_out >= TokenCount::new(pre_out));
        prop_assert!(totals.tokens_cached >= TokenCount::new(pre_cached));
        prop_assert!(totals.cache_write_tokens >= TokenCount::new(pre_write));
        prop_assert!(totals.cost_usd >= pre_cost.into());
    }
}
