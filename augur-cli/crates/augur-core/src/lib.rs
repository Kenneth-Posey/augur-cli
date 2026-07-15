#![allow(dead_code, unused_imports, non_snake_case)]

//! Core workspace crate for domain models, actors, persistence, and tools.

extern crate self as augur_core;

/// Actor implementations and handles for core runtime flows.
pub mod actors;
/// Configuration loading, defaults, and program settings.
pub mod config;
/// Shared domain types and invariants.
pub mod domain;
/// Shared test helpers: fake actors and adapters for unit and integration tests.
pub mod helpers;
/// Core crate helper macros.
pub mod macros;
/// Persistence abstractions and storage helpers.
pub mod persistence;
/// Token history tracking for chat and review flows.
pub mod token_history;
/// Tool registry, built-ins, and tool execution support.
pub mod tools;
// ── Test Discovery Stubs ──────────────────────────────────────────────────────
// These #[cfg(test)] #[path = "..."] stubs connect [[test]] entries from
// Cargo.toml to external test files under tests/.
// See: .github/local/rules.md

// ── actors/active_model ────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/active_model/active_model_actor_ops.tests.rs"]
mod nkc_actors__active_model__active_model_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/active_model/active_model_ops.tests.rs"]
mod nkc_actors__active_model__active_model_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/active_model/handle.tests.rs"]
mod nkc_actors__active_model__handle_tests;

// ── actors/agent ───────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/agent/agent_actor_ops.tests.rs"]
mod nkc_actors__agent__agent_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/agent/agent_actor.tests.rs"]
mod nkc_actors__agent__agent_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/agent/agent_ops.tests.rs"]
mod nkc_actors__agent__agent_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/agent/handle.tests.rs"]
mod nkc_actors__agent__handle_tests;
#[cfg(test)]
#[path = "../tests/actors/agent/history.tests.rs"]
mod nkc_actors__agent__history_tests;
#[cfg(test)]
#[path = "../tests/actors/agent/persistence_ops.tests.rs"]
mod nkc_actors__agent__persistence_ops_tests;

// ── actors/ask ─────────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/ask/ask_actor_ops.tests.rs"]
mod nkc_actors__ask__ask_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/ask/ask_actor.tests.rs"]
mod nkc_actors__ask__ask_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/ask/handle.tests.rs"]
mod nkc_actors__ask__handle_tests;

// ── actors/cache ───────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/cache/cache_actor_ops.tests.rs"]
mod nkc_actors__cache__cache_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/cache/cache_actor.tests.rs"]
mod nkc_actors__cache__cache_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/cache/cache_ops.tests.rs"]
mod nkc_actors__cache__cache_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/cache/deps.tests.rs"]
mod nkc_actors__cache__deps_tests;
#[cfg(test)]
#[path = "../tests/actors/cache/handle.tests.rs"]
mod nkc_actors__cache__handle_tests;

// ── actors/catalog_manager ─────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/catalog_manager/catalog_manager_actor.tests.rs"]
mod nkc_actors__catalog_manager__catalog_manager_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/catalog_manager/handle.tests.rs"]
mod nkc_actors__catalog_manager__handle_tests;

// ── actors/command ─────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/command/command_actor_ops.tests.rs"]
mod nkc_actors__command__command_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/command/handle.tests.rs"]
mod nkc_actors__command__handle_tests;

// ── actors/deterministic_orchestrator/deterministic_orchestrator_actor ─────────
#[cfg(test)]
#[path = "../tests/actors/deterministic_orchestrator/deterministic_orchestrator_actor/runtime.tests.rs"]
mod nkc_actors__deterministic_orchestrator__deterministic_orchestrator_actor__runtime_tests;

// ── actors/file_read ───────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/file_read/file_read_actor_ops.tests.rs"]
mod nkc_actors__file_read__file_read_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/file_read/file_read_actor.tests.rs"]
mod nkc_actors__file_read__file_read_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/file_read/file_read_ops.tests.rs"]
mod nkc_actors__file_read__file_read_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/file_read/handle.tests.rs"]
mod nkc_actors__file_read__handle_tests;
#[cfg(test)]
#[path = "../tests/actors/file_read/mod.tests.rs"]
mod nkc_actors__file_read__mod_tests;

// ── actors/file_scanner ────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/file_scanner/file_scanner_actor_ops.tests.rs"]
mod nkc_actors__file_scanner__file_scanner_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/file_scanner/handle.tests.rs"]
mod nkc_actors__file_scanner__handle_tests;

