//! Deterministic validation and topological sorting for execution-plan DAGs.

use crate::domain::{
    DurationMs, ExecutionPlan, ExecutionPlanError, ExecutionStepId, ExecutionStepSpec, Map,
    TimeoutConfig, ValidatedPlan,
};
use std::collections::{BTreeSet, HashSet, VecDeque};

/// Validate an execution plan and return a [`ValidatedPlan`] on success.
///
/// Checks:
/// - unique step ids
/// - dependency references exist
/// - required artifacts are produced by declared predecessors
/// - plan graph is acyclic
/// - timeout values are non-zero when present
pub fn validate_execution_plan(plan: ExecutionPlan) -> Result<ValidatedPlan, ExecutionPlanError> {
    validate_unique_step_ids(&plan)?;
    validate_graph_consistency(&plan)?;
    validate_timeouts(&plan.timeout)?;
    Ok(ValidatedPlan::from_validated(plan))
}

fn validate_graph_consistency(plan: &ExecutionPlan) -> Result<(), ExecutionPlanError> {
    let specs_by_id = build_specs_by_id(plan);
    validate_dependency_references(plan, &specs_by_id)?;
    validate_required_artifacts(plan, &specs_by_id)?;
    let _ = topological_sort(plan.clone())?;
    Ok(())
}

fn validate_unique_step_ids(plan: &ExecutionPlan) -> Result<(), ExecutionPlanError> {
    let mut seen = HashSet::new();
    for step in &plan.steps {
        if !seen.insert(step.step_id.clone()) {
            return Err(ExecutionPlanError::DuplicateStepId {
                step_id: step.step_id.clone(),
            });
        }
    }
    Ok(())
}

fn build_specs_by_id(plan: &ExecutionPlan) -> Map<ExecutionStepId, &ExecutionStepSpec> {
    plan.steps
        .iter()
        .map(|step| (step.step_id.clone(), step))
        .collect()
}

