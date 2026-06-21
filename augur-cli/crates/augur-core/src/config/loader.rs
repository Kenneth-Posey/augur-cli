//! YAML configuration loader.

use anyhow::Context;
use augur_domain::config::types::AppConfig;
use augur_domain::domain::string_newtypes::{FilePath, StringNewtype};
use serde_yaml::Value;
use std::path::{Path, PathBuf};

const PROVIDER_ANTHROPIC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/providers/anthropic.yaml"
));
const PROVIDER_OLLAMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/providers/ollama.yaml"
));
const PROVIDER_OPENAI: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/providers/openai.yaml"
));
const PROVIDER_OPENROUTER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/providers/openrouter.yaml"
));
const PROVIDER_COPILOT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/providers/copilot.yaml"
));
const SECRETS_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/application.secrets.template.yaml"
));

/// Load application configuration.
///
/// Resolution order:
/// 1. If `path` is `Some(p)`, read and parse that file.
/// 2. If `path` is `None`, check `~/.augur-cli/config/application.yaml`.
/// 3. Otherwise fall back to the compile-time embedded `application.yaml`.
///
/// After resolving the base config, looks for `application.secrets.yaml` in
/// the same directory and merges any fields it contains on top. A missing
/// secrets file is silently ignored; a present but malformed file returns an
/// error.
///
/// # Examples
///
/// ```ignore
/// # Example usage (would require actual config file)
/// use augur_core::config::load_config;
/// let config = load_config(None)?; // Use default config
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Errors
///
/// Returns `anyhow::Error` with file path context on any parse failure:
/// - File not found when explicitly specified
/// - YAML parsing error in config or secrets file
/// - Deserialization failure (type mismatch in config fields)
///
/// # See also
///
/// - [`AppConfig`] - Configuration type with all available settings
/// - `application.yaml` - Default embedded configuration file
pub fn load_config(path: Option<&FilePath>) -> anyhow::Result<AppConfig> {
    let (content, secrets_dir) = resolve_config_content(path)?;
    let base: Value = serde_yaml::from_str(&content).context("parsing config")?;
    let with_providers = apply_provider_overlays(base, &secrets_dir);
    let merged = apply_secrets(with_providers, &secrets_dir)?;
    serde_yaml::from_value(merged).context("deserializing merged config")
}

/// Return the raw YAML string and the directory to search for the secrets file.
///
/// Called by `load_config` to separate path resolution from parsing. The
/// returned `PathBuf` is the parent of whichever config file was chosen so
/// `apply_secrets` can locate `application.secrets.yaml` alongside it.
fn resolve_config_content(path: Option<&FilePath>) -> anyhow::Result<(String, PathBuf)> {
    match path {
        Some(p) => read_explicit_path(p),
        None => load_default_content(),
    }
}

/// Read the explicitly supplied config path and derive its parent directory.
///
/// Returns the raw YAML file content and the absolute parent directory of `p`.
/// Propagates an `anyhow::Error` with the file path embedded in the context
/// message when the file cannot be read.
fn read_explicit_path(p: &FilePath) -> anyhow::Result<(String, PathBuf)> {
    let abs = PathBuf::from(p.as_str());
    let dir = abs.parent().unwrap_or(Path::new(".")).to_path_buf();
    let content = std::fs::read_to_string(p.as_str())
        .with_context(|| format!("reading config file: {}", p.as_str()))?;
    Ok((content, dir))
}

/// Create or incrementally update the installed config layout at `~/.augur-cli/`.
///
/// Always checks every file and creates any that are missing so that existing
/// installs pick up new files (e.g. `application.secrets.yaml`) without having
/// to re-install.
///
/// # Panics
/// Panics if the `HOME` environment variable is not set.
fn ensure_install_layout() {
    let home = std::env::var("HOME")
        .expect("HOME environment variable must be set to initialise the config layout");
    let base = PathBuf::from(&home).join(".augur-cli");
    init_config_layout(&base);
}

