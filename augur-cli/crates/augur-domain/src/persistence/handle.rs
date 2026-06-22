//! `PersistenceHandle`: async wrapper for session auto-save.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::domain::newtypes::TimestampMs;
use crate::domain::string_newtypes::{EndpointName, SdkSessionId, SessionId, StringNewtype};
use crate::domain::types::Message;
use crate::domain::IsPredicate;
use crate::persistence::store;
use crate::persistence::types::{
    MessageRecord, SessionMeta, SessionMetaFlags, SessionRecord, SessionState,
};

#[derive(bon::Builder)]
struct SessionIdentity {
    session_id: SessionId,
    created_at: TimestampMs,
    sdk_session_id: Option<SdkSessionId>,
    #[builder(default)]
    ask_session: IsPredicate,
}

#[derive(bon::Builder)]
struct PersistenceInner {
    identity: SessionIdentity,
    dir: PathBuf,
    #[builder(default)]
    queued_commands: Vec<MessageRecord>,
    openrouter_context_history: Option<Vec<Message>>,
}

#[derive(Clone)]
pub struct PersistenceHandle {
    inner: Arc<Mutex<PersistenceInner>>,
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl PersistenceHandle {
    pub fn new(dir: PathBuf) -> Self {
        Self::with_session_id(dir, SessionId::new(uuid::Uuid::new_v4().to_string()))
    }

    pub fn with_session_id(dir: PathBuf, session_id: SessionId) -> Self {
        let created_at = TimestampMs::now();
        PersistenceHandle {
            inner: Arc::new(Mutex::new(
                PersistenceInner::builder()
                    .identity(
                        SessionIdentity::builder()
                            .session_id(session_id)
                            .created_at(created_at)
                            .build(),
                    )
                    .dir(dir)
                    .build(),
            )),
        }
    }

    pub fn session_id(&self) -> SessionId {
        let g = lock_or_recover(&self.inner);
        g.identity.session_id.clone()
    }

    pub fn sessions_dir(&self) -> PathBuf {
        let g = lock_or_recover(&self.inner);
        g.dir.clone()
    }

    pub fn sdk_session_id(&self) -> Option<SdkSessionId> {
        let g = lock_or_recover(&self.inner);
        g.identity.sdk_session_id.clone()
    }

    pub fn set_sdk_session_id(&self, id: SdkSessionId) {
        let mut g = lock_or_recover(&self.inner);
        g.identity.sdk_session_id = Some(id);
    }

    pub fn restore_from(&self, record: &SessionRecord) {
        let mut g = lock_or_recover(&self.inner);
        g.identity.session_id = record.meta.id.clone();
        g.identity.created_at = record.meta.created_at;
        g.identity.sdk_session_id = record.meta.flags.sdk_session_id.clone();
        g.openrouter_context_history = record.state.openrouter_context_history.clone();
    }

    pub fn reset_to_new_session(&self) {
        let mut g = lock_or_recover(&self.inner);
        g.identity.session_id = SessionId::new(uuid::Uuid::new_v4().to_string());
        g.identity.created_at = TimestampMs::now();
        g.identity.sdk_session_id = None;
        g.identity.ask_session = false.into();
        g.openrouter_context_history = None;
    }

    pub fn queue_user_command(&self, record: MessageRecord) {
        let mut g = lock_or_recover(&self.inner);
        g.queued_commands.push(record);
    }

    pub fn mark_as_ask_session(&self) {
        let mut g = lock_or_recover(&self.inner);
        g.identity.ask_session = true.into();
    }

    pub fn set_openrouter_context_history(&self, messages: Vec<Message>) {
        let mut g = lock_or_recover(&self.inner);
        g.openrouter_context_history = Some(messages);
    }

    pub fn clear_openrouter_context_history(&self) {
        let mut g = lock_or_recover(&self.inner);
        g.openrouter_context_history = None;
    }

    pub fn openrouter_context_history(&self) -> Option<Vec<Message>> {
        let g = lock_or_recover(&self.inner);
        g.openrouter_context_history.clone()
    }

    pub async fn save_turn(&self, endpoint: EndpointName, messages: Vec<MessageRecord>) {
        let (record, dir) = self.build_record(endpoint, messages);
        let result = tokio::task::spawn_blocking(move || store::save_session(&record, &dir)).await;
        if let Ok(Err(e)) = result {
            tracing::warn!(error = %e, "session save failed");
        }
    }

    fn build_record(
        &self,
        endpoint: EndpointName,
        messages: Vec<MessageRecord>,
    ) -> (SessionRecord, PathBuf) {
        let mut g = lock_or_recover(&self.inner);
        let now = TimestampMs::now();
        let dir = g.dir.clone();
        let queued = std::mem::take(&mut g.queued_commands);
        let merged = if queued.is_empty() {
            messages
        } else {
            let mut all = Vec::with_capacity(messages.len() + queued.len());
            all.extend(messages);
            all.extend(queued);
            all.sort_by_key(|r| r.message.timestamp);
            all
        };
        let record = SessionRecord {
            meta: SessionMeta {
                id: g.identity.session_id.clone(),
                created_at: g.identity.created_at,
                last_updated_at: now,
                endpoint_name: endpoint,
                flags: SessionMetaFlags {
                    sdk_session_id: g.identity.sdk_session_id.clone(),
                    ask_session: g.identity.ask_session,
                },
            },
            state: SessionState {
                messages: merged,
                openrouter_context_history: g.openrouter_context_history.clone(),
                current_strategy: None,
            },
        };
        (record, dir)
    }
}