// ── actors/history_adapter ─────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/history_adapter/history_adapter_actor_ops.tests.rs"]
mod nkc_actors__history_adapter__history_adapter_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/history_adapter/history_adapter_actor.tests.rs"]
mod nkc_actors__history_adapter__history_adapter_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/history_adapter/history_adapter_ops.tests.rs"]
mod nkc_actors__history_adapter__history_adapter_ops_tests;

// ── actors/llm_feed_consumer ───────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/llm_feed_consumer/llm_feed_consumer_actor_ops.tests.rs"]
mod nkc_actors__llm_feed_consumer__llm_feed_consumer_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/llm_feed_consumer/llm_feed_consumer_actor.tests.rs"]
mod nkc_actors__llm_feed_consumer__llm_feed_consumer_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/llm_feed_consumer/llm_feed_consumer_ops.tests.rs"]
mod nkc_actors__llm_feed_consumer__llm_feed_consumer_ops_tests;

// ── actors/logger ──────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/logger/handle.tests.rs"]
mod nkc_actors__logger__handle_tests;
#[cfg(test)]
#[path = "../tests/actors/logger/logger_actor_ops.tests.rs"]
mod nkc_actors__logger__logger_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/logger/logger_actor.tests.rs"]
mod nkc_actors__logger__logger_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/logger/logger_ops.tests.rs"]
mod nkc_actors__logger__logger_ops_tests;

// ── actors/session ─────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/session/session_actor_ops.tests.rs"]
mod nkc_actors__session__session_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/session/session_actor.tests.rs"]
mod nkc_actors__session__session_actor_tests;

// ── actors/supervisor ──────────────────────────────────────────────────────────

// ── actors/token_tracker ───────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/token_tracker/handle.tests.rs"]
mod nkc_actors__token_tracker__handle_tests;
#[cfg(test)]
#[path = "../tests/actors/token_tracker/token_tracker_actor_ops.tests.rs"]
mod nkc_actors__token_tracker__token_tracker_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/token_tracker/token_tracker_actor.tests.rs"]
mod nkc_actors__token_tracker__token_tracker_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/token_tracker/token_tracker_ops.tests.rs"]
mod nkc_actors__token_tracker__token_tracker_ops_tests;

// ── actors/tool ────────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/tool/tool_actor_ops.tests.rs"]
mod nkc_actors__tool__tool_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/tool/tool_actor.tests.rs"]
mod nkc_actors__tool__tool_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/tool/tool_ops.tests.rs"]
mod nkc_actors__tool__tool_ops_tests;

// ── actors/user_message_consumer ───────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/user_message_consumer/user_message_consumer_actor_ops.tests.rs"]
mod nkc_actors__user_message_consumer__user_message_consumer_actor_ops_tests;

// ── compile_fail ───────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/compile_fail/hybrid_intent_action_routing.tests.rs"]
mod nkc_compile_fail__hybrid_intent_action_routing_tests;

// ── config ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/config/endpoint_catalog_discovery.tests.rs"]
mod nkc_config__endpoint_catalog_discovery_tests;
#[cfg(test)]
#[path = "../tests/config/loader.tests.rs"]
mod nkc_config__loader_tests;
#[cfg(test)]
#[path = "../tests/config/program_settings.tests.rs"]
mod nkc_config__program_settings_tests;
#[cfg(test)]
#[path = "../tests/config/types.tests.rs"]
mod nkc_config__types_tests;
#[cfg(test)]
#[path = "../tests/config/user_settings.tests.rs"]
mod nkc_config__user_settings_tests;