/// Initialise or repair the install layout rooted at `base`.
///
/// Creates `config/`, `config/providers/`, `sessions/`, and `logs/`
/// subdirectories under `base`. Each file is written only if it does not
/// already exist so that user edits are preserved on upgrade.
///
/// Exposed as `pub` so integration tests can call it with a temporary
/// base directory instead of `~/.augur-cli`.
pub fn init_config_layout(base: &Path) {
    let config_dir = base.join("config");
    let providers_dir = config_dir.join("providers");
    let sessions_dir = base.join("sessions");
    let logs_dir = base.join("logs");

    let _ = std::fs::create_dir_all(&config_dir);
    let _ = std::fs::create_dir_all(&providers_dir);
    let _ = std::fs::create_dir_all(&sessions_dir);
    let _ = std::fs::create_dir_all(&logs_dir);

    // Write application.yaml only on first install (preserve user edits on upgrade).
    let app_yaml_path = config_dir.join("application.yaml");
    if !app_yaml_path.exists() {
        let embedded = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../configs/application.yaml"
        ));
        let log_dir_str = logs_dir.display().to_string();
        let sessions_dir_str = sessions_dir.display().to_string();
        let content = format!(
            "{}\npersistence:\n  log_dir: \"{}\"\n  sessions_dir: \"{}\"\n",
            embedded, log_dir_str, sessions_dir_str
        );
        let _ = std::fs::write(&app_yaml_path, content.as_bytes());
    }

    // Write provider templates only if missing (preserve user edits on upgrade).
    for (filename, content) in [
        ("anthropic.yaml", PROVIDER_ANTHROPIC),
        ("copilot.yaml", PROVIDER_COPILOT),
        ("ollama.yaml", PROVIDER_OLLAMA),
        ("openai.yaml", PROVIDER_OPENAI),
        ("openrouter.yaml", PROVIDER_OPENROUTER),
    ] {
        let path = providers_dir.join(filename);
        if !path.exists() {
            let _ = std::fs::write(&path, content.as_bytes());
        }
    }

    // Write secrets template only if missing (never clobber user keys).
    let secrets_path = config_dir.join("application.secrets.yaml");
    if !secrets_path.exists() {
        let _ = std::fs::write(&secrets_path, SECRETS_TEMPLATE.as_bytes());
    }
}

/// Try `~/.augur-cli/config/application.yaml`; fall back to the compile-time
/// embedded `application.yaml`.
///
/// Returns the raw YAML content and its effective parent directory so
/// `apply_secrets` can locate `application.secrets.yaml` alongside it.
fn load_default_content() -> anyhow::Result<(String, PathBuf)> {
    ensure_install_layout();
    let user_path = installed_config_path();
    if user_path.exists() {
        let dir = user_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let content = std::fs::read_to_string(&user_path)
            .with_context(|| format!("reading config file: {}", user_path.display()))?;
        return Ok((content, dir));
    }
    let repo_config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../configs");
    load_embedded_default_content(&repo_config_dir)
}

/// Returns the path to the installed config file: `~/.augur-cli/config/application.yaml`.
///
/// # Panics
/// Panics if the `HOME` environment variable is not set.
fn installed_config_path() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME environment variable must be set");
    PathBuf::from(home)
        .join(".augur-cli")
        .join("config")
        .join("application.yaml")
}

fn load_embedded_default_content(repo_config_dir: &Path) -> anyhow::Result<(String, PathBuf)> {
    if repo_config_dir.join("application.secrets.yaml").exists() {
        return Ok((
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../configs/application.yaml"
            ))
            .to_owned(),
            repo_config_dir.to_path_buf(),
        ));
    }
    let dir = installed_config_path()
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    Ok((
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../configs/application.yaml"
        ))
        .to_owned(),
        dir,
    ))
}

/// Load `application.secrets.yaml` from `secrets_dir` and merge it over `base`.
/// Merge provider-specific config keys from `{config_dir}/providers/copilot.yaml`
/// into `base`.
///
/// Extracts only `executor:` and `copilot_chat:` from the copilot provider file
/// and merges them into the base config. Other keys (e.g. `provider:`) are ignored.
/// Returns `base` unchanged when the copilot provider file is missing or unreadable.
fn apply_provider_overlays(base: Value, config_dir: &Path) -> Value {
    let copilot_path = config_dir.join("providers").join("copilot.yaml");
    let content = if copilot_path.exists() {
        match std::fs::read_to_string(&copilot_path) {
            Ok(c) => c,
            Err(_) => return base,
        }
    } else {
        PROVIDER_COPILOT.to_owned()
    };
    let overlay: Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return base,
    };
    let filtered = extract_keys(&overlay, &["executor", "copilot_chat"]);
    // Provider YAML supplies defaults; application YAML values take priority.
    merge_yaml_values(filtered, base)
}

/// Build a new YAML mapping containing only the specified `keys` from `source`.
fn extract_keys(source: &Value, keys: &[&str]) -> Value {
    let mut map = serde_yaml::Mapping::new();
    if let Value::Mapping(m) = source {
        for &key in keys {
            if let Some(v) = m.get(key) {
                map.insert(Value::String(key.to_owned()), v.clone());
            }
        }
    }
    Value::Mapping(map)
}

///
///
/// Fallback: if no secrets file exists alongside the config file, checks
/// `~/.augur-cli/config/application.secrets.yaml` so that an installed user
/// secrets file is picked up even when an explicit `--config` points to a
/// repo-local config that has no sibling secrets file (e.g. in a fresh clone
/// where `application.secrets.yaml` is gitignored)./// Returns `base` unchanged when the file does not exist. Returns an error
/// when the file exists but cannot be read or parsed - a malformed secrets
/// file should not be silently ignored.
fn apply_secrets(base: Value, secrets_dir: &Path) -> anyhow::Result<Value> {
    let secrets_path = secrets_dir.join("application.secrets.yaml");
    if secrets_path.exists() {
        let overlay = parse_secrets_overlay(&secrets_path)?;
        return if overlay.is_null() {
            Ok(base)
        } else {
            Ok(merge_yaml_values(base, overlay))
        };
    }

    // Fallback: if no secrets file lives alongside the config, try the
    // installed ~/.augur-cli/config/application.secrets.yaml so that a
    // --config pointing to a repo-local file in a fresh clone (where the
    // secrets file is gitignored) still picks up the user's keys.
    let home = std::env::var("HOME").ok();
    if let Some(home_dir) = home {
        let installed_secrets = PathBuf::from(home_dir)
            .join(".augur-cli")
            .join("config")
            .join("application.secrets.yaml");
        if installed_secrets.exists() {
            let overlay = parse_secrets_overlay(&installed_secrets)?;
            return if overlay.is_null() {
                Ok(base)
            } else {
                Ok(merge_yaml_values(base, overlay))
            };
        }
    }

    Ok(base)
}

