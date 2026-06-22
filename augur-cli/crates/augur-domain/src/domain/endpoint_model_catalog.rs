//! Endpoint model catalog metadata for provider discovery.

use crate::domain::newtypes::SupportsAuto;
use crate::domain::string_newtypes::{EndpointName, ModelLabel, StringNewtype};
use crate::domain::types::ModelOption;

/// Model-catalog metadata for a single endpoint.
#[derive(Clone, bon::Builder)]
pub struct EndpointModelCatalog {
    /// Endpoint name this catalog row belongs to.
    pub endpoint_name: EndpointName,
    /// Models available for this endpoint context.
    pub models: Vec<ModelOption>,
    /// Status-bar display label to apply immediately after switching endpoints.
    pub default_display: ModelLabel,
    /// Whether this endpoint supports explicit "auto model" mode.
    #[builder(default)]
    pub supports_auto: SupportsAuto,
}
