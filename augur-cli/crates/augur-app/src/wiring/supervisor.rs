use super::{SupervisorRuntime, TaskJoin};
use augur_core::plan_store::PlanTreeStore;
use augur_domain::config::types::ExecutorConfig;

/// Optionally spawns an executor and supervisor, returning a broadcast receiver
/// for supervisor events, the executor actor's join handle, and the supervisor
/// handle.
///
/// Spawns both actors and returns the supervisor event receiver plus joins/handles.
///
/// `PlanTreeStore` is constructed internally; the supervisor is the sole owner.
pub async fn wire_supervisor(
    config: &ExecutorConfig,
) -> (
    Option<tokio::sync::broadcast::Receiver<augur_domain::domain::types::SupervisorEvent>>,
    Option<TaskJoin>,
    Option<augur_core::actors::SupervisorHandle>,
) {
    let (executor_join, executor_handle) = spawn_executor(config).await;
    let store = PlanTreeStore::default();
    let supervisor_handle = spawn_supervisor(executor_handle, store);
    let rx = supervisor_handle.subscribe_events();
    (Some(rx), Some(executor_join), Some(supervisor_handle))
}

/// Spawn the optional supervisor and executor actors and return a [`SupervisorRuntime`].
///
/// Delegates to [`wire_supervisor`] with the executor sub-config from
/// `config.copilot.executor`. The returned [`SupervisorRuntime`] contains
/// optional join and handle fields.
pub async fn spawn_supervisor_runtime(
    config: &augur_domain::config::types::AppConfig,
) -> SupervisorRuntime {
    let (rx, join, handle) = wire_supervisor(&config.copilot.executor).await;
    SupervisorRuntime { rx, join, handle }
}

/// Spawn an `ExecutorActor` and return its join handle and handle.
///
/// Passes the executor config (CLI path, model, auth token) to the actor task.
async fn spawn_executor(
    config: &ExecutorConfig,
) -> (TaskJoin, augur_provider_copilot_sdk::actors::ExecutorHandle) {
    augur_provider_copilot_sdk::actors::executor::executor_actor::spawn(config.clone()).await
}

/// Spawn a `SupervisorActor` and return its handle.
///
/// The supervisor holds the plan tree store and drives the executor through plan steps.
fn spawn_supervisor(
    executor: augur_provider_copilot_sdk::actors::ExecutorHandle,
    store: PlanTreeStore,
) -> augur_core::actors::SupervisorHandle {
    augur_core::actors::supervisor::supervisor_actor::SupervisorActor::spawn(
        Box::new(executor),
        store,
    )
}
