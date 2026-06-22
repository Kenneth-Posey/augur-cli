---
name: Review Customization
description: >
  Use when asked to review a created or updated .github customization artifact
  against the add-* prompt that defines how that artifact type should be built.
argument-hint: "path or paths to the prompt/agent/skill/instruction to review"
agent: agent
---

# review-customization

Review the requested `.github/` customization artifact against the matching
`add-*` prompt.

## Artifact-to-Guideline Mapping

Determine the artifact type from the provided path or paths, then use the
matching `add-*` prompt as the review standard:

1. `.github/agents/*.agent.md` -> `.github/prompts/add-agent.prompt.md`
2. `.github/skills/<slug>/SKILL.md` or `.github/skills/<slug>/` -> `.github/prompts/add-skill.prompt.md`
3. `.github/prompts/*.prompt.md` -> `.github/prompts/add-prompt.prompt.md`
4. `.github/copilot-instructions.md`, `.github/AGENTS.md`, `.github/instructions/*.instructions.md`,
   `.github/local/*.md`, or `.github/routing.md` -> `.github/prompts/add-instructions.prompt.md`

If the input mixes artifact types, review each artifact against its own
guideline and group findings by artifact.

## Workflow

1. Identify the exact artifact path or paths to review.
2. Classify each artifact by type using the mapping above.
3. For each analyzer-supported artifact path, run
   `.github/skills/0-external-customization-analyzer/run.sh <artifact-path>`
   first. Supported paths are:
   - `.github/agents/*.agent.md`
   - `.github/skills/<slug>/SKILL.md`
   - `.github/prompts/*.prompt.md`
   - `.github/instructions/*.instructions.md`
   - `.github/local/*.md`
   For unsupported artifacts such as `.github/AGENTS.md`,
   `.github/copilot-instructions.md`, and `.github/routing.md`, skip the
   analyzer and start from a manual read instead.
4. Read the matching `add-*` prompt for the artifact's required structure,
   validation, and output expectations.
5. Limit follow-up reads to:
   - the reviewed artifact
   - supporting paths such as `.github/local/identity.md`,
     `.github/local/directories.md`, and `.github/local/rules.md` when the
     analyzer reports them or the artifact references them directly
   - skills, agents, instructions, prompts, or linked files reported by the
     analyzer or referenced directly by the artifact
   - files needed to confirm a missing or broken reference
6. When analyzer output is available, use it as the structural gate. Do not
   repeat manual checks for required sections/frontmatter, `.github/local`
   placement, or reference existence unless confirming a specific analyzer
   finding. For unsupported artifacts, do those checks manually.
7. Review the artifact against the governing prompt for the remaining
    human-judgment checks:
   - the correct customization type was chosen
   - the required file location and naming were used
   - the artifact avoids duplicating an existing role or workflow
   - the workflow is self-contained enough for a fresh context
8. Apply type-specific semantic checks from the matching add prompt:
    - **Agents**: trigger clarity, least-privilege tools, skill reuse, explicit
      outputs and handback contract
    - **Skills**: task-focused scope, correct directory/name shape, justified
      supporting files and tool restrictions
    - **Prompts**: scope-appropriate task flow, explicit output contract,
      correct reuse of agents/skills/instructions
    - **Instructions**: correct instruction layer and correct `applyTo` when
      applicable
9. Keep analyzer findings and prompt-rule follow-up findings distinct.
10. Report only real gaps against the governing prompt. Do not rewrite files
    unless the user explicitly asks for fixes.

## Output

Return:

1. The reviewed artifact path, the analyzer command used or a note that the
   artifact is analyzer-unsupported and was reviewed manually, and the
   governing `add-*` prompt used as the standard
2. A gate result for each artifact: `pass`, `pass with fixes`, or `fail`
3. Analyzer findings grouped by artifact, ordered by severity, or `none`
4. Prompt-rule follow-up findings grouped by artifact, ordered by severity,
   with the exact missing or violated requirement text, or `none`
5. Required fixes, each tied to the specific artifact and governing prompt rule
6. Any additional file that should also be updated for consistency, or `none`
