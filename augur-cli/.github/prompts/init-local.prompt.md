---
description: "Use when setting up a new project: initialize the .github/local/ files from the current repo state."
name: "Init Local"
agent: "agent"
---
Inspect the current repository and populate or initialize all files under
`.github/local/`. Create the directory if it does not exist.

The broader `.github/` bundle (agents, skills, prompts, instructions,
plan_execution.yml base) is already present in the repo. Only the inner
`.github/local/` files need per-repo discovery or initialization.

Do not copy content from any existing project. Discover everything from the repo itself.

---

## Step 1 - Discover identity

Gather:

- ask `global-git-operator` as a background task for `git remote get-url origin` -
  repo remote URL
  (extract owner and repo name)
- `pwd` - confirm absolute project root
- ask `global-git-operator` as a background task for `git branch --list` - identify
  branches; ask the user which branch is the stable trunk if it is not obvious.
  If the repo has a dedicated Copilot merge target, record it. If it does not,
  record the normal trunk + feature-branch model instead of inventing a Copilot branch.
- Discover build, test, lint, and check commands from actual repo evidence in this order:
  1. language/tool manifests and project files
  2. repo scripts, package manager config, and task runners
  3. CI/workflow commands if those are the clearest authoritative commands
  4. if none exist for a category, state explicitly that no repo-native build/test/lint/check command was found
- Do not assume Cargo, Rust, or any other specific toolchain is present. Use only evidence that actually exists in the repo.

Produce `.github/local/identity.md` with these sections:

```
# Project Identity

## Repository
- Root Directory: <absolute path>
- Repository Owner: <owner>
- Repository Name: <name>

## Build Commands
- <build command or explicit "none found"> - build the project
- <test command or explicit "none found"> - run all tests
- <lint command or explicit "none found"> - lint or static analysis
- <check command or explicit "none found"> - quick syntax/type check / validation

## Branching Model
- <trunk branch> - stable production trunk
- <copilot branch or "no dedicated Copilot merge branch"> - merge target policy actually used by this repo

Policy: Record the real merge policy used by this repo. If there is a dedicated
Copilot branch, note whether the user controls merges from that branch into
<trunk branch>. If there is not, say that the repo uses a normal trunk +
feature-branch model.

## Path Rules
- Always use absolute paths. Never relative paths or wrong home directories.
  Use <absolute project root> as the project root.
```

---

## Step 2 - Map the source tree

Walk the top two levels of the repo with `find . -maxdepth 2 -type d` (or equivalent).
Identify:

- Source code directories and entry points, only if they actually exist
- Test directories and naming conventions, only if they actually exist
- Documentation directories, only if they actually exist
- Configuration files at root level
- Changelog and planning directories if present
- The `.github/` tooling layout

Do not infer a standard project tree. If `src/`, `tests/`, `docs/`, or other
common directories are absent, say they are absent. Only record directories,
files, and entrypoints that you verified exist.

Produce `.github/local/directories.md` with these sections:

```
# Project Directory Structure

## Source Tree
<list of verified source directories with one-line descriptions, or explicitly say they are absent>

## Test Tree
<verified test layout, naming conventions, mirror rules, or explicitly say test directories are absent>

## Documentation
<verified docs directory contents and index, or explicitly say docs directories are absent>

## Changes and Tracking
<changelog format and location>

## Planning
<plans directory format and location, if used>

## Configuration
<root-level config files and .github/ layout>

## Critical Rules
- Never use unverified paths - always verify against this list
- Never invent paths - always verify against this list
- Always use absolute paths - <root>/...
- <any discovered layout invariants>
```

---

## Step 3 - Establish project rules

Before writing `.github/local/rules.md`, read:

- `.github/copilot-instructions.md` if it exists
- `.github/routing.md` if it exists

Use them to capture existing repo workflow conventions.

Ask the user the following questions (or answer them from repo evidence if clear):

1. **Commit policy** - should Copilot auto-commit, or wait for user confirmation?
   - For large phased work, should local rules describe only commit authorization
     policy and refer execution sequencing to orchestration surfaces?
2. **Branching policy** - should Copilot stay on the current branch always, or are branch switches allowed?
3. **TDD requirement** - is test-first development mandatory for all changes?
4. **Definition of done** - what must be true before a task is considered complete?
5. **Standards** - are there function/struct size limits, no-unsafe rules, no-magic-number rules, or other code style invariants?

If commit policy, branching policy, TDD requirement, or definition-of-done rules
are still ambiguous after reading the repo, ask the user instead of guessing.

Produce `.github/local/rules.md` with these sections:

```
# Project-Specific Rules

## Commit Policy
<discovered or user-confirmed commit and phase rules>

## Branching and Merging
<discovered or user-confirmed branch policy>

## Implementation Requirements
<TDD policy, bug fix policy, completeness requirements>

## Standards Enforcement
<code style rules, size limits, prohibited patterns>

## Definition of Done
<ordered checklist that must be true before a task is closed>
```

---

## Step 4 - Detect language and populate language companion file

Detect the repo's primary language context and build the companion skill routing
table in `.github/local/language-companions.md`.

Determine language context from multiple evidence sources in this order:

1. manifests and project files
2. source file extensions
3. `.github/instructions/*.instructions.md`
4. existing language-prefixed skill directories under `.github/skills/`

Typical evidence:

