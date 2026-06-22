---
name: 0-global-line-count-check
description: >
  Rules for checking source file and plan-file size limits. Use when
  planning, reviewing, or deciding whether files need to be split.
---

# 0-global-line-count-check

## Source Code Files

**Threshold:** 200 lines of logic.

Lines that count toward the 200-line limit:

- Branching (`if`, `else`, `match` arms with logic)
- Computation and arithmetic expressions
- State transitions and mutation
- Decision-making and control flow (`for`, `while`, `loop`, early returns)
- Function or method signatures that contain logic-bearing defaults or guards
- Closure bodies
- Macro invocations that perform logic

Lines excluded from the logic-line count:

- Import and module declaration lines
- Comment lines (line comments, doc comments)
- Annotation and attribute lines (decorators, macros, attributes)
- Pure type-declaration lines (type aliases, interface declarations)
- Constructor boilerplate with no logic (e.g., standard `new()`/`init()` patterns)
- Constant and static value declarations with no computation
- Test module stubs (file-level test module declarations with no inline logic)
- Structural punctuation lines (standalone opening/closing braces or brackets)

Guidance: the threshold measures behavioral density. A file with 250 total
lines but only 150 lines of logic is compliant. A file with 210 total lines
where 205 contain logic is over the limit.

When a file exceeds the threshold, refactor by extracting focused helper
functions, splitting into sub-modules, or moving reusable logic into an
`_ops` companion module.

## Key Files

- `README.md` - overview and usage notes

## Plan Files (Markdown `.md` in `plans/`)

**Threshold:** 300 lines total, with no exclusions.

All lines count: prose, tables, code blocks, blank lines, headers, and links.
Use raw `wc -l` so plan files stay small and easy to review.

When a plan file approaches or exceeds 300 lines, split it into linked part
files and follow the plan layout rules in the `0-global-plan-implementation` skill.

## Quick-Check Commands

Source code logic lines (approximate):

```text
Count non-blank, non-comment lines in a source file using your language's
equivalent blank-line and comment exclusion pattern.
```

Plan file total lines:

```bash
wc -l plans/*.md
```
