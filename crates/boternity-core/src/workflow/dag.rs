//! DAG builder, cycle detection, and parallel wave computation.
//!
//! Uses `petgraph` to model step dependencies as a directed graph. Topological
//! sort detects cycles, and depth-based grouping produces parallel execution
//! waves where all steps in a wave can run concurrently.

use std::collections::HashMap;

use boternity_types::workflow::StepDefinition;
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;

use super::definition::WorkflowError;

// ---------------------------------------------------------------------------
// Execution plan (wave computation)
// ---------------------------------------------------------------------------

/// Build an execution plan from workflow steps, grouping them into parallel waves.
///
/// Each wave contains steps that can execute concurrently because all their
/// dependencies are satisfied by prior waves. The algorithm:
///
/// 1. Build a `DiGraph` with step IDs as nodes and `depends_on` edges.
/// 2. Run `petgraph::algo::toposort` to verify acyclicity.
/// 3. Compute each node's depth (max dependency depth + 1).
/// 4. Group steps by depth into waves.
///
/// Returns `Vec<Vec<&StepDefinition>>` where index 0 is the first wave to execute.
pub fn build_execution_plan<'a>(
    steps: &'a [StepDefinition],
) -> Result<Vec<Vec<&'a StepDefinition>>, WorkflowError> {
    if steps.is_empty() {
        return Ok(vec![]);
    }

    // Map step IDs to indices for petgraph
    let id_to_step: HashMap<&str, &StepDefinition> =
        steps.iter().map(|s| (s.id.as_str(), s)).collect();
    let id_to_idx: HashMap<&str, usize> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();

    // Build directed graph: edge from dependency -> dependent
    let mut graph = DiGraph::<&str, ()>::new();
    let node_indices: Vec<_> = steps.iter().map(|s| graph.add_node(s.id.as_str())).collect();

    for step in steps {
        let to_idx = id_to_idx[step.id.as_str()];
        for dep in &step.depends_on {
            let from_idx = id_to_idx.get(dep.as_str()).ok_or_else(|| {
                WorkflowError::UnknownDependency(format!(
                    "step '{}' depends on unknown step '{}'",
                    step.id, dep
                ))
            })?;
            graph.add_edge(node_indices[*from_idx], node_indices[to_idx], ());
        }
    }

    // Topological sort -- detects cycles
    let sorted = toposort(&graph, None).map_err(|cycle| {
        let node_id = graph[cycle.node_id()];
        WorkflowError::CycleDetected(format!("cycle detected involving step '{}'", node_id))
    })?;

    // Compute depth for each node: root nodes have depth 0
    let mut depths: HashMap<&str, usize> = HashMap::new();
    for &node_idx in &sorted {
        let step_id = graph[node_idx];
        let step = id_to_step[step_id];
        let depth = if step.depends_on.is_empty() {
            0
        } else {
            step.depends_on
                .iter()
                .map(|dep| depths.get(dep.as_str()).copied().unwrap_or(0) + 1)
                .max()
                .unwrap_or(0)
        };
        depths.insert(step_id, depth);
    }

    // Group by depth into waves
    let max_depth = depths.values().copied().max().unwrap_or(0);
    let mut waves: Vec<Vec<&StepDefinition>> = vec![vec![]; max_depth + 1];
    for step in steps {
        let depth = depths[step.id.as_str()];
        waves[depth].push(step);
    }

    Ok(waves)
}

// ---------------------------------------------------------------------------
// DAG validation (lighter weight, no wave computation)
// ---------------------------------------------------------------------------

