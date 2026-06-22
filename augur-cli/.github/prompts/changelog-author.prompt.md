---
name: Write Changelog Entry
description: >
  Write a changelog entry for completed work in a correctly named
  `changelogs/` file.
argument-hint: "brief description of the change (used in filename slug)"
agent: agent
---

# changelog-author

## Workflow

1. Get current timestamp: `date '+%m-%d-%Y-%H%M'`.
2. Build filename: `changelogs/<timestamp>-<slug>.md` where slug is the argument
   lowercased with spaces replaced by hyphens.
3. Ask `global-git-operator` for recent commits and diff context for the completed
   work.
4. Write these sections: Summary, Issues Resolved, Root Causes, Solutions,
   Files Changed, Status.
5. Plain text only. No emoji. No marketing language.
6. Write the file and return its path.

## Output

Path to created changelog file.
