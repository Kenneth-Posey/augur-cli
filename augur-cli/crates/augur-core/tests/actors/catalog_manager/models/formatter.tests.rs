use augur_core::actors::catalog_manager::models::formatter::{
    to_markdown_catalog, to_yaml_snippet,
};
use augur_core::actors::catalog_manager::models::{
    ContextWindowSize, ModelId, ModelInfo, ModelPricing, ProviderName,
};
use augur_domain::domain::UsdCost;

fn model(id: &str, provider: &str, input_cost: f64, output_cost: f64) -> ModelInfo {
    ModelInfo {
        id: ModelId(id.to_owned()),
        name: format!("{id} name"),
        provider: ProviderName(provider.to_owned()),
        context_window: ContextWindowSize(200_000),
        pricing: ModelPricing {
            input_price_per_mtok: UsdCost::from(input_cost),
            output_price_per_mtok: UsdCost::from(output_cost),
        },
    }
}

#[test]
fn to_yaml_snippet_serializes_all_models() {
    let models = vec![model("gpt-4o", "openai", 5.0, 15.0)];
    let yaml = to_yaml_snippet(&models).0;
    assert!(yaml.contains("id: gpt-4o"));
    assert!(yaml.contains("provider: openai"));
}

#[test]
fn to_markdown_catalog_renders_header_and_row() {
    let models = vec![model("claude-3-5-sonnet", "anthropic", 3.0, 15.0)];
    let markdown = to_markdown_catalog(&models).0;
    assert!(markdown.contains("| ID | Name | Provider | Context Window |"));
    assert!(markdown.contains("| claude-3-5-sonnet |"));
    assert!(markdown.contains("| anthropic |"));
}