fn parse_secrets_overlay(secrets_path: &Path) -> anyhow::Result<Value> {
    let content = std::fs::read_to_string(secrets_path)
        .with_context(|| format!("reading secrets: {}", secrets_path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("parsing secrets: {}", secrets_path.display()))
}

/// Deep-merge `overlay` into `base`.
///
/// For `Mapping` values: merges each key recursively; overlay keys take
/// precedence. The `endpoints` key delegates to `merge_endpoint_sequences`
/// for name-based merging instead of wholesale replacement.
/// For all other value types: overlay replaces base.
fn merge_yaml_values(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Mapping(base_map), Value::Mapping(overlay_map)) => {
            Value::Mapping(merge_yaml_mappings(base_map, overlay_map))
        }
        (_, overlay) => overlay,
    }
}

fn merge_yaml_mappings(
    mut base_map: serde_yaml::Mapping,
    overlay_map: serde_yaml::Mapping,
) -> serde_yaml::Mapping {
    for (key, overlay_val) in overlay_map {
        merge_yaml_mapping_key(&mut base_map, key, overlay_val);
    }
    base_map
}

fn merge_yaml_mapping_key(base_map: &mut serde_yaml::Mapping, key: Value, overlay_val: Value) {
    let entry = base_map.entry(key.clone()).or_insert(Value::Null);
    let merged = merge_yaml_key_value(key, entry.clone(), overlay_val);
    *entry = merged;
}

fn merge_yaml_key_value(key: Value, base_value: Value, overlay_value: Value) -> Value {
    match key.as_str() {
        Some("endpoints") => merge_endpoint_sequences(base_value, overlay_value),
        _ => merge_yaml_values(base_value, overlay_value),
    }
}

/// Merge an `endpoints` sequence overlay into a base sequence by `name` key.
///
/// Each overlay item is matched to a base item with the same `name` field.
/// When a match is found, the overlay item is deep-merged into the base item.
/// Overlay items with no matching name are appended to the sequence.
fn merge_endpoint_sequences(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Sequence(mut base_seq), Value::Sequence(overlay_seq)) => {
            for overlay_ep in overlay_seq {
                merge_endpoint_sequence_item(&mut base_seq, overlay_ep);
            }
            Value::Sequence(base_seq)
        }
        // Null overlay means "no override" - preserve base unchanged.
        (base, Value::Null) => base,
        (_, overlay) => overlay,
    }
}

fn merge_endpoint_sequence_item(base_seq: &mut Vec<Value>, overlay_ep: Value) {
    match matching_endpoint_index(base_seq, &overlay_ep) {
        Some(index) => {
            base_seq[index] = merge_yaml_values(base_seq[index].clone(), overlay_ep);
        }
        None => base_seq.push(overlay_ep),
    }
}

fn matching_endpoint_index(base_seq: &[Value], overlay_ep: &Value) -> Option<usize> {
    let endpoint = endpoint_name(overlay_ep)?;
    base_seq
        .iter()
        .position(|base_endpoint| endpoint_name(base_endpoint) == Some(endpoint))
}

/// Extract the `name` string field from an endpoint YAML mapping.
fn endpoint_name(ep: &Value) -> Option<&str> {
    ep.get("name").and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::{apply_secrets, load_embedded_default_content};
    use augur_domain::config::types::AppConfig;
    use augur_domain::domain::{ApiKey, StringNewtype};

    #[test]
    fn embedded_default_uses_repo_local_secrets_overlay_when_present() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let config_dir = temp_dir.path().join("configs");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        std::fs::write(
            config_dir.join("application.secrets.yaml"),
            "endpoints:\n  - name: openrouter\n    api_key: sk-or-v1-test\n",
        )
        .expect("write secrets");

        let (content, secrets_dir) = load_embedded_default_content(&config_dir)
            .expect("embedded default content should load");
        assert_eq!(secrets_dir, config_dir);

        let merged = apply_secrets(
            serde_yaml::from_str(&content).expect("parse embedded yaml"),
            &secrets_dir,
        )
        .expect("merge secrets");
        let cfg: AppConfig = serde_yaml::from_value(merged).expect("merged config deserializes");
        let ep = cfg
            .endpoints
            .iter()
            .find(|e| e.name.as_str() == "openrouter")
            .expect("openrouter endpoint exists");
        assert_eq!(ep.credentials.api_key, Some(ApiKey::new("sk-or-v1-test")));
    }
}
