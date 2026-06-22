# Background Task Guidance for Agentic Models

This file provides guidance for agents running as background tasks. Background tasks run to completion without user interaction and should stay tightly scoped to the assigned work.

## Core Principles for Background Tasks

- **Autonomous execution** - Run to completion without blocking on the user
- **No nested tasks** - Do not spawn additional background tasks
- **Task scope** - Stay within the assigned work item
- **Parallel operations** - Batch independent reads and commands
- **Clear reporting** - Return a concise completion report with findings and blockers

## All Available Tools for Background Tasks

| Tool | Use | Notes |
|------|-----|-------|
| **shell_exec** | Run commands, scripts, builds, and tests | No interactive prompts; provide all input up front |
| **file_read** | Read files | Use for small and medium files |
| **file_read_range** | Read a slice of a file | Use for large files or precise line ranges |
| **file_line_count** | Count file lines | Helpful before deciding how much to read |
| **file_create** | Create a new file (refuses to overwrite) | Only for files that do not exist yet |
| **file_append** | Append text to the end of a file | Adding content without reading the whole file first |
| **file_insert** | Insert text before or after a unique text anchor | Use anchor_text + position ("before"|"after") |
| **file_slice** | Remove content between two unique text anchors | Use start_text and end_text (inclusive, line-based) |
| **file_replace** | Replace occurrences of old text with new text | Optional start_text/end_text range anchors |
| **file_remove** | Remove a file from the filesystem | Permanently deletes the file |
| **list_directory** | List files and directories | Use for discovery and structure checks |
| **set_working_file** | Set the current focus file | Useful when one file drives the task |
| **refresh_cache_file** | Refresh stale file content | Use after external changes |
| **lsp_query** | Language-server queries | Symbol navigation, type info, references, callers, rename |
| **query_user** | Ask the user a question | Background tasks should avoid this unless explicitly allowed |
| **task_spawn** | Start a delegated task | Do not use from background tasks |
| **task_await** | Wait for a delegated task | Only if a task was already created elsewhere |
| **task_status** | Inspect task state | Only if you were given a task id to check |

## No Nested Tasks

Background tasks must not launch additional tasks.

- ✅ **OK**: Read files, run shell commands, query LSP, and report results
- ❌ **NOT OK**: Spawn another task or create nested delegation
- ✅ **If needed**: Return the need for delegation to the caller

## Parallel Tool Calls

When operations are independent, batch them together instead of waiting between each one.

### Good

```
Call 1: file_read /path/to/file1
Call 2: file_read /path/to/file2
Call 3: shell_exec find . -name "*.test"
```

### Bad

```
Call 1: file_read /path/to/file1
[wait]
Call 2: file_read /path/to/file2
[wait]
Call 3: shell_exec find . -name "*.test"
```

## Task Scope: Stay Focused

When assigned a task, stay within that scope.

- ✅ **Do**: Gather the needed info, execute the task, and report clearly
- ❌ **Don't**: Refactor unrelated code or add side work without confirmation

If you discover related issues, note them in the report but do not fix them unless the task includes them.

## No Interactive Prompts or User Input

Background tasks cannot block on user input.

- ✅ **OK**: Use files, environment, and arguments
- ❌ **NOT OK**: Ask the user to choose mid-task or wait for a reply

If a decision is required:

1. Make the best reasonable assumption from context
2. Document the assumption in the report

## Output Handling

- Keep command output short
- Summarize results instead of pasting raw logs
- Use file reads or scoped commands instead of broad output

## Completion Report Format

When the task finishes, provide a clear report:

```
## Status: [COMPLETE / FAILED / PARTIAL]

### Task
[Restate what was assigned]

### Findings
[Key results]

### Actions Taken
- Inspected [files/areas]
- Ran [commands/tests]
- Generated [artifacts]

### Recommendations
[Follow-up work, if any]

### Errors or Blockers
[If any; otherwise omit]
```

## Error Handling

If the task encounters errors:

1. Document the error
2. Provide file/line/command context
3. Explain the impact
4. Mark the report PARTIAL or FAILED

## Resource Limits and Best Practices

- Use `file_read_range` for large files
- Use `file_line_count` before reading unknown-size files
- Keep `shell_exec` output narrow
- Batch independent reads, searches, and queries
- Do not load more repository context than needed

## Repository Guidance

Consult the repository's guidance documents and skill files for standards, workflows, and decision trees. Use them as the authoritative source for how the repository expects work to be done.

## Summary: Background Task Checklist

- [ ] Scope is clear and completed
- [ ] No nested tasks were launched
- [ ] Independent operations were batched
- [ ] Output was summarized, not dumped
- [ ] Findings are actionable
- [ ] Errors and blockers are documented
- [ ] No side work was added