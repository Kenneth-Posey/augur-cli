use crate::domain::types::Message;
use crate::domain::{Count, NumericNewtype, OutputText};

pub struct ConversationHistory {
    system_prompt: OutputText,
    messages: Vec<Message>,
    openrouter_context_messages: Vec<Message>,
    live_offset: usize,
}

pub mod history {
    pub use super::ConversationHistory;
}

impl ConversationHistory {
    pub fn new(system_prompt: OutputText) -> Self {
        Self {
            system_prompt,
            messages: vec![],
            openrouter_context_messages: vec![],
            live_offset: 0,
        }
    }

    pub fn push(&mut self, message: Message) {
        self.push_conversation(message.clone());
        self.push_openrouter_context(message);
    }

    pub fn push_conversation(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn push_openrouter_context(&mut self, message: Message) {
        self.openrouter_context_messages.push(message);
    }

    pub fn messages_for_request(&self) -> Vec<Message> {
        let mut result = Vec::with_capacity(self.messages.len() + 1);
        result.push(Message::system(self.system_prompt.clone()));
        result.extend(self.messages.iter().cloned());
        result
    }

    pub fn openrouter_context_messages_for_request(&self) -> Vec<Message> {
        let mut result = Vec::with_capacity(self.openrouter_context_messages.len() + 1);
        result.push(Message::system(self.system_prompt.clone()));
        result.extend(self.openrouter_context_messages.iter().cloned());
        result
    }

    pub fn live_messages_for_request(&self) -> Vec<Message> {
        let live = &self.messages[self.live_offset..];
        let mut result = Vec::with_capacity(live.len() + 1);
        result.push(Message::system(self.system_prompt.clone()));
        result.extend(live.iter().cloned());
        result
    }

    pub fn len(&self) -> Count {
        Count::new(self.messages.len())
    }

    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn openrouter_context_messages(&self) -> &[Message] {
        &self.openrouter_context_messages
    }

    /// Replace all conversation messages and the OpenRouter context messages.
    ///
    /// The first message in `compact_messages` (if it is a `Role::System` message)
    /// is used as the new system prompt. All subsequent messages replace the
    /// conversation history. The `live_offset` is reset to the length of the new
    /// message list so future turns are appended from the compacted state.
    /// The `openrouter_context_messages` are also replaced with the full set.
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        let mut remaining = messages;
        // If the first message is a system prompt, store it as the system prompt
        // and remove it from the message list.
        let is_system = remaining
            .first()
            .map(|m| matches!(m.role, crate::domain::types::Role::System))
            .unwrap_or(false);
        if is_system {
            if let Some(system) = remaining.first().cloned() {
                self.system_prompt = system.content;
            }
            remaining.remove(0);
        }
        self.messages = remaining.clone();
        self.openrouter_context_messages = remaining;
        self.live_offset = self.messages.len();
    }

    pub fn from_messages(system_prompt: OutputText, messages: Vec<Message>) -> Self {
        Self::from_messages_with_openrouter_context(system_prompt, messages, None)
    }

    pub fn from_messages_with_openrouter_context(
        system_prompt: OutputText,
        messages: Vec<Message>,
        openrouter_context_messages: Option<Vec<Message>>,
    ) -> Self {
        let live_offset = messages.len();
        Self {
            system_prompt,
            messages,
            openrouter_context_messages: openrouter_context_messages.unwrap_or_default(),
            live_offset,
        }
    }
}
