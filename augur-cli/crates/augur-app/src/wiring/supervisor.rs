use super::{SupervisorRuntime, TaskJoin};
use augur_domain::config::types::ExecutorConfig;

/// Guided-plan runtime is removed, so no supervisor or executor is spawned.
pub async fn wire_supervisor(
    _config: &ExecutorConfig,
) -> (
    Option<tokio::sync::broadcast::Receiver<augur_domain::domain::types::SupervisorEvent>>,
    Option<TaskJoin>,
    Option<augur_core::actors::SupervisorHandle>,
) {
    (None, None, None)
}

pub async fn spawn_supervisor_runtime(
    config: &augur_domain::config::types::AppConfig,
) -> SupervisorRuntime {
    let (rx, join, handle) = wire_supervisor(&config.copilot.executor).await;
    SupervisorRuntime { rx, join, handle }
}
