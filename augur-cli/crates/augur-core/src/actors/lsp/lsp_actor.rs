//! LSP actor: spawn factory and run loop for the rust-analyzer child process.
//!
//! **Public items:** `spawn`, `LspActorConfig`.
//! **Private items:** `run`, `LspActorState`, `LspPhase`, `JsonRpcMsg`.
//!
//! All helper logic executed inside `run` lives in `actor_ops.rs` and is
//! imported here. `LspActorState`, `LspPhase`, and `JsonRpcMsg` are defined
//! here (not in `actor_ops`) so that `spawn` can construct the initial state
//! before handing it to `run`.

use super::handle::{LspHandle, LspRequest};
use super::lsp_actor_ops as actor_ops;
use augur_domain::domain::lsp::LspError;
use augur_domain::domain::string_newtypes::{RootUri, StringNewtype};
use std::collections::{HashMap, HashSet};
use tokio::io::BufReader;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{info, warn};

const LSP_EXECUTABLE: &str = "rust-analyzer";
const LSP_REQUEST_CHANNEL_CAPACITY: usize = 64;
const FAILED_STATE_PIPE_CAPACITY_BYTES: usize = 4096;
const TEST_ROOT_URI_FALLBACK: &str = "file:///tmp";

// ── Public configuration ──────────────────────────────────────────────────────

/// Configuration for [`spawn`].
///
/// `root_uri` is forwarded as the `rootUri` field in the LSP `initialize`
/// request. Construct with an explicit `file://` URI or derive it from the
/// current working directory at the call site.
///
/// # Example
///
/// ```ignore
/// let config = LspActorConfig { root_uri: RootUri::new("file:///home/user/project") };
/// let (join, handle) = actors::lsp::actor::spawn(config);
/// ```
pub struct LspActorConfig {
    /// Workspace root as a `file://` URI, e.g. `"file:///home/user/project"`.
    pub root_uri: RootUri,
}

// ── Private implementation types ─────────────────────────────────────────────

/// Private lifecycle-phase enum, stored as a field of [`LspActorState`].
///
/// Determines how `dispatch_request` handles each incoming [`LspRequest`].
pub(super) enum LspPhase {
    /// `initialize` sent; awaiting `InitializeResult`; incoming [`LspRequest`]s
    /// are buffered in `LspActorState.pending_queue`.
    Initializing,
    /// `initialize` + `initialized` handshake complete; requests are forwarded
    /// immediately to rust-analyzer.
    Ready,
    /// Degraded; the stored [`LspError`] is delivered to all current and future
    /// callers without contacting rust-analyzer.
    Failed(LspError),
}

/// Private bundled mutable state for the actor run loop.
///
/// Fields are grouped into lifecycle and I/O sub-structs to keep top-level
/// struct size bounded while preserving actor ownership semantics.
pub(super) struct LspActorState {
    /// Mutable lifecycle/correlation state for request routing.
    pub(super) lifecycle: LspActorLifecycle,
    /// Child-process I/O state.
    pub(super) io: LspActorIo,
    /// Workspace root URI forwarded in the `initialize` request.
    pub(super) root_uri: RootUri,
}

/// Lifecycle and request-correlation state for the LSP actor.
pub(super) struct LspActorLifecycle {
    /// Current lifecycle phase of the actor.
    pub(super) phase: LspPhase,
    /// Map from in-flight JSON-RPC request ID to its reply channel.
    pub(super) pending:
        HashMap<u64, tokio::sync::oneshot::Sender<Result<serde_json::Value, LspError>>>,
    /// Set of `file://` URIs for which a `textDocument/didOpen` has been sent
    /// this session, keyed by URI string.
    pub(super) open_docs: HashSet<String>,
    /// Monotonically increasing counter used to assign JSON-RPC request IDs.
    ///
    /// Plain `u64` is used (not `AtomicU64`) because the run loop is a single
    /// tokio task; no concurrent access occurs (deviation D-07).
    pub(super) id_counter: u64,
    /// Requests buffered during the `Initializing` phase.
    pub(super) pending_queue: Vec<LspRequest>,
}

/// Child-process I/O resources owned by the LSP actor.
pub(super) struct LspActorIo {
    /// Piped stdin of the rust-analyzer child process (boxed trait object).
    pub(super) stdin: Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
    /// Buffered reader over the piped stdout of the rust-analyzer child.
    pub(super) stdout: BufReader<Box<dyn tokio::io::AsyncRead + Unpin + Send>>,
    /// Owned handle to the rust-analyzer child process with `kill_on_drop`
    /// enabled. `None` for error-path states (duplex-backed fakes). When
    /// this field drops, Tokio sends SIGKILL to the child process, preventing
    /// orphaned rust-analyzer instances on panic or ungraceful shutdown.
    pub(super) _child: Option<tokio::process::Child>,
    /// Kill watch receiver. When the sender is triggered (via
    /// `LspHandle::kill()`), the actor terminates the child process and
    /// exits. This provides a deterministic graceful-shutdown path
    /// independent of mpsc channel ordering.
    pub(super) kill_rx: watch::Receiver<bool>,
}