/// Validate that steps form a valid DAG (no cycles, all references exist).
pub fn validate_dag(steps: &[StepDefinition]) -> Result<(), WorkflowError> {
    let id_to_idx: HashMap<&str, usize> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();

    let mut graph = DiGraph::<&str, ()>::new();
    let node_indices: Vec<_> = steps.iter().map(|s| graph.add_node(s.id.as_str())).collect();

    for step in steps {
        let to_idx = id_to_idx[step.id.as_str()];
        for dep in &step.depends_on {
            let from_idx = id_to_idx.get(dep.as_str()).ok_or_else(|| {
                WorkflowError::UnknownDependency(format!(
                    "step '{}' depends on unknown step '{}'",
                    step.id, dep
                ))
            })?;
            graph.add_edge(node_indices[*from_idx], node_indices[to_idx], ());
        }
    }

    toposort(&graph, None).map_err(|cycle| {
        let node_id = graph[cycle.node_id()];
        WorkflowError::CycleDetected(format!("cycle detected involving step '{}'", node_id))
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Transitive dependency closure
// ---------------------------------------------------------------------------

/// Returns the transitive closure of all dependencies for a given step.
///
/// If `step_id` is not found, returns an empty vec.
pub fn get_step_dependencies<'a>(step_id: &str, steps: &'a [StepDefinition]) -> Vec<&'a str> {
    let step_map: HashMap<&str, &StepDefinition> =
        steps.iter().map(|s| (s.id.as_str(), s)).collect();

    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![step_id];

    while let Some(current) = stack.pop() {
        if let Some(step) = step_map.get(current) {
            for dep in &step.depends_on {
                if visited.insert(dep.as_str()) {
                    stack.push(dep.as_str());
                }
            }
        }
    }

    visited.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::workflow::{StepConfig, StepType};

    /// Helper: build a simple agent step with given ID and dependencies.
    fn agent_step(id: &str, depends_on: Vec<&str>) -> StepDefinition {
        StepDefinition {
            id: id.to_string(),
            name: id.to_string(),
            step_type: StepType::Agent,
            depends_on: depends_on.into_iter().map(String::from).collect(),
            condition: None,
            timeout_secs: None,
            retry: None,
            config: StepConfig::Agent {
                bot: "test-bot".to_string(),
                prompt: "do something".to_string(),
                model: None,
            },
            ui: None,
        }
    }

    // -----------------------------------------------------------------------
    // Wave computation
    // -----------------------------------------------------------------------

    #[test]
    fn test_no_dependencies_single_wave() {
        let steps = vec![
            agent_step("a", vec![]),
            agent_step("b", vec![]),
            agent_step("c", vec![]),
        ];
        let waves = build_execution_plan(&steps).unwrap();
        assert_eq!(waves.len(), 1, "all independent steps -> single wave");
        assert_eq!(waves[0].len(), 3);
    }

    #[test]
    fn test_linear_chain_n_waves() {
        // A -> B -> C
        let steps = vec![
            agent_step("a", vec![]),
            agent_step("b", vec!["a"]),
            agent_step("c", vec!["b"]),
        ];
        let waves = build_execution_plan(&steps).unwrap();
        assert_eq!(waves.len(), 3, "linear chain -> 3 waves");
        assert_eq!(waves[0].len(), 1);
        assert_eq!(waves[0][0].id, "a");
        assert_eq!(waves[1].len(), 1);
        assert_eq!(waves[1][0].id, "b");
        assert_eq!(waves[2].len(), 1);
        assert_eq!(waves[2][0].id, "c");
    }

    #[test]
    fn test_diamond_three_waves() {
        // A -> {B, C} -> D
        let steps = vec![
            agent_step("a", vec![]),
            agent_step("b", vec!["a"]),
            agent_step("c", vec!["a"]),
            agent_step("d", vec!["b", "c"]),
        ];
        let waves = build_execution_plan(&steps).unwrap();
        assert_eq!(waves.len(), 3, "diamond -> 3 waves");
        assert_eq!(waves[0].len(), 1);
        assert_eq!(waves[0][0].id, "a");
        assert_eq!(waves[1].len(), 2, "B and C should be in same wave");
        let wave1_ids: Vec<&str> = waves[1].iter().map(|s| s.id.as_str()).collect();
        assert!(wave1_ids.contains(&"b"));
        assert!(wave1_ids.contains(&"c"));
        assert_eq!(waves[2].len(), 1);
        assert_eq!(waves[2][0].id, "d");
    }

    #[test]
    fn test_cycle_detected() {
        // A -> B -> A (cycle)
        let steps = vec![
            agent_step("a", vec!["b"]),
            agent_step("b", vec!["a"]),
        ];
        let err = build_execution_plan(&steps).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cycle detected"), "got: {msg}");
    }

    #[test]
    fn test_empty_steps() {
        let waves = build_execution_plan(&[]).unwrap();
        assert!(waves.is_empty());
    }

    // -----------------------------------------------------------------------
    // DAG validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_dag_valid() {
        let steps = vec![
            agent_step("a", vec![]),
            agent_step("b", vec!["a"]),
        ];
        assert!(validate_dag(&steps).is_ok());
    }

    #[test]
    fn test_validate_dag_cycle() {
        let steps = vec![
            agent_step("a", vec!["c"]),
            agent_step("b", vec!["a"]),
            agent_step("c", vec!["b"]),
        ];
        let err = validate_dag(&steps).unwrap_err();
        assert!(err.to_string().contains("cycle detected"));
    }

    #[test]
    fn test_validate_dag_unknown_dep() {
        let steps = vec![agent_step("a", vec!["missing"])];
        let err = validate_dag(&steps).unwrap_err();
        assert!(err.to_string().contains("unknown step"));
    }

    // -----------------------------------------------------------------------
    // Transitive dependencies
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_transitive_dependencies() {
        // A -> B -> C -> D
        let steps = vec![
            agent_step("a", vec![]),
            agent_step("b", vec!["a"]),
            agent_step("c", vec!["b"]),
            agent_step("d", vec!["c"]),
        ];
        let mut deps = get_step_dependencies("d", &steps);
        deps.sort();
        assert_eq!(deps, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_get_dependencies_root_node() {
        let steps = vec![agent_step("a", vec![])];
        let deps = get_step_dependencies("a", &steps);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_get_dependencies_unknown_step() {
        let steps = vec![agent_step("a", vec![])];
        let deps = get_step_dependencies("nonexistent", &steps);
        assert!(deps.is_empty());
    }

    // -----------------------------------------------------------------------
    // Complex DAG: fork-join with multiple paths
    // -----------------------------------------------------------------------

    #[test]
    fn test_complex_fork_join() {
        //     A
        //    / \
        //   B   C
        //   |   |
        //   D   E
        //    \ /
        //     F
        let steps = vec![
            agent_step("a", vec![]),
            agent_step("b", vec!["a"]),
            agent_step("c", vec!["a"]),
            agent_step("d", vec!["b"]),
            agent_step("e", vec!["c"]),
            agent_step("f", vec!["d", "e"]),
        ];
        let waves = build_execution_plan(&steps).unwrap();
        assert_eq!(waves.len(), 4);
        // Wave 0: [A]
        assert_eq!(waves[0].len(), 1);
        // Wave 1: [B, C]
        assert_eq!(waves[1].len(), 2);
        // Wave 2: [D, E]
        assert_eq!(waves[2].len(), 2);
        // Wave 3: [F]
        assert_eq!(waves[3].len(), 1);
        assert_eq!(waves[3][0].id, "f");
    }
}
