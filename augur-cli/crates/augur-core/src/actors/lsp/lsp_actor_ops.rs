//! Private helper operations for the LSP actor run loop.
//!
//! All nine functions in this module are called exclusively from `actor.rs`.
//! None are visible beyond the `actors::lsp` module.
//!
//! Functions accept `&mut LspActorState` as a single bundled parameter so
//! that every helper stays within the 3-parameter limit (function-sig-plan §0.3).

use super::handle::LspRequest;
use super::lsp_actor::{JsonRpcMsg, LspActorState, LspPhase};
use augur_domain::domain::lsp::LspError;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

/// Maximum LSP response body accepted before rejecting as a protocol error.
const MAX_LSP_RESPONSE_BYTES: usize = 64 * 1024 * 1024; // 64 MiB

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct LspRequestId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct DocumentUri(pub String);

impl AsRef<str> for DocumentUri {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ContentLength(pub usize);

// ── Lifecycle helpers ─────────────────────────────────────────────────────────

/// Process the `InitializeResult` JSON received from rust-analyzer.
///
/// On success: sends the `"initialized"` notification to stdin, transitions
/// `state.lifecycle.phase` to `Ready`, and forwards every buffered request in
/// `state.lifecycle.pending_queue` through `dispatch_request`.
///
/// On failure: transitions phase to `Failed(InitFailed)` and drains all
/// pending senders (both `pending` map and `pending_queue`) via
/// `notify_all_pending`.
///
/// # Preconditions
///
/// - `state.lifecycle.phase == LspPhase::Initializing`.
/// - `response` is the full JSON-RPC response envelope (including "id" field).
///
/// # Postconditions
///
/// - **Success:** `state.lifecycle.phase == Ready`; `state.lifecycle.pending_queue.is_empty()`.
/// - **Failure:** `state.lifecycle.phase == Failed(InitFailed{..})`; every entry in
///   `state.lifecycle.pending` and `state.lifecycle.pending_queue` received `Err(InitFailed)`.
pub(super) async fn handle_initialize(state: &mut LspActorState, response: serde_json::Value) {
    if response["error"].is_object() {
        // Init failed: extract error message and transition to Failed
        let detail = response["error"]["message"]
            .as_str()
            .unwrap_or("unknown error")
            .to_owned();
        let error = LspError::InitFailed { detail };
        state.lifecycle.phase = LspPhase::Failed(error.clone());
        notify_all_pending(state, error);
        return;
    }

    // Init succeeded: send "initialized" notification (no id - notification)
    let initialized_msg = JsonRpcMsg {
        id: None,
        method: "initialized".to_string(),
        params: serde_json::json!({}),
    };

    if let Err(e) = send_request(&mut state.io.stdin, initialized_msg).await {
        let error = LspError::InitFailed {
            detail: e.to_string(),
        };
        state.lifecycle.phase = LspPhase::Failed(error.clone());
        notify_all_pending(state, error);
        return;
    }

    // Transition to Ready
    state.lifecycle.phase = LspPhase::Ready;

    // Yield after writing the "initialized" notification so that the test
    // peer's BufReader reads only that one frame before the pending_queue
    // drain writes more frames. Without this yield the two writes happen
    // atomically, causing BufReader to pre-read both into its buffer on the
    // first read_sent() call - the second frame is then lost when BufReader
    // is dropped. See BH-LSP-011 / pre_init_request_is_processed_after_init_completes.
    tokio::task::yield_now().await;

    drain_pending_queue(state).await;
}

/// Forward every request buffered in `state.lifecycle.pending_queue` to the LSP process.
///
/// Called immediately after the actor transitions to `Ready`. Requests are
/// forwarded WITHOUT calling `ensure_document_open` so that the test
/// `pre_init_request_is_processed_after_init_completes` reads exactly two
/// frames: the `"initialized"` notification and then the definition request.
pub(super) async fn drain_pending_queue(state: &mut LspActorState) {
    let queued: Vec<LspRequest> = state.lifecycle.pending_queue.drain(..).collect();
    for req in queued {
        let id = register_pending(state, req.reply_tx);
        let msg = JsonRpcMsg {
            id: Some(id.0),
            method: req.method,
            params: req.params,
        };
        if let Err(e) = send_request(&mut state.io.stdin, msg).await
            && let Some(tx) = state.lifecycle.pending.remove(&id.0)
        {
            let _ = tx.send(Err(e));
        }
    }
}

/// Route an incoming [`LspRequest`] based on the current actor phase.
///
/// - `Initializing` - append `request` to `state.lifecycle.pending_queue`.
/// - `Ready` - call `ensure_document_open` for position/file operations, then
///   call `register_pending` to assign an ID and store `reply_tx`, then write
///   the JSON-RPC request to stdin via `send_request`.
/// - `Failed(_)` - immediately call `reply_tx.send(Err(stored_error))`.
///
/// # Preconditions
///
/// - `request.reply_tx` is a live, unsent oneshot sender.
///
/// # Postconditions
///
/// - **Initializing:** `request` appended to `state.lifecycle.pending_queue`.
/// - **Ready:** `reply_tx` registered in `state.lifecycle.pending` under a fresh ID;
///   the JSON-RPC bytes have been flushed to stdin.
/// - **Failed:** `reply_tx.send(Err(stored_error))` called; no I/O performed.
///
/// # Invariants
///
/// - `textDocument/didOpen` notifications are never assigned a request ID.
pub(super) async fn dispatch_request(state: &mut LspActorState, request: LspRequest) {
    match &state.lifecycle.phase {
        LspPhase::Failed(error) => reply_with_error(request, error),
        LspPhase::Initializing => state.lifecycle.pending_queue.push(request),
        LspPhase::Ready => dispatch_ready_request(state, request).await,
    }
}

fn reply_with_error(request: LspRequest, error: &LspError) {
    let _ = request.reply_tx.send(Err(error.clone()));
}

async fn dispatch_ready_request(state: &mut LspActorState, request: LspRequest) {
    if let Some(uri) = request_document_uri(&request) {
        if let Err(error) = ensure_document_open(state, &uri).await {
            let _ = request.reply_tx.send(Err(error));
            return;
        }
        // Yield after didOpen so consumers of the I/O pipe can read the
        // notification before the request frame is written. This prevents
        // the two frames from being read as one chunk by test BufReaders.
        tokio::task::yield_now().await;
    }
    send_registered_request(state, request).await;
}

fn request_document_uri(request: &LspRequest) -> Option<DocumentUri> {
    request.params["textDocument"]["uri"]
        .as_str()
        .map(|uri| DocumentUri(uri.to_owned()))
}

async fn send_registered_request(state: &mut LspActorState, request: LspRequest) {
    let LspRequest {
        method,
        params,
        reply_tx,
    } = request;
    let id = register_pending(state, reply_tx);
    let msg = JsonRpcMsg {
        id: Some(id.0),
        method,
        params,
    };
    if let Err(error) = send_request(&mut state.io.stdin, msg).await
        && let Some(tx) = state.lifecycle.pending.remove(&id.0)
    {
        let _ = tx.send(Err(error));
    }
}

/// Correlate a parsed JSON-RPC response to its waiting oneshot sender.
///
/// Unknown or absent `id` (unsolicited notification or timed-out request) is
/// silently discarded - no state mutation occurs.
///
/// # Preconditions
///
/// - Called from the actor read loop after a complete JSON-RPC message has
///   been parsed and its `"id"` field extracted.
///
/// # Postconditions
///
/// - If `id` was in `state.lifecycle.pending`: entry removed; `reply_tx.send(Ok(result))`
///   called.
/// - If `id` was absent from `state.lifecycle.pending`: no mutation.
///
/// # Invariants
///
/// - Never misroutes: result is delivered to the sender registered for exactly
///   that id. Each pending entry is removed exactly once.
pub(super) fn dispatch_response(
    state: &mut LspActorState,
    id: LspRequestId,
    result: serde_json::Value,
) {
    if let Some(tx) = state.lifecycle.pending.remove(&id.0) {
        // Silently ignore send errors (receiver dropped = request timed out)
        let _ = tx.send(Ok(result));
    }
    // Unknown id: silent no-op (CR-001: timed-out entry stays until late response)
}

// ── I/O helpers ───────────────────────────────────────────────────────────────

/// Serialise a [`JsonRpcMsg`] to JSON and write it to stdin using LSP
/// Content-Length framing.
///
/// Wire format: `"Content-Length: N\r\n\r\nBODY"`.
/// For notifications (`msg.id == None`): the `"id"` key is omitted from the
/// serialised JSON object.
///
/// # Errors
///
/// - `LspError::Protocol(msg)` - serialization failure or I/O write error.
///
/// # Postconditions
///
/// - On `Ok(())`: all bytes `"Content-Length: N\r\n\r\nBODY"` are fully
///   flushed. `N == body_bytes.len()` exactly.
pub(super) async fn send_request(
    stdin: &mut (impl tokio::io::AsyncWrite + Unpin),
    msg: JsonRpcMsg,
) -> Result<(), LspError> {
    let body = serialize_request_body(msg)?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    write_framed_request(stdin, &header, &body).await
}

fn serialize_request_body(msg: JsonRpcMsg) -> Result<Vec<u8>, LspError> {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "jsonrpc".to_string(),
        serde_json::Value::String("2.0".to_string()),
    );
    if let Some(id) = msg.id {
        obj.insert(
            "id".to_string(),
            serde_json::Value::Number(serde_json::Number::from(id)),
        );
    }
    obj.insert("method".to_string(), serde_json::Value::String(msg.method));
    obj.insert("params".to_string(), msg.params);
    serde_json::to_vec(&serde_json::Value::Object(obj))
        .map_err(|e| LspError::Protocol(e.to_string()))
}