/// Bundles a JSON-RPC request for [`send_request`][super::lsp_actor_ops::send_request],
/// satisfying the 3-parameter limit (domain-spec §8.3).
///
/// `id` is `None` for notifications: `textDocument/didOpen` carries no id per
/// the JSON-RPC 2.0 specification.
#[derive(bon::Builder)]
pub(super) struct JsonRpcMsg {
    /// Request ID. `None` for notifications; the `"id"` key is omitted from
    /// the serialised JSON when this is `None`.
    pub(super) id: Option<u64>,
    /// JSON-RPC method string, e.g. `"textDocument/definition"`.
    pub(super) method: String,
    /// JSON-encoded parameters object.
    pub(super) params: serde_json::Value,
}

// ── Public factory ────────────────────────────────────────────────────────────

/// Spawn the `LspActor` task and return its join handle and a channel handle.
///
/// Starts the rust-analyzer child process with piped stdin/stdout, then
/// spawns a tokio task running the actor event loop. Returns immediately.
///
/// If rust-analyzer is absent from `$PATH`, the actor enters
/// `LspPhase::Failed(LspError::NotInstalled)` and all subsequent requests
/// via the returned [`LspHandle`] receive `Err(LspError::NotInstalled)`.
///
/// Must be called exactly once per session (enforced by `spawn_core_runtime`).
///
/// # Preconditions
///
/// - Must be called from within a tokio runtime context.
///
/// # Postconditions
///
/// - `JoinHandle` represents a live task.
/// - `LspHandle` is immediately usable; requests queue until the actor is ready.
///
/// # Invariants
///
/// - `tokio::process::Command::new("rust-analyzer")` is called exactly once.
/// - The child process has `stdin(Stdio::piped())` and `stdout(Stdio::piped())`.
///
/// # Examples
///
/// ```ignore
/// let config = LspActorConfig { root_uri: "file:///workspace".into() };
/// let (join, handle) = spawn(config);
/// // handle is immediately usable; errors surface via reply channels
/// ```
pub fn spawn(config: LspActorConfig) -> (JoinHandle<()>, LspHandle) {
    let (tx, rx) = mpsc::channel::<LspRequest>(LSP_REQUEST_CHANNEL_CAPACITY);
    let (kill_tx, kill_rx) = watch::channel(false);
    let handle = LspHandle::new(tx, kill_tx);
    let state = spawn_state(config.root_uri, kill_rx);
    let join = tokio::spawn(run(rx, state));
    (join, handle)
}

fn spawn_state(root_uri: RootUri, kill_rx: watch::Receiver<bool>) -> LspActorState {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    // Safety: `pre_exec` runs in the child process after fork. Setting
    // `PR_SET_PDEATHSIG` is safe here because the child has not yet started
    // executing rust-analyzer code; the only mutation is a single libc call
    // that the kernel validates before returning.
    match unsafe {
        tokio::process::Command::new(LSP_EXECUTABLE)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .pre_exec(|| {
                // PR_SET_PDEATHSIG = 1; SIGKILL = 9
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                Ok(())
            })
            .spawn()
    } {
        Ok(child) => state_from_spawned_child(child, root_uri, kill_rx),
        Err(e) => {
            warn!(LspActor = "spawn", error = %e, "rust-analyzer not found; LSP tool will return errors");
            failed_state(root_uri, LspError::NotInstalled, kill_rx)
        }
    }
}

fn state_from_spawned_child(
    mut child: tokio::process::Child,
    root_uri: RootUri,
    kill_rx: watch::Receiver<bool>,
) -> LspActorState {
    info!(LspActor = "spawn", "rust-analyzer process started");
    let stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => {
            return missing_pipe_state(child, root_uri, "stdin pipe missing after spawn", kill_rx)
        }
    };
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            return missing_pipe_state(child, root_uri, "stdout pipe missing after spawn", kill_rx)
        }
    };
    LspActorState {
        lifecycle: lifecycle_with_phase(LspPhase::Initializing),
        io: LspActorIo {
            stdin: Box::new(stdin),
            stdout: BufReader::new(Box::new(stdout)),
            _child: Some(child),
            kill_rx,
        },
        root_uri,
    }
}

