use std::collections::{HashMap, HashSet, VecDeque};

use async_trait::async_trait;
use knowledge_core::model::{Document, ProcessStep};

use crate::community_detector::SymbolGraph;
use crate::pipeline::ParseStage;

/// Execution flow tracer
///
/// Detects entry points in a symbol graph and traces execution flows
/// using BFS traversal. Entry points are symbols with `out_degree` > 2
/// and `in_degree` <= 1. Confidence decays by 0.1 per hop (min 0.3).
pub struct ExecutionFlowTracer;

impl ExecutionFlowTracer {
    /// Detects entry points in the given symbol graph
    ///
    /// An entry point is a symbol where `out_degree` > 2 AND `in_degree` <= 1.
    /// Degrees are computed from directed edges (from → to).
    ///
    /// # Arguments
    ///
    /// * `graph` - The symbol graph to analyze
    ///
    /// # Returns
    ///
    /// A sorted list of symbol IDs that satisfy the entry point condition
    #[must_use]
    pub fn detect_entry_points(graph: &SymbolGraph) -> Vec<String> {
        let mut out_degree: HashMap<&str, usize> = HashMap::new();
        let mut in_degree: HashMap<&str, usize> = HashMap::new();

        for (from, to, _) in graph.edges() {
            *out_degree.entry(from.as_str()).or_insert(0) += 1;
            *in_degree.entry(to.as_str()).or_insert(0) += 1;
        }

        let mut entry_points: Vec<String> = graph
            .nodes()
            .iter()
            .filter(|node| {
                let out = *out_degree.get(node.as_str()).unwrap_or(&0);
                let inc = *in_degree.get(node.as_str()).unwrap_or(&0);
                out > 2 && inc <= 1
            })
            .cloned()
            .collect();

        entry_points.sort();
        entry_points
    }

    /// Traces execution flow from a given entry point using BFS
    ///
    /// Follows directed edges (from → to) starting from `entry_point`.
    /// Each visited node becomes a `ProcessStep` with:
    /// - `symbol_id`: the visited node's ID
    /// - `step_order`: sequential visitation order (0-based)
    /// - `confidence`: starts at 1.0, decays by 0.1 per hop, minimum 0.3
    ///
    /// Traversal stops when `max_depth` is reached or no more edges are found.
    ///
    /// # Arguments
    ///
    /// * `entry_point` - The starting symbol ID for the trace
    /// * `graph` - The symbol graph to traverse
    /// * `max_depth` - Maximum traversal depth from the entry point
    ///
    /// # Returns
    ///
    /// A list of `ProcessStep` instances ordered by `step_order`
    #[must_use]
    pub fn trace_execution_flow(
        entry_point: &str,
        graph: &SymbolGraph,
        max_depth: u32,
    ) -> Vec<ProcessStep> {
        let process_id = make_process_id(entry_point);

        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
        for (from, to, _) in graph.edges() {
            adjacency
                .entry(from.as_str())
                .or_default()
                .push(to.as_str());
        }

        let mut visited: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<(&str, u32)> = VecDeque::new();
        queue.push_back((entry_point, 0));
        visited.insert(entry_point);

        let mut steps: Vec<ProcessStep> = Vec::new();
        let mut step_order: u32 = 0;

        while let Some((node, depth)) = queue.pop_front() {
            if depth > max_depth {
                continue;
            }

            let confidence = f64::from(depth).mul_add(-0.1, 1.0).max(0.3);

            steps.push(ProcessStep {
                id: None,
                process_id: process_id.clone(),
                symbol_id: make_symbol_id(node),
                step_order,
                confidence,
            });
            step_order += 1;

            if depth < max_depth {
                if let Some(neighbors) = adjacency.get(node) {
                    for &neighbor in neighbors {
                        if visited.insert(neighbor) {
                            queue.push_back((neighbor, depth + 1));
                        }
                    }
                }
            }
        }

        steps
    }
}

#[cfg(feature = "db")]
fn make_process_id(entry_point: &str) -> knowledge_core::model::RecordIdType {
    surrealdb::sql::Thing::from((String::from("process"), entry_point.to_string()))
}

#[cfg(not(feature = "db"))]
fn make_process_id(entry_point: &str) -> knowledge_core::model::RecordIdType {
    format!("process:{entry_point}")
}

#[cfg(feature = "db")]
fn make_symbol_id(node: &str) -> knowledge_core::model::RecordIdType {
    surrealdb::sql::Thing::from((String::from("symbol"), node.to_string()))
}

#[cfg(not(feature = "db"))]
fn make_symbol_id(node: &str) -> knowledge_core::model::RecordIdType {
    format!("symbol:{node}")
}

/// Pipeline stage for execution flow tracing
///
/// This stage passes documents through unchanged. Actual execution flow
/// tracing is performed by `ExecutionFlowTracer::detect_entry_points`
/// and `ExecutionFlowTracer::trace_execution_flow` after parsing is complete.
pub struct ProcessExecutionFlowsStage;

