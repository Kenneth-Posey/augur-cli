//! Tests for the `approve_phase` verdict tool.

use crate::tools::builtin::approve_phase::ApprovePhase;
use crate::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;
use tokio::sync::oneshot;
