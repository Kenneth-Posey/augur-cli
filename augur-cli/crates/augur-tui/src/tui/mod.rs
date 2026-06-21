//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Terminal UI subsystem: components, layout, screens, and render helpers.
//!
//! Provides the terminal user interface for the chat agent, including:
//! - Interactive chat pane with message rendering
//! - Tool result display and streaming output handling
//! - Status bar with agent state and context usage
//! - Keyboard input processing and command dispatch
//!
//! Built on ratatui and crossterm for terminal manipulation.

/// Reusable UI component primitives (widgets, overlays, and pane renderers).
pub mod components;
/// Layout computation utilities for terminal dimensions.
pub mod layout;
/// Interactive picker widget for file and session selection.
pub mod picker;
/// Plan panel rendering and plan-tree display helpers.
pub mod plan_panel;
/// Query dialog state and rendering helpers.
pub mod query;
/// Top-level render dispatch for screen-specific renderers.
pub mod render;
/// Screen definitions and per-screen rendering implementations.
pub mod screens;
