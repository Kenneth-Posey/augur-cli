---
name: Code Audit Rust
description: >
  Use when asked to run a deterministic Rust code audit on a repository or
  scoped Rust surface. Runs only repo-supported Rust tooling and reports
  supported findings separately from partially supported or unsupported audit
  categories.
argument-hint: "optional: Rust path, crate, or module to audit"
agent: agent
---

# code-audit-rust

Run a deterministic audit of the requested Rust code surface.

Use only deterministic output from repository-supported tools. Do not perform
manual source inspection, manual follow-up, convention inference, completeness
inference from plans, specs, or source, or unsupported semantic judgment.

## Inputs

- Optional Rust scope path, crate, package, or module.
- If no scope is provided, audit the repository's default Rust code surface
  using `.github/local/identity.md` and `.github/local/directories.md`.

## Workflow

1. Confirm the requested scope is Rust-specific. If the request is not for Rust
   code, say this prompt only supports deterministic Rust audits.
2. Read the applicable local guidance before running checks:
   - `.github/local/identity.md`
   - `.github/local/directories.md`
   - `.github/local/rules.md`
   - `.github/local/language-companions.md`
3. Determine which deterministic Rust tools are available and relevant for the
   scoped code. Use checked-in commands, checked-in analyzer wrappers,
   compiler/linter/test output, and existing coverage artifacts when available.
   Do not add manual review steps.
4. Before running broad repository search/list/read commands as part of the
   audit workflow, run `size-check` when available and follow its recommendation
   (`Proceed`, `Filter`, `Paginate`, `Split`) to keep command output bounded.
5. Run deterministic compiler, clippy, and test diagnostics for the Rust scope
   using the repository's supported commands and tool wrappers. When
   machine-readable diagnostic artifacts already exist or are explicitly
   provided, normalize them with
   `.github/skills/0-external-cargo-diagnostics/run.sh`.
6. Run deterministic structural coverage-gap tooling where available:
   - `.github/skills/0-external-test-gap-fusion/run.sh` for structural source ↔
     test gap evidence; add `--cobertura-full` only when file-level coverage is needed
   - coverage percentage or line-level coverage only when deterministic coverage
     artifacts already exist or the repository already supports producing them
     in-scope
7. Run deterministic complexity and decomposition tooling where available with
   `.github/skills/0-external-syn-analyzer/run.sh`. Report only tool-backed
   findings such as complexity, long functions, parameter/field counts, deep
   conditionals, magic literals, missing docs, bare primitive signatures,
   repeated trait bounds, and deep boolean formulas.
8. Run deterministic dependency-direction, cycle, and architecture tooling where
   available:
   - `.github/skills/0-external-module-graph/run.sh` for module dependencies,
     cycles, and layer-direction evidence
   - `.github/skills/0-external-arch-linter/run.sh` only when present and
     applicable for the scoped Rust surface
9. Treat stub or placeholder detection as unsupported unless the scoped run has
   explicit deterministic evidence from a documented repo-supported tool that
   emits that category. Do not assume compiler output, normalized diagnostics,
   or other audit artifacts provide dedicated placeholder/stub detection unless
   that support is explicitly available for the current scope. Do not search
   source manually for stubs.
10. For dead, unused, or abandoned code, report only categories supported by
   deterministic tool output already available for the scope, such as compiler
   or clippy unused-code diagnostics. If broader abandoned/dead-code analysis
   is not supported by repo tooling, mark it unsupported / not available rather
   than infer it from source.
11. Keep the audit limited to deterministic evidence the repository can
    support. Do not claim universal coverage, direct the caller to inspect
    source files, or infer repository pattern conformance from plans, specs, or
    source reading.
12. Consolidate results and clearly separate:
    - supported deterministic findings
    - partially supported categories with explicit scope limits
    - unsupported or unavailable audit categories
13. Do not auto-fix and do not expand this prompt into an orchestration or
    workflow-control surface. Return the audit results only.

## Output Format

1. **Tool run summary**
   - tool or command
   - audited Rust scope
   - status
   - evidence source
2. **Supported deterministic findings**
   - category (`compiler`, `clippy`, `tests`, `coverage-gap`, `complexity`,
     `decomposition`, `dependency-direction`, `cycle`, `architecture`,
     `unused-code`, or another category only when backed by documented
     deterministic tool output available for the scoped run)
   - severity
   - file, module, or symbol
   - tool source
   - evidence
3. **Partially supported categories**
   - category
   - deterministic evidence that was available
   - exact limitation for this repository or scope
4. **Unsupported / not available**
   - audit category
   - reason it is unsupported in current repo tooling or scope
   - explicit status: `not inferred`
   - include `stub-placeholder` here when no documented deterministic tool in
     the scoped run provides placeholder/stub evidence
5. **Audit gate**
   - `pass`
   - `pass with deterministic findings`
   - `fail`
