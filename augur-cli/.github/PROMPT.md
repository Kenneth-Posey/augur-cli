# Shared Base Instructions for Agentic Models

This file contains the instructions you need to do your work.

## Available Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| **shell_exec** | Run shell commands, tests, builds, and scripted repo operations | Commands, validation, search pipelines, and any work that needs the shell |
| **file_read** | Read file content | Small and medium files |
| **file_read_range** | Read a slice of a file | Large files or precise line ranges |
| **file_line_count** | Count file lines | Before reading an unknown-size file |
| **file_create** | Write text content to a new file (refuses to overwrite) | Creating a new file that does not exist yet |
| **file_remove** | Remove a file from the filesystem | Deleting a file entirely |
| **file_append** | Append text to the end of a file | Adding content without reading the whole file first |
| **file_insert** | Insert text before or after a unique text anchor | Adding a line before or after a known unique string |
| **file_slice** | Remove content between two unique text anchors (inclusive) | Removing lines from a file by their content |
| **file_replace** | Replace occurrences of old text with new text (with optional text-anchor range) | Renaming a symbol or fixing a typo across a file |
| **list_directory** | List files and directories | Discovering structure and file names |
| **set_working_file** | Mark the current file of focus | Tasks that need a stable file context |
| **refresh_cache_file** | Refresh cached file content | After external edits or when stale content is suspected |
| **lsp_query** | Language-server queries for code intelligence | goToDefinition, findReferences, hover, documentSymbol, workspaceSymbol, goToImplementation, findCallers, rename |
| **query_user** | Ask the user a question | Only when the task genuinely needs clarification |
| **task_spawn** | Start a delegated task | When the runtime allows background or parallel work |
| **task_await** | Wait for a delegated task to finish | When you already have a task id and need its result |
| **task_status** | Inspect delegated task state | Checking progress or confirming completion |

## Tool Selection

```
Need to understand code?
├─ Read a file → file_read or file_read_range
├─ Find where something is defined or used → lsp_query
└─ Check file structure → list_directory

Need to make changes?
├─ Create a new file → file_create (refuses to overwrite existing files)
├─ Delete a file → file_remove
├─ Edit text in a file → file_append, file_insert, file_slice, or file_replace
├─ Run a command or test → shell_exec
└─ Explore which files to change → list_directory or shell_exec with a search command

Need file size or line counts?
├─ Count lines → file_line_count
└─ Read large files safely → file_read_range

Need coordination?
├─ Start delegated work → task_spawn
├─ Wait for a task result → task_await
└─ Check task progress → task_status

Need human input?
└─ Ask the user directly → query_user
```

## LSP Tool: Code Intelligence

Use `lsp_query` for precise, semantic code navigation. Prefer it when you need
symbol-level answers instead of text matching.

**Use LSP when:**
- Finding definitions
- Finding all references
- Getting type or hover information
- Listing symbols in a file
- Searching symbols across the workspace
- Finding implementations of a trait
- Finding callers of a function
- Renaming a symbol consistently

**Use shell-based search when:**
- You need literal text matches, comments, or string searches
- You are scanning by filename pattern or content pattern
- The symbol is not yet defined or not recognized by the language server

**Rule of thumb**: use `lsp_query` for symbols; use `shell_exec` for text search.

**Coordinate rule:** `lsp_query` input coordinates (`line`, `character`) are
zero-based. Results are displayed with one-based coordinates. When using a
coordinate from an `lsp_query` result as input to a subsequent call, subtract 1
from both the line and character values. Failing to do this causes the follow-up
call to target the wrong position.

For complete per-operation parameter requirements, workflow patterns, and error
handling, invoke the `lsp-query-usage` skill.

## Repository Guidance

Use the repository's guidance documents and skill files for standards, decision trees, architecture rules, and workflow conventions. Those documents define the repo-specific behavior; this file only supplies the shared execution model.

## Workflow: Read → Understand → Apply

1. **Read** - Inspect the relevant files with `file_read`, `file_read_range`, or `list_directory`
2. **Understand** - Identify the intent, patterns, and validation points
3. **Apply** - Make the targeted change with `file_create`, `file_append`, `file_insert`, `file_slice`, `file_replace`, `file_remove`, or `shell_exec`
4. **Verify** - Confirm the result with `shell_exec`, `file_line_count`, or follow-up reads

## Context Discipline

- Use `file_read_range` for large files instead of reading them whole
- Use `file_line_count` before deciding how much to read
- Use a size-check tool call before high-volume requests:
  - Estimate file count before broad directory listings
  - Count lines before full-file reads
- If a tool returns a large-request warning, immediately retry with a smaller request by narrowing scope or paginating results
- Keep command output short when using `shell_exec`
- Batch independent reads and searches instead of doing them one by one
- Prefer targeted requests (specific paths, bounded ranges, limited result windows) to avoid context overload
- Avoid loading more repository context than needed for the task

## Large Tool Requests

- Treat large-request warnings as required guidance, not optional advice
- Shrink request size proactively before sending:
  - Narrow path scope to the minimum relevant directory or file set
  - Use pagination or chunked reads for long listings and large outputs
  - Prefer `file_read_range` over full `file_read` when size is uncertain
- Verify request size first with tool calls (for example, file counts or line counts) before loading content at scale
- Continue in bounded chunks until complete rather than issuing one broad request

## Delegation

Treat delegated tasks as separate executors. Keep the work item scoped, provide the necessary context, and wait for the task result before building on it.

## Summary: Shared Checklist

- [ ] Tool names match the actual runtime
- [ ] Shared guidance stays neutral across roles
- [ ] Large files are handled with ranged reads
- [ ] Large requests are pre-sized and paginated when needed
- [ ] Symbol work uses `lsp_query`
- [ ] Text work uses shell-based search
- [ ] Output stays focused and concise