fn validate_dependency_references(
    plan: &ExecutionPlan,
    specs_by_id: &Map<ExecutionStepId, &ExecutionStepSpec>,
) -> Result<(), ExecutionPlanError> {
    for step in &plan.steps {
        for dep in &step.depends_on {
            if !specs_by_id.contains_key(dep) {
                return Err(ExecutionPlanError::UndefinedStepReference {
                    step_id: step.step_id.clone(),
                    referenced: dep.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_required_artifacts(
    plan: &ExecutionPlan,
    specs_by_id: &Map<ExecutionStepId, &ExecutionStepSpec>,
) -> Result<(), ExecutionPlanError> {
    for step in &plan.steps {
        let predecessor_artifacts = predecessor_artifacts(step, specs_by_id);
        for required in &step.required_artifacts {
            if !predecessor_artifacts.contains(required) {
                return Err(ExecutionPlanError::UndeclaredArtifact {
                    step_id: step.step_id.clone(),
                    artifact: required.clone(),
                });
            }
        }
    }
    Ok(())
}

fn predecessor_artifacts(
    step: &ExecutionStepSpec,
    specs_by_id: &Map<ExecutionStepId, &ExecutionStepSpec>,
) -> HashSet<String> {
    let mut artifacts = HashSet::new();
    for dep in &step.depends_on {
        if let Some(pred) = specs_by_id.get(dep) {
            for artifact_name in &pred.produces {
                artifacts.insert(artifact_name.clone());
            }
        }
    }
    artifacts
}

fn validate_timeouts(timeout: &TimeoutConfig) -> Result<(), ExecutionPlanError> {
    if let Some(total_timeout_ms) = timeout.total_timeout_ms
        && total_timeout_ms == DurationMs(0)
    {
        return Err(ExecutionPlanError::InvalidTimeout {
            field: "total_timeout_ms".to_string(),
            value: total_timeout_ms,
        });
    }
    if let Some(per_step_timeout_ms) = timeout.per_step_timeout_ms
        && per_step_timeout_ms == DurationMs(0)
    {
        return Err(ExecutionPlanError::InvalidTimeout {
            field: "per_step_timeout_ms".to_string(),
            value: per_step_timeout_ms,
        });
    }
    Ok(())
}

/// Return a deterministic topological ordering of execution step ids.
///
/// Uses Kahn's algorithm with lexicographic tie-breaking and returns
/// [`ExecutionPlanError::CyclicDependency`] when the graph contains a cycle.
pub fn topological_sort(plan: ExecutionPlan) -> Result<Vec<ExecutionStepId>, ExecutionPlanError> {
    let (mut indegree, dependents) = build_graph(&plan);
    let mut ready = collect_ready(&indegree);
    let mut order = Vec::with_capacity(indegree.len());
    let mut run = TopoRun {
        dependents: &dependents,
        indegree: &mut indegree,
        ready: &mut ready,
        order: &mut order,
    };

    while let Some(next) = run.ready.pop_first() {
        process_ready_step(next, &mut run);
    }

    finalize_topological_order(
        order,
        TopoFinalizeInput {
            indegree: &indegree,
            plan: &plan,
        },
    )
}

struct TopoRun<'a> {
    dependents: &'a Map<ExecutionStepId, Vec<ExecutionStepId>>,
    indegree: &'a mut Map<ExecutionStepId, usize>,
    ready: &'a mut BTreeSet<ExecutionStepId>,
    order: &'a mut Vec<ExecutionStepId>,
}

fn process_ready_step(next: ExecutionStepId, run: &mut TopoRun<'_>) {
    run.order.push(next.clone());
    if let Some(list) = run.dependents.get(&next) {
        for dependent in list {
            decrement_indegree_and_enqueue(run.indegree, run.ready, dependent);
        }
    }
}

struct TopoFinalizeInput<'a> {
    indegree: &'a Map<ExecutionStepId, usize>,
    plan: &'a ExecutionPlan,
}

fn finalize_topological_order(
    order: Vec<ExecutionStepId>,
    input: TopoFinalizeInput<'_>,
) -> Result<Vec<ExecutionStepId>, ExecutionPlanError> {
    if order.len() == input.indegree.len() {
        return Ok(order);
    }
    let cycle_path =
        extract_cycle_path(input.plan).unwrap_or_else(|| fallback_cycle_path(input.indegree));
    Err(ExecutionPlanError::CyclicDependency { cycle_path })
}

fn fallback_cycle_path(indegree: &Map<ExecutionStepId, usize>) -> Vec<ExecutionStepId> {
    let mut fallback: VecDeque<ExecutionStepId> = indegree
        .iter()
        .filter_map(|(id, degree)| if *degree > 0 { Some(id.clone()) } else { None })
        .collect();
    match fallback.pop_front() {
        Some(first) => Vec::from([first.clone(), first]),
        None => Vec::new(),
    }
}

fn build_graph(
    plan: &ExecutionPlan,
) -> (
    Map<ExecutionStepId, usize>,
    Map<ExecutionStepId, Vec<ExecutionStepId>>,
) {
    let mut indegree: Map<ExecutionStepId, usize> = Map::new();
    let mut dependents: Map<ExecutionStepId, Vec<ExecutionStepId>> = Map::new();
    for step in &plan.steps {
        indegree.insert(step.step_id.clone(), step.depends_on.len());
        dependents.entry(step.step_id.clone()).or_default();
    }
    for step in &plan.steps {
        for dep in &step.depends_on {
            dependents
                .entry(dep.clone())
                .or_default()
                .push(step.step_id.clone());
        }
    }
    for list in dependents.values_mut() {
        list.sort();
        list.dedup();
    }
    (indegree, dependents)
}

fn collect_ready(indegree: &Map<ExecutionStepId, usize>) -> BTreeSet<ExecutionStepId> {
    indegree
        .iter()
        .filter_map(|(id, degree)| if *degree == 0 { Some(id.clone()) } else { None })
        .collect()
}

fn decrement_indegree_and_enqueue(
    indegree: &mut Map<ExecutionStepId, usize>,
    ready: &mut BTreeSet<ExecutionStepId>,
    dependent: &ExecutionStepId,
) {
    if let Some(entry) = indegree.get_mut(dependent) {
        *entry -= 1;
        if *entry == 0 {
            ready.insert(dependent.clone());
        }
    }
}

fn extract_cycle_path(plan: &ExecutionPlan) -> Option<Vec<ExecutionStepId>> {
    let specs_by_id: Map<ExecutionStepId, &ExecutionStepSpec> = plan
        .steps
        .iter()
        .map(|step| (step.step_id.clone(), step))
        .collect();

    let mut visit_state: Map<ExecutionStepId, u8> =
        specs_by_id.keys().cloned().map(|id| (id, 0)).collect();
    let mut stack: Vec<ExecutionStepId> = Vec::new();

    for step_id in specs_by_id.keys() {
        if *visit_state.get(step_id).unwrap_or(&0) == 0 {
            let mut ctx = DfsCycleCtx {
                specs_by_id: &specs_by_id,
                visit_state: &mut visit_state,
                stack: &mut stack,
            };
            if let Some(cycle) = dfs_cycle(step_id, &mut ctx) {
                return Some(cycle);
            }
        }
    }

    None
}

struct DfsCycleCtx<'a> {
    specs_by_id: &'a Map<ExecutionStepId, &'a ExecutionStepSpec>,
    visit_state: &'a mut Map<ExecutionStepId, u8>,
    stack: &'a mut Vec<ExecutionStepId>,
}

fn dfs_cycle(current: &ExecutionStepId, ctx: &mut DfsCycleCtx<'_>) -> Option<Vec<ExecutionStepId>> {
    ctx.visit_state.insert(current.clone(), 1);
    ctx.stack.push(current.clone());

    if let Some(step) = ctx.specs_by_id.get(current) {
        for dep in sorted_dependencies(step) {
            if let Some(cycle) = traverse_dependency_for_cycle(dep, ctx) {
                return Some(cycle);
            }
        }
    }

    ctx.stack.pop();
    ctx.visit_state.insert(current.clone(), 2);
    None
}

fn sorted_dependencies(step: &ExecutionStepSpec) -> Vec<ExecutionStepId> {
    let mut deps = step.depends_on.clone();
    deps.sort();
    deps
}

fn traverse_dependency_for_cycle(
    dep: ExecutionStepId,
    ctx: &mut DfsCycleCtx<'_>,
) -> Option<Vec<ExecutionStepId>> {
    match *ctx.visit_state.get(&dep).unwrap_or(&0) {
        1 => Some(cycle_from_back_edge(&dep, ctx)),
        0 => dfs_cycle(&dep, ctx),
        _ => None,
    }
}

fn cycle_from_back_edge(dep: &ExecutionStepId, ctx: &DfsCycleCtx<'_>) -> Vec<ExecutionStepId> {
    if let Some(pos) = ctx.stack.iter().position(|id| id == dep) {
        let mut cycle = ctx.stack[pos..].to_vec();
        cycle.push(dep.clone());
        return cycle;
    }
    Vec::from([dep.clone(), dep.clone()])
}
