use augur_core::config::provider_catalog::{
    ProviderCatalogFile, ProviderCatalogModel, load_provider_catalog, provider_catalog_path,
    write_provider_catalog,
};
use augur_domain::config::types::Provider;
use augur_domain::domain::{ModelId, ModelLabel, StringNewtype};

#[test]
fn load_provider_catalog_parses_valid_yaml() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = ProviderCatalogFile {
        provider: "openrouter".to_owned().into(),
        models: vec![ProviderCatalogModel {
            id: ModelId::new("anthropic/claude-sonnet-4-5"),
            display_name: Some(ModelLabel::new("Claude Sonnet 4.5")),
            cost_input_per_mtok: 3.0.into(),
            cost_output_per_mtok: 15.0.into(),
            supports_tools: Some(true),
            max_context_length: Default::default(),
            tool_compaction_ratio: Default::default(),
            max_tool_iterations: Default::default(),
            compaction_target: Default::default(),
            auto_compact_threshold: Default::default(),
        }],
        openrouter: None,
    };
    write_provider_catalog(dir.path(), &file).expect("write");

    let loaded = load_provider_catalog(dir.path(), Provider::OpenRouter)
        .expect("load ok")
        .expect("catalog exists");
    assert_eq!(loaded.provider, "openrouter");
    assert_eq!(loaded.models.len(), 1);
    assert_eq!(loaded.models[0].id.as_str(), "anthropic/claude-sonnet-4-5");
}

#[test]
fn load_provider_catalog_returns_none_when_missing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let loaded = load_provider_catalog(dir.path(), Provider::OpenAi).expect("load should not fail");
    assert!(loaded.is_none());
}

#[test]
fn load_provider_catalog_returns_error_for_malformed_yaml() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = provider_catalog_path(dir.path(), Provider::OpenAi);
    std::fs::write(
        &path,
        r#"
provider: openai
models:
  - id: gpt-4o
    cost_input_per_mtok: abc.try_into()
"#,
    )
    .expect("write malformed");

    let err =
        load_provider_catalog(dir.path(), Provider::OpenAi).expect_err("malformed yaml must error");
    assert!(err.to_string().contains("parsing provider catalog file"));
}

#[test]
fn catalog_without_openrouter_block_parses_correctly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = provider_catalog_path(dir.path(), Provider::Anthropic);
    std::fs::write(
        &path,
        "provider: anthropic\nmodels:\n  - id: claude-3-5-sonnet\n    cost_input_per_mtok: 3.0\n    cost_output_per_mtok: 15.0\n",
    )
    .expect("write");
    let loaded = load_provider_catalog(dir.path(), Provider::Anthropic)
        .expect("load ok")
        .expect("catalog exists");
    assert_eq!(loaded.provider, "anthropic");
    assert!(loaded.openrouter.is_none());
}
