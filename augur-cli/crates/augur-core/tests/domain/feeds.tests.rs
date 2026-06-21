use augur_domain::domain::feeds::{
    HistoryFeedMessage, LlmFeedMessage, LlmFeedTag, UserFeedMessage, UserInputTag,
};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::domain::types::{Message, Role, StreamChunk};

/// Verifies that LlmFeedMessage can be constructed with UserChunk tag and Done chunk.
#[test]
fn test_llm_feed_message_construction() {
    let msg = LlmFeedMessage {
        tag: LlmFeedTag::UserChunk,
        chunk: StreamChunk::Done,
    };
    assert_eq!(msg.tag, LlmFeedTag::UserChunk);
    assert_eq!(msg.chunk, StreamChunk::Done);
}

/// Verifies that UserFeedMessage can be constructed with RawCommand tag and text.
#[test]
fn test_user_feed_message_construction() {
    let msg = UserFeedMessage {
        tag: UserInputTag::RawCommand,
        text: OutputText::new("hello"),
    };
    assert_eq!(msg.tag, UserInputTag::RawCommand);
    assert_eq!(msg.text.as_str(), "hello");
}

/// Verifies that HistoryFeedMessage::UserEntry holds a user-role Message.
#[test]
fn test_history_feed_message_user_variant() {
    let message = Message::user("test input");
    let feed = HistoryFeedMessage::UserEntry(message);
    match feed {
        HistoryFeedMessage::UserEntry(m) => assert_eq!(m.role, Role::User),
        HistoryFeedMessage::LlmEntry(_) => panic!("expected UserEntry"),
    }
}

/// Verifies that HistoryFeedMessage::LlmEntry holds an assistant-role Message.
#[test]
fn test_history_feed_message_llm_variant() {
    let message = Message::assistant(OutputText::new("response text"));
    let feed = HistoryFeedMessage::LlmEntry(message);
    match feed {
        HistoryFeedMessage::LlmEntry(m) => assert_eq!(m.role, Role::Assistant),
        HistoryFeedMessage::UserEntry(_) => panic!("expected LlmEntry"),
    }
}