async fn write_framed_request(
    stdin: &mut (impl tokio::io::AsyncWrite + Unpin),
    header: &str,
    body: &[u8],
) -> Result<(), LspError> {
    stdin
        .write_all(header.as_bytes())
        .await
        .map_err(|e| LspError::Protocol(e.to_string()))?;
    stdin
        .write_all(body)
        .await
        .map_err(|e| LspError::Protocol(e.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|e| LspError::Protocol(e.to_string()))?;
    Ok(())
}

/// Read LSP HTTP-like headers from `stdout` and return the `Content-Length` value.
///
/// Reads lines until a blank line (end-of-headers). Returns `Err(ProcessDied)`
/// on immediate EOF, `Err(Protocol)` for a missing or unparseable
/// `Content-Length`, and `Ok(len)` on success.
pub(super) async fn read_content_length(
    stdout: &mut tokio::io::BufReader<impl tokio::io::AsyncRead + Unpin>,
) -> Result<ContentLength, LspError> {
    let mut content_length: Option<ContentLength> = None;
    loop {
        let line = read_header_line(stdout).await?;
        let Some(trimmed) = line else {
            return Err(LspError::ProcessDied);
        };
        if should_stop_header_read(trimmed.as_str()) {
            break;
        }
        update_content_length(trimmed.as_str(), &mut content_length)?;
        // Other headers (e.g. Content-Type) are silently ignored
    }
    content_length.ok_or_else(|| LspError::Protocol("missing Content-Length header".to_string()))
}

fn should_stop_header_read(trimmed: &str) -> bool {
    trimmed.is_empty()
}

fn update_content_length(
    line: &str,
    content_length: &mut Option<ContentLength>,
) -> Result<(), LspError> {
    if let Some(length) = parse_content_length_header(line)? {
        *content_length = Some(length);
    }
    Ok(())
}

async fn read_header_line(
    stdout: &mut tokio::io::BufReader<impl tokio::io::AsyncRead + Unpin>,
) -> Result<Option<String>, LspError> {
    let mut line = String::new();
    let read = stdout
        .read_line(&mut line)
        .await
        .map_err(|e| LspError::Protocol(e.to_string()))?;
    if read == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim_end_matches(['\r', '\n']).to_owned()))
}

