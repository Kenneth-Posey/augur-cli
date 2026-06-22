use augur_core::tools::builtin::sql_query::{SqlQueryTool, SqlSession};
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn create_table_and_insert() {
    let session = Arc::new(Mutex::new(SqlSession::new().expect("in-memory db")));
    let tool = SqlQueryTool::new(session);
    let ddl = tool
        .execute(serde_json::json!({"query": "CREATE TABLE t (id INTEGER, val TEXT)"}))
        .await;
    assert!(!ddl.is_error, "DDL failed: {}", ddl.output.as_str());
    assert_eq!(ddl.output.as_str(), "OK");
    let dml = tool
        .execute(serde_json::json!({"query": "INSERT INTO t VALUES (1, 'hello')"}))
        .await;
    assert!(!dml.is_error, "INSERT failed: {}", dml.output.as_str());
    assert_eq!(dml.output.as_str(), "OK");
}

#[tokio::test]
async fn select_returns_markdown_table() {
    let session = Arc::new(Mutex::new(SqlSession::new().expect("in-memory db")));
    let tool = SqlQueryTool::new(session);
    let result = tool
        .execute(serde_json::json!({"query": "SELECT 1 AS n"}))
        .await;
    assert!(
        !result.is_error,
        "SELECT failed: {}",
        result.output.as_str()
    );
    let out = result.output.as_str();
    assert!(out.contains("| n |"), "missing header: {out}");
    assert!(out.contains("| 1 |"), "missing value: {out}");
}

#[tokio::test]
async fn invalid_sql_returns_error_not_panic() {
    let session = Arc::new(Mutex::new(SqlSession::new().expect("in-memory db")));
    let tool = SqlQueryTool::new(session);
    let result = tool
        .execute(serde_json::json!({"query": "THIS IS NOT VALID SQL !!!"}))
        .await;
    assert!(result.is_error, "expected error for invalid SQL");
    assert!(
        !result.output.as_str().is_empty(),
        "error output should not be empty"
    );
}

#[tokio::test]
async fn shared_session_persists_across_calls() {
    let session = Arc::new(Mutex::new(SqlSession::new().expect("in-memory db")));
    let tool = SqlQueryTool::new(session);
    tool.execute(serde_json::json!({"query": "CREATE TABLE items (x INTEGER)"}))
        .await;
    tool.execute(serde_json::json!({"query": "INSERT INTO items VALUES (42)"}))
        .await;
    let result = tool
        .execute(serde_json::json!({"query": "SELECT x FROM items"}))
        .await;
    assert!(
        !result.is_error,
        "SELECT failed: {}",
        result.output.as_str()
    );
    assert!(
        result.output.as_str().contains("42"),
        "expected 42 in output: {}",
        result.output.as_str()
    );
}
