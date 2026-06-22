//! # StreamState - Parameter Remediation Bundle Type
//!
//! This module defines the `StreamState<T>` value object that bundles three
//! LLM-orthogonal parameters (tools, endpoint, last_usage) into a semantic unit
//! representing LLM context state. This bundling reduces function parameter
//! complexity in refactored functions like `finalize_iteration`.
//!
//! ## Purpose
//!
//! Previously, the `finalize_iteration` function accepted 6 parameters, including
//! three that form a coherent "LLM context state" concept:
//! - `tools: &T` (tool executor reference)
//! - `endpoint: &EndpointName` (LLM provider identifier)
//! - `last_usage: Option<LlmUsage>` (prior invocation metadata)
//!
//! By bundling these into `StreamState<T>`, we reduce the function signature
//! from 6 parameters to 4 while making the semantic relationship explicit.
//!
//! ## Invariants
//!
//! - `tools` reference must remain valid for the entire lifetime of StreamState
//! - `endpoint` must point to a valid, recognized LLM provider (enforced by EndpointName validation)
//! - `last_usage` represents the immediate prior invocation; `None` indicates first invocation
//!
//! ## Lifetime Management
//!
//! `StreamState<T>` is designed to be immutably borrowed:
//! - Constructed once per LLM invocation cycle
//! - Passed as `&StreamState<T>` to consuming functions
//! - Discarded after iteration completes

use crate::domain::newtypes::IsPredicate;
use crate::domain::{EndpointName, LlmUsage, ToolExecutor};

/// Bundles LLM context state parameters into a single semantic unit.
///
/// This type represents the conjunction of three LLM-orthogonal concepts:
/// - The available tool executor (orchestration context)
/// - The active LLM endpoint (provider context)
/// - The prior invocation's usage metadata (state context)
///
/// By bundling these, dependent functions accept a single `&StreamState<T>` parameter
/// instead of three separate parameters, improving readability and reducing cognitive load.
///
/// # Type Parameter
///
/// - `T: ToolExecutor` - The tool executor implementation. Typically `&DynToolExecutor`
///   or a concrete executor type in tests.
///
/// # Lifetimes
///
/// The `tools` and `endpoint` references are borrowed and must outlive any use of
/// the StreamState.
///
/// # Example
///
/// ```ignore
/// let executor = ToolExecutor::new();
/// let endpoint = EndpointName::new("openrouter");
/// let prior_usage = Some(LlmUsage { tokens: 500 });
///
/// let state = StreamState {
///     tools: &executor,
///     endpoint: &endpoint,
///     last_usage: prior_usage,
/// };
///
/// // Pass to refactored function
/// let result = finalize_iteration(consumed_chunks, &state, history, output_tx)?;
/// ```
#[derive(Clone, Debug)]
pub struct StreamState<'a, T: ToolExecutor + ?Sized> {
    /// Reference to the tool executor providing all registered tool definitions and
    /// the ability to execute tools. Must remain valid for the lifetime of this
    /// StreamState.
    pub tools: &'a T,

    /// Reference to the LLM endpoint/provider identifier (e.g., "openrouter", "anthropic").
    /// Must be a valid, recognized provider name.
    pub endpoint: &'a EndpointName,

    /// Optional metadata from the immediately prior LLM invocation.
    /// - `Some(usage)` indicates this is not the first invocation in a session
    /// - `None` indicates this is the first invocation or no prior usage was tracked
    pub last_usage: Option<LlmUsage>,
}

impl<'a, T: ToolExecutor + ?Sized> StreamState<'a, T> {
    /// Creates a new StreamState with the given components.
    ///
    /// # Arguments
    ///
    /// - `tools`: Reference to a tool executor
    /// - `endpoint`: Reference to an endpoint name
    /// - `last_usage`: Optional prior usage metadata
    ///
    /// # Returns
    ///
    /// A new `StreamState<T>` with all fields initialized.
    ///
    /// # Note
    ///
    /// Invariants are enforced at the type level:
    /// - `tools` reference validity is guaranteed by Rust's borrow checker
    /// - `endpoint` validity is guaranteed by `EndpointName` validation (at construction)
    /// - `last_usage` option is type-safe via `Option<LlmUsage>`
    pub fn new(tools: &'a T, endpoint: &'a EndpointName, last_usage: Option<LlmUsage>) -> Self {
        StreamState {
            tools,
            endpoint,
            last_usage,
        }
    }

    /// Returns true if this is the first invocation (no prior usage).
    pub fn is_first_invocation(&self) -> IsPredicate {
        IsPredicate::from(self.last_usage.is_none())
    }

    /// Returns a reference to the prior usage if available.
    pub fn prior_usage(&self) -> Option<&LlmUsage> {
        self.last_usage.as_ref()
    }
}
