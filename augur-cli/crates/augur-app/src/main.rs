//! CLI argument parsing and application entrypoint.
//!
//! Supports `--config` for explicit config path and `--log-filter` for
//! tracing-level overrides. All remaining arguments are forwarded to the
//! runtime as-is (currently unused).

use augur_domain::domain::string_newtypes::{FilePath, StringNewtype};
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "augur-cli",
    about = "augur-cli — multi-provider LLM chat assistant"
)]
struct Cli {
    /// Path to application.yaml config file.
    ///
    /// When omitted, the loader checks `~/.augur-cli/config/application.yaml`
    /// and falls back to the compile-time embedded default.
    #[arg(long = "config")]
    config: Option<String>,

    /// Tracing filter directive (e.g. `warn,augur_cli=info`).
    ///
    /// When omitted, falls back to `RUST_LOG` or `info`.
    #[arg(long = "log-filter")]
    log_filter: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config_path = cli.config.map(FilePath::new);

    augur_cli::run(config_path, cli.log_filter).await
}
