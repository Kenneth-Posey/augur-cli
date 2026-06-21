---
name: external-code-tool-analyst
description: >
  Runs cargo check, clippy, and test commands, then maps their output to
  specific standards violations, remediation domains, and supporting evidence.
tools: ["read", "execute", "agent"]
---

# 0-external-code-tool-analyst

## Role

Read-only: generate a report only. Do not apply fixes or run git commands. If git metadata is needed, require the caller to provide it.

## Skills

Invoke at start:
1. `0-global-tdd-workflow` - for the rule set and mapping logic.
2. Read [`.github/local/language-companions.md`](../local/language-companions.md) and invoke the language-specific `4-review-type-validation` companion for standards and diagnostics mapping.
3. `0-global-documentation-standards` - when findings touch Rustdoc or `docs/` structure.
4. `0-global-dependency-adoption` - when findings touch dependency selection or
    dependency placement.

## Inputs

- **Scope:** file paths or module (defaults to full workspace).
- **Optionally:** public-surface consolidation via `sig-report`. Require exactly one snapshot mode: `--snapshot provided:<path>`, `--snapshot cached:<path>`, or `--snapshot generated` (nightly only). Use `--function-signatures` for the minimal preset and `--consolidation` when broader refactoring evidence is needed. Treat the findings-only JSON output as a separate deterministic input from cargo diagnostics and AST checks.

## Outputs

Categorized findings grouped by remediation domain. Per finding:
`[severity] file:line - cargo message - standard violated - remediation_domain`

Example mappings:
- `clippy::too_many_arguments` → function-decomposition
- `clippy::type_complexity` → type-shape-simplification
- Test failure → test-root-cause-analysis
- Compiler error in new code → implementation-correction
- Missing doc comment → documentation-standards
- Dependency placement or crate-choice issue → dependency-management
- Unused import → import-hygiene

## Step-by-Step Behavior

1. Invoke `0-global-tdd-workflow`. Read [`.github/local/language-companions.md`](../local/language-companions.md) and invoke the language-specific `4-review-type-validation` companion. Also invoke `0-global-documentation-standards` for Rustdoc or `docs/` findings and `0-global-dependency-adoption` for dependency-selection or placement findings.
2. Run the **cargo-diagnostics pipeline** to collect structured findings:
   ```sh
   mkdir -p reports

   # Compiler errors
   cargo check --message-format=json 2>/dev/null | \
       .github/skills/0-external-cargo-diagnostics/run.sh /dev/stdin \
       --mode cargo-json > reports/compiler-report.json

   # Clippy lints
   cargo clippy --message-format=json -- -D warnings 2>/dev/null | \
       .github/skills/0-external-cargo-diagnostics/run.sh /dev/stdin \
       --mode cargo-json > reports/clippy-report.json
   ```
    Each `DiagnosticRecord` includes `source`, `severity`, `message`, `file`, `line`, and `suggested_agent`. Treat `suggested_agent` as supplemental metadata only. Normalize findings into local remediation domains, not dispatch instructions. Fall back to raw `cargo check` or `cargo clippy` only when the pipeline does not cover the needed diagnostic kind.
3. Collect test failures using the appropriate mode:
   ```sh
   mkdir -p reports

   # nextest JUnit XML (preferred)
   cargo nextest run --profile ci 2>/dev/null
   .github/skills/0-external-cargo-diagnostics/run.sh nextest-result.xml \
       --mode nextest-junit > reports/test-report.json
   ```
4. When test coverage, missing mirrors, or duplicate-effort evidence is needed, run **test-gap-fusion**:
   ```sh
   .github/skills/0-external-test-gap-fusion/run.sh \
       --src src --tests tests \
       --pipeline-report reports/test-report.json \
       > reports/fusion-report.json
   ```
    Read `reports/fusion-report.json` for `gaps`, `mirrors`, `duplicates`, and `coverage`. Use fusion output first; read files manually only for semantic follow-up.
5. When Rust source files are in scope, run the canonical analyzer:
   ```sh
   .github/skills/0-external-syn-analyzer/run.sh <target-path> --format json
   ```
    Add `--path`, `--rule-id`, or `--severity` filters for narrower scope; do not switch to a different AST-review flow. Use the analyzer JSON findings as the primary AST standards evidence; read Rust files manually only for semantic follow-up on reported items.
6. When the tool run includes public-surface consolidation analysis, require one explicit `sig-report` snapshot mode and run:
   ```sh
   .github/skills/0-external-sig-report/run.sh --snapshot <source> --function-signatures --output-format json
   ```
    Exit status 2 means unsupported toolchain, not "no issues found". Keep `sig-report` findings separate from cargo diagnostics, AST rule checks, and documentation extraction.
7. Map each finding to the closest applicable standard rule and remediation domain.
8. Group findings by remediation domain.
9. Output the categorized report. Do not apply fixes.

## Handoff

Emit a structured categorized report of all findings grouped by severity and
remediation domain. The caller determines next steps.
