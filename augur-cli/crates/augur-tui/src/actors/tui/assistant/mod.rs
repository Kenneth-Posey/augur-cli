//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Focused helper modules for clipboard handling, key dispatch, output
//! buffering, picker/session restore flows, plan helpers, and status-bar data.

/// Clipboard and selection helpers.
pub mod clipboard;
/// Key-dispatch and submit/cancel helper functions.
pub mod key_dispatch;
/// Buffered output and channel-draining helpers.
pub mod output_buf;
/// Session picker event handling helpers.
pub mod picker;
/// Plan-mode and query lifecycle helpers.
pub mod plan_view;
/// Session restore and hydration helpers.
pub mod session_restore;
/// Status-bar construction helpers.
pub mod status_bar;
