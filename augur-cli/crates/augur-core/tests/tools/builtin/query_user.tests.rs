use augur_core::tools::builtin::query_user::QueryUserTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::{ChoiceText, OutputText, StringNewtype};
use augur_domain::tools::builtin::query_user::QueryUserRequest;

#[test]
fn query_user_definition_has_required_fields() {
    let (tx, _rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(1);
    let tool = QueryUserTool::new(tx);
    let def = tool.definition();
    assert_eq!(def.name.as_str(), "query_user");
    let props = &def.parameters["properties"];
    assert_eq!(props["question"]["type"], "string");
    assert_eq!(props["choices"]["type"], "array");
    assert_eq!(props["choices"]["items"]["type"], "string");
    let required = def.parameters["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("question")));
    assert!(!required.iter().any(|v| v.as_str() == Some("choices")));
}

#[tokio::test]
async fn query_user_execute_sends_request_and_returns_reply() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(1);
    let tool = QueryUserTool::new(tx);
    tokio::spawn(async move {
        if let Some(req) = rx.recv().await {
            assert_eq!(req.question.as_str(), "Are you sure?");
            let _ = req.reply_tx.send(OutputText::new("yes"));
        }
    });
    let args = serde_json::json!({"question": "Are you sure?", "choices": ["yes", "no"]});
    let result = tool.execute(args).await;
    assert!(!result.is_error);
    assert_eq!(result.output.as_str(), "yes");
}

#[tokio::test]
async fn query_user_execute_no_choices_still_works() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(1);
    let tool = QueryUserTool::new(tx);
    tokio::spawn(async move {
        if let Some(req) = rx.recv().await {
            assert!(req.choices.is_empty());
            let _ = req.reply_tx.send(OutputText::new("free-form response"));
        }
    });
    let args = serde_json::json!({"question": "Tell me something", "choices": []});
    let result = tool.execute(args).await;
    assert!(!result.is_error);
    assert_eq!(result.output.as_str(), "free-form response");
}

#[tokio::test]
async fn query_user_execute_omitted_choices_still_works() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(1);
    let tool = QueryUserTool::new(tx);
    tokio::spawn(async move {
        if let Some(req) = rx.recv().await {
            assert!(req.choices.is_empty());
            let _ = req.reply_tx.send(OutputText::new("typed answer"));
        }
    });
    let result = tool
        .execute(serde_json::json!({"question": "Tell me something"}))
        .await;
    assert!(!result.is_error);
    assert_eq!(result.output.as_str(), "typed answer");
}

#[tokio::test]
async fn query_user_execute_non_array_choices_are_ignored() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(1);
    let tool = QueryUserTool::new(tx);
    tokio::spawn(async move {
        if let Some(req) = rx.recv().await {
            assert!(req.choices.is_empty());
            let _ = req.reply_tx.send(OutputText::new("typed answer"));
        }
    });
    let result = tool
        .execute(serde_json::json!({"question": "Tell me something", "choices": null}))
        .await;
    assert!(!result.is_error);
    assert_eq!(result.output.as_str(), "typed answer");
}

#[tokio::test]
async fn query_user_execute_filters_empty_choice_strings() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(1);
    let tool = QueryUserTool::new(tx);
    let verifier = tokio::spawn(async move {
        let req = rx.recv().await.expect("request should be sent");
        assert_eq!(
            req.choices,
            vec![ChoiceText::new("yes"), ChoiceText::new("no")]
        );
        let _ = req.reply_tx.send(OutputText::new("yes"));
    });
    let args = serde_json::json!({
        "question": "Are you sure?",
        "choices": ["yes", "", "no"]
    });
    let result = tool.execute(args).await;
    verifier.await.expect("choice verifier should not panic");
    assert!(!result.is_error);
    assert_eq!(result.output.as_str(), "yes");
}

#[tokio::test]
async fn query_user_execute_missing_question_returns_error() {
    let (tx, _rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(1);
    let tool = QueryUserTool::new(tx);
    let args = serde_json::json!({});
    let result = tool.execute(args).await;
    assert!(result.is_error);
}

#[tokio::test]
async fn query_user_execute_empty_question_returns_error() {
    let (tx, _rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(1);
    let tool = QueryUserTool::new(tx);
    let result = tool
        .execute(serde_json::json!({"question": "", "choices": ["yes"]}))
        .await;
    assert!(result.is_error);
}

#[tokio::test]
async fn query_user_execute_returns_error_when_query_channel_closed() {
    let (tx, rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(1);
    drop(rx);
    let tool = QueryUserTool::new(tx);
    let result = tool
        .execute(serde_json::json!({"question": "Still there?"}))
        .await;
    assert!(result.is_error);
    assert_eq!(result.output.as_str(), "TUI query channel closed");
}

#[tokio::test]
async fn query_user_execute_returns_error_when_query_cancelled() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(1);
    let tool = QueryUserTool::new(tx);
    tokio::spawn(async move {
        let req = rx.recv().await.expect("request should be sent");
        drop(req.reply_tx);
    });
    let result = tool
        .execute(serde_json::json!({"question": "Answer me"}))
        .await;
    assert!(result.is_error);
    assert_eq!(result.output.as_str(), "query cancelled");
}
