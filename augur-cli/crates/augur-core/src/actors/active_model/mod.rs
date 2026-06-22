//! Active-model actor: data-provider actor for the currently selected model.
//!
//! Stores the model chosen in the main chat and makes it available to background
//! task runners so that spawned OpenRouter agents use the same model as the user.
//! Uses a watch channel for zero-cost synchronous reads and an mpsc channel for
//! fire-and-forget `Set` writes. No `Arc<RwLock<>>`.

/// Actor task that owns the model watch sender.
pub mod active_model_actor;
/// Private helper operations for the active-model actor.
mod active_model_actor_ops;
/// Command types processed by the active-model actor.
pub mod active_model_ops;
/// Public handle for reading and updating the current model.
pub mod handle;

pub use active_model_actor::spawn;
pub use handle::ActiveModelHandle;