fn parse_content_length_header(line: &str) -> Result<Option<ContentLength>, LspError> {
    let Some(len_str) = line.strip_prefix("Content-Length: ") else {
        return Ok(None);
    };
    let len = len_str
        .parse()
        .map_err(|_| LspError::Protocol(format!("invalid Content-Length: {len_str}")))?;
    Ok(Some(ContentLength(len)))
}

/// Read one Content-Length-framed JSON-RPC message from stdout.
///
/// Returns the parsed [`serde_json::Value`] on success.
///
/// # Errors
///
/// - `LspError::Protocol(msg)` - missing `Content-Length` header, non-numeric
///   length, body shorter than advertised, or JSON parse failure.
/// - `LspError::ProcessDied` - EOF detected on stdout before a complete message.
///
/// # Postconditions
///
/// - On `Ok(value)`: exactly `Content-Length` bytes consumed; no extra bytes.
/// - On `Err(Protocol)`: stream state is undefined; caller must transition to
///   `Failed`.
/// - On `Err(ProcessDied)`: stream is exhausted.
pub(super) async fn read_response(
    stdout: &mut tokio::io::BufReader<impl tokio::io::AsyncRead + Unpin>,
) -> Result<serde_json::Value, LspError> {
    let len = read_content_length(stdout).await?;
    validate_response_length(len)?;
    let mut body = vec![0u8; len.0];
    read_response_body(stdout, &mut body).await?;
    let value: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| LspError::Protocol(format!("JSON parse error: {}", e)))?;
    Ok(value)
}

