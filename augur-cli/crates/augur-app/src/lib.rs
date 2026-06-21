#![allow(unused_imports)]

//! Application crate that wires core services into the runtime entry point.

/// Startup wiring and runtime assembly for the application crate.
pub mod wiring;

use std::sync::OnceLock;

use augur_domain::domain::string_newtypes::FilePath;

static _TRACING_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

fn init_tracing(log_dir: &std::path::Path, session_secs: u64, log_filter: Option<&str>) {
    // Ensure the log directory exists.
    if let Err(e) = std::fs::create_dir_all(log_dir) {
        // If we cannot create the directory, fall back to the default "logs"
        // so the application can still produce diagnostic output.
        eprintln!(
            "warning: could not create log directory {:?}: {e}; falling back to ./logs",
            log_dir
        );
        let fallback: &std::path::Path = std::path::Path::new("logs");
        return init_tracing(fallback, session_secs, log_filter);
    }

    let file_name = format!("{session_secs}_trace.log");
    let file_appender = tracing_appender::rolling::never(log_dir, file_name);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = _TRACING_GUARD.set(guard);

    // Priority: explicit --log-filter > RUST_LOG env var > "info"
    let filter_str = log_filter
        .map(|s| s.to_owned())
        .or_else(|| std::env::var("RUST_LOG").ok())
        .unwrap_or_else(|| "info".to_owned());
    let env_filter = tracing_subscriber::EnvFilter::new(&filter_str);

    use tracing_subscriber::prelude::*;
    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .try_init();
}

/// Run the application runtime using the configured wiring and renderer.
///
/// Loads config and program settings, initializes tracing once, and then
/// delegates execution to `wiring::run`.
///
/// # Arguments
///
/// * `config_path` - Optional explicit path to `application.yaml`. When `None`,
///   the loader checks `~/.augur-cli/config/application.yaml` then falls back to
///   the compile-time embedded default.
/// * `log_filter` - Optional tracing filter directive (e.g.
///   `warn,augur_cli=info`). When `None`, falls back to `RUST_LOG` or
///   `"info"`.
pub async fn run(config_path: Option<FilePath>, log_filter: Option<String>) -> anyhow::Result<()> {
    let session_secs = augur_core::actors::logger::logger_ops::current_unix_secs();

    // Load config first so we can use the configured log_dir for tracing.
    let config = augur_core::config::load_config(config_path.as_ref())?;

    // Convert the configured log_dir to a filesystem path for the tracing
    // appender, scoped to the current repo subdirectory (same pattern as
    // the message logger and session files).
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let log_dir = augur_domain::persistence::store::apply_repo_subdir(
        std::path::PathBuf::from(&*config.persistence.log_dir),
        &cwd,
    );
    init_tracing(&log_dir, *session_secs, log_filter.as_deref());

    let program_settings = augur_core::config::load_program_settings();
    wiring::run(
        wiring::RunConfig {
            config,
            program_settings,
        },
        augur_tui::tui::render::render_with_overlays,
        session_secs,
    )
    .await
}
