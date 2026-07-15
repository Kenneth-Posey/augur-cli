---
name: 0-external-test-gap-fusion
description: >
  Deterministic test-gap fusion analyzer that combines mirror mapping, coverage
  data, pipeline test results, and duplicate-effort signals into a minimal
  gaps-only report by default.
---

# test-gap-fusion

## When to use

Use this skill when you need a consolidated, prioritized test-gap report that
combines mirror mapping, coverage data, pipeline test results, and
duplicate-effort signals. The default output is a minimal gaps-only report.
Use `--cobertura-full` when per-file coverage details are needed, and `--full`
when the caller needs mirrors, duplicates, and the rest of the collected
payload.

## Purpose

Produce a prioritized test-gap report by fusing mirror-mapping, pipeline test
results, coverage data, and duplicate-effort signals into a single JSON output.

## Run

```bash
mkdir -p reports
.github/skills/0-external-test-gap-fusion/run.sh \
  --src src \
  --tests tests \
  --output reports/gap-report.json
```

## Arguments

| Argument | Description | Default |
|---|---|---|
| `--src <SRC>` | Path to the Rust `src/` directory | `src` |
| `--tests <TESTS>` | Path to the `tests/` directory | `tests` |
| `--pipeline-report <PIPELINE_REPORT>` | Path to a `cargo-diagnostics` pipeline JSON report (`0155` output) | — |
| `--cobertura <COBERTURA>` | Path to a Cobertura XML coverage file | — |
| `--llvm-cov <LLVM_COV>` | Path to an llvm-cov JSON summary file | — |
| `--cobertura-full` | Include per-file coverage details in the JSON output | — |
| `--full` | Include all collected data in the JSON output | — |
| `--output <OUTPUT>` | Write output to this file instead of stdout | stdout |
| `-h`, `--help` | Print help | — |
| `-V`, `--version` | Print version | — |

## Output

Produces a JSON report. By default the report is written to stdout and
contains only gap findings. With `--cobertura-full` the report includes
per-file coverage details. With `--full` the report includes the complete
collected payload (mirrors, duplicates, coverage, gaps). When `--output` is
provided the report is written to the specified file path instead of stdout.
Exit codes: `0` on success, non-zero on failure.

## Examples

```bash
# Minimal gaps-only output to stdout
.github/skills/0-external-test-gap-fusion/run.sh

# Minimal gaps-only output to file
mkdir -p reports
.github/skills/0-external-test-gap-fusion/run.sh \
  --output reports/gap-report.json

# With coverage detail
.github/skills/0-external-test-gap-fusion/run.sh \
  --cobertura reports/cobertura.xml \
  --cobertura-full \
  --output reports/gap-report.json

# Full report with all data sources
.github/skills/0-external-test-gap-fusion/run.sh \
  --src src \
  --tests tests \
  --pipeline-report reports/diagnostics.json \
  --cobertura reports/cobertura.xml \
  --llvm-cov reports/llvm-cov.json \
  --full \
  --output reports/gap-report.json
```

## Key Files

- `.github/skills/0-external-test-gap-fusion/run.sh` — entrypoint wrapper script
- `.github/skills/0-external-test-gap-fusion/SKILL.md` — this file
