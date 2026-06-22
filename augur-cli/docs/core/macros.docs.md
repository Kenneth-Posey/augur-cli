# Macros Module

The `macros` module (`macros.rs`) provides four utility macros that simplify common Rust synchronization and trait-composition patterns across the crate. These are `#[macro_export]` macros available to any downstream crate that depends on `augur-core`.

## Macros

**`trait_alias!`** creates a trait alias on stable Rust by generating a new supertrait with a blanket implementation. It accepts visibility modifiers, doc comments, and arbitrary trait bounds, making it useful for combining up to five traits into a single bound without waiting for the unstable `trait_alias` feature. **`lock_or_recover!`** acquires a `std::sync::Mutex` guard, recovering from a poisoned lock by consuming the inner value. **`read_or_recover!`** and **`write_or_recover!`** do the same for `std::sync::RwLock` shared and exclusive guards respectively.

## Architectural Role

These macros are a small but important part of the crate's concurrency hygiene. The lock-recovery macros eliminate the repetitive `lock().unwrap_or_else(|p| p.into_inner())` pattern that would otherwise appear at every mutex or rwlock acquisition site. The `trait_alias!` macro enables type-level composition that would otherwise require verbose bound repetition, keeping function signatures readable across actor boundaries and generic interfaces.