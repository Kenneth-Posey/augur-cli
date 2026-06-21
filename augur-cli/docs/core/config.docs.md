# Config Module

The `config` module handles loading, saving, and runtime access to application settings. It owns two configuration domains: **program settings** (editable YAML defaults such as excluded directories, read-only path patterns, and tool-availability flags) and **user settings** (per-user preferences persisted as `user-settings.yaml`). Both are loaded from disk at startup, exposed through typed Rust structs, and saved back when modified.

## Submodule Organization

**`loader`** provides the top-level `load_config` function that reads and merges settings from the configured paths. **`program_settings`** defines the `ProgramSettings` struct and the `load_program_settings` / `save_program_settings` pair, with `save_program_settings_sync` for contexts where async I/O is unavailable. **`user_settings`** mirrors this pattern for `UserSettings`. **`provider_catalog`** and **`endpoint_catalog_discovery`** handle catalog-based provider lookup: they read a list of known provider endpoints (Anthropic, OpenAI, Ollama) and their capabilities from the settings files. **`write_section`** is an internal helper for atomically updating individual configuration sections.

## Architectural Role

The config module is the single source of truth for all mutable application settings. It sits at the boundary between static program defaults (compiled into the binary or bundled as `program_settings.yml`) and user- or environment-specific overrides. The agent and tool actors read configuration through this module to determine file access policies, provider selections, cache behavior, and other runtime parameters that can change between sessions or be updated by user commands.