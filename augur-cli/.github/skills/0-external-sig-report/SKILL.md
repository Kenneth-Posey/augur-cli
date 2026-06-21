---
name: 0-external-sig-report
description: >
  Consolidation signal analyzer that loads rustdoc JSON and runs minimal
  signature reports by default, with broader presets for consolidation and
  full-report review handoffs.
---

# 0-external-sig-report

## Purpose

Use this skill to analyze rustdoc JSON for signature-review evidence. Default
JSON output is findings-only, and the minimal preset is
`--function-signatures`.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-sig-report
cargo build --release
```

## Run

```bash
.github/skills/0-external-sig-report/run.sh <rustdoc.json> \
  --function-signatures \
  --output-format json
```

## Presets

- `--function-signatures` - minimal default for signature review
- `--consolidation` - broader consolidation evidence
- `--all-reports` - every JSON-capable report family
- `--reports <A-H>` - explicit report selection, overrides presets

## Snapshot handling

- `--snapshot generated` - build rustdoc and write the snapshot to
  `reports/rustdoc.json` unless `--snapshot-output` overrides the path
- `--snapshot provided:<path>` - use an existing rustdoc JSON file
- `--snapshot cached:<path>` - use a cached snapshot path

## When to request more detail

- Use `--consolidation` when the review handoff needs duplicate-signature and
  related refactoring evidence.
- Use `--all-reports` only when the caller explicitly needs every report family.

## Examples

```bash
# Minimal signature review
.github/skills/0-external-sig-report/run.sh <myapp>.json \
  --function-signatures \
  --output-format json

# Broader consolidation pass
.github/skills/0-external-sig-report/run.sh <myapp>.json \
  --consolidation \
  --output-format json

# Generate rustdoc into the repo-root reports directory
.github/skills/0-external-sig-report/run.sh \
  --snapshot generated \
  --snapshot-output reports/rustdoc.json \
  --function-signatures
```
