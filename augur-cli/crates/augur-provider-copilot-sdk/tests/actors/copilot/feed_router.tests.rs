//! Tests for `copilot::feed_router::FeedRouter` and `FeedChannels`.
//!
//! These tests require the `copilot-executor` feature because they use
//! `copilot_sdk::SessionEvent` directly and reference `FeedRouter`/`FeedChannels`.
//!
//! Each test verifies a single routing decision: where does `route_event` send
//! a given `SessionEvent` - to `main_out`, to `feed_out`, or neither?

mod suite {
    use tokio::sync::mpsc;

    use copilot_sdk::{
        AssistantMessageDeltaData, CustomAgentCompletedData, CustomAgentStartedData, SessionEvent,
        SessionEventData, SessionIdleData, ToolExecutionCompleteData, ToolExecutionStartData,
        UserMessageData,
    };

    use augur_domain::string_newtypes::{OutputText, StringNewtype};
    use augur_domain::types::{AgentFeedOutput, FeedId, RouteResult};
    use augur_provider_copilot_sdk::actors::copilot::feed_router::{FeedChannels, FeedRouter};

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_event(data: SessionEventData) -> SessionEvent {
        SessionEvent {
            id: "test-id".to_owned(),
            timestamp: "2024-01-01T00:00:00Z".to_owned(),
            event_type: "test".to_owned(),
            parent_id: None,
            ephemeral: None,
            data,
        }
    }

    fn make_tool_start(
        tool_name: &str,
        tool_call_id: &str,
        parent_id: Option<&str>,
    ) -> SessionEvent {
        make_event(SessionEventData::ToolExecutionStart(
            ToolExecutionStartData {
                tool_call_id: tool_call_id.to_owned(),
                tool_name: tool_name.to_owned(),
                arguments: None,
                parent_tool_call_id: parent_id.map(|s| s.to_owned()),
            },
        ))
    }

    fn make_custom_agent_started(tool_call_id: &str) -> SessionEvent {
        make_event(SessionEventData::CustomAgentStarted(
            CustomAgentStartedData {
                tool_call_id: tool_call_id.to_owned(),
                agent_name: "test-agent".to_owned(),
                agent_display_name: "Test Agent".to_owned(),
                agent_description: "A test agent".to_owned(),
            },
        ))
    }

    fn make_custom_agent_completed(tool_call_id: &str) -> SessionEvent {
        make_event(SessionEventData::CustomAgentCompleted(
            CustomAgentCompletedData {
                tool_call_id: tool_call_id.to_owned(),
                agent_name: "test-agent".to_owned(),
            },
        ))
    }

    fn make_tool_complete(tool_call_id: &str, parent_id: Option<&str>) -> SessionEvent {
        make_event(SessionEventData::ToolExecutionComplete(
            ToolExecutionCompleteData {
                tool_call_id: tool_call_id.to_owned(),
                success: true,
                is_user_requested: None,
                result: None,
                error: None,
                tool_telemetry: None,
                parent_tool_call_id: parent_id.map(|s| s.to_owned()),
                mcp_server_name: None,
                mcp_tool_name: None,
            },
        ))
    }