// ── domain ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/domain/agent_spec_parser.tests.rs"]
mod nkc_domain__agent_spec_parser_tests;
#[cfg(test)]
#[path = "../tests/domain/background_events_priority.tests.rs"]
mod nkc_domain__background_events_priority_tests;
#[cfg(test)]
#[path = "../tests/domain/background_events.tests.rs"]
mod nkc_domain__background_events_tests;
#[cfg(test)]
#[path = "../tests/domain/channels.tests.rs"]
mod nkc_domain__channels_tests;
#[cfg(test)]
#[path = "../tests/domain/context_management_algorithm_integration.tests.rs"]
mod nkc_domain__context_management_algorithm_integration_tests;
#[cfg(test)]
#[path = "../tests/domain/context_management.tests.rs"]
mod nkc_domain__context_management_tests;
#[cfg(test)]
#[path = "../tests/domain/dag_validation.tests.rs"]
mod nkc_domain__dag_validation_tests;
#[cfg(test)]
#[path = "../tests/domain/effort_level.tests.rs"]
mod nkc_domain__effort_level_tests;
#[cfg(test)]
#[path = "../tests/domain/feeds.tests.rs"]
mod nkc_domain__feeds_tests;
#[cfg(test)]
#[path = "../tests/domain/newtypes.tests.rs"]
mod nkc_domain__newtypes_tests;
#[cfg(test)]
#[path = "../tests/domain/plan_state.tests.rs"]
mod nkc_domain__plan_state_tests;
#[cfg(test)]
#[path = "../tests/domain/scheduler.tests.rs"]
mod nkc_domain__scheduler_tests;
#[cfg(test)]
#[path = "../tests/domain/stream_state.tests.rs"]
mod nkc_domain__stream_state_tests;
#[cfg(test)]
#[path = "../tests/domain/string_newtypes.tests.rs"]
mod nkc_domain__string_newtypes_tests;
#[cfg(test)]
#[path = "../tests/domain/thinking_mode.tests.rs"]
mod nkc_domain__thinking_mode_tests;
#[cfg(test)]
#[path = "../tests/domain/tool_types.tests.rs"]
mod nkc_domain__tool_types_tests;
#[cfg(test)]
#[path = "../tests/domain/types.tests.rs"]
mod nkc_domain__types_tests;

// ── domain/events ──────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/domain/events/contracts.tests.rs"]
mod nkc_domain__events__contracts_tests;
#[cfg(test)]
#[path = "../tests/domain/events/inventory_routing.tests.rs"]
mod nkc_domain__events__inventory_routing_tests;
#[cfg(test)]
#[path = "../tests/domain/events/inventory.tests.rs"]
mod nkc_domain__events__inventory_tests;
#[cfg(test)]
#[path = "../tests/domain/events/protocols.tests.rs"]
mod nkc_domain__events__protocols_tests;

// ── domain/support ─────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/domain/support/rustdoc.tests.rs"]
mod nkc_domain__support__rustdoc_tests;

// ── macros ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/macros.tests.rs"]
mod nkc_macros_tests;

// ── persistence ────────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/persistence/handle.tests.rs"]
mod nkc_persistence__handle_tests;
#[cfg(test)]
#[path = "../tests/persistence/plan_persistence.tests.rs"]
mod nkc_persistence__plan_persistence_tests;
#[cfg(test)]
#[path = "../tests/persistence/store.tests.rs"]
mod nkc_persistence__store_tests;
#[cfg(test)]
#[path = "../tests/persistence/types.tests.rs"]
mod nkc_persistence__types_tests;


// ── token_history ──────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/token_history.tests.rs"]
mod nkc_token_history_tests;

// ── tools/builtin ──────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/tools/builtin/file_append.tests.rs"]
mod nkc_tools__builtin__file_append_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/file_create.tests.rs"]
mod nkc_tools__builtin__file_create_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/file_insert.tests.rs"]
mod nkc_tools__builtin__file_insert_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/file_line_count.tests.rs"]
mod nkc_tools__builtin__file_line_count_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/file_read_range.tests.rs"]
mod nkc_tools__builtin__file_read_range_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/file_read.tests.rs"]
mod nkc_tools__builtin__file_read_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/file_remove.tests.rs"]
mod nkc_tools__builtin__file_remove_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/file_replace.tests.rs"]
mod nkc_tools__builtin__file_replace_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/file_slice.tests.rs"]
mod nkc_tools__builtin__file_slice_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/list_directory.tests.rs"]
mod nkc_tools__builtin__list_directory_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/query_user.tests.rs"]
mod nkc_tools__builtin__query_user_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/refresh_cache_file.tests.rs"]
mod nkc_tools__builtin__refresh_cache_file_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/request_rework.tests.rs"]
mod nkc_tools__builtin__request_rework_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/scoped_shell_exec.tests.rs"]
mod nkc_tools__builtin__scoped_shell_exec_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/set_working_file.tests.rs"]
mod nkc_tools__builtin__set_working_file_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/shell_exec.tests.rs"]
mod nkc_tools__builtin__shell_exec_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/size_check.tests.rs"]
mod nkc_tools__builtin__size_check_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/spawn_agent.tests.rs"]
mod nkc_tools__builtin__spawn_agent_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/sql_query.tests.rs"]
mod nkc_tools__builtin__sql_query_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/task_await.tests.rs"]
mod nkc_tools__builtin__task_await_tests;
#[cfg(test)]
#[path = "../tests/tools/builtin/task_status.tests.rs"]
mod nkc_tools__builtin__task_status_tests;

