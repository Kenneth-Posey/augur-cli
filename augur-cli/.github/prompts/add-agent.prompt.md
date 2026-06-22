---
name: Add Agent
description: >
  Use when asked to add a new custom agent under .github/agents/. Creates the
  agent profile and defines its trigger, tools, skills, task contract, and
  handback behavior.
argument-hint: "agent purpose, scope, and what work it should perform"
agent: agent
---

# add-agent

Create a custom agent in `.github/agents/` for the requested specialization.

## Decision Gate

Before writing anything, decide whether the requested capability should be a:

1. **Prompt** - repeatable workflow command
2. **Agent** - specialized sub-agent with distinct tools and responsibilities
3. **Skill** - on-demand guidance
4. **Instruction** - always-on or path-specific rule

Only continue if **agent** is the right fit. If another fit is better, say so
and explain which file type should be added instead.

## Required File

Create exactly one file at:

- `.github/agents/<slug>.agent.md`

Include:

1. YAML frontmatter with:
   - `name`
   - `description`
   - `tools` using least-privilege primary aliases such as `read`, `search`,
     `edit`, `execute`, `agent`
2. A markdown body with these sections:
     - `# <agent-name>`
     - `## Role`
     - `## Skills`
     - `## Inputs`
     - `## Outputs`
     - `## Step-by-Step Behavior`
     - `## Handoff`

## Agent Design Rules

- Make the description specific enough for correct selection: say what the
  agent does, when to use it, and what tasks should trigger it.
- Give the agent only the tools it truly needs.
- Reuse existing skills for rules and reasoning instead of copying those rules
  into the agent body.
- Agents must **explicitly name and invoke** the skills they depend on. Do not
  rely on path-based or implicit instruction loading as the primary source of
  agent rules.
- Write guidance that works when callers launch the agent as a background
  task. Do not treat inline primary-context execution as an equal default.
- The `## Skills` section must say which skills the agent invokes and under what
  conditions.
- In `## Step-by-Step Behavior`, include an explicit invoke step for those
  skills instead of assuming they are already in context.
- Only orchestrator agents may define multi-agent execution order, retries,
  checkpoints, or downstream routing. Non-orchestrator agents must keep
  `## Handoff` limited to returned artifacts, signals, and a note that the
  caller determines next steps.
- Do not duplicate path-specific instruction content. Treat any path-specific
  guidance as supplemental background, not the agent's primary standards
  source.
- If the agent writes files, name the exact output locations and expected file shape.
- If the agent is read-only, say so explicitly.
- If the agent can be part of planning workflows, ensure its name can be added
  to planning standards where appropriate.
- Keep project-specific identity data out of the agent body; reference
  `.github/local/` files only when genuinely needed.
- Respect the local-directory split:
  - `.github/local/identity.md` for repository identity and branch/build facts
  - `.github/local/directories.md` for repo layout and path conventions
  - `.github/local/rules.md` for project-specific workflow policy
- Do not duplicate local-file content into the agent body. Link or reference
  the specific local file instead.

## Placement

State:

1. Why an agent is better than a prompt, skill, or instruction
2. What unique responsibility this agent owns
3. What skills or companion artifacts it uses, and whether it is an
   orchestration control owner
4. Whether it should appear in any broader workflow prompts or planning lists
5. Whether it must be added to
   `.github/skills/0-global-plan-implementation/SKILL.md` Valid Agent Names
   (pipeline-canonical or auxiliary) and to any routing surfaces
   (`.github/AGENTS.md`, `.github/routing.md`,
   `.github/copilot-instructions.md`) that expose executable agent options

## Validation Checklist

Before finishing, verify:

1. Verify the agent does not duplicate an existing agent's role.
2. Verify the trigger description is concrete enough for correct selection.
3. Verify the tool list is least-privilege and sufficient.
4. Run `.github/skills/0-external-customization-analyzer/run.sh .github/agents/<slug>.agent.md`
   and address any structural findings before finishing.
5. Verify the workflow is self-contained from a fresh context.
6. Verify the agent explicitly invokes every skill it depends on and does not
   rely on implicit instruction loading.
7. Verify companion routing/planning surfaces were checked and updated when
   needed: `.github/skills/0-global-plan-implementation/SKILL.md` Valid Agent
   Names sections, `.github/AGENTS.md`, `.github/routing.md`, and
   `.github/copilot-instructions.md`.

## Output

Return:

1. The created agent path
2. The agent's purpose and trigger summary
3. The chosen tool list and why each tool is needed
4. Any existing prompt or planning file that should reference this agent
