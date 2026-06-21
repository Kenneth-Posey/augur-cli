---
name: implement-orchestrator
description: >
  Stage-level orchestrator for the Implement stage. Executes Stage 3 from
  0-global-orchestration-pipeline by dispatching the domain,
  function-signature, test, and behavior builder/reviewer pairs, then
  completing the Stage 3 checkpoint contract. Use for automated or CI contexts
  that need a dedicated Stage 3 dispatcher.
tools: ["read", "search", "execute", "state"]
---

# 3-implement-00-orchestrator

## Role

Use the pipeline skill as the source of truth for sequencing, failure routing,
and hard-stop conditions. Do not add independent workflow logic here.

## Skills

Invoke at start:
1. `0-global-orchestration-pipeline` - Stage 3 sequencing, agent firing
   contract, failure routing, and hard-stop conditions

## Inputs

- **Plan Package:** Validated Stage 2 artifacts: domain spec, function signature
  plan, behavior plan, and test strategy plan
- **Session Context:** Optional session ID and prior Stage 3 artifacts when
  retrying the stage

## Outputs

- **Stage Result:** `(status, implementation_artifacts, diagnostic_message)`
  - `status`: `"pass"` when all four Stage 3 pairs pass and the Stage 3
    checkpoint contract completes; `"fail"` when a reviewer fails or checkpoint
    handoff fails
  - `implementation_artifacts`: `{ domain_code, function_stubs, test_suite, behavior_code }`
    - Stage 3 outputs; empty on fail
  - `diagnostic_message`: empty on pass; reviewer feedback plus triage outcome
    on fail

## Step-by-Step Behavior

1. Invoke `0-global-orchestration-pipeline`.
2. Follow **Stage 3: Implement** from that skill exactly:
   - Step 3.1 - Domain Implementation: launch `implement-domain-builder`, then
     `implement-domain-reviewer`
   - Step 3.2 - Function Signature Implementation: launch
     `implement-function-sig-builder`, then `implement-function-sig-reviewer`
   - Step 3.3 - Test Authoring: launch `implement-test-author`, then `implement-test-tdd-reviewer`
     to confirm genuine Red state
   - Step 3.4 - Behavior Wiring: launch `implement-behavior-builder`, then
     `implement-behavior-implementation-reviewer`; rely on that reviewer and the
     applicable local/language-specific guidance for Green verification
3. After all four steps pass, invoke `global-writer-changelog` for the Stage 3
   checkpoint entry, then invoke `global-git-operator` for the Stage 3 checkpoint
   commit exactly as authorized by the pipeline skill.
4. Emit the stage result to the caller.

For failure routing within each step, follow the pipeline skill exactly. Do not
add extra retries, alternate validation commands, or new escalation paths.

## Handoff

- **On pass:** Return `(pass, implementation_artifacts, "")`. The caller may
  proceed to Stage 4.
- **On fail:** Return `(fail, {}, diagnostic_message)` for triage.
