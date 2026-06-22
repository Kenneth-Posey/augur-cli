---
name: 0-external-test-gap-fusion
description: >
  Deterministic test-gap fusion analyzer that combines mirror mapping, coverage
  data, pipeline test results, and duplicate-effort signals into a minimal
  gaps-only report by default.
---

# 0-external-test-gap-fusion

## Purpose

Use this skill to produce a prioritized test-gap report. The default JSON
output is `gaps` only. Add `--cobertura-full` when file-level coverage details
are needed, and `--full` when the caller needs mirrors, duplicates, and the
rest of the collected payload.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-test-gap-fusion
cargo build --release
```

## Run

```bash
mkdir -p reports
.github/skills/0-external-test-gap-fusion/run.sh \
  --src src \
  --tests tests \
  --output reports/gap-report.json
```

## Detail flags

- `--cobertura-full` - include per-file coverage details
- `--full` - include the complete report payload
- `--output <file>` - override default output path (default: `reports/gap-report.json`)

## When to use more detail

- Use `--cobertura-full` for tarpaulin or llvm-cov handoffs that need file-level
  coverage evidence.
- Use `--full` only when the caller needs mirrors, duplicates, and coverage
  together for a deeper review pass.

## Examples

```bash
# Minimal default output (writes to reports/gap-report.json)
.github/skills/0-external-test-gap-fusion/run.sh

# Add coverage detail
mkdir -p reports
.github/skills/0-external-test-gap-fusion/run.sh \
  --cobertura reports/cobertura.xml \
  --cobertura-full \
  --output reports/gap-report.json

# Full report
mkdir -p reports
.github/skills/0-external-test-gap-fusion/run.sh \
  --src src \
  --tests tests \
  --pipeline-report reports/diagnostics.json \
  --cobertura reports/cobertura.xml \
  --llvm-cov reports/llvm-cov.json \
  --full \
  --output reports/gap-report.json
```
