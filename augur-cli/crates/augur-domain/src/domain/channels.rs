//! Channel buffer-size constants - the single source of truth for all actor
//! channel capacities. Every `mpsc::channel`, `broadcast::channel`, and
//! `watch::channel` construction site must import a constant from here.
//! No inline capacity literals are permitted anywhere else in the codebase.

use crate::domain::newtypes::Count;

/// Backpressure limit for the LLM actor command queue.
/// One active request is typical; 16 provides headroom for burst submission.
pub const LLM_COMMAND_CAPACITY: Count = Count::of(16);

/// Buffer size for per-request LLM streaming channels (`mpsc::channel<StreamChunk>`).
/// Sized for a full LLM response to arrive before the consumer drains the channel.
pub const STREAM_CHUNK_CAPACITY: Count = Count::of(512);

/// Buffer size for the tool actor command queue.
/// Allows a burst of sequential tool calls from the agent re-entry loop.
pub const TOOL_COMMAND_CAPACITY: Count = Count::of(32);

/// Buffer size for the agent actor command queue.
/// The agent receives one user prompt at a time; a small buffer is sufficient.
pub const AGENT_COMMAND_CAPACITY: Count = Count::of(8);

/// Buffer size for the agent output broadcast channel.
/// Sized to prevent the TUI subscriber from lagging behind token emission.
pub const AGENT_OUTPUT_CAPACITY: Count = Count::of(256);

/// Buffer size for the session actor command queue.
/// Session receives low-volume endpoint-change and config commands.
pub const SESSION_COMMAND_CAPACITY: Count = Count::of(8);

/// Buffer size for the file-read actor command queue.
/// Allows a burst of parallel file-read requests from the agent tool loop.
pub const FILE_READ_COMMAND_CAPACITY: Count = Count::of(32);

/// Buffer size for the logger actor command queue.
/// Sized for the typical turn rate; the actor serializes writes so a modest
/// buffer avoids back-pressure on the agent while absorbing burst turns.
pub const LOGGER_COMMAND_CAPACITY: Count = Count::of(64);

/// Buffer size for the cache actor command queue.
/// Sized for snapshot refresh and working-file update bursts without back-pressure.
pub const CACHE_COMMAND_CAPACITY: Count = Count::of(64);

/// Backpressure limit for the executor actor command queue.
/// Low volume: one prompt at a time with occasional mode/compact commands.
pub const EXECUTOR_COMMAND_CAPACITY: Count = Count::of(16);

/// Buffer size for the executor actor output broadcast channel.
/// Mirrors `AGENT_OUTPUT_CAPACITY`; the supervisor is the primary subscriber.
pub const EXECUTOR_EVENT_BUFFER: Count = Count::of(256);

/// Backpressure limit for the supervisor actor command queue.
/// Low volume: one plan at a time; pause/resume/cancel are infrequent.
pub const SUPERVISOR_COMMAND_CAPACITY: Count = Count::of(8);

/// Buffer size for the supervisor event broadcast channel.
/// Sized to hold a burst of step events before the TUI drains them.
pub const SUPERVISOR_OUTPUT_CAPACITY: Count = Count::of(256);

/// Backpressure limit for the Copilot chat actor command queue.
/// Low volume: one user message at a time with occasional compact/shutdown commands.
pub const COPILOT_COMMAND_CAPACITY: Count = Count::of(16);

/// Backpressure limit for the file-scanner actor command queue.
/// The TUI sends a scan command on each keypress after `@`; a small buffer
/// absorbs rapid typing without back-pressure on the event loop.
pub const FILE_SCAN_COMMAND_CAPACITY: Count = Count::of(8);

/// Capacity of the agent-feed channel; buffers [`crate::domain::types::AgentFeedOutput`]
/// events from background and external sessions before the TUI drains them.
pub const AGENT_FEED_CAPACITY: Count = Count::of(256);

/// Buffer size for the query-user channel. Capacity of 1 enforces backpressure,
/// ensuring the TUI processes user queries one at a time before the tool accepts
/// the next query from the agent.
pub const QUERY_USER_CHANNEL_CAPACITY: Count = Count::of(1);

/// Buffer size for the token-tracker actor command queue.
///
/// Low-volume: one event per LLM turn. 64 absorbs a burst of concurrent
/// background pipeline steps without back-pressure on callers.
pub const TOKEN_TRACKER_COMMAND_CAPACITY: Count = Count::of(64);

/// Buffer size for the LLM feed consumer output channels.
/// Sized for a full streaming response to be buffered before consumer drains.
pub const LLM_FEED_CAPACITY: Count = Count::of(256);

/// Buffer size for the user message consumer output channels.
/// Low volume: one user input at a time.
pub const USER_FEED_CAPACITY: Count = Count::of(64);

/// Buffer size for the history adapter feed channels.
/// Matches turn rate; small buffer avoids back-pressure on feed producers.
pub const HISTORY_FEED_CAPACITY: Count = Count::of(128);

/// Buffer size for the TUI panel feed channels.
/// Sized to prevent panel subscriber from lagging behind feed emission.
pub const TUI_FEED_CAPACITY: Count = Count::of(256);

/// Buffer size for the spawn-agent request channel.
/// Low-volume: one background task invocation per tool call.
pub const SPAWN_AGENT_CHANNEL_CAPACITY: Count = Count::of(32);
