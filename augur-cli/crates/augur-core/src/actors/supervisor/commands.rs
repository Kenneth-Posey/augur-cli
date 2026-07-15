//! Commands accepted by the `SupervisorActor` command channel.

/// Commands sent to the running `SupervisorActor` via its command channel.
#[derive(Debug)]
pub enum SupervisorCmd {
    /// Shut down the supervisor actor task.
    Stop,
}
