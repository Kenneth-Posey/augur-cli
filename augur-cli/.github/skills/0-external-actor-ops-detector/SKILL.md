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

## Purpose

Discovers `actor.rs` and `actor_ops.rs` by module directory, reports missing
pairs and orphaned files, flags non-trivial functions and public helper
functions in `actor.rs`, and elevates severity when non-trivial actor logic
exists without `actor_ops` delegation. The tool is read-only and produces
deterministic text or JSON output.

## Run

```bash
.github/skills/0-external-actor-ops-detector/run.sh [OPTIONS] [TARGET_PATH]
```

## Arguments

| Argument / Flag | Description | Default |
|---|---|---|
| `TARGET_PATH` | Path to analyze | `src` |
| `--format <FORMAT>` | Output format: `text` or `json` | `text` |
| `--max-lines <MAX_LINES>` | Maximum function line span before non-trivial signal | `12` |
| `--max-chain <MAX_CHAIN>` | Maximum method-call chain length before non-trivial signal | `3` |
| `--max-complexity <MAX_COMPLEXITY>` | Maximum complexity heuristic score before non-trivial signal | `8` |
| `--allow-fn <ALLOWED_FN_NAMES>` | Additional exact allowlisted function name (repeatable) | — |
| `--allow-fn-regex <ALLOWED_FN_REGEX>` | Additional allowlisted name regex (repeatable) | — |
| `--include-fragment <INCLUDE_FRAGMENTS>` | Only analyze paths containing fragment (repeatable) | — |
| `--exclude-fragment <EXCLUDE_FRAGMENTS>` | Skip paths containing fragment (repeatable) | — |
| `--orphan-actor-ops-severity <ORPHAN_ACTOR_OPS_SEVERITY>` | Severity for orphaned `actor_ops` files: `warning` or `info` | `warning` |
| `-h`, `--help` | Print help | — |
| `-V`, `--version` | Print version | — |

## Output

The tool scans the target path and reports any `actor.rs`/`actor_ops.rs`
pairing violations, orphaned files, and non-trivial actor logic. All findings
are sorted for stable, deterministic output.

Exit codes:
- `0` — No error findings
- `1` — Error findings present
- `2` — Runtime or configuration error

Output format is controlled by `--format`:
- `text` — Human-readable report
- `json` — Machine-readable JSON

## Examples

Check the default `src` path for actor delegation violations:

```bash
.github/skills/0-external-actor-ops-detector/run.sh
```

Analyze a specific directory with relaxed line-count and complexity thresholds:

```bash
.github/skills/0-external-actor-ops-detector/run.sh my_crate/src --max-lines 20 --max-complexity 10
```

Exclude test helper paths and emit JSON for downstream processing:

```bash
.github/skills/0-external-actor-ops-detector/run.sh --format json --exclude-fragment test
```

Suppress orphaned-actor_ops warnings to informational level:

```bash
.github/skills/0-external-actor-ops-detector/run.sh --orphan-actor-ops-severity info
```

## Key Files

- `.github/skills/0-external-actor-ops-detector/run.sh` — Canonical wrapper for the actor-ops detector