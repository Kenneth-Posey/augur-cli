/// Unit and async-unit tests for `src/tools/builtin/lsp_query.rs`.
///
/// All tests that exercise functions in this module are in Red state because the
/// production functions (`definition`, `execute`, `validate_input`,
/// `handle_lsp_response`, `format_locations`, `format_symbols`,
/// `flatten_document_symbols`, `make_session_log`) are all compile-target stubs
/// containing `todo!()`. Tests trigger those panics during the Red phase.
use super::*;
use crate::actors::lsp::{LspHandle, LspRequest};
use augur_domain::domain::lsp::{LspError, LspLocation, LspOperation, LspQueryInput, LspSymbol};
use augur_domain::domain::string_newtypes::StringNewtype;
use serde_json::json;
