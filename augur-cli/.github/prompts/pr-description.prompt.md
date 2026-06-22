---
name: Write PR Description
description: >
  Use when asked to write a pull request description for the current branch.
  Produces plain text from commits and diffs.
argument-hint: "optional: target branch (if omitted, read from .github/local/identity.md)"
agent: agent
---

# pr-description

## Workflow

1. Determine the target branch from the argument. If omitted, read the Copilot
   merge target branch from `.github/local/identity.md`. If neither is
   available, ask the user for the target branch before continuing.
2. Delegate git metadata gathering to `global-git-operator` as a background task.
   Request:
   - current branch
   - commits in `<target>..<current>`
   - diff summary for `<target>..<current>`
3. If `plans/` contains plan files, read the most recent one for context.
4. Write the PR description with:
   - Summary
   - Changes (bulleted summary, not a commit list)
   - Testing (test files and `cargo test` result)
   - Notes (follow-up items or known gaps)
5. Use plain text only. No emoji. Do not open with "this PR...".
6. Return text ready to paste into GitHub.

## Output

PR description in plain text, ready to paste.