    fn make_user_message(content: &str) -> SessionEvent {
        make_event(SessionEventData::UserMessage(UserMessageData {
            content: content.to_owned(),
            transformed_content: None,
            attachments: None,
            source: None,
        }))
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    /// A `SessionIdle` event has no parent tool call and is not agent-related.
    /// `feed_out` must be `None`; `main_out` must be `Some` (maps to `TurnComplete`).
    #[test]
    fn extract_parent_id_returns_none_for_session_idle() {
        let mut router = FeedRouter::new();
        let event = make_event(SessionEventData::SessionIdle(SessionIdleData {}));
        let result: RouteResult = router.route_event(&event);

        assert!(
            result.feed_out.is_none(),
            "SessionIdle must not route to any feed, got {:?}",
            result.feed_out
        );
        assert!(
            result.main_out.is_some(),
            "SessionIdle must produce main_out (TurnComplete), got None"
        );
    }

    /// An `AssistantMessageDelta` with `parent_tool_call_id` set routes exclusively
    /// to the agent feed keyed by that parent id. `main_out` must be suppressed.
    #[test]
    fn extract_parent_id_returns_some_for_delta_with_parent() {
        let mut router = FeedRouter::new();
        let event = make_event(SessionEventData::AssistantMessageDelta(
            AssistantMessageDeltaData {
                message_id: "m1".to_owned(),
                delta_content: "thinking...".to_owned(),
                total_response_size_bytes: None,
                parent_tool_call_id: Some("tc-outer".to_owned()),
            },
        ));
        let result: RouteResult = router.route_event(&event);

        assert!(
            result.main_out.is_none(),
            "delta with parent must be suppressed from main, got {:?}",
            result.main_out
        );
        match result.feed_out {
            Some(entry) => {
                assert_eq!(
                    entry.feed_id,
                    FeedId::Agent("tc-outer".into()),
                    "feed_id must be Agent(\"tc-outer\")"
                );
            }
            None => panic!("expected feed_out to be Some, got None"),
        }
    }

    /// `FeedChannels::single` wraps one `mpsc::Sender`. Sending a `FeedEntry`
    /// with `FeedId::Agent` delivers `AgentFeedOutput` to the receiver.
    #[tokio::test]
    async fn feed_channels_single_send_agent_feed() {
        use augur_domain::types::FeedEntry;

        let (tx, mut rx) = mpsc::channel::<augur_domain::types::FeedEntry>(8);
        let channels = FeedChannels::single(tx);

        let sent = channels
            .send(FeedEntry {
                feed_id: FeedId::Agent("tc1".into()),
                output: AgentFeedOutput::StatusLine(OutputText::new("hello from agent".to_owned())),
            })
            .await;

        assert!(sent.is_ok(), "send to agent feed must succeed");
        let received = rx.try_recv().expect("receiver must have one item");
        match received.output {
            AgentFeedOutput::StatusLine(text) => {
                assert_eq!(text.to_string(), "hello from agent");
            }
            other => panic!("expected StatusLine, got {:?}", other),
        }
    }

    /// `FeedChannels::send` with `FeedId::MainConversation` is a no-op.
    /// Returns `true` and nothing arrives on the agent receiver.
    #[tokio::test]
    async fn feed_channels_main_conversation_is_noop() {
        use augur_domain::types::FeedEntry;

        let (tx, mut rx) = mpsc::channel::<augur_domain::types::FeedEntry>(8);
        let channels = FeedChannels::single(tx);

        let sent = channels
            .send(FeedEntry {
                feed_id: FeedId::MainConversation,
                output: AgentFeedOutput::StatusLine(OutputText::new("noop".to_owned())),
            })
            .await;

        assert!(
            sent.is_ok(),
            "send for MainConversation must succeed (no-op)"
        );
        assert!(
            rx.try_recv().is_err(),
            "no item must be delivered to agent receiver for MainConversation"
        );
    }

    /// An `AssistantMessageDelta` with no `parent_tool_call_id` while state is
    /// `Idle` routes to `main_out` as a `Token`. `feed_out` must be `None`.
    #[test]
    fn router_idle_main_session_delta_routes_to_main() {
        use augur_domain::types::AgentOutput;

        let mut router = FeedRouter::new();
        let event = make_event(SessionEventData::AssistantMessageDelta(
            AssistantMessageDeltaData {
                message_id: "m2".to_owned(),
                delta_content: "hello main".to_owned(),
                total_response_size_bytes: None,
                parent_tool_call_id: None,
            },
        ));
        let result: RouteResult = router.route_event(&event);

        assert!(
            result.feed_out.is_none(),
            "Idle delta without parent must not route to feed, got {:?}",
            result.feed_out
        );
        match result.main_out {
            Some(AgentOutput::Token(text)) => {
                assert_eq!(text.to_string(), "hello main");
            }
            other => panic!("expected main_out=Some(Token), got {:?}", other),
        }
    }

    /// An `AssistantMessageDelta` with no parent while state is `AgentActive`
    /// must still reach `main_out`; the agent panel may also receive the feed copy.
    #[test]
    fn router_agent_active_delta_routes_to_main_and_feed() {
        use augur_domain::types::AgentOutput;

        let mut router = FeedRouter::new();

        // Advance state: TaskPending → AgentActive
        let tc1 = "tc-task-1";
        let _ = router.route_event(&make_tool_start("task", tc1, None));
        let _ = router.route_event(&make_custom_agent_started(tc1));

        // Now state == AgentActive; delta without parent must still reach main.
        let event = make_event(SessionEventData::AssistantMessageDelta(
            AssistantMessageDeltaData {
                message_id: "m3".to_owned(),
                delta_content: "agent output".to_owned(),
                total_response_size_bytes: None,
                parent_tool_call_id: None,
            },
        ));
        let result: RouteResult = router.route_event(&event);

        assert!(
            matches!(result.main_out, Some(AgentOutput::Token(_))),
            "AgentActive delta must reach main_out as Token, got {:?}",
            result.main_out
        );
        assert!(
            result.feed_out.is_some(),
            "AgentActive delta must still be routed to feed_out, got None"
        );
    }

    /// An `AssistantMessageDelta` with `parent_tool_call_id` set routes to the
    /// agent feed regardless of router state. `main_out` is always `None`.
    #[test]
    fn router_parent_tool_call_id_routes_delta_to_feed() {
        let mut router = FeedRouter::new();
        let event = make_event(SessionEventData::AssistantMessageDelta(
            AssistantMessageDeltaData {
                message_id: "m4".to_owned(),
                delta_content: "outer delta".to_owned(),
                total_response_size_bytes: None,
                parent_tool_call_id: Some("outer-tc".to_owned()),
            },
        ));
        let result: RouteResult = router.route_event(&event);

        assert!(
            result.main_out.is_none(),
            "delta with parent must not appear in main_out, got {:?}",
            result.main_out
        );
        match result.feed_out {
            Some(entry) => {
                assert_eq!(
                    entry.feed_id,
                    FeedId::Agent("outer-tc".into()),
                    "feed_id must be Agent(\"outer-tc\")"
                );
            }
            None => panic!("expected feed_out=Some(Agent(\"outer-tc\")), got None"),
        }
    }

    /// A `ToolExecutionStart` with `tool_name="task"` is the scaffold event that
    /// spawns a background agent. It must be suppressed from `main_out` and the
    /// router must transition to `TaskPending`.
    ///
    /// We verify the state transition indirectly: a subsequent `CustomAgentStarted`
    /// (which only produces feed output from `TaskPending`) must yield `feed_out=Some`.
    #[test]
    fn router_task_tool_start_suppressed_from_main() {
        let mut router = FeedRouter::new();
        let event = make_tool_start("task", "tc-task-1", None);
        let result: RouteResult = router.route_event(&event);

        assert!(
            result.main_out.is_none(),
            "ToolExecutionStart(task) must be suppressed from main_out, got {:?}",
            result.main_out
        );

        // Verify state advanced to TaskPending by confirming the next
        // CustomAgentStarted produces a feed entry (only valid after TaskPending)
        let started_result = router.route_event(&make_custom_agent_started("tc-task-1"));
        assert!(
            started_result.feed_out.is_some(),
            "CustomAgentStarted after task-start must route to feed, got None"
        );
    }

    /// A `ToolExecutionStart` for an inner tool (not "task") with
    /// `parent_tool_call_id` set must be suppressed from `main_out` and routed
    /// to the agent feed identified by the parent id.
    #[test]
    fn router_inner_tool_start_with_parent_routes_to_feed() {
        let mut router = FeedRouter::new();
        let event = make_tool_start("bash", "tc-bash-1", Some("outer-tc"));
        let result: RouteResult = router.route_event(&event);

        assert!(
            result.main_out.is_none(),
            "inner tool start must be suppressed from main_out, got {:?}",
            result.main_out
        );
        match result.feed_out {
            Some(entry) => {
                assert_eq!(
                    entry.feed_id,
                    FeedId::Agent("outer-tc".into()),
                    "feed_id must be Agent(\"outer-tc\")"
                );
            }
            None => panic!("expected feed_out=Some for inner tool start with parent, got None"),
        }
    }

    /// After advancing state to `TaskPending` via `ToolExecutionStart("task")`,
    /// a `CustomAgentStarted` event routes to `feed_out` and suppresses `main_out`.
    #[test]
    fn router_custom_agent_started_routes_to_feed() {
        let mut router = FeedRouter::new();
        let tc1 = "tc-task-2";

        // Advance to TaskPending
        let _ = router.route_event(&make_tool_start("task", tc1, None));

        let event = make_custom_agent_started(tc1);
        let result: RouteResult = router.route_event(&event);

        assert!(
            result.main_out.is_none(),
            "CustomAgentStarted must not produce main_out, got {:?}",
            result.main_out
        );
        match result.feed_out {
            Some(entry) => {
                assert_eq!(
                    entry.feed_id,
                    FeedId::Agent(tc1.into()),
                    "feed_id must match the task tool_call_id"
                );
            }
            None => panic!("expected feed_out=Some for CustomAgentStarted, got None"),
        }
    }

    /// Regression: when multiple task tool calls are queued in parallel, each
    /// `CustomAgentStarted` must emit `TaskStarted` into its own feed id even
    /// when start events arrive interleaved.
    #[test]
    fn router_parallel_interleaved_custom_agent_started_emits_each_feed_once() {
        let mut router = FeedRouter::new();
        let ids = ["tc-par-1", "tc-par-2", "tc-par-3", "tc-par-4"];

        // Queue multiple background task tool starts first.
        for id in ids {
            let _ = router.route_event(&make_tool_start("task", id, None));
        }

        // Interleave custom-start events; each one should still route.
        let started_order = ["tc-par-1", "tc-par-3", "tc-par-2", "tc-par-4"];
        let mut routed_feed_ids = std::collections::HashSet::new();
        for id in started_order {
            let result = router.route_event(&make_custom_agent_started(id));
            let Some(entry) = result.feed_out else {
                panic!("CustomAgentStarted({id}) must route to feed_out");
            };
            assert_eq!(
                entry.feed_id,
                FeedId::Agent(id.into()),
                "CustomAgentStarted({id}) must route to its own feed id"
            );
            routed_feed_ids.insert(entry.feed_id);
        }

        assert_eq!(
            routed_feed_ids.len(),
            4,
            "parallel interleaved starts must produce four distinct feed ids"
        );
    }

    /// After completing the full lifecycle (Start → AgentActive → AwaitingCompletion),
    /// the matching `ToolExecutionComplete` must be suppressed from `main_out` and
    /// the router must reset to `Idle` (verified by a subsequent Idle-state routing check).
    #[test]
    fn router_tool_complete_awaiting_suppressed_from_main() {
        let mut router = FeedRouter::new();
        let tc1 = "tc-task-3";

        // Build full lifecycle: Idle → TaskPending → AgentActive → AwaitingCompletion
        let _ = router.route_event(&make_tool_start("task", tc1, None));
        let _ = router.route_event(&make_custom_agent_started(tc1));
        let _ = router.route_event(&make_custom_agent_completed(tc1));

        // ToolExecutionComplete matching tc1 while state=AwaitingCompletion
        let event = make_tool_complete(tc1, None);
        let result: RouteResult = router.route_event(&event);

        assert!(
            result.main_out.is_none(),
            "ToolExecutionComplete in AwaitingCompletion must be suppressed from main_out, got {:?}",
            result.main_out
        );

        // Verify state reset to Idle: a fresh delta without parent should route to main
        use augur_domain::types::AgentOutput;
        let idle_check = router.route_event(&make_event(SessionEventData::AssistantMessageDelta(
            AssistantMessageDeltaData {
                message_id: "m5".to_owned(),
                delta_content: "back to main".to_owned(),
                total_response_size_bytes: None,
                parent_tool_call_id: None,
            },
        )));
        assert!(
            matches!(idle_check.main_out, Some(AgentOutput::Token(_))),
            "after reset to Idle, delta without parent must route to main_out"
        );
    }

    /// Regression: if a task tool completes without an explicit
    /// `CustomAgentCompleted` event, the router must still return to `Idle` so the
    /// next main-conversation assistant output is not suppressed.
    #[test]
    fn router_task_tool_complete_without_custom_completed_restores_main_feed() {
        use augur_domain::types::AgentOutput;
        use copilot_sdk::AssistantMessageData;

        let mut router = FeedRouter::new();
        let tc1 = "tc-task-4";

        // 1) Main-conversation response appears in main feed.
        let first_main_delta = make_event(SessionEventData::AssistantMessageDelta(
            AssistantMessageDeltaData {
                message_id: "m-main-1".to_owned(),
                delta_content: "main prelude".to_owned(),
                total_response_size_bytes: None,
                parent_tool_call_id: None,
            },
        ));
        let first_main_result = router.route_event(&first_main_delta);
        assert!(
            matches!(first_main_result.main_out, Some(AgentOutput::Token(_))),
            "main delta before background task must reach main_out"
        );

        // 2) Background agent runs and updates feed panel.
        let _ = router.route_event(&make_tool_start("task", tc1, None));
        let _ = router.route_event(&make_custom_agent_started(tc1));
        let bg_delta = make_event(SessionEventData::AssistantMessageDelta(
            AssistantMessageDeltaData {
                message_id: "m-bg-1".to_owned(),
                delta_content: "background update".to_owned(),
                total_response_size_bytes: None,
                parent_tool_call_id: Some(tc1.to_owned()),
            },
        ));
        let bg_result = router.route_event(&bg_delta);
        assert!(
            bg_result.main_out.is_none(),
            "background delta must not hit main_out"
        );
        assert!(
            bg_result.feed_out.is_some(),
            "background delta must route to feed_out"
        );

        // 3) Task tool completes even though no CustomAgentCompleted event arrived.
        let tool_done = router.route_event(&make_tool_complete(tc1, None));
        assert!(
            tool_done.main_out.is_none(),
            "task tool completion scaffold must stay suppressed from main_out"
        );

        // 4) Main-conversation response must resume in the main feed.
        let resumed_delta = make_event(SessionEventData::AssistantMessageDelta(
            AssistantMessageDeltaData {
                message_id: "m-main-2".to_owned(),
                delta_content: "main resumed".to_owned(),
                total_response_size_bytes: None,
                parent_tool_call_id: None,
            },
        ));
        let resumed_delta_result = router.route_event(&resumed_delta);
        assert!(
            matches!(resumed_delta_result.main_out, Some(AgentOutput::Token(_))),
            "main delta after background completion must reach main_out"
        );
        assert!(
            resumed_delta_result.feed_out.is_none(),
            "main delta after background completion must not be routed to feed_out"
        );

        let resumed_message =
            make_event(SessionEventData::AssistantMessage(AssistantMessageData {
                message_id: "m-main-3".to_owned(),
                content: "main done".to_owned(),
                chunk_content: None,
                total_response_size_bytes: None,
                tool_requests: None,
                parent_tool_call_id: None,
            }));
        let resumed_message_result = router.route_event(&resumed_message);
        assert!(
            matches!(resumed_message_result.main_out, Some(AgentOutput::Done)),
            "main AssistantMessage after background completion must reach main_out as Done"
        );
    }

    /// Regression: a tool-request assistant boundary must not emit `Done`, and a
    /// subsequent failed tool completion must not stop follow-up main-feed deltas.
    #[test]
    fn router_tool_failure_keeps_main_feed_progressing() {
        use augur_domain::types::AgentOutput;
        use copilot_sdk::{AssistantMessageData, ToolExecutionCompleteData, ToolExecutionError};

        let mut router = FeedRouter::new();

        let assistant_tool_boundary =
            make_event(SessionEventData::AssistantMessage(AssistantMessageData {
                message_id: "m-tool-boundary".to_owned(),
                content: "calling tool".to_owned(),
                chunk_content: None,
                total_response_size_bytes: None,
                tool_requests: Some(vec![]),
                parent_tool_call_id: None,
            }));
        let boundary_result = router.route_event(&assistant_tool_boundary);
        assert!(
            matches!(boundary_result.main_out, Some(AgentOutput::MessageBreak)),
            "tool-request assistant message must remain in-turn as MessageBreak; got {:?}",
            boundary_result.main_out
        );

        let failed_tool_complete = make_event(SessionEventData::ToolExecutionComplete(
            ToolExecutionCompleteData {
                tool_call_id: "tc-fail-main".to_owned(),
                success: false,
                is_user_requested: None,
                result: None,
                error: Some(ToolExecutionError {
                    message: "No such file or directory (os error 2)".to_owned(),
                    code: None,
                }),
                tool_telemetry: None,
                parent_tool_call_id: None,
                mcp_server_name: None,
                mcp_tool_name: None,
            },
        ));
        let _ = router.route_event(&failed_tool_complete);

        let resumed_delta = make_event(SessionEventData::AssistantMessageDelta(
            AssistantMessageDeltaData {
                message_id: "m-resume".to_owned(),
                delta_content: "continued after tool failure".to_owned(),
                total_response_size_bytes: None,
                parent_tool_call_id: None,
            },
        ));
        let resumed_result = router.route_event(&resumed_delta);
        assert!(
            matches!(resumed_result.main_out, Some(AgentOutput::Token(_))),
            "main-feed delta after failed tool must continue routing; got {:?}",
            resumed_result.main_out
        );
    }

    /// A `SessionIdle` event on a fresh router produces no `feed_out`.
    /// (Duplicate of test 1 from a state-verification angle rather than
    /// `extract_parent_id` angle - verifies the fallback branch produces `None`.)
    #[test]
    fn router_fallback_idle_state_no_feed_output() {
        let mut router = FeedRouter::new();
        let event = make_event(SessionEventData::SessionIdle(SessionIdleData {}));
        let result: RouteResult = router.route_event(&event);

        assert!(
            result.feed_out.is_none(),
            "fallback Idle state must yield feed_out=None, got {:?}",
            result.feed_out
        );
    }

    /// An `AssistantMessage` (end-of-turn) while `AgentActive` must still reach
    /// `main_out` so the main conversation can render the assistant boundary.
    #[test]
    fn router_agent_active_assistant_message_reaches_main_out() {
        use augur_domain::types::AgentOutput;
        use copilot_sdk::AssistantMessageData;

        let mut router = FeedRouter::new();
        let tc1 = "tc-task-1";
        let _ = router.route_event(&make_tool_start("task", tc1, None));
        let _ = router.route_event(&make_custom_agent_started(tc1));

        let event = make_event(SessionEventData::AssistantMessage(AssistantMessageData {
            message_id: "m1".to_owned(),
            content: "finished".to_owned(),
            chunk_content: None,
            total_response_size_bytes: None,
            tool_requests: None,
            parent_tool_call_id: None,
        }));
        let result: RouteResult = router.route_event(&event);

        assert!(
            matches!(result.main_out, Some(AgentOutput::Done)),
            "AssistantMessage while AgentActive must reach main_out as Done; got {:?}",
            result.main_out
        );
        assert!(
            matches!(
                result.feed_out.as_ref().map(|e| &e.output),
                Some(AgentFeedOutput::MessageBreak)
            ),
            "AssistantMessage while AgentActive must still produce MessageBreak in feed_out; got {:?}",
            result.feed_out
        );
    }

    /// An `AssistantMessage` while `AgentActive` must produce a `MessageBreak` in
    /// `feed_out` so the agent panel flushes accumulated streaming text as one line.
    #[test]
    fn router_agent_active_assistant_message_routes_message_break_to_feed() {
        use copilot_sdk::AssistantMessageData;

        let mut router = FeedRouter::new();
        let tc1 = "tc-task-1";
        let _ = router.route_event(&make_tool_start("task", tc1, None));
        let _ = router.route_event(&make_custom_agent_started(tc1));

        let event = make_event(SessionEventData::AssistantMessage(AssistantMessageData {
            message_id: "m2".to_owned(),
            content: "done".to_owned(),
            chunk_content: None,
            total_response_size_bytes: None,
            tool_requests: None,
            parent_tool_call_id: None,
        }));
        let result: RouteResult = router.route_event(&event);

        assert!(
            matches!(
                result.feed_out.as_ref().map(|e| &e.output),
                Some(AgentFeedOutput::MessageBreak)
            ),
            "AssistantMessage while AgentActive must produce MessageBreak in feed_out; got {:?}",
            result.feed_out
        );
    }

    /// While a background agent is in `AwaitingCompletion`, an `AssistantMessage`
    /// without parent id must still reach the main feed.
    #[test]
    fn router_awaiting_completion_assistant_message_reaches_main_out() {
        use augur_domain::types::AgentOutput;
        use copilot_sdk::AssistantMessageData;

        let mut router = FeedRouter::new();
        let tc1 = "tc-task-awaiting-msg";
        // Idle -> TaskPending -> AgentActive -> AwaitingCompletion
        let _ = router.route_event(&make_tool_start("task", tc1, None));
        let _ = router.route_event(&make_custom_agent_started(tc1));
        let _ = router.route_event(&make_custom_agent_completed(tc1));

        let event = make_event(SessionEventData::AssistantMessage(AssistantMessageData {
            message_id: "awaiting-msg".to_owned(),
            content: "subagent boundary".to_owned(),
            chunk_content: None,
            total_response_size_bytes: None,
            tool_requests: None,
            parent_tool_call_id: None,
        }));
        let result: RouteResult = router.route_event(&event);

        assert!(
            matches!(result.main_out, Some(AgentOutput::Done)),
            "AssistantMessage in AwaitingCompletion must reach main_out as Done; got {:?}",
            result.main_out
        );
    }

    /// While a background agent is in `AwaitingCompletion`, `SessionIdle` must
    /// still reach the main feed as `TurnComplete`.
    #[test]
    fn router_awaiting_completion_session_idle_reaches_main_out() {
        use augur_domain::types::AgentOutput;

        let mut router = FeedRouter::new();
        let tc1 = "tc-task-awaiting-idle";

        // Idle -> TaskPending -> AgentActive -> AwaitingCompletion
        let _ = router.route_event(&make_tool_start("task", tc1, None));
        let _ = router.route_event(&make_custom_agent_started(tc1));
        let _ = router.route_event(&make_custom_agent_completed(tc1));

        let result: RouteResult = router.route_event(&make_event(SessionEventData::SessionIdle(
            SessionIdleData {},
        )));

        assert!(
            matches!(result.main_out, Some(AgentOutput::TurnComplete)),
            "SessionIdle in AwaitingCompletion must emit TurnComplete on main_out; got {:?}",
            result.main_out
        );
    }

    /// Regression: a new top-level user turn must recover routing from stale
    /// background-agent state so subsequent no-parent assistant deltas return
    /// to the main conversation feed.
    #[test]
    fn router_user_message_resets_stale_background_state_before_next_main_delta() {
        use augur_domain::types::AgentOutput;

        let mut router = FeedRouter::new();
        let tc1 = "tc-stale-agent-active";

        // Simulate stale state stuck in AgentActive.
        let _ = router.route_event(&make_tool_start("task", tc1, None));
        let _ = router.route_event(&make_custom_agent_started(tc1));

        // New top-level user turn begins.
        let _ = router.route_event(&make_user_message("fresh prompt"));

        // Next no-parent assistant delta must route back to main.
        let resumed_delta = make_event(SessionEventData::AssistantMessageDelta(
            AssistantMessageDeltaData {
                message_id: "m-main-after-user".to_owned(),
                delta_content: "main response chunk".to_owned(),
                total_response_size_bytes: None,
                parent_tool_call_id: None,
            },
        ));
        let resumed = router.route_event(&resumed_delta);
        assert!(
            matches!(resumed.main_out, Some(AgentOutput::Token(_))),
            "no-parent delta after UserMessage must route to main_out, got {:?}",
            resumed.main_out
        );
        assert!(
            resumed.feed_out.is_none(),
            "no-parent delta after UserMessage must not route to agent feed, got {:?}",
            resumed.feed_out
        );
    }

    /// An `AssistantMessage` while `Idle` (main session turn) must NOT be suppressed.
    /// The main conversation feed relies on this `Done` event to end the turn display.
    #[test]
    fn router_idle_assistant_message_reaches_main_out() {
        use copilot_sdk::AssistantMessageData;

        let mut router = FeedRouter::new();

        let event = make_event(SessionEventData::AssistantMessage(AssistantMessageData {
            message_id: "m3".to_owned(),
            content: "main reply".to_owned(),
            chunk_content: None,
            total_response_size_bytes: None,
            tool_requests: None,
            parent_tool_call_id: None,
        }));
        let result: RouteResult = router.route_event(&event);

        assert!(
            matches!(
                result.main_out,
                Some(augur_domain::types::AgentOutput::Done)
            ),
            "AssistantMessage while Idle must reach main_out as Done; got {:?}",
            result.main_out
        );
        assert!(
            result.feed_out.is_none(),
            "AssistantMessage while Idle must produce no feed_out; got {:?}",
            result.feed_out
        );
    }
}
