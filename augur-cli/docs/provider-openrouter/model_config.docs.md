# OpenRouter Model Configuration

## Scope

Documents the per-model configuration resolution logic in `crates/augur-provider-openrouter/src/model_config.rs`. This module loads the OpenRouter provider catalog YAML at runtime and extracts per-model tuning parameters: compaction target, tool-result strip fraction, max tool iterations, and auto-compact threshold. It does not handle catalog fetching or caching -- those belong to the shared domain catalog infrastructure in `augur_domain::config::provider_catalog`.

## Key Components

The central type is `ResolvedModelConfig`, which bundles five fields: `compaction_target` (token count to compact toward), `max_context_length` (absolute context window of the model), `strip_fraction` (proportion of oldest tool-result messages to strip), `max_iterations` (tool-call iteration limit), and `auto_compact_threshold` (token count that triggers automatic compaction). Every field falls back to a hardcoded compile-time default when the model ID is absent, the model is not found in the catalog, or the field in the catalog is set to its zero sentinel.

The `resolve_model_config()` function accepts an optional `ModelId`. When `None`, it returns defaults immediately without disk I/O. When `Some`, it loads the OpenRouter provider catalog from the default provider catalog directory, searches for the matching model entry, and resolves each field using a zero-checking helper (`resolve_target`, `resolve_fraction`, `resolve_iterations`) that returns the fallback default if the catalog value is zero.

## Data Flow

1. The caller provides an optional `&ModelId` to `resolve_model_config()`.
2. If `None`, fallback defaults are returned immediately (no I/O).
3. If `Some`, the module reads the provider catalog YAML from the default directory via `load_provider_catalog()`.
4. The catalog is searched for the model entry matching the given `ModelId`.
5. Each parameter is resolved: if the catalog value is non-zero, it is used; otherwise the hardcoded fallback is returned.
6. The assembled `ResolvedModelConfig` is returned to the caller.

## Contracts and Invariants

- The fallback compaction target is 400,000 tokens; fallback max iterations is 100; fallback auto-compact threshold is 80% of the compaction target (320,000 tokens); fallback strip fraction is 0.9.
- A zero value in the catalog YAML always means "use provider default" and triggers the fallback.
- `max_context_length` is the one field that does NOT fall back to a non-zero default -- its zero value from the catalog is passed through as-is, signaling to callers that the catalog did not specify a context length.
- The provider catalog directory is determined by `default_provider_catalog_dir()`, which lives in the shared domain crate.

## Validation

The module includes a `#[cfg(test)]` test suite covering four scenarios: model found with all non-zero values, model found with zero values (verifying fallback), model not found in catalog, and `None` model ID (resolves to defaults). These tests use a synthetic `ProviderCatalogFile` rather than real YAML on disk.

## References

- Source: `crates/augur-provider-openrouter/src/model_config.rs`
- Provider catalog types and loading: `augur_domain::config::provider_catalog`
- Compaction utilities that consume resolved config: [compaction.docs.md](compaction.docs.md)