fn missing_pipe_state(
    mut child: tokio::process::Child,
    root_uri: RootUri,
    msg: &str,
    kill_rx: watch::Receiver<bool>,
) -> LspActorState {
    drop(child.kill());
    failed_state(root_uri, LspError::Protocol(msg.to_string()), kill_rx)
}

fn failed_state(
    root_uri: RootUri,
    error: LspError,
    kill_rx: watch::Receiver<bool>,
) -> LspActorState {
    let (stdin_w, _stdin_r) = tokio::io::duplex(FAILED_STATE_PIPE_CAPACITY_BYTES);
    let (_stdout_w, stdout_r) = tokio::io::duplex(FAILED_STATE_PIPE_CAPACITY_BYTES);
    LspActorState {
        lifecycle: lifecycle_with_phase(LspPhase::Failed(error)),
        io: LspActorIo {
            stdin: Box::new(stdin_w),
            stdout: BufReader::new(Box::new(stdout_r)),
            _child: None,
            kill_rx,
        },
        root_uri,
    }
}

fn lifecycle_with_phase(phase: LspPhase) -> LspActorLifecycle {
    LspActorLifecycle {
        phase,
        pending: HashMap::new(),
        open_docs: HashSet::new(),
        id_counter: 0,
        pending_queue: Vec::new(),
    }
}

// ── Test seam ─────────────────────────────────────────────────────────────────

/// Spawn the `LspActor` task backed by caller-supplied I/O streams.
///
/// This entry-point exists **only for testing**; production code must call
/// [`spawn`]. It creates the same actor run-loop as `spawn` but accepts any
/// `AsyncWrite`/`AsyncRead` pair instead of a child-process pipe.
///
/// # Preconditions
///
/// - Must be called from within a tokio runtime context.
///
/// # Returns
///
/// `(join_handle, lsp_handle)` - the task handle and the channel handle.
pub(crate) fn spawn_with_io<W, R>(stdin: W, stdout: BufReader<R>) -> (JoinHandle<()>, LspHandle)
where
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<LspRequest>(LSP_REQUEST_CHANNEL_CAPACITY);
    let (kill_tx, kill_rx) = watch::channel(false);
    let handle = LspHandle::new(tx, kill_tx);

    let root_uri = std::env::current_dir()
        .map(|p| RootUri::new(format!("file://{}", p.display())))
        .unwrap_or_else(|_| RootUri::new(TEST_ROOT_URI_FALLBACK));

    // Box the generic types into trait objects so LspActorState is uniform
    let boxed_stdin: Box<dyn tokio::io::AsyncWrite + Unpin + Send> = Box::new(stdin);
    let inner = stdout.into_inner();
    let boxed_inner: Box<dyn tokio::io::AsyncRead + Unpin + Send> = Box::new(inner);
    let boxed_stdout = BufReader::new(boxed_inner);

    let state = LspActorState {
        lifecycle: LspActorLifecycle {
            phase: LspPhase::Initializing,
            pending: HashMap::new(),
            open_docs: HashSet::new(),
            id_counter: 0,
            pending_queue: Vec::new(),
        },
        io: LspActorIo {
            stdin: boxed_stdin,
            stdout: boxed_stdout,
            _child: None,
            kill_rx,
        },
        root_uri,
    };

    let join = tokio::spawn(run(rx, state));
    (join, handle)
}

// ── Private run loop ──────────────────────────────────────────────────────────

/// Private actor event loop.
///
/// Receives [`LspRequest`] messages from the mpsc channel and drives
/// stdin/stdout I/O with the rust-analyzer process. Returns when all
/// [`LspHandle`] clones are dropped (i.e., the channel closes).
///
/// Never panics; all error paths transition phase and drain pending senders
/// via `notify_all_pending`.
///
/// # Preconditions
///
/// - `state.phase == LspPhase::Initializing`; stdin/stdout handles are live.
///
/// # Postconditions
///
/// - When `rx` closes: function returns; all pending senders are drained.
async fn run(mut rx: mpsc::Receiver<LspRequest>, mut state: LspActorState) {
    if !prepare_run_loop(&mut state).await {
        drain_requests(&mut rx, &mut state).await;
        return;
    }

    loop {
        let event = next_event(&mut rx, &mut state).await;
        let control = process_event(&mut rx, &mut state, event).await;
        if matches!(control, LoopControl::DrainAndReturn) {
            drain_requests(&mut rx, &mut state).await;
            return;
        }
    }
}

enum Event {
    Request(LspRequest),
    ChannelClosed,
    Response(Result<serde_json::Value, LspError>),
    KillReceived,
}

enum LoopControl {
    Continue,
    DrainAndReturn,
}

