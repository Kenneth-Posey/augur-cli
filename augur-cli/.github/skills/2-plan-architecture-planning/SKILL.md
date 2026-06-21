---
name: 2-plan-architecture-planning
description: "Defines module structure, ownership, and dependency direction at plan time, producing dependency graphs and validating acyclic flow before implementation. Use when module boundaries and dependency direction must be established before implementation."
---

# Skill: 2-Plan-Architecture-Planning

## Scope

**In Scope:**
- Module decomposition: identifying logical boundaries, ownership, and layer tiers
- Dependency direction validation: ensuring acyclic dependency graph (DAG)
- Interface contracts: specifying what each module exports and what it depends on
- Layer ordering: establishing which tiers must exist before others
- Cross-module reuse: identifying common abstractions and avoiding duplication
- Circular dependency detection and resolution strategies
- Boundary enforcement rules and module isolation constraints

**Out of Scope:**
- Implementation code (algorithms, business logic)
- Language-specific patterns, syntax, or idiomatic conventions
- Test infrastructure or testing strategies
- Performance optimization
- Build system configuration
- Runtime deployment or infrastructure

---

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### 1. Module

- **Characteristics:** cohesive, independently testable, with minimal external coupling
- **Representation:** a directory with public interface and internal implementation
- **Ownership:** a single owner is responsible for changes; no dual ownership

### 2. Dependency

A directed relationship: module A depends on module B when A consumes B's public interface.

- **Direction:** one-way only (A → B); B must never import A
- **Representation:** import statements, trait bounds, or function parameters
- **Strength:** hard dependencies (compile time) vs. soft dependencies (runtime config)
- **Reuse:** shared abstractions that multiple modules depend on (common layer)

### 3. Layer (Tier)

A horizontal stratum of modules organized by abstraction level and dependency direction.

**Layer Types (lowest to highest):**
1. **Domain Contracts Layer:** domain-specific types, enums, errors; no external dependencies except standard library
2. **Core Logic Layer:** algorithms, decision helpers, pure functions; depends only on domain contracts
3. **Boundary & Adapters Layer:** I/O, external integrations, middleware; adapts core logic to external systems
4. **Composition Layer:** wiring, configuration, actor setup; composes lower layers into behaviors
5. **Application Layer:** entry points, CLI, API handlers; most specific surfaces

All dependencies point downward (higher layers depend on lower layers; lower layers never depend on higher).

### 4. Dependency Graph (DAG)

A directed acyclic graph representing all modules and their dependencies.

- **Acyclic:** no cycles allowed (A → B → C → A is forbidden)
- **Paths:** every dependency chain must have a clear beginning (leaf nodes with no dependencies) and end (root nodes consumed by nothing)
- **Validation:** detect circular imports, transitive cycles, and implicit bidirectional coupling
- **Tools:** graph visualization, topological sort, strongly connected component analysis

### 5. Interface Contract

The public surface of a module: what it exports, what it requires, and what guarantees it provides.

- **Exports:** list of public types, traits, functions, constants
- **Dependencies:** explicit list of modules/libraries this module depends on
- **Guarantees:** error handling, latency, persistence contracts, or semantic invariants
- **Breaking vs. Stable:** which symbols are stable; which are internal-only

### 6. Boundary Constraint

A rule that enforces module isolation and prevents inappropriate coupling.

- **Examples:**
  - Persistence modules never export domain types directly; they return newtyped wrappers
  - I/O modules never depend on business logic; logic modules never depend on I/O
  - Configuration is passed down; modules never read global state

### 7. Reuse Candidate

A module, trait, or abstraction that multiple modules depend on, reducing duplication.

- **Criteria:** used by 2+ modules; logically independent; no circular dependency risk
- **Placement:** must live in a lower layer than all consumers
- **Example:** error types, ID generators, validation helpers

---

## Composition & References

### Architecture Artifact Structure

An architecture plan typically includes:

1. **Module Inventory**
   - Name, purpose, current/proposed structure
   - Layer assignment (domain, core, boundary, composition, app)
   - Ownership

2. **Dependency Matrix**
   - Which modules depend on which (rows = dependents, columns = dependencies)
   - Identifies gaps, redundancy, and cycles

