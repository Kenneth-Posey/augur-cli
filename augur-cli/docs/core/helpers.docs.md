# Helpers Module

The `helpers` module provides a suite of fake actor implementations for deterministic testing. These fakes substitute for real actor handles during unit and integration tests, allowing test code to drive the system without real LLM endpoints, filesystem operations, or concurrent actor mailboxes.

## Available Fakes

The module includes fakes for every major actor role: **`fake_llm`** (simulates LLM completion responses with configurable output), **`fake_tool`** (captures tool-call invocations and returns canned results), **`fake_logger`** (records log entries in memory for assertion), **`fake_orchestrator`** (replaces the deterministic orchestrator with a predictable state machine), **`fake_ask`** (returns pre-configured answers to user prompts), **`fake_history_adapter`** (produces formatted conversation history without real LLM bindings), **`fake_token_tracker`** (tracks token counts in memory), **`fake_catalog_manager`** (serves a fixed provider catalog), and **`fake_user_message_consumer`** (simulates user message ingestion).

## Architectural Role

The helpers module is the test infrastructure that makes the actor-based architecture testable at multiple granularities. Individual actor tests use the relevant fake (for example, a test for the agent actor instantiates `fake_tool` and `fake_llm` to control what the agent sees during a turn). Integration tests compose multiple fakes to simulate full workflows without network or filesystem dependencies. Because all fakes implement the same handle interfaces as their real counterparts, test code never needs conditional compilation or feature flags--it simply chooses which handle implementation to wire into the subject under test.