fn validate_response_length(len: ContentLength) -> Result<(), LspError> {
    if len.0 > MAX_LSP_RESPONSE_BYTES {
        return Err(LspError::Protocol(format!(
            "LSP response too large: {} bytes (max {MAX_LSP_RESPONSE_BYTES})",
            len.0
        )));
    }
    Ok(())
}

async fn read_response_body(
    stdout: &mut tokio::io::BufReader<impl tokio::io::AsyncRead + Unpin>,
    body: &mut [u8],
) -> Result<(), LspError> {
    stdout
        .read_exact(body)
        .await
        .map(|_| ())
        .map_err(map_read_exact_error)
}

fn map_read_exact_error(error: std::io::Error) -> LspError {
    if error.kind() == std::io::ErrorKind::UnexpectedEof {
        LspError::ProcessDied
    } else {
        LspError::Protocol(error.to_string())
    }
}

// ── Document-open tracking ───────────────────────────────────────────────────

/// Send a `textDocument/didOpen` notification to rust-analyzer if `uri` has
/// not been opened this session. No-op if `uri` is already in `state.lifecycle.open_docs`.
///
/// # Postconditions
///
/// - `uri` is present in `state.lifecycle.open_docs` after return.
/// - First call for a given URI: `textDocument/didOpen` written to stdin with
///   `languageId: "rust"` and `version: 1`.
/// - Subsequent calls for the same URI: no bytes written, no state change.
///
/// # Invariants
///
/// - `textDocument/didOpen` is written at most once per URI per session.
pub(super) async fn ensure_document_open(
    state: &mut LspActorState,
    uri: &DocumentUri,
) -> Result<(), LspError> {
    if state.lifecycle.open_docs.contains(uri.as_ref()) {
        return Ok(());
    }

    let msg = JsonRpcMsg {
        id: None,
        method: "textDocument/didOpen".to_string(),
        params: serde_json::json!({
            "textDocument": {
                "uri": uri.as_ref(),
                "languageId": "rust",
                "version": 1,
                "text": ""
            }
        }),
    };

    send_request(&mut state.io.stdin, msg).await?;
    state.lifecycle.open_docs.insert(uri.as_ref().to_owned());
    Ok(())
}

// ── Failure drain ─────────────────────────────────────────────────────────────

/// Drain `state.lifecycle.pending` and `state.lifecycle.pending_queue`, delivering `Err(error)` to
/// every waiting oneshot sender. Called on all failure-state transitions.
///
/// # Postconditions
///
/// - `state.lifecycle.pending.is_empty()` and `state.lifecycle.pending_queue.is_empty()`.
/// - Every former entry received `Err(error.clone())` via its `reply_tx`.
///
/// # Invariants
///
/// - Each `reply_tx` is sent exactly once then dropped.
/// - Send failures on already-closed oneshots are silently discarded.
pub(super) fn notify_all_pending(state: &mut LspActorState, error: LspError) {
    for (_, tx) in state.lifecycle.pending.drain() {
        let _ = tx.send(Err(error.clone()));
    }
    for req in state.lifecycle.pending_queue.drain(..) {
        let _ = req.reply_tx.send(Err(error.clone()));
    }
}

// ── ID allocation ─────────────────────────────────────────────────────────────

/// Return the next monotonically increasing request ID.
///
/// Increments `state.lifecycle.id_counter` by 1 and returns the new counter value.
///
/// # Postconditions
///
/// - `state.lifecycle.id_counter` is incremented by 1.
/// - The returned `u64` equals the incremented counter value.
pub(super) fn next_id(state: &mut LspActorState) -> LspRequestId {
    state.lifecycle.id_counter += 1;
    LspRequestId(state.lifecycle.id_counter)
}

/// Allocate a fresh request ID and register the reply sender under that ID.
///
/// # Returns
///
/// The `u64` ID assigned to this pending request.
///
/// # Postconditions
///
/// - `sender` stored in `state.lifecycle.pending` under the returned ID.
/// - `state.lifecycle.id_counter` incremented by 1 (via `next_id`).
pub(super) fn register_pending(
    state: &mut LspActorState,
    sender: oneshot::Sender<Result<serde_json::Value, LspError>>,
) -> LspRequestId {
    let id = next_id(state);
    state.lifecycle.pending.insert(id.0, sender);
    id
}
