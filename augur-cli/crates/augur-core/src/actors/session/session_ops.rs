//! Commands processed by the session actor.

use augur_domain::domain::string_newtypes::EndpointName;

/// Commands accepted by the session actor's mpsc channel.
pub enum SessionCommand {
    /// Change the currently active endpoint to the given name.
    SetEndpoint(EndpointName),
    /// Stop the session actor task.
    Shutdown,
}
