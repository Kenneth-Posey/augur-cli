//! `SupervisorActor` - lightweight event-channel owner.

use tokio::sync::{broadcast, mpsc};

use augur_domain::domain::channels::SUPERVISOR_COMMAND_CAPACITY;
use augur_domain::domain::types::SupervisorEvent;

use super::commands::SupervisorCmd;
use super::handle::{make_event_channel, SupervisorHandle};

pub struct SupervisorActor;

impl SupervisorActor {
    pub fn spawn() -> SupervisorHandle {
        let event_tx = make_event_channel();
        let (cmd_tx, cmd_rx) = mpsc::channel::<SupervisorCmd>(*SUPERVISOR_COMMAND_CAPACITY);
        let handle = SupervisorHandle::new(cmd_tx, event_tx.clone());
        tokio::spawn(run(event_tx, cmd_rx));
        handle
    }
}

async fn run(
    _event_tx: broadcast::Sender<SupervisorEvent>,
    mut cmd_rx: mpsc::Receiver<SupervisorCmd>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        if matches!(cmd, SupervisorCmd::Stop) {
            break;
        }
    }
}
