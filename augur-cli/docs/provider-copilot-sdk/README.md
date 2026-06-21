# augur-provider-copilot-sdk

Integrates GitHub Copilot chat functionality via the Copilot SDK. Contains the Copilot chat actor for session lifecycle, the executor actor for CLI-based plan execution, guided-plan hook runners for agent reviews, background agent dispatch, and feed routing infrastructure for multi-feed output distribution.## Documents

- [Crate Overview](crate-overview.docs.md) -- Architecture, major subsystems, and design decisions for the augur-provider-copilot-sdk crate.
- [actors](actors.docs.md) -- Chat actor, background agent dispatch, and executor actor for Copilot SDK session lifecycle and event streaming.
- [guided_plan](guided_plan.docs.md) -- Guided-plan hook runners that create Copilot SDK sessions for post-phase verification with approve/rework verdicts.
- [shared](shared.docs.md) -- Shared permission handler and session identity helpers used across all Copilot SDK sessions.