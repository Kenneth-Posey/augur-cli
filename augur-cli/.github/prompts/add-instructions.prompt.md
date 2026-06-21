---
name: Add Instructions
description: >
  Use when asked to add or restructure instructions under .github/. Chooses the
  right instruction layer and updates the matching repo-wide, path-specific,
  local, or routing file.
argument-hint: "rule or behavior to encode, including where it should apply"
agent: agent
---

# add-instructions

Add the requested instruction in the right `.github/` instruction layer.

## Decision Gate

Choose the target instruction layer:

1. **Repository-wide baseline** - `.github/copilot-instructions.md`
2. **Agent behavior instructions** - `.github/AGENTS.md`
3. **Path-specific instructions** - `.github/instructions/*.instructions.md`
4. **Project-specific local data** - `.github/local/*.md`
5. **Centralized routing guide** - `.github/routing.md`

Use these rules:

- If the rule should apply to nearly every task, use the baseline.
- If the content is the central agent-delegation and routing guide that
  baseline or agent-behavior files should link to, use `.github/routing.md`.
- If the rule is only for matching files or paths, create/update a path-specific instruction.
- If the content is project-specific identity, pathing, build commands, or local policy,
  put it in `.github/local/`.
- Use `.github/AGENTS.md` for agent workflow rules rather than code rules.
- If the content defines or recommends agent delegation or launch behavior, say
  whether it runs as a background task unless an existing explicit exception
  must be preserved.
- Use the specific local file that matches the content:
  - `.github/local/identity.md` for repo/root/build/branch facts
  - `.github/local/directories.md` for directory structure and path rules
  - `.github/local/rules.md` for project-specific workflow and standards policy

## Instruction Requirements

### If adding repository-wide baseline

- Keep it minimal.
- Only include rules needed in general sessions.
- Do not inline project identity; point to `.github/local/` instead.
- Do not move specialized guidance here if a skill or path-specific instruction is better.

### If adding path-specific instructions

Create or update:

- `.github/instructions/<slug>.instructions.md`

The file must include:

1. YAML frontmatter with:
   - `description`
   - `applyTo`
   - optional `excludeAgent` only when truly needed
2. A markdown body with specific, enforceable rules
3. Scope it narrowly enough that unrelated files do not load it

### If adding project-specific local files

- Write only to `.github/local/`
- Keep project-specific identity, pathing, and branch/build policy there
- Do not repeat those details in global instructions
- Update the correct local file rather than creating overlapping local files
   unless the user explicitly wants a new local document

## Authoring Rules

- Prefer the narrowest instruction scope that correctly enforces the rule.
- Avoid duplication: instructions are rules, skills are on-demand guidance, and
  agents are task executors.
- Keep global instruction files reusable across projects whenever possible.
- Keep path-specific instructions language- and path-appropriate.
- If the rule belongs in an existing instruction file, extend that file instead
  of creating a near-duplicate.
- When instructions mention custom agents, state clearly when they must run as
  background tasks rather than leaving the execution mode implicit.

## Validation Checklist

Before finishing:

1. Verify the chosen instruction layer is the right one.
2. Verify any new `.instructions.md` file has a correct `applyTo`.
3. If the updated instruction path is analyzer-supported, run
   `.github/skills/0-external-customization-analyzer/run.sh <created-or-updated-instruction-path>`
   and address any structural findings before finishing. Supported paths are
   `.github/instructions/*.instructions.md` and `.github/local/*.md`.
   Do not run the analyzer on `.github/AGENTS.md`, `.github/copilot-instructions.md`,
   or `.github/routing.md`; validate those files manually instead.
4. Verify there is no duplicated rule that should be consolidated instead.

## Output

Return:

1. The file path(s) created or updated
2. Which instruction layer was chosen and why
3. Any existing file that absorbed the new rule instead of creating a new file
