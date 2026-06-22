//! Output formatters for model catalog results.
//!
//! Provides two output modes:
//! - [`to_yaml_snippet`] - serialises a slice of [`ModelInfo`] values as a
//!   multi-document YAML string suitable for pasting into `application.yaml`.
//! - [`to_markdown_catalog`] - renders a [`ModelInfo`] slice as a GitHub
//!   Flavoured Markdown table.

use super::{MarkdownCatalog, ModelInfo, YamlSnippet};

// ── Public API ──────────────────────────────────────────────────────────────

/// Serialises a slice of models to a multi-document YAML string.
///
/// Each model is rendered as a YAML mapping block. Multiple models are
/// separated by `---\n` (YAML document separator), making the output
/// directly appendable to an `application.yaml` `models:` list.
///
/// An empty slice returns an empty string.
///
/// # Arguments
/// - `models` - Slice of models to serialise.
///
/// # Returns
/// A `String` containing one YAML document per model, separated by `---\n`.
/// Returns a YAML comment describing the error for any model that fails
/// serialisation.
pub fn to_yaml_snippet(models: &[ModelInfo]) -> YamlSnippet {
    YamlSnippet(
        models
            .iter()
            .map(|m| match serde_yaml::to_string(m) {
                Ok(yaml) => yaml,
                Err(e) => format!("# serialisation error: {e}\n"),
            })
            .collect::<Vec<_>>()
            .join("---\n"),
    )
}

/// Renders a slice of models as a Markdown table.
///
/// Columns: `ID`, `Name`, `Provider`, `Context Window`,
/// `Input $/Mtok`, `Output $/Mtok`.
///
/// An empty slice produces a header-only table (header + separator rows).
///
/// # Arguments
/// - `models` - Slice of models to include in the table.
///
/// # Returns
/// A `String` containing a GitHub-Flavoured Markdown table.
pub fn to_markdown_catalog(models: &[ModelInfo]) -> MarkdownCatalog {
    let header = "| ID | Name | Provider | Context Window | Input $/Mtok | Output $/Mtok |";
    let separator = "|----|------|----------|----------------|--------------|---------------|";

    let rows: Vec<String> = models
        .iter()
        .map(|m| {
            format!(
                "| {} | {} | {} | {} | {:.4} | {:.4} |",
                m.id.0,
                m.name,
                m.provider.0,
                m.context_window.0,
                m.pricing.input_price_per_mtok,
                m.pricing.output_price_per_mtok,
            )
        })
        .collect();

    let mut lines = vec![header.to_string(), separator.to_string()];
    lines.extend(rows);
    MarkdownCatalog(lines.join("\n"))
}

// ── Tests ────────────────────────────────────────────────────────────────────