// ── tools ──────────────────────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/agent/assistant_core_refactored.tests.rs"]
mod nkc_actors__agent__assistant_core_refactored_tests;
#[cfg(test)]
#[path = "../tests/actors/agent/assistant_core.tests.rs"]
mod nkc_actors__agent__assistant_core_tests;
#[cfg(test)]
#[path = "../tests/tools/definition.tests.rs"]
mod nkc_tools__definition_tests;
#[cfg(test)]
#[path = "../tests/tools/handler.tests.rs"]
mod nkc_tools__handler_tests;
#[cfg(test)]
#[path = "../tests/tools/registry.tests.rs"]
mod nkc_tools__registry_tests; // ── dead files: actors/agent ────────────────────────────────────────────────────

// ── dead files: actors/cache ────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/cache/tiers.tests.rs"]
mod nkc_actors__cache__tiers_tests;

// ── dead files: actors/catalog_manager ──────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/catalog_manager/actor_ops.tests.rs"]
mod nkc_actors__catalog_manager__actor_ops_tests;

// ── dead files: actors/catalog_manager/models ───────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/catalog_manager/models/filter.tests.rs"]
mod nkc_actors__catalog_manager__models__filter_tests;
#[cfg(test)]
#[path = "../tests/actors/catalog_manager/models/formatter.tests.rs"]
mod nkc_actors__catalog_manager__models__formatter_tests;

// ── dead files: actors/command ──────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/command/actor.tests.rs"]
mod nkc_actors__command__actor_tests;
#[cfg(test)]
#[path = "../tests/actors/command/mod.tests.rs"]
mod nkc_actors__command__mod_tests;
#[cfg(test)]
#[path = "../tests/actors/command/registry.tests.rs"]
mod nkc_actors__command__registry_tests;
#[cfg(test)]
#[path = "../tests/actors/command/types.tests.rs"]
mod nkc_actors__command__types_tests;

// ── dead files: actors/deterministic_orchestrator ───────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/deterministic_orchestrator/artifact_store.tests.rs"]
mod nkc_actors__deterministic_orchestrator__artifact_store_tests;
#[cfg(test)]
#[path = "../tests/actors/deterministic_orchestrator/background_dispatch.tests.rs"]
mod nkc_actors__deterministic_orchestrator__background_dispatch_tests;
#[cfg(test)]
#[path = "../tests/actors/deterministic_orchestrator/commands.tests.rs"]
mod nkc_actors__deterministic_orchestrator__commands_tests;
#[cfg(test)]
#[path = "../tests/actors/deterministic_orchestrator/decision.tests.rs"]
mod nkc_actors__deterministic_orchestrator__decision_tests;
#[cfg(test)]
#[path = "../tests/actors/deterministic_orchestrator/handle.tests.rs"]
mod nkc_actors__deterministic_orchestrator__handle_tests;
#[cfg(test)]
#[path = "../tests/actors/deterministic_orchestrator/loader.tests.rs"]
mod nkc_actors__deterministic_orchestrator__loader_tests;

// ── dead files: actors/deterministic_orchestrator/deterministic_orchestrator_actor
#[cfg(test)]
#[path = "../tests/actors/deterministic_orchestrator/deterministic_orchestrator_actor/deterministic_orchestrator_actor.tests.rs"]
mod nkc_actors__deterministic_orchestrator__deterministic_orchestrator_actor__deterministic_orchestrator_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/deterministic_orchestrator/deterministic_orchestrator_actor/deterministic_orchestrator_ops.tests.rs"]
mod nkc_actors__deterministic_orchestrator__deterministic_orchestrator_actor__deterministic_orchestrator_ops_tests;