3. **Layer Diagram**
   - Visual representation of layers and their dependencies
   - Shows module grouping by tier

4. **Interface Contracts** (for each module)
   - Public symbols (traits, types, functions, constants)
   - Required inputs from dependencies
   - Guarantees and invariants

5. **Circular Dependency Analysis**
   - Detected cycles, if any
   - Resolution strategy (merge, new shared module, refactor boundary)

6. **Reuse Register**
   - Common abstractions, helpers, error types
   - Candidates for shared modules or base libraries
   - Dependencies satisfied by reuse

7. **Boundary Rules**
   - Constraints on what each module can do or consume
   - Examples: "X-layer modules never import Y-layer modules"

### Cross-Skill References

- **Depends On:** None (no prerequisite skills; language-agnostic)
- **Feeds:** dependency-plan-evaluator (audits existing code against the planned DAG)
- **Produces:** DAG and interface contracts used during implementation

---

## Examples

### Example 1: Single-Tier (Monolithic) to Multi-Tier Refactor

**Before:** All code in one module; no clear layers; cyclic imports possible.

**After:**
```
Layer 1 (Domain):     User, Order, Payment types
Layer 2 (Core):       OrderProcessor, PaymentValidator (depend on Layer 1 only)
Layer 3 (Boundary):   DatabaseAdapter, PaymentGatewayClient (depend on Layer 2 & 1)
Layer 4 (Composition): OrderService (wires Layers 1-3)
```

**Dependency Check:** Layer 4 → Layer 3 → Layer 2 → Layer 1 ✓ No cycles.

**Interface Contracts:**
- **Layer 1 (User, Order, Payment):** Export types; no dependencies
- **Layer 2 (OrderProcessor):** Import User, Order, Payment from Layer 1; export business logic
- **Layer 3 (DatabaseAdapter):** Import OrderProcessor from Layer 2; export adapter trait
- **Layer 4 (OrderService):** Import OrderProcessor and DatabaseAdapter; export ready-to-use service

---

### Example 2: Circular Dependency Detection

**Before (Problematic):**
```
ServiceA imports ServiceB
ServiceB imports ServiceC
ServiceC imports ServiceA  ← CYCLE DETECTED
```

**Resolution Options:**
1. **Extract shared module:** Create shared module (Logger, Config) that both A and C depend on; C no longer imports A
2. **Merge modules:** If A and C are tightly coupled, merge into single module
3. **Invert dependency:** ServiceA imports ServiceC (not vice versa); remove C → A

**After (Resolved):**
```
ServiceA → ServiceB → ServiceC
ServiceA → SharedConfig
ServiceC → SharedConfig
```

No cycles; all dependencies point in one direction.

---

### Example 3: Reuse Register

**Problem:** Multiple modules need error handling, ID generation, logging.

**Solution: Shared Foundation Module**
```
Layer 1 (Foundation):
  - ErrorCode, ErrorContext (exported)
  - IdGenerator trait (exported)
  - ValidationHelpers (internal)

Layer 2 (Domain):
  - User, Order types (depend on Foundation for error types)

Layer 3 (Core):
  - UserService, OrderService (depend on Foundation for ID generation)

Layer 4 (Boundary):
  - DatabaseAdapter, ApiHandler (depend on Layer 3 and Foundation)
```

**Benefit:** No duplication; single source of truth for errors and IDs; all modules can reuse without coupling.

---

## Decision Criteria

### When to Create a New Module

A new module is justified when:
1. **Single Responsibility:** the module has one reason to change
2. **Reuse:** 2+ other modules can depend on it without creating cycles
3. **Clear Boundary:** input/output contracts are unambiguous
4. **Layer Fit:** it fits into a well-defined layer (not straddling multiple tiers)
5. **No Circular Risk:** no dependency path creates a cycle

**Red Flags:**
- "This module is used by nearly everything" → likely too low-level; check for missing abstraction
- "No other module depends on this; it's internal-only" → may belong as sub-module, not top-level
- "This module imports from 5+ layers" → likely spans layers; refactor into focused sub-modules

### When to Merge Modules