| Language | Typical evidence | Skill prefix |
|---|---|---|
| Rust | `Cargo.toml`, `Cargo.lock`, `*.rs` | `rust-` |
| C# | `*.sln`, `*.csproj`, `Directory.Build.props`, `*.cs` | `csharp-` |
| Java | `pom.xml`, `build.gradle`, `build.gradle.kts`, `*.java` | `java-` |
| Kotlin | `build.gradle.kts`, `settings.gradle.kts`, `*.kt` | `kotlin-` |
| Python | `pyproject.toml`, `setup.py`, `requirements.txt`, `*.py` | `python-` |
| TypeScript | `package.json`, `tsconfig.json`, `*.ts`, `*.tsx` | `ts-` |
| Ruby | `Gemfile`, `*.gemspec`, `*.rb` | `ruby-` |

If multiple languages are plausible, ask the user which one is primary. If no
single primary language is obvious, you may record that the repo is
language-agnostic or multi-language and ask the user how `language-companions.md`
should be scoped.

Before writing `.github/local/language-companions.md`, read:

- the relevant language instruction file if one exists (for example `.github/instructions/rust.instructions.md`)
- `.github/copilot-instructions.md` if it exists
- `.github/routing.md` if it exists

Use those files to align the local routing layer.

Build the capability inventory from actual repo capabilities:

- inspect `.github/skills/` for universal workflow skills and existing language-prefixed companions
- use `.github/routing.md` and related instructions to confirm the capability names already used in repo guidance
- do not rely only on a short placeholder list if the repo already exposes a richer capability set

For each capability key, record one of these explicit outcomes:

1. **language companion exists**
2. **universal only**
3. **no companion exists yet / placeholder needed**

This file is the authoritative routing bridge. Do not infer companion names
from conventions alone.

**Produce `.github/local/language-companions.md`:**

```
---
name: Language-Specific Skill Routing
description: >
  Maps capability keys to their {Language} companion skills.
---

# Language-Specific Skill Routing

When working in this repo's language context, use this table to find the correct
companion routing for the capability you are executing.

## Capability Key → {Language} Companion Map

| Capability Key | Outcome | Companion / Notes |
|---|---|---|
| `<key>` | `language companion exists` | `<companion-skill-name>` |
| `<key>` | `universal only` | `<universal skill only / no language companion required>` |
| `<key>` | `no companion exists yet / placeholder needed` | `<expected companion name or placeholder note>` |
...

## Usage

Agents must always consult this table rather than hardcoding language-specific skill names.

- **Capabilities with a universal skill counterpart**: invoke the universal skill first, then look up the capability key here and invoke the listed companion.
- **Capabilities with only a language companion**: look up the capability key and invoke the listed companion directly.
- **Always reference this file** for the authoritative mapping. Do not infer or hardcode companion skill names from the capability key alone.
```

---

## Step 5 - Initialize plan execution contract

Copy `.github/plan_execution.yml` to `.github/local/plan_execution.yml`, then
customize it for this specific repository.

1. Copy the base template:
   ```
   cp .github/plan_execution.yml .github/local/plan_execution.yml
   ```
2. Update `default_model` under `runner_contract` to the preferred model for
   this repo. Discover from:
   - The user's preference if they state one
   - Existing `.github/local/plan_execution.yml` model keys if the repo
     already has one
   - Defaulting to `"deepseek/deepseek-v4-flash"` if no preference is found
3. Update `failure_retry_cap` under `runner_contract`:
   - Set to `2` for mature/stable repos where failures are rare
   - Set to `5` for actively developed repos where agent retries are valuable
4. Update the `source_of_truth` field in the metadata block to
   `.github/local/plan_execution.yml` so the queue runner reads the per-repo
   copy as authoritative.
5. Review checkpoint commit steps to ensure the agent name references
   use the correct executable name (`global-git-operator`, not `git-operator`).
   Fix any mismatches found in the copied file.
6. Review checkpoint changelog and checkpoint commit steps to ensure model
   selections are appropriate for the repo's budget. Use cheaper models
   (e.g. the same as the default_model) for changelog and git steps.
   Only use premium models (e.g. claude-haiku or claude-sonnet) if the
   repo explicitly allocates budget for them.

The resulting `.github/local/plan_execution.yml` is the file the queue runner
uses. It is safe to commit alongside the other local metadata.

---

## Step 6 - Link from core files

If `.github/copilot-instructions.md` exists and does not already reference all
five local files (identity, directories, rules, language-companions,
plan_execution.yml), add pointer lines near the top of the file (before
`## Orchestration Entry Guidance`) in this format:

```
Project identity (root, build commands, branch policy): [`.github/local/identity.md`](../local/identity.md)
Source tree, test layout, path rules: [`.github/local/directories.md`](../local/directories.md)
Commit policy, TDD rules, definition of done: [`.github/local/rules.md`](../local/rules.md)
Language-specific skill routing: [`.github/local/language-companions.md`](../local/language-companions.md)
Pipeline execution contract: [`.github/local/plan_execution.yml`](../local/plan_execution.yml)
```

These are pointer links, not mandatory startup reads. Use the same
`local/...` relative links in `.github/AGENTS.md` only if that file exists and
the same guidance is relevant there.

---

## Step 7 - Report

After all files are written, output:

1. The file paths created or updated
2. Which local files were generated from scratch vs. copied from a template
3. The `default_model` and `failure_retry_cap` values set in the plan execution contract
4. Any ambiguous repo facts that require user confirmation
5. Whether the repo was treated as single-language, multi-language, or language-agnostic
