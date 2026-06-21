---
description: "Use when user asks: create commit, create message and commit, commit this phase, commit completed phase"
name: "Create Commit"
argument-hint: "phase scope or summary to include in commit message"
agent: "agent"
---
Create a phase-scoped commit for the current implementation work through
`global-git-operator`.

## Workflow

1. Confirm commit creation is authorized by either:
   - the user's explicit request, or
   - the current implementation plan's explicitly requested commit event.
2. Follow repository commit policy from `.github/copilot-instructions.md` and
   `.github/local/rules.md`.
3. Build a commit message that references the completed phase acceptance criteria.
4. Run a test-documentation consistency check for the changed scope:
   - ensure test methods in the project's Rust test layout have concise,
     behavior-focused docs,
   - confirm test behavior matches the documented intent,
   - if behavior is the correct contract, update docs to match,
   - if docs are the correct contract, update tests and/or implementation to
     match.
5. Delegate staging and commit execution to `global-git-operator` as a background
   task. Pass:
   - the authorization basis,
   - the commit message summary,
   - the file scope that may be staged.
6. Return the staged-file summary and commit details from `global-git-operator`.

## Output

1. Commit message
2. Files staged
3. Commit result (hash + summary)