Merge is justified when:
1. **Tight Coupling:** modules always change together
2. **Circular Dependency:** the only way to resolve a cycle is to unify them
3. **Thin Module:** one module is a thin wrapper around another
4. **Single Concept:** the modules represent parts of a single coherent idea

### When to Create a Shared (Reuse) Module

A shared module is justified when:
1. **Multi-Module Use:** 2+ independent modules need the same abstraction
2. **No Circular Risk:** shared module depends only on lower layers; all consumers are higher
3. **Stable Interface:** the abstraction is unlikely to change
4. **Semantic Cohesion:** items in the module belong together logically

**Examples:** Error types, ID generators, validation helpers, common traits.

---

## Validation Rules

### Rule 1: Acyclic Dependency Graph

**Requirement:** No cycles. Every dependency path must terminate (no A → ... → A).

**Check:**
```
FOR each module M in graph:
  IF any dependency path from M leads back to M:
    FAIL "Cycle detected"
  ELSE:
    PASS
```

**Remediation:** See circular dependency resolution in Examples.

---

### Rule 2: Layer Ordering

**Requirement:** All dependencies point downward (higher layers depend on lower layers).

**Check:**
```
FOR each dependency (A depends on B):
  IF layer(A) < layer(B):  # A is lower than B
    FAIL "Dependency points upward"
  ELSE IF layer(A) > layer(B):  # A is higher than B
    PASS "Dependency points downward"
  ELSE:
    FAIL "Same-layer dependency without explicit horizontal justification"
```

**Same-Layer Dependencies:** Allowed only if:
- Both modules are in the **same layer** and the dependency is explicitly documented
- The dependency does not create a cycle when combined with other layers

---

### Rule 3: Interface Clarity

**Requirement:** Each module has a clear, explicit interface contract.

**Check:**
```
FOR each module M:
  IF (public symbols are defined) AND (dependencies are listed) AND (guarantees are stated):
    PASS "Interface is clear"
  ELSE:
    FAIL "Interface is ambiguous; add explicit contract"
```

---

### Rule 4: Reuse Register Integrity

**Requirement:** Shared modules do not depend on modules that depend on them.

**Check:**
```
FOR each shared module S:
  FOR each consumer C of S:
    IF any dependency path from S leads to C:
      FAIL "Shared module creates reverse dependency"
    ELSE:
      PASS
```

---

### Rule 5: Boundary Enforcement

**Requirement:** Boundary constraints are documented and validated during code review.

**Check:**
```
FOR each boundary rule R:
  IF code violates R (e.g., I/O module imports business logic):
    FAIL "Boundary constraint violated"
  ELSE:
    PASS "Boundary enforced"
```

**Example:** "Persistence modules must never export domain types directly. Instead, return newtyped wrappers or DTOs."

---

### Rule 6: Module Ownership

**Requirement:** Each module has a single owner responsible for changes.

**Check:**
```
FOR each module M:
  IF exactly_one_owner(M):
    PASS "Ownership is clear"
  ELSE:
    FAIL "Module has no owner or multiple owners"
```

---

## Validation Rules: DAG Validation Process

### Input
- Module inventory with layer assignments
- Dependency matrix or import statements
- Interface contracts

### Steps

1. **Parse Dependencies:** Extract all edges (A depends on B) from source or spec
2. **Build Graph:** Create directed graph with modules as nodes, dependencies as edges
3. **Topological Sort:** Attempt to topologically sort the graph
   - **Success:** Graph is acyclic; output sorted order
   - **Failure:** Graph has cycles; identify cycle(s) and resolution strategies
4. **Layer Validation:** For each edge, confirm layer(dependent) > layer(dependency)
   - **Pass:** All edges point downward
   - **Fail:** List upward dependencies; require refactor or layer reassignment
5. **Reuse Validation:** Confirm shared modules have no reverse dependencies
6. **Boundary Checks:** Audit rules against the DAG (no I/O in core logic, etc.)

### Output

- **DAG Diagram:** Visual representation showing all modules, layers, and dependency directions
- **Cycle Report:** List of any cycles (or "None" if acyclic)
- **Layer Assignment Report:** Module → Layer mapping
- **Interface Contracts:** Exported symbols and dependencies for each module
- **Validation Status:** PASS (no issues) or FAIL (list issues and remediation)

---
