---
name: 1-design-feature-decomposition
description: "Breaks high-level requirements into atomic, implementable feature specifications with full traceability and testable acceptance criteria. Use during design when translating requirements, user stories, or acceptance criteria into buildable features."
---

# Skill: Design Feature Decomposition

## Scope

### Input Artifacts
- Requirements documents (user stories, acceptance criteria, technical specifications, RFCs)
- Constraints (timeline, resource, platform, performance, security)
- Dependency maps (external services, legacy systems, build tools)
- Acceptance criteria (explicit or implicit in requirements)

### Output Artifacts
- Feature specification document: a structured list of atomic features, each with:
  - Unique identifier (feature ID)
  - Acceptance criteria (testable, non-ambiguous)
  - Dependencies on other features (feature DAG)
  - Implementability markers (estimated complexity, assumptions)
  - Scope boundary (what is in/out)

### Non-Goals
- Implementation details (algorithms, data structures, API design beyond interface contracts)
- Code-level architecture (modules, types, traits - that comes later in planning)
- Test automation scripts
- Deployment automation

---

## Key Files

- `README.md` - overview and usage notes

## Key Concepts

### Requirement vs. Feature
- **Requirement:** A user need or business objective (often high-level, may be ambiguous).
  - Example: "Users must be able to search for items."
- **Feature:** A discrete, testable unit of behavior derived from one or more requirements.
  - Example: "Full-text search over item titles with pagination, returning up to 100 results per page."

### Atomicity
A feature is atomic if splitting it further would lose user value or make implementation less clear. Test it by asking:
- Can a single test case verify the feature? If yes, likely atomic.
- Does the feature require coordination across multiple subsystems that could fail independently? If yes, decompose.

### Granularity
- **Too coarse:** "Payment system" (spans authorization, validation, settlement, reconciliation - multiple features).
- **Just right:** "Process credit card authorization and return approval/denial within 2 seconds" (testable, single concern).
- **Too fine:** "Initialize HTTP client library" (implementation detail, not a feature).

### Implementability Markers
Each feature must declare:
- **Assumed complexity:** simple, moderate, complex
- **Known blockers:** missing data, third-party API delays, build tool gaps
- **Hidden assumptions:** "Assumes item schema includes `title` field" or "Assumes external cache is available"
- **Acceptance risk:** low, medium, high (based on unknown factors or technical uncertainty)

### Feature Dependency Graph
- Features that must exist before others can be tested or deployed
- Example: "User authentication" must precede "User profile editing"
- Expressed as a DAG: no cycles allowed, and every feature must have a path to the root

---

## Feature Specification Format

### Structure of a Feature Specification

Each feature spec includes:

```
---
feature_id: FE-001
requirement_sources: [REQ-A1, REQ-A2]  # Traceability to original requirements
acceptance_criteria:
  - Criterion 1 (testable condition)
  - Criterion 2 (testable condition)
scope_in:
  - What is included
scope_out:
  - What is excluded and why
dependencies:
  feature: [FE-001-dependency, FE-002-dependency]  # or "none"
  external: [service-name, library-name]  # or "none"
complexity: moderate | simple | complex
assumptions: "List of environment/schema assumptions or none"
---
```

### Requirement Traceability Matrix
Maintain a table mapping each requirement ID to the features it generates:

| Requirement ID | Feature ID(s) | Status | Notes |
|---|---|---|---|
| REQ-A1 | FE-001, FE-002 | Covered | Split into search and result formatting |
| REQ-A2 | FE-001 | Covered | Covered by search acceptance criteria |

**Validation Rule:** Every requirement must map to at least one feature. No requirement may be left unmapped.

---

## Examples

### Example 1: Payment Feature Decomposition

**Raw Requirement:**
> "Users must be able to pay for orders with credit cards."

**Decomposed Features:**

**FE-AUTH-CC:** Validate Credit Card Format and Expiry
- Acceptance: Input validation passes for valid Visa/Mastercard; rejected for invalid/expired cards
- Complexity: simple
- External deps: none

**FE-AUTH-CHARGE:** Process Credit Card Charge via Payment Provider
- Acceptance: Charge succeeds within 2 seconds; returns authorization token; failure reason logged
- Complexity: moderate
- External deps: payment-provider-api
- Known blockers: API credentials must be configured at deploy time
- Assumptions: User identity verified before charge

**FE-AUTH-RECEIPT:** Generate and Store Payment Receipt
- Acceptance: Receipt emailed to user; record stored in audit log; user can retrieve receipt from dashboard
- Complexity: moderate
- External deps: email service
- Dependencies: FE-AUTH-CHARGE (must charge before receipt generated)

**Traceability:**
| Requirement | Features | Status |
|---|---|---|
| Users must pay with credit cards | FE-AUTH-CC, FE-AUTH-CHARGE, FE-AUTH-RECEIPT | Covered |

---

### Example 2: Search Feature Decomposition

**Raw Requirements:**
> "Users need to search for items. Results should be paginated and sortable."

**Decomposed Features:**

**FE-SEARCH-QUERY:** Accept and Validate Search Query
- Acceptance: Query string 1–500 chars, URL-decoded, trimmed; non-ASCII characters accepted; special regex chars escaped
- Complexity: simple
- Cross-cutting: Security (input sanitization); Logging (query hash logged)

