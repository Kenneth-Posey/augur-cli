---
name: 0-external-actor-ops-detector
description: >
  Deterministic static analyzer that enforces actor.rs/actor_ops.rs pairing and
  flags likely business logic left in actor.rs. Reporting only; no code changes.
---

# actor-ops-detector

## When to use

Use this skill when you need deterministic CI-safe checks that actor behavior is
delegated to `actor_ops.rs` instead of being implemented in `actor.rs`.

## Scope

- Discovers `actor.rs` and `actor_ops.rs` by module directory.
- Reports missing pairs and orphaned files.
- Flags non-trivial functions and public helper functions in `actor.rs`.
- Elevates severity when non-trivial actor logic exists without `actor_ops` delegation.
- Emits deterministic text or JSON output.

## Run

```bash
.github/skills/0-external-actor-ops-detector/run.sh [src-path] [--format <format>]
```

## Arguments

- `[src-path]` - Path to analyze (default: `src`)
- `--format <format>` - Output format: `text` | `json` (default: `text`)
- `--max-lines <n>` - Maximum function line span before non-trivial signal
- `--max-chain <n>` - Maximum method-call chain length before non-trivial signal
- `--max-complexity <n>` - Maximum complexity heuristic score before non-trivial signal
- `--allow-fn <name>` - Additional exact allowlisted function name (repeatable)
- `--allow-fn-regex <re>` - Additional allowlisted name regex (repeatable)
- `--include-fragment <text>` - Only analyze paths containing fragment (repeatable)
- `--exclude-fragment <text>` - Skip paths containing fragment (repeatable)

## Determinism and safety

- Read-only reporting workflow.
- Files and findings are sorted for stable output.
- Exit codes: `0` no error findings, `1` error findings present, `2` runtime/config error.

## Key Files

- `run.sh` - Canonical wrapper for actor ops detector
