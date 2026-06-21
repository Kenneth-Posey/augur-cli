//! OpenRouter prompt-cache header builder.
//!
//! Converts a [`augur_domain::config::provider_catalog::OpenRouterCacheConfig`] into
//! HTTP header pairs that are
//! forwarded to the OpenRouter API to opt into prompt caching.

use augur_domain::config::provider_catalog::OpenRouterCacheConfig;

/// Semantic wrapper for OpenRouter cache headers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenRouterCacheHeaders(pub Vec<(String, String)>);

/// Build the OpenRouter cache HTTP headers from `config`.
///
/// Returns an empty vector when `config.enabled` is `false`.
/// When enabled, always emits `("X-OpenRouter-Cache", "true")`.
/// When `ttl_seconds` is also set, additionally emits
/// `("X-OpenRouter-Cache-TTL", "<ttl>")`.
///
/// # Inputs
/// - `config`: the cache configuration read from the provider catalog.
///
/// # Outputs
/// A `Vec<(String, String)>` of header name-value pairs; empty when caching is
/// disabled.
pub fn build_openrouter_cache_headers(config: &OpenRouterCacheConfig) -> OpenRouterCacheHeaders {
    if !config.enabled {
        return OpenRouterCacheHeaders::default();
    }
    let mut headers = vec![("X-OpenRouter-Cache".to_owned(), "true".to_owned())];
    if let Some(ttl) = config.ttl_seconds {
        headers.push(("X-OpenRouter-Cache-TTL".to_owned(), ttl.to_string()));
    }
    OpenRouterCacheHeaders(headers)
}
