//! Background feed dispatcher for streaming classified and mapped events.
//!
//! This module provides an async streaming interface for transforming `SessionEventData`
//! events into `AgentFeedOutput` items for display in the background panel.
//!
//! ## Core Components
//!
//! - `StreamFeedConfig`: Configuration for event stream buffering, flushing, and filtering
//! - `stream_to_feed()`: Async function that receives, classifies, maps, buffers, and yields
//!   background events according to configuration
//!
//! ## Behavior
//!
//! The stream operates in a loop:
//!
//! 1. **Receive** `SessionEventData` events from the input channel
//! 2. **Classify** each event using the injected `BackgroundEventClassifier`
//! 3. **Map** the classified event using `map_background_event()`
//! 4. **Buffer** mapped outputs up to `max_queued_events` capacity
//! 5. **Flush** either when:
//!    - Buffer reaches capacity (immediate yield all)
//!    - Timer interval elapses (yield all, restart timer)
//! 6. **Skip** unmappable events (None returns) gracefully
//! 7. **Yield** each flushed output as a stream item
//!
//! ## Newtypes
//!
//! - `QueueCapacity(usize)`: Maximum buffered events before flush
//! - `FlushIntervalMs(u64)`: Milliseconds between periodic flushes

use crate::actors::copilot::background_event_mapper::map_background_event_with_usage;
use augur_domain::TokenTrackerHandle;
use augur_domain::background_events::{
    BackgroundEventClassifier, BackgroundPanelMode, FlushIntervalMs, QueueCapacity,
};
use augur_domain::newtypes::NumericNewtype;
use augur_domain::types::AgentFeedOutput;
use copilot_sdk::SessionEventData;
use futures_util::stream::BoxStream;
use std::any::Any;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio::time::{Duration, Interval, interval};

/// Placeholder duration used to initialize the flush timer before first poll.
///
/// The actual flush interval is applied on the first call to `poll_next`, replacing
/// this sentinel value with the value from `StreamFeedConfig::flush_interval_ms`.
const TIMER_INIT_SENTINEL_MS: u64 = 1;

/// Duration value representing an uninitialized timer (zero-length period sentinel).
///
/// Used to detect whether the flush timer has already been initialized to its
/// configured interval. A period of zero indicates the timer is in its initial
/// placeholder state and must be replaced on the first `poll_next` call.
const TIMER_UNINIT_PERIOD_MS: u64 = 0;

/// Configuration for event stream buffering and flushing behavior.
///
/// Controls how background events are buffered, flushed, and filtered when
/// streaming from a channel to the agent feed display.
///
/// # Fields
///
/// - `mode`: Display mode determining which event priority tiers are shown
///   (Critical, Normal, or Debug)
/// - `max_queued_events`: Maximum number of mapped events to hold before flushing
///   to the output stream
/// - `flush_interval_ms`: Milliseconds between periodic auto-flush intervals
///
/// # Example
///
/// ```ignore
/// use crate::actors::copilot::background_feed_dispatcher::StreamFeedConfig;
/// use augur_domain::background_events::BackgroundPanelMode;
/// use augur_domain::newtypes::QueueCapacity;
/// use augur_domain::newtypes::FlushIntervalMs;
///
/// let config = StreamFeedConfig {
///     mode: BackgroundPanelMode::Normal,
///     max_queued_events: QueueCapacity::new(50),
///     flush_interval_ms: FlushIntervalMs::new(500),
/// };
/// ```
#[derive(Clone)]
pub struct StreamFeedConfig {
    /// Current display mode (Critical, Normal, or Debug).
    ///
    /// Determines which event priority tiers are included in output.
    /// - `Critical`: Only session blockers
    /// - `Normal`: Session blockers and progress updates
    /// - `Debug`: All events including verbose internal diagnostics
    pub mode: BackgroundPanelMode,

    /// Maximum number of mapped events to buffer before flushing.
    ///
    /// When the buffer reaches this capacity, all queued events are
    /// immediately flushed to the output stream.
    pub max_queued_events: QueueCapacity,

    /// Milliseconds between automatic flush intervals.
    ///
    /// Regardless of buffer fill level, all buffered events are flushed
    /// when this timer interval elapses. Use in combination with
    /// `max_queued_events` to ensure timely delivery of low-volume event
    /// streams.
    pub flush_interval_ms: FlushIntervalMs,

    /// Handle to the token-tracker actor for recording per-turn LLM usage.
    ///
    /// Background sessions emit `AssistantUsage` events; the dispatcher
    /// extracts the structured `LlmUsage` and forwards it here so costs
    /// accumulate in the same store as foreground turns.
    pub token_tracker: TokenTrackerHandle,

    /// Provider-owned classifier for mapping raw session events to domain priority tiers.
    pub classifier: Arc<dyn BackgroundEventClassifier>,
}

