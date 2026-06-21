use crate::domain::string_newtypes::ModelId;
use tokio::sync::{mpsc, watch};

#[derive(Clone, Debug)]
pub enum ActiveModelCommand {
    Set(ModelId),
}

#[derive(Clone)]
pub struct ActiveModelHandle {
    tx: mpsc::Sender<ActiveModelCommand>,
    rx: watch::Receiver<Option<ModelId>>,
}

impl ActiveModelHandle {
    pub fn new(tx: mpsc::Sender<ActiveModelCommand>, rx: watch::Receiver<Option<ModelId>>) -> Self {
        Self { tx, rx }
    }

    pub fn set_model(&self, model_id: ModelId) {
        let _ = self.tx.try_send(ActiveModelCommand::Set(model_id));
    }

    pub fn current_model(&self) -> Option<ModelId> {
        self.rx.borrow().clone()
    }
}
