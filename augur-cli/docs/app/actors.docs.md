# actors

## Scope

The `actors` directory at `crates/augur-app/src/actors/` holds test-only scaffolding for
actor integration testing within the application crate. It is **not** declared as a
`pub mod actors` in `lib.rs` and contains no runtime module or executable code. The
directory exists solely to host test fixtures that exercise `crates/augur-core` actors
through the application wiring layer.

## Key Components

- `tests/actors/lsp/` - An empty directory reserved for LSP actor test fixtures. No
  test files currently reside here; the path provides a convention for future
  integration tests that require wire-protocol stubs or canned LSP responses.

## Role in the Ecosystem

The application crate (`augur-app`) is the wiring composition root that connects
actors from `augur-core` and `augur-tui` into a running process. The actors
themselves live in those downstream crates. The `src/actors/tests/` structure
mirrors the pattern used elsewhere in the project for test-only code that verifies
actor integration at the wiring boundary, but the module is not yet active in the
crate's public surface.

Developers adding new actor integration tests should place wire-protocol stubs and
mock actors in this directory tree, respecting the same path conventions used by
the crate's test mirror under `tests/`.