#![allow(dead_code, unused_imports)]

//! Core workspace crate for domain models, actors, persistence, and tools.

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
/// Plan storage helpers and backing directories.
pub mod plan_store;
/// Token history tracking for chat and review flows.
pub mod token_history;
/// Tool registry, built-ins, and tool execution support.
pub mod tools;
