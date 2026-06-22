//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! TUI component submodules for the conversation container, footer, primary
//! feed, secondary container, and text entry areas.

/// Conversation container layout and secondary-pane orchestration.
pub mod conversation_container;
/// Footer controls row and status-bar rendering helpers.
pub mod footer;
/// Primary feed rendering, selection overlay, and scrollbar helpers.
pub mod primary_feed;
pub mod primary_feed_utils;
/// Secondary ask/task container rendering helpers.
pub mod secondary_container;
/// Text-entry rendering and completion-hint widgets.
pub mod text_entry;