#[async_trait]
impl ParseStage for ProcessExecutionFlowsStage {
    fn name(&self) -> &'static str {
        "process_execution_flows"
    }

    async fn execute(&self, input: Vec<Document>) -> crate::Result<Vec<Document>> {
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knowledge_core::model::RecordIdType;

    fn make_graph_with_entry_point() -> SymbolGraph {
        let mut graph = SymbolGraph::new();
        graph.add_edge("A", "B", 1.0);
        graph.add_edge("A", "C", 1.0);
        graph.add_edge("A", "D", 1.0);
        graph.add_edge("B", "C", 0.5);
        graph
    }

    fn make_graph_no_entry() -> SymbolGraph {
        let mut graph = SymbolGraph::new();
        graph.add_edge("X", "A", 1.0);
        graph.add_edge("Y", "A", 1.0);
        graph.add_edge("X", "B", 1.0);
        graph.add_edge("Y", "B", 1.0);
        graph.add_edge("A", "X", 0.5);
        graph.add_edge("B", "X", 0.5);
        graph.add_edge("A", "Y", 0.5);
        graph.add_edge("B", "Y", 0.5);
        graph
    }

    fn make_linear_chain_graph() -> SymbolGraph {
        let mut graph = SymbolGraph::new();
        graph.add_edge("A", "B", 1.0);
        graph.add_edge("B", "C", 1.0);
        graph.add_edge("C", "D", 1.0);
        graph
    }

    fn make_branching_graph() -> SymbolGraph {
        let mut graph = SymbolGraph::new();
        graph.add_edge("A", "B", 1.0);
        graph.add_edge("A", "C", 1.0);
        graph.add_edge("A", "D", 1.0);
        graph.add_edge("B", "E", 1.0);
        graph
    }

    fn make_deep_chain_graph() -> SymbolGraph {
        let mut graph = SymbolGraph::new();
        graph.add_edge("A", "B", 1.0);
        graph.add_edge("B", "C", 1.0);
        graph.add_edge("C", "D", 1.0);
        graph.add_edge("D", "E", 1.0);
        graph
    }

    #[test]
    fn test_detect_entry_points_single_entry() {
        let graph = make_graph_with_entry_point();
        let entry_points = ExecutionFlowTracer::detect_entry_points(&graph);

        assert_eq!(entry_points, vec!["A"]);
    }

    #[test]
    fn test_detect_entry_points_no_entry() {
        let graph = make_graph_no_entry();
        let entry_points = ExecutionFlowTracer::detect_entry_points(&graph);

        assert!(entry_points.is_empty());
    }

    #[test]
    fn test_trace_execution_flow_linear_chain() {
        let graph = make_linear_chain_graph();
        let steps = ExecutionFlowTracer::trace_execution_flow("A", &graph, 10);

        assert_eq!(steps.len(), 4);
        assert_eq!(steps[0].symbol_id, make_symbol_id("A"));
        assert_eq!(steps[0].step_order, 0);
        assert!((steps[0].confidence - 1.0).abs() < f64::EPSILON);

        assert_eq!(steps[1].symbol_id, make_symbol_id("B"));
        assert_eq!(steps[1].step_order, 1);
        assert!((steps[1].confidence - 0.9).abs() < f64::EPSILON);

        assert_eq!(steps[2].symbol_id, make_symbol_id("C"));
        assert_eq!(steps[2].step_order, 2);
        assert!((steps[2].confidence - 0.8).abs() < f64::EPSILON);

        assert_eq!(steps[3].symbol_id, make_symbol_id("D"));
        assert_eq!(steps[3].step_order, 3);
        assert!((steps[3].confidence - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_trace_execution_flow_branching() {
        let graph = make_branching_graph();
        let steps = ExecutionFlowTracer::trace_execution_flow("A", &graph, 10);

        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0].symbol_id, make_symbol_id("A"));
        assert_eq!(steps[0].step_order, 0);
        assert!((steps[0].confidence - 1.0).abs() < f64::EPSILON);

        let depth1_symbols: Vec<RecordIdType> =
            steps[1..4].iter().map(|s| s.symbol_id.clone()).collect();
        assert!(depth1_symbols.contains(&make_symbol_id("B")));
        assert!(depth1_symbols.contains(&make_symbol_id("C")));
        assert!(depth1_symbols.contains(&make_symbol_id("D")));
        for step in &steps[1..4] {
            assert!((step.confidence - 0.9).abs() < f64::EPSILON);
        }

        assert_eq!(steps[4].symbol_id, make_symbol_id("E"));
        assert_eq!(steps[4].step_order, 4);
        assert!((steps[4].confidence - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_trace_execution_flow_max_depth() {
        let graph = make_deep_chain_graph();
        let steps = ExecutionFlowTracer::trace_execution_flow("A", &graph, 2);

        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].symbol_id, make_symbol_id("A"));
        assert_eq!(steps[0].step_order, 0);
        assert!((steps[0].confidence - 1.0).abs() < f64::EPSILON);

        assert_eq!(steps[1].symbol_id, make_symbol_id("B"));
        assert_eq!(steps[1].step_order, 1);
        assert!((steps[1].confidence - 0.9).abs() < f64::EPSILON);

        assert_eq!(steps[2].symbol_id, make_symbol_id("C"));
        assert_eq!(steps[2].step_order, 2);
        assert!((steps[2].confidence - 0.8).abs() < f64::EPSILON);
    }
}
