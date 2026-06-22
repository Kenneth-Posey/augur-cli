# guided_plan Module

The `guided_plan` module provides Copilot-powered hook runners for the guided-plan post-phase verification pipeline. Its single sub-module, `guided_plan::hooks`, implements a `CopilotAgentHookRunner` that creates short-lived Copilot SDK sessions to review a phase's output and return a verdict of `Passed`, `NeedsRework`, or `Failed`.

## Hook Runner Architecture

The hook runner (`build_copilot_hook_runner()`) returns a closure that accepts `CopilotAgentHookArgs` and returns `HookOutcome`. When invoked, it builds a fresh `copilot_sdk::Client`, starts it, creates a session configured with two custom tools---`approve_phase` and `request_rework`---and sends the review prompt. The session's streaming events are consumed in a loop: `AssistantMessageDelta` tokens are forwarded as `GuidedPlanEvent::ReviewToken` on a broadcast channel for TUI rendering, and when the session reaches `SessionIdle`, the verdict is resolved. Test-only agent names (`guided-plan-test-approve`, `guided-plan-test-request-rework`) short-circuit deterministically without SDK interaction.

## Verdict Resolution

The module supports two verdict strategies via `VerdictKind`. In `ToolCall` mode, the `approve_phase` and `request_rework` tool handlers set a shared `HookOutcome` behind an `Arc<Mutex<...>>`; the `SessionIdle` event reads this value, falling back to `Failed` if no tool was called. In `VerdictSuffix` mode, the accumulated assistant text is scanned for `VERDICT: PASS` or `VERDICT: REWORK(...)` markers using `check_verdict_suffix()`. A 300-second timeout guards against hung SDK sessions, returning `Failed` with a timeout reason if the session does not complete in time.