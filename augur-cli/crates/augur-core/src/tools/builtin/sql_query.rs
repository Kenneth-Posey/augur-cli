//! Built-in sql_query tool: executes SQL against a per-session in-memory SQLite database.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use std::sync::{Arc, Mutex};

const TOOL_NAME: &str = "sql_query";

/// In-memory SQLite session for a single agent task.
///
/// Owns a `rusqlite::Connection` opened against an in-memory database. All tool
/// calls within the same task share the same connection, so DDL and DML from
/// one call are visible to subsequent calls.
///
/// `SqlSession` is `Send` (rusqlite connections are `Send` since v0.29) but not
/// `Sync`. Wrap in `Arc<Mutex<SqlSession>>` where shared access across calls
/// is required.
pub struct SqlSession {
    conn: rusqlite::Connection,
}

impl SqlSession {
    /// Open a new in-memory SQLite database.
    ///
    /// Returns an error if rusqlite fails to open the connection.
    pub fn new() -> Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open_in_memory()?;
        Ok(Self { conn })
    }
}

/// Returns `true` when the SQL string appears to be a SELECT query.
///
/// Used to choose between `query` (row-producing) and `execute` (DDL/DML) paths.
fn is_select(sql: &str) -> bool {
    let upper = sql.trim().to_uppercase();
    upper.starts_with("SELECT") || upper.starts_with("WITH")
}

/// Format the results of a SELECT statement as a Markdown table.
///
/// Returns the formatted table string, or an error string on failure.
fn format_select(conn: &rusqlite::Connection, sql: &str) -> Result<String, rusqlite::Error> {
    let mut stmt = conn.prepare(sql)?;
    let col_count = stmt.column_count();
    let headers: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_owned())
        .collect();

    let header_row = format!("| {} |", headers.join(" | "));
    let separator = format!(
        "| {} |",
        headers
            .iter()
            .map(|_| "---")
            .collect::<Vec<_>>()
            .join(" | ")
    );

    let mut rows: Vec<String> = vec![header_row, separator];
    let mut result_rows = stmt.query([])?;
    while let Some(row) = result_rows.next()? {
        let cells: Vec<String> = (0..col_count)
            .map(|i| {
                let val: rusqlite::types::Value =
                    row.get(i).unwrap_or(rusqlite::types::Value::Null);
                value_to_string(val)
            })
            .collect();
        rows.push(format!("| {} |", cells.join(" | ")));
    }
    Ok(rows.join("\n"))
}

/// Convert a rusqlite `Value` to a display string.
fn value_to_string(val: rusqlite::types::Value) -> String {
    match val {
        rusqlite::types::Value::Blob(bytes) => format!("<blob {} bytes>", bytes.len()),
        other => scalar_value_to_string(other),
    }
}

fn scalar_value_to_string(val: rusqlite::types::Value) -> String {
    if let Some(number) = number_value_to_string(&val) {
        return number;
    }
    text_value_to_string(val).unwrap_or_else(|| "NULL".to_owned())
}

fn number_value_to_string(val: &rusqlite::types::Value) -> Option<String> {
    match val {
        rusqlite::types::Value::Integer(value) => Some(value.to_string()),
        rusqlite::types::Value::Real(value) => Some(value.to_string()),
        _ => None,
    }
}

fn text_value_to_string(val: rusqlite::types::Value) -> Option<String> {
    if let rusqlite::types::Value::Text(value) = val {
        Some(value)
    } else {
        None
    }
}

fn generic_sql_error_message() -> &'static str {
    "sql query failed"
}

fn result_with_output(output: OutputText, is_error: bool) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(output)
        .is_error(IsPredicate::from(is_error))
        .build()
}

fn parse_query_arg(args: &serde_json::Value) -> Result<String, ToolCallResult> {
    match args["query"].as_str() {
        Some(s) if !s.is_empty() => Ok(s.to_owned()),
        _ => Err(result_with_output(
            OutputText::new("missing or empty 'query' argument"),
            true,
        )),
    }
}

fn lock_session(
    session: &Arc<Mutex<SqlSession>>,
) -> Result<std::sync::MutexGuard<'_, SqlSession>, ToolCallResult> {
    session.lock().map_err(|e| {
        result_with_output(
            OutputText::new(format!(
                "sql session unavailable: {}",
                e.to_string().chars().take(64).collect::<String>()
            )),
            true,
        )
    })
}

fn sql_error_result(error: rusqlite::Error) -> ToolCallResult {
    result_with_output(
        OutputText::new(format!(
            "{} ({}).",
            generic_sql_error_message(),
            error
                .sqlite_error_code()
                .map(|code| format!("{code:?}"))
                .unwrap_or_else(|| "unknown".to_string())
        )),
        true,
    )
}

fn run_sql(conn: &rusqlite::Connection, sql: &str) -> ToolCallResult {
    if is_select(sql) {
        return run_select_sql(conn, sql);
    }
    run_execute_sql(conn, sql)
}

fn run_select_sql(conn: &rusqlite::Connection, sql: &str) -> ToolCallResult {
    match format_select(conn, sql) {
        Ok(table) => result_with_output(OutputText::new(table), false),
        Err(error) => sql_error_result(error),
    }
}

fn run_execute_sql(conn: &rusqlite::Connection, sql: &str) -> ToolCallResult {
    match conn.execute(sql, []) {
        Ok(_) => result_with_output(OutputText::new("OK"), false),
        Err(error) => sql_error_result(error),
    }
}

/// Executes SQL against a shared per-session in-memory SQLite database.
///
/// SELECT queries return results as a Markdown table. DDL and DML return `"OK"`.
/// Errors are returned as error `ToolCallResult` values rather than panics.
pub struct SqlQueryTool {
    session: Arc<Mutex<SqlSession>>,
}

impl SqlQueryTool {
    /// Create a `SqlQueryTool` sharing the given session.
    ///
    /// Multiple tool instances sharing the same `Arc<Mutex<SqlSession>>` will
    /// operate on the same in-memory database, preserving state across calls.
    pub fn new(session: Arc<Mutex<SqlSession>>) -> Self {
        Self { session }
    }
}

#[async_trait::async_trait]
impl ToolHandler for SqlQueryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Execute SQL against the per-session in-memory SQLite database.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "SQL query to execute against the session database"
                    }
                },
                "required": ["query"]
            }),
        )
    }

    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let sql = match parse_query_arg(&args) {
            Ok(sql) => sql,
            Err(result) => return result,
        };
        let guard = match lock_session(&self.session) {
            Ok(guard) => guard,
            Err(result) => return result,
        };
        run_sql(&guard.conn, &sql)
    }
}