async fn prepare_run_loop(state: &mut LspActorState) -> bool {
    // If already in Failed state (e.g., spawn failed with NotInstalled), go
    // straight to drain loop - never send initialize.
    if matches!(state.lifecycle.phase, LspPhase::Failed(_)) {
        return false;
    }
    info!(LspActor = "run", root_uri = %state.root_uri, "sending initialize request");
    send_initialize(state).await
}

async fn process_event(
    rx: &mut mpsc::Receiver<LspRequest>,
    state: &mut LspActorState,
    event: Event,
) -> LoopControl {
    match event {
        Event::ChannelClosed => LoopControl::DrainAndReturn,
        Event::KillReceived => kill_received(state),
        Event::Request(request) => {
            actor_ops::dispatch_request(state, request).await;
            LoopControl::Continue
        }
        Event::Response(Ok(msg)) => handle_response_ok(rx, state, msg).await,
        Event::Response(Err(e)) => handle_response_error(state, e),
    }
}

fn handle_response_error(state: &mut LspActorState, error: LspError) -> LoopControl {
    let error = classify_response_error(state, error);
    warn!(LspActor = "run", error = %error, "rust-analyzer process died; entering error drain loop");
    state.lifecycle.phase = LspPhase::Failed(error.clone());
    actor_ops::notify_all_pending(state, error);
    LoopControl::DrainAndReturn
}

fn kill_received(state: &mut LspActorState) -> LoopControl {
    // Kill the child process to free OS resources immediately, then
    // transition to Failed state so all in-flight and future callers
    // receive a clean error.
    if let Some(mut child) = state.io._child.take() {
        drop(child.kill());
        // Drop `child` so `kill_on_drop` is not required to finish the job.
        drop(child);
    }
    let error = LspError::ProcessDied;
    state.lifecycle.phase = LspPhase::Failed(error.clone());
    actor_ops::notify_all_pending(state, error);
    LoopControl::DrainAndReturn
}

async fn next_event(rx: &mut mpsc::Receiver<LspRequest>, state: &mut LspActorState) -> Event {
    tokio::select! {
        msg = rx.recv() => {
            match msg {
                None => Event::ChannelClosed,
                Some(req) => Event::Request(req),
            }
        }
        result = actor_ops::read_response(&mut state.io.stdout) => {
            Event::Response(result)
        }
        _ = state.io.kill_rx.changed() => {
            Event::KillReceived
        }
    }
}

async fn send_initialize(state: &mut LspActorState) -> bool {
    let init_msg = JsonRpcMsg {
        id: Some(0),
        method: "initialize".to_string(),
        params: serde_json::json!({
            "processId": std::process::id(),
            "rootUri": state.root_uri,
            "capabilities": {}
        }),
    };
    if actor_ops::send_request(&mut state.io.stdin, init_msg)
        .await
        .is_ok()
    {
        return true;
    }
    state.lifecycle.phase = LspPhase::Failed(LspError::NotInstalled);
    false
}

async fn handle_response_ok(
    rx: &mut mpsc::Receiver<LspRequest>,
    state: &mut LspActorState,
    msg: serde_json::Value,
) -> LoopControl {
    if matches!(state.lifecycle.phase, LspPhase::Initializing) {
        return handle_initializing_response_ok(rx, state, msg).await;
    }
    handle_ready_response_ok(state, msg);
    LoopControl::Continue
}

async fn handle_initializing_response_ok(
    rx: &mut mpsc::Receiver<LspRequest>,
    state: &mut LspActorState,
    msg: serde_json::Value,
) -> LoopControl {
    while let Ok(req) = rx.try_recv() {
        state.lifecycle.pending_queue.push(req);
    }
    actor_ops::handle_initialize(state, msg).await;
    if matches!(state.lifecycle.phase, LspPhase::Failed(_)) {
        warn!(
            LspActor = "run",
            "LSP initialization failed; entering error drain loop"
        );
        return LoopControl::DrainAndReturn;
    }
    info!(LspActor = "run", "LSP ready");
    LoopControl::Continue
}

fn handle_ready_response_ok(state: &mut LspActorState, msg: serde_json::Value) {
    if matches!(state.lifecycle.phase, LspPhase::Ready)
        && let Some(id) = msg["id"].as_u64()
    {
        actor_ops::dispatch_response(state, actor_ops::LspRequestId(id), msg);
    }
}

fn classify_response_error(state: &LspActorState, error: LspError) -> LspError {
    if matches!(state.lifecycle.phase, LspPhase::Initializing) {
        LspError::NotInstalled
    } else {
        error
    }
}

async fn drain_requests(rx: &mut mpsc::Receiver<LspRequest>, state: &mut LspActorState) {
    while let Some(request) = rx.recv().await {
        actor_ops::dispatch_request(state, request).await;
    }
}
