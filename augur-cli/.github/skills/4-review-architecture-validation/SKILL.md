---
name: 4-review-architecture-validation
description: >
  Stage 4 architecture validation. Check that module structure, dependency direction,
  ownership boundaries, and feed-graph wiring match the Stage 2 design, independent of
  language.
---

# Skill: 4-Review Architecture Validation

## Purpose

Validates that the implemented module structure, dependency graph, ownership boundaries,
and actor/feed wiring match the Stage 2 design. This skill is read-only: report findings
without applying fixes.

## Key Files

- `README.md` - overview and usage notes

## What to Validate

### 1. Module Placement
- Every new or modified module is placed in the correct tier as documented in the design
- No cross-tier misplacement (e.g., business logic in adapters, adapters inside the domain)
- Helper/utility modules are positioned correctly relative to their consuming module

### 2. Dependency Direction
- All imports/uses respect the allowed direction per architectural layer
- Core/domain modules do not import from adapter or infrastructure modules
- No circular imports or dependency cycles (A → B → A)
- Dependency direction matches the directed graph documented in the Stage 2 design

### 3. Ownership and Encapsulation Boundaries
- Module public surfaces expose only what the design specifies
- Internal state and invariants are not leaked through the public API
- Cross-module interactions occur only through declared interfaces

### 4. Feed and Wiring Graph
- Inter-module or actor-to-actor feeds form a directed acyclic graph (DAG)
- No bidirectional feeds or circular subscriptions
- Spawn or initialization order respects the topological sort of the dependency graph
- Each module/actor has clearly defined upstream sources and downstream outputs

## Pass Conditions

- All modules are present and placed in the correct tier
- Dependency graph is acyclic and matches the Stage 2 design artifact
- No public surface leakage (internal state exposed through public API)
- All cross-boundary interactions use declared interfaces
- Feed/wiring graph is a valid DAG

## Fail Conditions

- **Critical:** Cycle detected in the dependency graph
- **Critical:** Layer boundary violated (e.g., core depending on an adapter)
- **Critical:** Encapsulation leak (private invariants accessible from outside)
- **High:** Module placed in the wrong tier
- **High:** Public surface expands beyond what the design specified
- **Medium:** Potential future violation (structural smell, not yet a violation)

## Validation Signal

| Severity present | Signal |
|---|---|
| Critical or High findings | `fail` |
| Medium or Low findings only | `pass` with warnings |
| Validation timed out | `fail` |

## Report Format

**On pass (signal = pass):**
- Emit one summary line per validation category in the form:
  `Category Name: ✓ (brief note, e.g., "12 modules verified")`
- Emit the JSON diagnostic block with `findings: []` (or `findings` with only
  Medium/Low entries if present)
- Omit: detailed row-by-row verification tables, per-item bullet lists,
  validation checklists, and any duplicate `## Signal` section at the bottom
  - the signal is already stated in the report header

**On fail (signal = fail):**
- Emit full detail (table/bullets/evidence) only for the failing categories
- Emit the summary line format for all passing categories
- Emit the JSON diagnostic block with all findings fully populated

## Language Companion

Look up `4-review-architecture-validation` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for
language-specific validation rules, module tier definitions, dependency direction rules,
and checker logic.
