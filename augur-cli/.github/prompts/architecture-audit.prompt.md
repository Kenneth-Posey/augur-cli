---
description: "Use when user asks: architecture audit, whole-tree audit, run all analyzers, analyze codebase architecture"
name: "Architecture Audit"
argument-hint: "optional scope path (defaults to full src/ tree)"
agent: "agent"
---
Run the project's analyzer suite in the fixed order below to perform a
whole-tree architecture audit. Report violations only when backed by analyzer
output.

## Inputs

- Optional scope path within `src/` (defaults to the full `src/` tree).
- Optionally: a rustdoc JSON file for public-surface analysis.

## Workflow

Run analyzers in the following fixed order. Do not skip or reorder steps.

1. **Syn analyzer** - run
   `.github/skills/0-external-syn-analyzer/run.sh`
   on the selected scope. Collect findings for: max parameters exceeded, max
   struct fields exceeded, long functions, deep `if` chains, complexity
   violations, and magic literals. Record file/line/rule for each finding.

2. **Module-graph** - run
   `.github/skills/0-external-module-graph/run.sh <src-root> --format text --layers`
   Collect findings for: dependency-direction violations, wrong-direction imports,
   and cycles. Classify each as critical (cycle) or major (wrong-direction).

3. **Arch-linter** - run
   `.github/skills/0-external-arch-linter/run.sh`
   on the selected scope and collect layer-rule and placement violations.

4. **Doc extractor** - run
   `.github/skills/0-external-doc-extractor/run.sh`
   and collect missing Rustdoc findings for public functions, types, and constants.

5. **Test-gap fusion evidence** - run
   `.github/skills/0-external-test-gap-fusion/run.sh`
   for the selected scope and collect behavioral coverage gaps per module.

6. **Sig report** - when a rustdoc JSON file is provided, run
   `.github/skills/0-external-sig-report/run.sh <rustdoc-json-path> --consolidation --output-format json`
   Collect duplicate-signature, repeated-return-shape, and doc-related findings.

7. **Src deadcode analysis** - run
   `.github/skills/0-external-src-deadcode-analysis/run.sh`
   on the selected scope. Collect symbols unreachable from crate entrypoints.

8. **Stub detection** - run
   `.github/skills/0-external-stub-detector/run.sh`
   on the selected scope. Collect deferred patterns (`todo!()`, `unimplemented!()`,
   `panic!()`, `unwrap()`, `expect()`) with severity classification.

9. **Consolidator** - run
   `.github/skills/0-external-consolidator/run.sh`
   on the selected scope. Collect dead code, duplicate functions, and
   chain-collapse candidates with confidence scores.

10. **Rustc dependency check** - run
    `.github/skills/0-external-rustc-dependency-check/run.sh`
    on the workspace root. Collect package-layer direction violations and
    forbidden edges from cargo-resolved metadata.

11. **Actor-ops detector** - when the selected scope contains actor patterns, run
    `.github/skills/0-external-actor-ops-detector/run.sh`
    on the selected scope. Collect actor.rs/actor_ops.rs pairing violations and
    non-trivial actor logic.

12. **Consolidate** - merge findings from steps 1-11. Deduplicate overlapping
    reports that point to the same symbol. Order findings: critical > major >
    minor.

13. **Rule mapping** - retain only findings that map to an explicit documented
    rule.

14. **Follow-up planning** - when a finding requires plan-level remediation,
    describe the needed follow-up scope, affected files/symbols, required
    behavior change, TDD expectations, and validation commands.

## Output

1. Analyzer run summary (tool, scope, finding count per tool)
2. Consolidated findings ordered by severity (critical > major > minor)
   - each finding: file path, symbol, rule violated, tool source, correction
3. Follow-up scope list (or `none`)
4. Audit gate decision: `pass`, `pass with follow-ups`, or `fail`
