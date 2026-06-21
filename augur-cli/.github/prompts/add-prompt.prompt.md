---
name: Add Prompt
description: >
  Use when asked to add a reusable prompt command under .github/prompts/.
  Creates the prompt file and defines its workflow, inputs, and output contract.
argument-hint: "purpose and intended workflow for the new prompt"
agent: agent
---

# add-prompt

Create a prompt command in `.github/prompts/` for the requested workflow.

## Decision Gate

Decide whether the requested capability should be a:

1. **Prompt** - repeatable workflow command in the main context
2. **Agent** - specialized sub-agent with its own context and tool restrictions
3. **Skill** - on-demand guidance for a specialized task or pattern
4. **Instruction** - always-on or path-specific rule

Continue only if **prompt** is the right fit. If another type fits better, say
which one and why.

## Required Prompt Structure

Create exactly one file at:

- `.github/prompts/<slug>.prompt.md`

The file must include:

1. YAML frontmatter with:
   - `name`
   - `description`
   - `argument-hint` when the prompt takes meaningful input
   - `agent: agent`
2. A markdown body that defines:
    - the task flow at the prompt's scope
    - required input interpretation
    - the output format shown to the user
3. Enough detail to run from a fresh context without unstated conversation
   memory.

## Authoring Rules

- Reuse existing agents, skills, and instructions instead of duplicating them.
- Only orchestration prompts may define multi-agent execution graphs, retries,
  or downstream routing. Non-orchestration prompts should focus on local task
  framing, required inputs, validations, and outputs.
- When a prompt calls a custom agent, keep the call high-level unless the prompt
  itself is an orchestrator-owned control surface.
- Do not embed project-specific identity data directly in the prompt body.
  Reference `.github/local/` files when project-specific information is needed.
- Respect the local-directory split:
  - `.github/local/identity.md` for repo identity, root path, build commands, and branch names
  - `.github/local/directories.md` for source tree, test layout, docs layout, and path conventions
  - `.github/local/rules.md` for project-specific commit, branching, and completion policy
- Do not copy facts from `.github/local/` into the prompt unless the prompt is
  specifically about initializing or updating those local files.
- Prefer prompts for reusable task entrypoints, not for general policy.
- If the workflow should always happen automatically rather than by command,
  it belongs in instructions instead of a prompt.
- If the workflow requires a persistent specialized role, prefer an agent.
- Keep the prompt focused on one clear command purpose.

## Validation Checklist

Before finishing:

1. Verify the filename is unique and the slug is clear.
2. Verify the workflow does not duplicate an existing prompt command.
3. Run `.github/skills/0-external-customization-analyzer/run.sh .github/prompts/<slug>.prompt.md`
   and address any structural findings before finishing.
4. Verify the output section tells the caller exactly what the command returns.
5. Verify the prompt is actionable with only `.github/` guidance plus current repo state.

## Output

Return:

1. The created prompt path
2. A short statement of what workflow it orchestrates
3. Any existing prompt/agent/skill it intentionally reuses