/// Streams background events from a channel, classifies, maps, and buffers them.
///
/// Transforms a stream of `SessionEventData` events into a stream of `AgentFeedOutput`
/// items for display in the background panel. Events are classified by priority,
/// mapped to display text, filtered according to the display mode, and buffered
/// for batched delivery.
///
/// # Arguments
///
/// - `config`: Configuration controlling buffer capacity, flush interval, and display mode
/// - `rx`: MPSC receiver channel receiving `SessionEventData` events
///
/// # Returns
///
/// An async stream yielding `AgentFeedOutput` items. Each item represents a mapped
/// background event ready for display.
///
/// # Behavior
///
/// The stream operates continuously:
///
/// 1. **Receive** events from `rx`
/// 2. **Classify** using `config.classifier.classify(event as &dyn Any)`
/// 3. **Map** using `map_background_event(event, priority, config.mode)`
/// 4. **Buffer** outputs up to `config.max_queued_events`
/// 5. **Flush** when:
///    - Buffer reaches capacity (all buffered outputs yielded)
///    - Timer interval `config.flush_interval_ms` elapses
/// 6. **Skip** unmappable events (None returns from map_background_event)
///
/// The stream terminates when the receiver channel closes (all senders dropped).
///
/// # Example
///
/// ```ignore
/// use tokio::sync::mpsc;
/// use crate::actors::copilot::background_feed_dispatcher::{StreamFeedConfig, stream_to_feed};
/// use augur_domain::background_events::BackgroundPanelMode;
/// use augur_domain::newtypes::QueueCapacity;
/// use augur_domain::newtypes::FlushIntervalMs;
/// use copilot_sdk::SessionEventData;
/// use futures_util::stream::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let (tx, rx) = mpsc::channel(100);
///     let config = StreamFeedConfig {
///         mode: BackgroundPanelMode::Normal,
///         max_queued_events: QueueCapacity::new(10),
///         flush_interval_ms: FlushIntervalMs::new(500),
///     };
///     
///     let mut stream = stream_to_feed(config, rx);
///     
///     // Send an event
///     // tx.send(event).await.ok();
///     
///     // Receive mapped output
///     // while let Some(output) = stream.next().await {
///     //     println!("Received: {:?}", output);
///     // }
/// }
/// ```
pub fn stream_to_feed(
    config: StreamFeedConfig,
    rx: mpsc::Receiver<SessionEventData>,
) -> BoxStream<'static, AgentFeedOutput> {
    let stream = BackgroundEventStream {
        config,
        rx,
        buffer: Vec::new(),
        flush_timer: interval(Duration::from_millis(TIMER_INIT_SENTINEL_MS)), // Will be reset to config value on first poll
    };
    Box::pin(stream)
}

/// Internal stream implementation for background event processing.
struct BackgroundEventStream {
    config: StreamFeedConfig,
    rx: mpsc::Receiver<SessionEventData>,
    buffer: Vec<AgentFeedOutput>,
    flush_timer: Interval,
}

impl BackgroundEventStream {
    /// Processes a received `SessionEventData` event: classifies, maps, records usage, and buffers.
    ///
    /// Returns `true` when the buffer has reached capacity, signalling the caller to yield.
    fn process_received_event(&mut self, event: SessionEventData) -> bool {
        let Some(priority) = self.config.classifier.classify(&event as &dyn Any) else {
            return false;
        };
        let mapped = map_background_event_with_usage(&event, priority, self.config.mode);
        if let Some(usage) = mapped.usage {
            self.config.token_tracker.record_usage(usage);
        }
        if let Some(output) = mapped.display {
            self.buffer.push(output);
            return self.buffer.len() >= self.config.max_queued_events.inner();
        }
        false
    }

    /// Polls the flush timer and yields from the buffer if it fires with pending items.
    ///
    /// Returns `Some(Poll)` when the caller of `poll_next` should return that value.
    /// Returns `None` when the timer fired but the buffer was empty - the outer loop should continue.
    fn poll_flush_timer(&mut self, cx: &mut Context<'_>) -> Option<Poll<Option<AgentFeedOutput>>> {
        match Pin::new(&mut self.flush_timer).poll_tick(cx) {
            Poll::Ready(_) if !self.buffer.is_empty() => {
                Some(Poll::Ready(Some(self.buffer.remove(0))))
            }
            Poll::Ready(_) => None,
            Poll::Pending => Some(Poll::Pending),
        }
    }

    fn initialize_flush_timer_if_needed(&mut self) {
        if self.flush_timer.period() == Duration::from_millis(TIMER_UNINIT_PERIOD_MS) {
            self.flush_timer =
                interval(Duration::from_millis(self.config.flush_interval_ms.inner()));
        }
    }

    fn pop_buffered_output(&mut self) -> Option<AgentFeedOutput> {
        (!self.buffer.is_empty()).then(|| self.buffer.remove(0))
    }

    fn poll_disconnected(&mut self) -> Poll<Option<AgentFeedOutput>> {
        self.pop_buffered_output()
            .map_or(Poll::Ready(None), |output| Poll::Ready(Some(output)))
    }

    fn poll_iteration(&mut self, cx: &mut Context<'_>) -> Option<Poll<Option<AgentFeedOutput>>> {
        if let Some(output) = self.pop_buffered_output() {
            return Some(Poll::Ready(Some(output)));
        }
        self.poll_iteration_from_receiver(cx)
    }

    fn poll_iteration_from_receiver(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Option<Poll<Option<AgentFeedOutput>>> {
        match self.rx.try_recv() {
            Ok(event) => self.poll_iteration_with_event(event),
            Err(mpsc::error::TryRecvError::Empty) => self.poll_flush_timer(cx),
            Err(mpsc::error::TryRecvError::Disconnected) => Some(self.poll_disconnected()),
        }
    }

    fn poll_iteration_with_event(
        &mut self,
        event: SessionEventData,
    ) -> Option<Poll<Option<AgentFeedOutput>>> {
        if !self.process_received_event(event) {
            return None;
        }
        self.pop_buffered_output()
            .map(|output| Poll::Ready(Some(output)))
    }
}

impl futures_util::Stream for BackgroundEventStream {
    type Item = AgentFeedOutput;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.initialize_flush_timer_if_needed();

        loop {
            if let Some(result) = self.poll_iteration(cx) {
                return result;
            }
        }
    }
}
