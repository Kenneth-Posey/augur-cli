use augur_core::actors::catalog_manager::models::filter::filter_models;
use augur_core::actors::catalog_manager::models::{
    ContextWindowSize, CostTier, FilterOpts, ModelId, ModelInfo, ModelPricing, ProviderName,
};
use augur_domain::domain::UsdCost;

fn model(id: &str, provider: &str, input_cost: f64) -> ModelInfo {
    ModelInfo {
        id: ModelId(id.to_owned()),
        name: id.to_owned(),
        provider: ProviderName(provider.to_owned()),
        context_window: ContextWindowSize(128_000),
        pricing: ModelPricing {
            input_price_per_mtok: UsdCost::from(input_cost),
            output_price_per_mtok: UsdCost::from(input_cost),
        },
    }
}

#[test]
fn filter_models_applies_provider_filter_case_insensitive() {
    let models = vec![
        model("gpt-4o", "openai", 1.0),
        model("claude-3-5-sonnet", "anthropic", 2.0),
    ];

    let opts = FilterOpts::builder()
        .provider_filter(ProviderName("OPENAI".to_owned()))
        .build();

    let filtered = filter_models(models, &opts);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].provider.0, "openai");
}

#[test]
fn filter_models_can_restrict_to_tool_use_providers() {
    let models = vec![
        model("gpt-4o", "openai", 1.0),
        model("llama3.1", "ollama", 0.0),
    ];
    let opts = FilterOpts::builder().tool_use_only(true).build();

    let filtered = filter_models(models, &opts);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].provider.0, "openai");
}

#[test]
fn filter_models_applies_cost_tier_and_latest_only() {
    let models = vec![
        model("gpt-4-0613", "openai", 1.0),
        model("gpt-4-1106", "openai", 1.0),
        model("expensive", "openai", 6.0),
    ];
    let opts = FilterOpts::builder()
        .latest_only(true)
        .max_cost_tier(CostTier::Standard)
        .build();

    let filtered = filter_models(models, &opts);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id.0, "gpt-4-1106");
}
