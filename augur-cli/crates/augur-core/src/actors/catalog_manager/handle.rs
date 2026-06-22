//! Catalog manager actor handle and message types.

use super::models::{OutputFormat, ProviderName};
use augur_domain::domain::string_newtypes::OutputText;

/// Handle to the catalog manager actor.
///
/// Provides a command interface for generating model catalogs from provider APIs.
#[derive(Clone)]
pub struct CatalogManagerHandle {
    tx: tokio::sync::mpsc::Sender<CatalogManagerCommand>,
}

pub(crate) enum CatalogManagerCommand {
    GenerateCatalog {
        provider_filter: Option<ProviderName>,
        format: OutputFormat,
        tx: tokio::sync::oneshot::Sender<anyhow::Result<OutputText>>,
    },
}

impl CatalogManagerHandle {
    /// Creates a new handle from a command sender.
    pub(crate) fn new(tx: tokio::sync::mpsc::Sender<CatalogManagerCommand>) -> Self {
        Self { tx }
    }

    /// Generates a model catalog from provider APIs.
    ///
    /// # Arguments
    /// - `provider` - Optional provider filter (openai, anthropic, openrouter, ollama, or None for all).
    /// - `format` - Output format (YAML or Markdown).
    ///
    /// # Returns
    /// Ok with the formatted catalog output; Err if any provider fetch or formatting failed.
    ///
    /// # Errors
    /// - Provider API fetch failures
    /// - Invalid format requests
    /// - File I/O errors
    pub async fn generate_catalog(
        &self,
        provider: Option<ProviderName>,
        format: OutputFormat,
    ) -> anyhow::Result<OutputText> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(CatalogManagerCommand::GenerateCatalog {
                provider_filter: provider,
                format,
                tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("catalog manager actor not running"))?;
        rx.await
            .map_err(|_| anyhow::anyhow!("catalog manager actor shutdown unexpectedly"))?
    }
}
