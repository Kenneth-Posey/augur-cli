use augur_core::actors::agent::history::ConversationHistory;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolCallId, ToolName};
use augur_domain::domain::types::{Message, Role};
use augur_domain::domain::{Count, NumericNewtype};

#[test]
fn new_history_is_empty() {
    let h = ConversationHistory::new(OutputText::new("SYS"));
    assert_eq!(h.len(), Count::ZERO);
}

#[test]
fn push_appends_message() {
    let mut h = ConversationHistory::new(OutputText::new("SYS"));
    h.push(Message::user("hello"));
    assert_eq!(h.len(), Count::of(1));
}

#[test]
fn len_returns_count_newtype() {
    let h = ConversationHistory::new(OutputText::new("SYS"));
    let len = h.len();
    assert_eq!(
        std::any::type_name_of_val(&len),
        std::any::type_name::<Count>(),
    );
}

#[test]
fn messages_for_request_prepends_system_prompt() {
    let mut h = ConversationHistory::new(OutputText::new("SYS"));
    h.push(Message::user("hello"));
    let msgs = h.messages_for_request();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, Role::System);
}

#[test]
fn messages_for_request_system_prompt_text_matches() {
    let h = ConversationHistory::new(OutputText::new("SYS"));
    let msgs = h.messages_for_request();
    assert_eq!(msgs[0].content, OutputText::new("SYS"));
}

#[test]
fn messages_for_request_empty_history_returns_system_only() {
    let h = ConversationHistory::new(OutputText::new("ONLY_SYS"));
    let msgs = h.messages_for_request();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].role, Role::System);
}

#[test]
fn from_messages_live_offset_excludes_restored() {
    let restored = vec![
        Message::user("old cmd 1"),
        Message::assistant("old reply 1"),
    ];
    let h = ConversationHistory::from_messages(OutputText::new("SYS"), restored);
    let live = h.live_messages_for_request();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].role, Role::System);
}

#[test]
fn new_history_live_messages_matches_all_messages() {
    let mut h = ConversationHistory::new(OutputText::new("SYS"));
    h.push(Message::user("hello"));
    h.push(Message::assistant("hi"));
    let live = h.live_messages_for_request();
    let all = h.messages_for_request();
    assert_eq!(live.len(), all.len());
    for (l, a) in live.iter().zip(all.iter()) {
        assert_eq!(l.role, a.role);
        assert_eq!(l.content, a.content);
    }
}

#[test]
fn live_messages_for_request_includes_only_post_restore_messages() {
    let restored = vec![
        Message::user("old cmd"),
        Message::assistant("old reply"),
        Message::user("another old cmd"),
    ];
    let mut h = ConversationHistory::from_messages(OutputText::new("SYS"), restored);
    h.push(Message::user("hello"));

    let live = h.live_messages_for_request();
    assert_eq!(live.len(), 2);
    assert_eq!(live[0].role, Role::System);
    assert_eq!(live[1].content, OutputText::new("hello"));
    assert_eq!(h.messages_for_request().len(), 5);
}

#[test]
fn live_messages_for_request_always_has_system_prompt() {
    let h =
        ConversationHistory::from_messages(OutputText::new("MY_SYS"), vec![Message::user("old")]);
    let live = h.live_messages_for_request();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].role, Role::System);
    assert_eq!(live[0].content, OutputText::new("MY_SYS"));
}

#[test]
fn openrouter_context_history_can_diverge_from_conversation() {
    let mut h = ConversationHistory::new(OutputText::new("SYS"));
    h.push_conversation(Message::tool_result(
        ToolCallId::new("tool_call_1"),
        &ToolName::new("file_read"),
        OutputText::new("raw output"),
    ));
    h.push_openrouter_context(Message::tool_result(
        ToolCallId::new("tool_call_1"),
        &ToolName::new("file_read"),
        OutputText::new("warning output"),
    ));

    assert_eq!(h.messages().len(), 1);
    assert_eq!(h.openrouter_context_messages().len(), 1);
    assert!(h.messages()[0].content.as_str().contains("raw output"));
    assert!(
        h.openrouter_context_messages()[0]
            .content
            .as_str()
            .contains("warning output")
    );
}

#[test]
fn from_messages_with_openrouter_context_uses_provided_context() {
    let conversation = vec![Message::user("conversation")];
    let context = vec![Message::assistant("context")];
    let h = ConversationHistory::from_messages_with_openrouter_context(
        OutputText::new("SYS"),
        conversation,
        Some(context),
    );
    assert_eq!(h.messages().len(), 1);
    assert_eq!(h.openrouter_context_messages().len(), 1);
    assert_eq!(
        h.openrouter_context_messages()[0].content,
        OutputText::new("context")
    );
}
