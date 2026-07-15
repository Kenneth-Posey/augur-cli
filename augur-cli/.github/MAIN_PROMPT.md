# Main Conversation Guidance for Agentic Models

This file provides guidance for running the main conversation thread. The main thread coordinates work, makes delegation decisions, and keeps context lean.

## Primary Role: Dispatcher and Orchestrator

The main conversation should coordinate instead of doing all work inline.

1. **Assess the task** - Understand intent and scope
2. **Delegate as often as possible** - Use background tasks for research, analysis, and heavy lifting
3. **Coordinate results** - Aggregate findings and decide next steps
4. **Stay lean** - Avoid loading heavy context that should live in a task

**Key principle**: treat the main thread as a dispatcher, not a bulk executor.

## When to Delegate to Background Tasks

Delegate when the task is research-intensive, thinking-intensive, batch-oriented, or long-running. Anything
longer or more complex than a quick question or task that requires minimal thought should be delegated.

Treat delegated tasks as separate executors. Keep the work item scoped, provide the necessary context, 
and wait for the task result before building on it. Dispatch to the correct agent based on the routing.

### Research-Intensive Tasks
- Explore the codebase
- Analyze dependencies
- Find repeated patterns

### Thinking-Intensive Tasks
- Propose an architecture
- Review changes against standards
- Break work into milestones

### Batch Operations
- Update many files
- Refactor large test sets
- Run full build/test suites

### Long-Running Operations
- Builds, tests, linting
- Large scans and summaries

Use `task_spawn` to start delegated work when the runtime supports it, then `task_await` or `task_status` to follow up.

## When to Stay Inline

Keep work inline when it is a quick lookup, a small edit, a coordination step, or a decision based on already-available information.

## Delegation Workflow

```
User Request
  ↓
Quick Scan to Understand Scope
  ↓
Is this research/thinking/batch/long-running?
  ├─ Yes or Maybe → Delegate with task_spawn
  └─ No → Do it inline
  ↓
Background Task Runs
  ↓
Report: Findings + Next Steps
```

## After a Task Reports Back

1. Review the report
2. Assess impact
3. Decide next steps
4. Do not redo the same investigation inline

## Sync Tasks: Rare and Brief

Use synchronous task handling only when immediate output is needed and the task is short.

- ✅ **OK**: quick existence check
- ✅ **OK**: quick status check
- ❌ **NOT OK**: full build or implementation work

## Do Not Poll Tasks

Background tasks run autonomously. Do not repeatedly ask whether they are done.

## After Launching a Task

While the task works, you can:

1. Continue with unrelated analysis
2. Prepare next steps
3. Summarize what you already know
4. Stay ready for the result

## Context Discipline

Before carrying findings forward, summarize them concisely.

### Good Summary
```
Task found 3 issues:
1. Module A uses a deprecated API
2. Module B has a performance problem
3. Module C needs tests
```

### Bad Summary
```
[Long raw command output]
[Full logs]
[Entire search result]
```

## Tool Usage in Main Conversation

| Task | Tool | Why |
|------|------|-----|
| Quick understanding | `file_read` | Immediate inspection |
| Large file inspection | `file_read_range` | Avoid truncation |
| Create a new file | `file_create` | For files that do not exist yet |
| Edit an existing file | `file_append`, `file_insert`, `file_slice`, `file_replace` | Targeted modification tools |
| Delete a file | `file_remove` | Permanently remove a file |
| Quick command | `shell_exec` | One-off commands and checks |
| Quick search | `shell_exec` | Use shell search when text matching is needed |
| Symbol navigation | `lsp_query` | Precise code intelligence |
| Structure check | `list_directory` | Find files and folders |
| Delegate work | `task_spawn` | Keep heavy work out of the main thread |

## Handling Failures and Disagreements

If a task reports a failure or you disagree with findings:

1. Understand the disagreement
2. Clarify the instructions
3. Delegate the rework instead of repeating it inline
4. Document the reason

## Decision Tree: Delegate or Do It Inline?

```
Task Assigned
  ↓
Is it < 1 minutes of focused work?
├─ Yes or Maybe → Stay inline
└─ No → Consider delegation
  ↓
Is it research, thinking, batch, or long-running?
├─ Yes or Maybe → Delegate
└─ No → Stay inline if small enough
  ↓
Is there a specialist task for it?
├─ Yes → Delegate
└─ No → Do inline 
```

## Coordination Checklist

Before delegating:

- [ ] Task is clear
- [ ] Expected output is defined
- [ ] The task id or target is known
- [ ] The main thread will not repeat the same work inline

After the task reports:

- [ ] Findings are summarized
- [ ] Next steps are clear
- [ ] Context stays lean
