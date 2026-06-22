# Config

The `config` module defines the full application configuration schema, provider catalog types, and YAML-backed loaders that govern how the application is initialized at startup. It contains three submodules: `types` (the configuration data model), `provider_catalog` (the per-provider model metadata system), and `install_path` (install-path resolution and configuration).

## Key Components

- **Configuration types** (`types`): The `AppConfig` struct is the top-level configuration root, loaded from `application.yaml`. It contains `EndpointConfig` entries (each defining a provider, base URL, model, and credentials), `AgentConfig` (system prompt, max tokens, temperature, allowed directories), `CopilotConfig` (executor and chat settings), `PersistenceConfig` (log and session directories), `ProgramSettings` (excluded directory names), and `UserSettings` (last endpoint, model, and reasoning effort persisted across sessions). Every string and numeric field uses a semantic newtype (`EndpointName`, `ModelName`, `OutputText`, `TokenCount`, `Temperature`, `FilePath`, `ApiKey`, `BearerToken`, etc.) rather than bare primitives.

- **Provider catalog** (`provider_catalog`): Defines `ProviderCatalogFile` and `ProviderCatalogModel`, the YAML schema for per-provider model metadata files stored under `configs/providers/`. Each model entry specifies pricing (`CostPerMtok`), context limits (`TokenCount`), compaction thresholds, tool support flags, and model identifiers. The `load_provider_catalog` and `write_provider_catalog` functions handle filesystem I/O with format validation, ensuring the provider name in the file matches the expected key. The `OpenRouterProviderConfig` sub-struct carries per-provider instruction file paths and cache configuration.

- **Helper functions**: `find_endpoint` provides the canonical linear scan for looking up an endpoint by name, `default_provider_catalog_dir` supports environment-driven catalog path overrides, and `default_excluded_directories` defines the standard `.git`/`target`/`changelogs` exclusion set used by file-scanning tools.

## Role in the Ecosystem

This module is the single point of definition for what configuration looks like in every runtime context - from YAML files on disk, through deserialization, to in-memory consumption by actors, the TUI, and provider adapters. Every other crate reads config values through these types, so the module enforces that all configuration access uses domain-typed fields rather than raw strings or floats. The provider catalog submodule adds an extensible per-model metadata layer that allows provider crates to define model-specific behavior (pricing, context limits, compaction parameters) without modifying the core config schema.