---
name: Standards Check
description: >
  Use when asked to run a standards audit. Runs cargo-diagnostics pipeline
  and syn-analyzer via `external-code-tool-analyst`, maps pipeline findings to specific
  rules and remediation domains, and presents a structured report.
argument-hint: "optional: file path or module to scope the check"
agent: agent
---

# standards-check

## Workflow

0. When you need broad repository search/list/read output to complete this
   prompt, run `size-check` first when available and apply the recommendation
   (`Proceed`, `Filter`, `Paginate`, `Split`) before issuing the command.
1. If the requested scope includes analyzer-supported customization artifacts,
   run `.github/skills/0-external-customization-analyzer/run.sh <artifact-path>`
   first for each supported path and collect the structural findings. Supported
   paths are:
   - `.github/agents/*.agent.md`
   - `.github/skills/<slug>/SKILL.md`
   - `.github/prompts/*.prompt.md`
   - `.github/instructions/*.instructions.md`
   - `.github/local/*.md`
   If the scope also includes unsupported companion files such as `.github/AGENTS.md`
   or `.github/copilot-instructions.md`, do not pass them to the analyzer;
   review them manually for routing and consistency instead.
2. Delegate to `external-code-tool-analyst` as a background task, passing the optional
   scope argument if provided. It uses `cargo-diagnostics` as the primary
   structured diagnostics source and falls back to raw cargo output only for
   unsupported diagnostic kinds.
3. If supported customization artifacts were in scope, present
   `customization-analyzer` findings first, grouped by artifact path and gate.
4. If `.github/AGENTS.md` or `.github/copilot-instructions.md` were in scope,
   report their manual-review findings separately.
5. Present `PipelineReport` findings grouped by `remediation_domain` and
   `severity`.
6. Highlight the highest-priority rules, files, or recurring failure patterns.
7. Do not auto-fix; wait for user direction before follow-up analysis.

## Output

1. `customization-analyzer` findings grouped by supported artifact and gate, or
   `none`
2. Manual-review findings for unsupported customization companion files
   (`.github/AGENTS.md`, `.github/copilot-instructions.md`) when they were in scope, or
   `none`
3. Summary: total pipeline findings by severity (`error` / `warning` / `note`)
4. Findings grouped by `remediation_domain`
5. Optional prompt: "Which finding groups should I examine more deeply?"
