use augur_domain::config::provider_catalog::OpenRouterCacheConfig;
use augur_domain::domain::newtypes::IsEnabled;
use augur_provider_openrouter::actors::llm::providers::openrouter_cache::build_openrouter_cache_headers;

#[test]
fn cache_disabled_returns_empty_headers() {
    let config = OpenRouterCacheConfig {
        enabled: IsEnabled::no(),
        ttl_seconds: None,
    };
    let headers = build_openrouter_cache_headers(&config).0;
    assert!(headers.is_empty(), "disabled cache must produce no headers");
}

#[test]
fn cache_enabled_returns_cache_header() {
    let config = OpenRouterCacheConfig {
        enabled: IsEnabled::yes(),
        ttl_seconds: None,
    };
    let headers = build_openrouter_cache_headers(&config).0;
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, "X-OpenRouter-Cache");
    assert_eq!(headers[0].1, "true");
}

#[test]
fn cache_enabled_with_ttl_returns_both_headers() {
    let config = OpenRouterCacheConfig {
        enabled: IsEnabled::yes(),
        ttl_seconds: Some(3600),
    };
    let headers = build_openrouter_cache_headers(&config).0;
    assert_eq!(headers.len(), 2, "should emit both cache and TTL headers");

    let names: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
    assert!(
        names.contains(&"X-OpenRouter-Cache"),
        "must contain X-OpenRouter-Cache"
    );
    assert!(
        names.contains(&"X-OpenRouter-Cache-TTL"),
        "must contain X-OpenRouter-Cache-TTL"
    );

    let ttl_val = headers
        .iter()
        .find(|(k, _)| k == "X-OpenRouter-Cache-TTL")
        .map(|(_, v)| v.as_str())
        .expect("TTL header present");
    assert_eq!(ttl_val, "3600");
}
