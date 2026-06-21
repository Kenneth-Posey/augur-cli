---
name: 0-global-changelog-writing
description: >
  Repository changelog contract for committed changes and pipeline stage
  checkpoints: file naming, required sections, status wording, and
  validation requirements for `global-writer-changelog` and related callers.
---

# 0-global-changelog-writing

## When To Use

- Use this skill whenever a committed change needs a repository changelog file.
- In the four-stage pipeline, use it after any stage checkpoint passes
  (Design, Plan, Implement, or Review) and before `global-git-operator` creates the
  checkpoint commit.
- For non-pipeline commits, use it alongside
  [`.github/local/rules.md`](../../local/rules.md) and
  [`.github/local/directories.md`](../../local/directories.md).

## Changelog Contract

### 1. Location and Naming

- Write changelog entries under `changelogs/` at the repository root.
- Filename pattern:
  `changelogs/MM-DD-YYYY-HHMM-<slug>.md`
- The timestamp must come from the actual write time.
- The slug must be lowercase, hyphenated, and scoped to the committed change or
  stage checkpoint.

### 2. Required Sections

Every changelog entry must contain these sections, in this order:

1. `Summary`
2. `Issues Resolved`
3. `Root Causes`
4. `Solutions`
5. `Files Changed`
6. `Status`

### 3. Pipeline Checkpoint Entries

- Stage checkpoint changelogs are valid after **every** completed pipeline stage:
  Design, Plan, Implement, and Review.
- The changelog should name the completed stage and summarize the artifacts
  produced or validated in that stage.
- The `Status` section should say that the stage is complete and the changelog
  is ready for the matching checkpoint commit.
- The changelog records the checkpoint. Repository authorization comes from
  orchestration rules, not the changelog itself.

### 4. Non-Pipeline Commit Entries

- Use the same `changelogs/` location, naming rule, and section order for any
  other commit-ready change covered by repository policy.
- Keep the entry commit-scoped. Do not mix unrelated work into the same file.

### 5. Content Rules

- Plain text only. No emoji.
- Use repo-relative paths in `Files Changed`.
- Describe completed work only. Do not log planned, partial, or failed work as
  if it were done.
- Keep wording aligned with [`.github/local/rules.md`](../../local/rules.md) and
  [`.github/local/directories.md`](../../local/directories.md).

## Workflow

1. Read [`.github/local/rules.md`](../../local/rules.md) and
   [`.github/local/directories.md`](../../local/directories.md).
2. Determine whether the request is for:
   - a pipeline stage checkpoint, or
   - another commit-ready change that still requires a repository changelog.
3. Generate the timestamp with `date '+%m-%d-%Y-%H%M'`.
4. Build `changelogs/MM-DD-YYYY-HHMM-<slug>.md`.
5. Draft the six required sections using only completed artifacts and verified
   outcomes.
6. For pipeline checkpoints, include the stage name and enough artifact summary
   to show what completed work the changelog records.
7. Validate the file path, headings, and status wording before finishing.

## Validation

- Path matches: `^changelogs/\d{2}-\d{2}-\d{4}-\d{4}-[a-z0-9-]+\.md$`
- All six required headings are present.
- `Status` explicitly marks the change or stage as complete.
- Pipeline entries explicitly name the completed stage.
- The changelog meets repository naming and existence requirements.

## Related Artifacts

- `global-writer-changelog` writes the changelog file.
- `0-global-orchestration-pipeline` defines which stage checkpoint commits are
  repository-authorized.
- [`.github/local/rules.md`](../../local/rules.md) and
  [`.github/local/directories.md`](../../local/directories.md) provide the
  repository baseline this skill must follow.