**FE-SEARCH-INDEX:** Search Index Lookup
- Acceptance: Full-text search over item titles; returns up to 100 results within 2 seconds
- Complexity: moderate
- External deps: search index (ElasticSearch/Solr)
- Known blockers: Index must be pre-populated; index schema must include title field
- Assumptions: Index refresh lag acceptable (1-hour eventual consistency)

**FE-SEARCH-SORT:** Sort and Filter Results
- Acceptance: Results sorted by relevance (default), name, date; user can toggle; invalid sort params rejected
- Complexity: moderate
- Dependencies: FE-SEARCH-INDEX (results must exist before sort)

**FE-SEARCH-PAGINATE:** Paginate Result Sets
- Acceptance: Default 20 results/page; supports page size 1–100; next/prev links provided
- Complexity: simple
- Dependencies: FE-SEARCH-INDEX (results must exist before pagination)

**Traceability:**
| Requirement | Features | Status |
|---|---|---|
| Users can search items | FE-SEARCH-QUERY, FE-SEARCH-INDEX | Covered |
| Results are sortable | FE-SEARCH-SORT | Covered |
| Results are paginated | FE-SEARCH-PAGINATE | Covered |

---

## Decision Criteria

### When to Decompose Further
1. **Independent test paths:** If two criteria need different test infrastructure or test data, they may belong in separate features.
2. **Different delivery timelines:** If one piece can ship without the other, decompose. Example: feature flag for new search algorithm can ship independently of the UI that uses it.
3. **Different risk profiles:** If one piece is high-uncertainty and others are low, decompose to isolate risk.
4. **Crossing system boundaries:** If a feature spans multiple independent systems (frontend, backend, database), break it into coordination features.

### When NOT to Decompose
1. **Artificial fragmentation:** "Initialize database connection" is not a feature - it's an implementation step.
2. **Tightly coupled logic:** If feature B cannot be meaningfully tested without feature A already existing, keep them together or mark A as a hard dependency.
3. **Sub-feature complexity insignificant:** Micro-features (< 2 hours estimated work) don't justify separate specification.

### Handling Ambiguity
If a requirement is ambiguous, decomposition **must stop and clarify with stakeholders**:
- Ask: "Does this mean X or Y?"
- Document the clarification as an assumption in the feature spec
- If no clarification is available, mark the feature as blocked and record the blocker explicitly

---

## Validation Rules

### Rule 1: Completeness (No Orphaned Requirements)
**Assertion:** Every requirement in the input maps to exactly one or more features in the output.

**Check:**
```
for each requirement in input_requirements:
  if requirement not in traceability_matrix:
    raise "Orphaned requirement: {requirement}"
```

**Pass Condition:** Traceability matrix 100% populated, no unmapped requirements.

---

### Rule 2: No Orphaned Features
**Assertion:** Every feature in the spec is traced back to at least one input requirement.

**Check:**
```
for each feature in output_features:
  if feature.requirement_sources is empty:
    raise "Orphaned feature: {feature_id}"
```

**Pass Condition:** Every feature has at least one `requirement_sources` entry.

---

### Rule 3: Implementability (All Features Are Buildable)
**Assertion:** Each feature specifies how it will be tested and what it depends on.

**Check per Feature:**
```
- acceptance_criteria.length > 0  (at least one criterion)
- acceptance_criteria all testable (no vague terms like "fast", "intuitive")
- dependencies resolved (no circular deps, all dependencies are other features or external services)
- complexity assigned (simple | moderate | complex)
```

**Pass Condition:** All checks pass for all features; no ambiguous criteria (e.g., no "should be fast" without latency bound).

---

### Rule 4: Atomicity (Features Are Decomposed to Appropriate Grain)
**Assertion:** Each feature is small enough to be implementable and testable as a unit.

**Check per Feature:**
```
- feature does not span more than 3 independent subsystems (e.g., frontend + backend + DB)
- feature has a single primary acceptance criterion (others are validations/error cases)
- feature can be tested in a single test suite or integration test
```

**Pass Condition:** No feature is "too big" (spans unrelated concerns) or "too small" (is an implementation detail).

---

### Rule 5: Dependency Consistency (Feature DAG is Valid)
**Assertion:** Feature dependencies form a DAG (no cycles).

**Check:**
```
build_dependency_graph(all_features)
for each feature:
  if has_cycle(feature):
    raise "Circular dependency detected: {feature_id}"
```

**Pass Condition:** Feature dependency graph is acyclic and all dependencies exist.

---

## Quick Reference: Decomposition Checklist

Use this checklist when decomposing a requirement into features:

- [ ] Each feature has a unique ID (FE-XXX)
- [ ] Each feature is traced to at least one requirement (via `requirement_sources`)
- [ ] Each feature has at least one acceptance criterion (testable, non-ambiguous)
- [ ] Each feature lists scope_in and scope_out
- [ ] Each feature declares complexity (simple | moderate | complex)
- [ ] Each feature declares dependencies (feature IDs or "none")
- [ ] Each feature declares external dependencies (service/library names or "none")
- [ ] No feature is circular-dependent on another
- [ ] No orphaned features (all features traceable to requirements)
- [ ] No orphaned requirements (all requirements mapped to features)
- [ ] All acceptance criteria are testable (no vague language)
- [ ] Feature DAG is valid (no cycles, all dependencies exist)
