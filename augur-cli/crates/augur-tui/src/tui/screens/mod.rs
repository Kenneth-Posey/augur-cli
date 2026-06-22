//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Screen-level renderers. Each module owns one full-screen rendering context.
//!
//! - `session_selector`: startup session picker screen.
//! - `conversation`: full conversation layout dispatcher.

pub(crate) mod conversation;
pub(crate) mod session_selector;
