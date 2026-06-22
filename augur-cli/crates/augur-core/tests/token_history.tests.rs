use augur_core::token_history::{
    ProjectSettings, ensure_initialized, load_or_create, token_history_path,
};
use augur_domain::domain::TokenCount;
use augur_domain::domain::newtypes::NumericNewtype;
use augur_domain::domain::types::{LlmUsage, ProjectTokenTotals};
use tempfile::TempDir;

fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("tempdir creation failed")
}

#[test]
fn token_history_path_points_to_json_file() {
    assert_eq!(
        token_history_path().as_path(),
        std::path::Path::new("./state/token-history.json")
    );
}

#[test]
fn load_or_create_succeeds_when_file_missing() {
    let dir = temp_dir();
    let path = dir.path().join("settings.json");
    let _settings = load_or_create(&path).expect("load_or_create must succeed");
    assert!(
        !path.exists(),
        "load_or_create must not create file on missing path"
    );
}

#[test]
fn ensure_initialized_creates_missing_token_history_file() {
    let dir = temp_dir();
    let path = dir.path().join("token-history.json");
    ensure_initialized(&path).expect("ensure_initialized must succeed");
    assert!(path.exists(), "ensure_initialized must create the file");
    let contents = std::fs::read_to_string(&path).expect("read token history file");
    let parsed: ProjectSettings = serde_json::from_str(&contents).expect("parse token history");
    assert_eq!(parsed.token_totals, ProjectSettings::default().token_totals);
}

#[test]
fn llm_usage_cost_usd_defaults_to_zero_when_missing_from_json() {
    let json = r#"{
        "model": "m",
        "tokens_in": 10,
        "tokens_out": 5,
        "tokens_cached": 0,
        "temperature": 0.0
    }"#;

    let usage: LlmUsage = serde_json::from_str(json).expect("deserialization must succeed");
    assert_eq!(usage.cost_usd, 0.0);
    assert_eq!(usage.cache_write_tokens, TokenCount::ZERO);
}

#[test]
fn project_token_totals_deserializes_prior_schema_json_without_new_fields() {
    let json = r#"{"tokens_in": 100, "tokens_out": 50, "tokens_cached": 10}"#;

    let totals: ProjectTokenTotals =
        serde_json::from_str(json).expect("deserialization must succeed");
    assert_eq!(totals.tokens_in, TokenCount::new(100));
    assert_eq!(totals.tokens_out, TokenCount::new(50));
    assert_eq!(totals.tokens_cached, TokenCount::new(10));
    assert_eq!(totals.cache_write_tokens, TokenCount::ZERO);
    assert_eq!(totals.cost_usd, 0.0);
}

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn prop_project_token_totals_serde_round_trip(
        tokens_in in 0u64..1_000_000,
        tokens_out in 0u64..1_000_000,
        tokens_cached in 0u64..1_000_000,
        cache_write_tokens in 0u64..1_000_000,
        cost_usd in 0.0f64..10_000.0,
    ) {
        let original = ProjectTokenTotals {
            tokens_in: TokenCount::new(tokens_in),
            tokens_out: TokenCount::new(tokens_out),
            tokens_cached: TokenCount::new(tokens_cached),
            cache_write_tokens: TokenCount::new(cache_write_tokens),
            cost_usd: cost_usd.into(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: ProjectTokenTotals = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored.tokens_in, original.tokens_in);
        prop_assert_eq!(restored.tokens_out, original.tokens_out);
        prop_assert_eq!(restored.tokens_cached, original.tokens_cached);
        prop_assert_eq!(restored.cache_write_tokens, original.cache_write_tokens);
        prop_assert!((restored.cost_usd - original.cost_usd).abs() < 1e-9);
    }
}
