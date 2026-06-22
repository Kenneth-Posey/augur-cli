# augur-app

The application entrypoint and wiring composition root. This crate holds the CLI argument parser, tracing initialization, config loading, and the actor-wiring surface that assembles all domain, planning, and UI actors into a running runtime. It also manages lifecycle and shutdown ordering.

## Documents

- [Crate Overview](crate-overview.docs.md) -- architectural overview, subsystem grouping, and wiring-layer role.
- [wiring](wiring.docs.md) -- composition root, actor-graph construction, and lifecycle management.
- [actors](actors.docs.md) -- test-only actor scaffolding and integration test fixtures.