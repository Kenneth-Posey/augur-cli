---
name: Add Skill
description: >
  Use when asked to add a new skill under .github/skills/. Creates the skill
  directory, SKILL.md, and any supporting resources for a focused reusable task.
argument-hint: "skill purpose, when it should be used, and any needed resources"
agent: agent
---

# add-skill

Create a new skill in `.github/skills/` for the requested task or guidance.

## Decision Gate

Before writing anything, decide whether the requested capability should be a:

1. **Prompt** - command workflow
2. **Agent** - specialized executor
3. **Skill** - on-demand guidance for a specialized task or reasoning pattern
4. **Instruction** - always-on or path-specific rule

Only continue if **skill** is the right fit. If another fit is better, say so
and explain which file type should be added instead.

## Required Files

Create a directory at:

- `.github/skills/<skill-slug>/`

Inside it, create:

- `.github/skills/<skill-slug>/SKILL.md`

Add supporting files in the same directory only when the skill needs scripts,
examples, or reference material.

`SKILL.md` must include:

1. YAML frontmatter with:
   - `name` (lowercase, hyphenated)
   - `description`
   - optional `allowed-tools` only when justified
2. A markdown body that defines:
    - when to use the skill
    - the decision process or workflow it teaches
    - how it relates to other skills, agents, or instructions
    - any needed references to local or architectural files

## Skill Design Rules

- Organize the skill around a **task, pattern, or reasoning workflow**, not a
  syntax topic unless syntax itself is the reusable task.
- Prefer skills for specialized guidance that should load on demand, not always.
- Keep project-specific identity and local pathing in `.github/local/`; only
  reference those files from the skill when needed.
- Respect the local-directory split:
  - `.github/local/identity.md` for repository identity, branch names, build commands, and root path
  - `.github/local/directories.md` for source/test/docs layout and path conventions
  - `.github/local/rules.md` for project-specific workflow policy
- Do not restate those local facts in the skill body unless the skill is
  explicitly about maintaining the local files themselves.
- Reuse existing instructions for enforced rules instead of copying large rule
  sets into the skill.
- Only orchestration skills may define multi-agent step order, retries,
  checkpoints, or downstream routing. Other skills should stay focused on local
  standards, artifact contracts, and task-local procedures.
- If scripts are added, explain exactly how the skill should use them.
- Only use `allowed-tools` when the need is explicit and safe.

## Scope Justification

State:

1. Why the item should be a skill instead of an agent, prompt, or instruction
2. What specialized knowledge or workflow the skill adds
3. Which existing instructions or agents it complements
4. Whether any agents should be updated to invoke the new skill

## Validation Checklist

Before finishing:

1. Verify the skill directory name and `name` match and are lowercase-hyphenated.
2. Verify the description clearly states when the skill should be used.
3. Verify the skill does not duplicate an existing skill.
4. Verify any `allowed-tools` entry is justified and minimal.
5. Run `.github/skills/0-external-customization-analyzer/run.sh .github/skills/<skill-slug>/`
   and address any structural findings before finishing.

## Output

Return:

1. The created skill directory and file path(s)
2. A short summary of what knowledge/workflow the skill adds
3. Any agents or prompts that should be updated to use the new skill
