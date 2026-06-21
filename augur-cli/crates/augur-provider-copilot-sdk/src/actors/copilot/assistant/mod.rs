//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Assistant sub-modules for `CopilotChatActor`.
//!
//! Each sub-module holds a focused slice of logic extracted from `actor.rs`
//! to keep the actor file within the 200-logic-line threshold.
//!
//! - `context_ops`: Startup context usage seeding, background persistence, and SDK error helpers.
//! - `sdk_client`: SDK client construction and auth verification.
//! - `sdk_session`: Session creation, resumption, and pre-session trigger dispatch.
//! - `sdk_tools`: Tool definition, registration, and permission handler setup.
//! - `session_ops`: Interruptible send/compact operations that race against Shutdown.
//! - `turn_log`: Per-turn token accumulation, logging, and persistence commit.

pub mod context_ops;
pub mod sdk_client;
pub mod sdk_session;
pub mod sdk_tools;
pub mod session_ops;
pub mod turn_log;

pub use context_ops::{format_sdk_error, log_sdk_error};

pub use sdk_client::{build_client, check_auth_status};

pub use sdk_session::{create_or_resume_session, create_session, CreateOrResumeSessionArgs};

pub use sdk_tools::{query_user_tool_def, register_query_user_tool};

pub use session_ops::{
    build_sdk_attachments, compact_or_shutdown, is_session_dead, keepalive_session,
    send_or_shutdown, start_event_dispatch, EventDispatchArgs, SessionOpOutcome,
};

pub use session_ops::KEEPALIVE_INTERVAL;

pub use turn_log::{apply_log_event, drain_log_events, LogHandles, LogState};
