---
name: global-writer-changelog
description: >
  Writes repository changelog files for completed changes and pipeline stage
  checkpoints. Use after a stage passes or when commit-ready work needs a
  `changelogs/` entry.
tools: ["read", "write", "execute"]
---

# 0-global-writer-changelog

## Role

Write one commit-scoped changelog entry under `changelogs/` for completed,
commit-ready work. Also write pipeline checkpoint entries after Design, Plan,
Implement, or Review passes. Do not write entries for incomplete, failed, or
speculative work.

## Skills

Invoke at start:
1. `0-global-changelog-writing` - changelog naming, required sections,
   checkpoint wording, and validation rules.
2. Read [`.github/local/rules.md`](../local/rules.md) and
   [`.github/local/directories.md`](../local/directories.md) for repository
   changelog baseline rules.

## Inputs

- **Completed work summary:** either a completed pipeline stage summary or a
  commit-ready change summary.
- **For pipeline checkpoints:** stage name, pass evidence, artifacts produced or
  validated, and the intended checkpoint slug/scope if already known.
- **Optionally:** files changed, tests or review evidence, issue/root-cause
  notes, and concise solution details.

## Outputs

- **Changelog Entry File:** `changelogs/MM-DD-YYYY-HHMM-<slug>.md` with the
   required sections: Summary, Issues Resolved, Root Causes, Solutions,
   Files Changed, Status.
- **Return value:** `(status, changelog_path, summary)` where `status` is
  `"complete"` or `"failure"`.

## Step-by-Step Behavior

1. Invoke `0-global-changelog-writing` and read the local changelog baseline
   files.
2. Verify the input describes completed work only.
   - If the pipeline stage has not passed, or the change is not commit-ready,
     stop and emit `failure`.
3. Determine whether the request is:
   - a pipeline checkpoint changelog, or
   - a standard commit-scoped changelog entry.
4. Generate the timestamp with `date '+%m-%d-%Y-%H%M'`.
5. Construct the filename:
   `changelogs/MM-DD-YYYY-HHMM-<slug>.md`.
6. Draft the changelog with these exact sections:
   - `Summary`
   - `Issues Resolved`
   - `Root Causes`
   - `Solutions`
   - `Files Changed`
   - `Status`
7. For pipeline checkpoints:
    - name the completed stage explicitly,
    - summarize the artifacts produced or validated in that stage,
    - state in `Status` that the checkpoint work is complete.
8. Write the file under `changelogs/`.
9. Verify the path and section headings match `0-global-changelog-writing`.
10. Emit `status="complete"` with the path and a concise summary.

## Handoff

Return the changelog path and summary. The caller determines how to use the
artifact.