// ── dead files: actors/executor ─────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/executor/commands.tests.rs"]
mod nkc_actors__executor__commands_tests;
#[cfg(test)]
#[path = "../tests/actors/executor/event_mapper.tests.rs"]
mod nkc_actors__executor__event_mapper_tests;
#[cfg(test)]
#[path = "../tests/actors/executor/executor_actor_ops.tests.rs"]
mod nkc_actors__executor__executor_actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/executor/executor_actor.tests.rs"]
mod nkc_actors__executor__executor_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/executor/executor_ops/integration.tests.rs"]
mod nkc_actors__executor__executor_ops__integration_tests;
#[cfg(test)]
#[path = "../tests/actors/executor/executor_ops.tests.rs"]
mod nkc_actors__executor__executor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/executor/handle.tests.rs"]
mod nkc_actors__executor__handle_tests;

// ── dead files: actors/file_scanner ─────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/file_scanner/commands.tests.rs"]
mod nkc_actors__file_scanner__commands_tests;
#[cfg(test)]
#[path = "../tests/actors/file_scanner/file_scanner_actor.tests.rs"]
mod nkc_actors__file_scanner__file_scanner_actor_tests;
#[cfg(test)]
#[path = "../tests/actors/file_scanner/mod.tests.rs"]
mod nkc_actors__file_scanner__mod_tests;

// ── dead files: actors/history_adapter ──────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/history_adapter/handle.tests.rs"]
mod nkc_actors__history_adapter__handle_tests;

// ── dead files: actors/llm ──────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/llm/actor_ops.tests.rs"]
mod nkc_actors__llm__actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/llm/actor.tests.rs"]
mod nkc_actors__llm__actor_tests;
#[cfg(test)]
#[path = "../tests/actors/llm/discovery.tests.rs"]
mod nkc_actors__llm__discovery_tests;
#[cfg(test)]
#[path = "../tests/actors/llm/handle.tests.rs"]
mod nkc_actors__llm__handle_tests;
#[cfg(test)]
#[path = "../tests/actors/llm/ops.tests.rs"]
mod nkc_actors__llm__ops_tests;
#[cfg(test)]
#[path = "../tests/actors/llm/providers/shared.tests.rs"]
mod nkc_actors__llm__providers__shared_tests;

// ── dead files: actors/llm_feed_consumer ────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/llm_feed_consumer/handle.tests.rs"]
mod nkc_actors__llm_feed_consumer__handle_tests;

// ── dead files: actors/lsp ──────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/lsp/actor_ops.tests.rs"]
mod nkc_actors__lsp__actor_ops_tests;
#[cfg(test)]
#[path = "../tests/actors/lsp/actor.tests.rs"]
mod nkc_actors__lsp__actor_tests;
#[cfg(test)]
#[path = "../tests/actors/lsp/handle.tests.rs"]
mod nkc_actors__lsp__handle_tests;

// ── dead files: actors/mod ──────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/mod.tests.rs"]
mod nkc_actors__mod_tests;

// ── dead files: actors/orchestrator ─────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/orchestrator/ingestion.tests.rs"]
mod nkc_actors__orchestrator__ingestion_tests;
#[cfg(test)]
#[path = "../tests/actors/orchestrator/timeout.tests.rs"]
mod nkc_actors__orchestrator__timeout_tests;

// ── dead files: actors/session ──────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/session/handle.tests.rs"]
mod nkc_actors__session__handle_tests;
#[cfg(test)]
#[path = "../tests/actors/session/mod.tests.rs"]
mod nkc_actors__session__mod_tests;
#[cfg(test)]
#[path = "../tests/actors/session/ops.tests.rs"]
mod nkc_actors__session__ops_tests;

// ── dead files: actors/tool ─────────────────────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/tool/handle.tests.rs"]
mod nkc_actors__tool__handle_tests;
#[cfg(test)]
#[path = "../tests/actors/tool/inline_executor.tests.rs"]
mod nkc_actors__tool__inline_executor_tests;
#[cfg(test)]
#[path = "../tests/actors/tool/mod.tests.rs"]
mod nkc_actors__tool__mod_tests;

// ── dead files: actors/user_message_consumer ────────────────────────────────────
#[cfg(test)]
#[path = "../tests/actors/user_message_consumer/actor.tests.rs"]
mod nkc_actors__user_message_consumer__actor_tests;
#[cfg(test)]
#[path = "../tests/actors/user_message_consumer/handle.tests.rs"]
mod nkc_actors__user_message_consumer__handle_tests;
#[cfg(test)]
#[path = "../tests/actors/user_message_consumer/ops.tests.rs"]
mod nkc_actors__user_message_consumer__ops_tests;
