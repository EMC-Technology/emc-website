use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use knowledge_core::model::{Community, Document, RecordIdType};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use crate::pipeline::ParseStage;

/// 确定性决胜容差（Deterministic Resolution Epsilon）
///
/// 当两个浮点数的差值小于此阈值时，视为相等。
/// 平局时使用社区 ID 的字典序作为确定性决胜规则，
/// 确保相同输入在不同平台/编译优化级别下产生相同结果。
const DETERMINISTIC_EPSILON: f64 = 1e-10;

/// Symbol graph for community detection
///
/// Represents a weighted undirected graph where nodes are symbol IDs
/// and edges represent weighted relationships between symbols.
pub struct SymbolGraph {
    nodes: Vec<String>,
    edges: Vec<(String, String, f64)>,
}

impl SymbolGraph {
    /// Creates an empty symbol graph
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Creates a symbol graph from a list of weighted edges
    ///
    /// Nodes are automatically extracted from edge endpoints and sorted
    /// for deterministic ordering.
    #[must_use]
    pub fn with_edges(edges: Vec<(String, String, f64)>) -> Self {
        let mut node_set = HashSet::new();
        for (from, to, _) in &edges {
            node_set.insert(from.clone());
            node_set.insert(to.clone());
        }
        let mut nodes: Vec<String> = node_set.into_iter().collect();
        nodes.sort();
        Self { nodes, edges }
    }

    /// Adds a node to the graph if it does not already exist
    pub fn add_node(&mut self, node_id: impl Into<String>) {
        let id = node_id.into();
        if !self.nodes.contains(&id) {
            self.nodes.push(id);
        }
    }

    /// Adds a weighted undirected edge between two nodes
    ///
    /// Both endpoints are automatically added as nodes if they do not exist.
    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>, weight: f64) {
        let from_id = from.into();
        let to_id = to.into();
        self.add_node(&from_id);
        self.add_node(&to_id);
        self.edges.push((from_id, to_id, weight));
    }
}

impl Default for SymbolGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolGraph {
    /// Returns a slice of all node IDs in the graph
    #[must_use]
    pub fn nodes(&self) -> &[String] {
        &self.nodes
    }

    /// Returns a slice of all edges in the graph
    #[must_use]
    pub fn edges(&self) -> &[(String, String, f64)] {
        &self.edges
    }
}

/// Leiden community detection algorithm
///
/// Implements the Leiden algorithm for detecting communities in symbol graphs.
/// Uses a seeded RNG (seed 42) for deterministic results.
///
/// The algorithm consists of three phases:
/// 1. **Local Moving**: Move nodes to neighboring communities that maximize modularity gain
/// 2. **Refinement**: Check if splitting communities improves modularity
/// 3. **Aggregation**: Aggregate communities into super-nodes and repeat
///
/// Modularity formula: `Q = (1/2m) * Σ[A_ij - γ·k_i·k_j/(2m)] · δ(c_i, c_j)`
pub struct CommunityDetector;

impl CommunityDetector {
    /// Detects communities in the given symbol graph using the Leiden algorithm
    ///
    /// Returns a list of communities sorted by community ID, each with a name,
    /// cohesion score, and member IDs. Results are deterministic for the same input graph.
    #[must_use]
    pub fn detect(graph: &SymbolGraph) -> Vec<Community> {
        if graph.nodes.is_empty() {
            return Vec::new();
        }

        let mut rng = StdRng::seed_from_u64(42);
        let n = graph.nodes.len();

        let idx_map: HashMap<String, usize> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i))
            .collect();

        let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for (from, to, w) in &graph.edges {
            if let (Some(&i), Some(&j)) = (idx_map.get(from), idx_map.get(to)) {
                adj[i].push((j, *w));
                adj[j].push((i, *w));
            }
        }

        let degree: Vec<f64> = (0..n)
            .map(|i| adj[i].iter().map(|(_, w)| w).sum())
            .collect();
        let total_weight: f64 = degree.iter().sum::<f64>() / 2.0;

        if total_weight < DETERMINISTIC_EPSILON {
            return graph
                .nodes
                .iter()
                .enumerate()
                .map(|(i, id)| Community {
                    id: None,
                    name: format!("community_{i}"),
                    cohesion_score: 0.0,
                    member_ids: vec![to_record_id(id)],
                })
                .collect();
        }

        let resolution = 1.0_f64;
        let final_community = Self::leiden(&adj, &degree, total_weight, resolution, &mut rng);

        let mut community_map: HashMap<usize, Vec<usize>> = HashMap::new();
        for (node_idx, &comm) in final_community.iter().enumerate() {
            community_map.entry(comm).or_default().push(node_idx);
        }

        let mut sorted_comms: Vec<usize> = community_map.keys().copied().collect();
        sorted_comms.sort_unstable();

        let mut result = Vec::new();
        for (idx, &comm_id) in sorted_comms.iter().enumerate() {
            let member_indices = &community_map[&comm_id];
            let member_ids: Vec<RecordIdType> = member_indices
                .iter()
                .map(|&i| to_record_id(&graph.nodes[i]))
                .collect();
            let cohesion = Self::compute_cohesion(&adj, member_indices);
            result.push(Community {
                id: None,
                name: format!("community_{idx}"),
                cohesion_score: cohesion,
                member_ids,
            });
        }

        result
    }

    fn leiden(
        adj: &[Vec<(usize, f64)>],
        degree: &[f64],
        total_weight: f64,
        resolution: f64,
        rng: &mut StdRng,
    ) -> Vec<usize> {
        let n = adj.len();
        if n <= 1 {
            return (0..n).collect();
        }

        let mut community: Vec<usize> = (0..n).collect();
        let mut community_nodes: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
        let mut community_total_degree: Vec<f64> = degree.to_vec();

        Self::local_moving(
            adj,
            degree,
            total_weight,
            resolution,
            rng,
            &mut community,
            &mut community_nodes,
            &mut community_total_degree,
        );

        let refined = Self::refinement_phase(
            adj,
            degree,
            total_weight,
            resolution,
            rng,
            &community,
            &community_nodes,
        );

        let mut ref_map: HashMap<usize, usize> = HashMap::new();
        let mut counter = 0;
        let mut refined_remapped = vec![0; n];
        for i in 0..n {
            let r = refined[i];
            let comm = *ref_map.entry(r).or_insert_with(|| {
                let c = counter;
                counter += 1;
                c
            });
            refined_remapped[i] = comm;
        }

        if counter == n {
            return community;
        }

        if counter <= 1 {
            return refined_remapped;
        }

        let num_super = counter;
        let mut super_adj_maps: Vec<HashMap<usize, f64>> = vec![HashMap::new(); num_super];

        for i in 0..n {
            let ci = refined_remapped[i];
            for &(j, w) in &adj[i] {
                let cj = refined_remapped[j];
                *super_adj_maps[ci].entry(cj).or_insert(0.0) += w;
            }
        }

        let super_adj: Vec<Vec<(usize, f64)>> = super_adj_maps
            .into_iter()
            .map(|hm| hm.into_iter().collect())
            .collect();

        let super_degree: Vec<f64> = super_adj
            .iter()
            .map(|neighbors| neighbors.iter().map(|(_, w)| w).sum())
            .collect();

        let super_total_weight: f64 = super_degree.iter().sum::<f64>() / 2.0;

        if super_total_weight < DETERMINISTIC_EPSILON {
            return refined_remapped;
        }

        let super_community = Self::leiden(
            &super_adj,
            &super_degree,
            super_total_weight,
            resolution,
            rng,
        );

        let mut result = vec![0; n];
        for i in 0..n {
            result[i] = super_community[refined_remapped[i]];
        }

        result
    }

    #[allow(clippy::too_many_arguments)]
    fn local_moving(
        adj: &[Vec<(usize, f64)>],
        degree: &[f64],
        total_weight: f64,
        resolution: f64,
        rng: &mut StdRng,
        community: &mut [usize],
        community_nodes: &mut [Vec<usize>],
        community_total_degree: &mut [f64],
    ) {
        const MAX_ITERATIONS: u32 = 100;
        let n = adj.len();
        let mut improved = true;
        let mut iterations: u32 = 0;
        while improved && iterations < MAX_ITERATIONS {
            improved = false;
            iterations += 1;
            let mut order: Vec<usize> = (0..n).collect();
            order.shuffle(rng);
            for &node in &order {
                let current_comm = community[node];
                let k_i = degree[node];

                let k_i_in_current: f64 = adj[node]
                    .iter()
                    .filter(|(nb, _)| community[*nb] == current_comm && *nb != node)
                    .map(|(_, w)| w)
                    .sum();

                let mut best_comm = current_comm;
                let mut best_delta = 0.0_f64;

                let mut neighbor_comms: Vec<usize> = adj[node]
                    .iter()
                    .map(|(nb, _)| community[*nb])
                    .filter(|c| *c != current_comm)
                    .collect();
                neighbor_comms.sort_unstable();
                neighbor_comms.dedup();

                for &target_comm in &neighbor_comms {
                    let k_i_in_target: f64 = adj[node]
                        .iter()
                        .filter(|(nb, _)| community[*nb] == target_comm)
                        .map(|(_, w)| w)
                        .sum();

                    let delta = Self::modularity_delta(
                        k_i_in_target,
                        k_i_in_current,
                        k_i,
                        community_total_degree[target_comm],
                        community_total_degree[current_comm],
                        total_weight,
                        resolution,
                    );

                    let is_better_delta = delta > best_delta + DETERMINISTIC_EPSILON;
                    let is_tie_with_smaller_comm = (delta - best_delta).abs() < DETERMINISTIC_EPSILON
                        && target_comm < best_comm;

                    if is_better_delta || is_tie_with_smaller_comm {
                        best_delta = delta;
                        best_comm = target_comm;
                    }
                }

                if best_comm != current_comm {
                    community[node] = best_comm;
                    community_nodes[current_comm].retain(|&x| x != node);
                    community_nodes[best_comm].push(node);
                    community_total_degree[current_comm] -= k_i;
                    community_total_degree[best_comm] += k_i;
                    improved = true;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn refinement_phase(
        adj: &[Vec<(usize, f64)>],
        degree: &[f64],
        total_weight: f64,
        resolution: f64,
        rng: &mut StdRng,
        community: &[usize],
        community_nodes: &[Vec<usize>],
    ) -> Vec<usize> {
        let n = adj.len();
        let mut refined: Vec<usize> = (0..n).collect();
        let mut refined_nodes: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
        let mut refined_total_degree: Vec<f64> = degree.to_vec();

        let mut active_comms: Vec<usize> = community
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        active_comms.sort_unstable();

        for &comm in &active_comms {
            let members = &community_nodes[comm];
            if members.len() <= 1 {
                continue;
            }

            let mut order = members.clone();
            order.shuffle(rng);

            for &node in &order {
                let current_ref = refined[node];
                let k_i = degree[node];

                let k_i_in_current: f64 = adj[node]
                    .iter()
                    .filter(|(nb, _)| refined[*nb] == current_ref && *nb != node)
                    .map(|(_, w)| w)
                    .sum();

                let mut best_ref = current_ref;
                let mut best_delta = 0.0_f64;

                let mut neighbor_refs: Vec<usize> = adj[node]
                    .iter()
                    .filter(|(nb, _)| community[*nb] == comm)
                    .map(|(nb, _)| refined[*nb])
                    .filter(|c| *c != current_ref)
                    .collect();
                neighbor_refs.sort_unstable();
                neighbor_refs.dedup();

                for &target_ref in &neighbor_refs {
                    let k_i_in_target: f64 = adj[node]
                        .iter()
                        .filter(|(nb, _)| community[*nb] == comm && refined[*nb] == target_ref)
                        .map(|(_, w)| w)
                        .sum();

                    let delta = Self::modularity_delta(
                        k_i_in_target,
                        k_i_in_current,
                        k_i,
                        refined_total_degree[target_ref],
                        refined_total_degree[current_ref],
                        total_weight,
                        resolution,
                    );

                    if delta > best_delta {
                        best_delta = delta;
                        best_ref = target_ref;
                    }
                }

                if best_ref != current_ref {
                    refined[node] = best_ref;
                    refined_nodes[current_ref].retain(|&x| x != node);
                    refined_nodes[best_ref].push(node);
                    refined_total_degree[current_ref] -= k_i;
                    refined_total_degree[best_ref] += k_i;
                }
            }
        }

        refined
    }

    fn modularity_delta(
        k_i_in_target: f64,
        k_i_in_current: f64,
        k_i: f64,
        sigma_tot_target: f64,
        sigma_tot_current: f64,
        total_weight: f64,
        resolution: f64,
    ) -> f64 {
        let m2 = 2.0 * total_weight;
        let gain = k_i_in_target - resolution * sigma_tot_target * k_i / m2;
        let loss = k_i_in_current - resolution * (sigma_tot_current - k_i) * k_i / m2;
        gain - loss
    }

    fn compute_cohesion(adj: &[Vec<(usize, f64)>], member_indices: &[usize]) -> f64 {
        if member_indices.len() <= 1 {
            return 0.0;
        }

        let member_set: HashSet<usize> = member_indices.iter().copied().collect();
        let mut internal = 0.0_f64;
        let mut total = 0.0_f64;

        for &i in member_indices {
            for &(j, w) in &adj[i] {
                total += w;
                if member_set.contains(&j) {
                    internal += w;
                }
            }
        }

        if total < DETERMINISTIC_EPSILON { 0.0 } else { internal / total }
    }
}

/// Pipeline stage for community detection
///
/// This stage passes documents through unchanged. Actual community detection
/// is performed by `CommunityDetector::detect` on the symbol graph
/// after parsing is complete.
pub struct ProcessCommunitiesStage;

#[cfg(feature = "db")]
fn to_record_id(id: &str) -> RecordIdType {
    surrealdb::sql::Thing::from((String::from("symbol"), id.to_string()))
}

#[cfg(not(feature = "db"))]
fn to_record_id(id: &str) -> RecordIdType {
    id.to_string()
}

#[async_trait]
impl ParseStage for ProcessCommunitiesStage {
    fn name(&self) -> &'static str {
        "process_communities"
    }

    async fn execute(&self, input: Vec<Document>) -> crate::Result<Vec<Document>> {
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_community_detector_single_node() {
        let mut graph = SymbolGraph::new();
        graph.add_node("node_a");
        let communities = CommunityDetector::detect(&graph);
        assert_eq!(communities.len(), 1);
        assert_eq!(communities[0].member_ids, vec![to_record_id("node_a")]);
    }

    #[test]
    fn test_community_detector_two_nodes_no_edge() {
        let mut graph = SymbolGraph::new();
        graph.add_node("node_a");
        graph.add_node("node_b");
        let communities = CommunityDetector::detect(&graph);
        assert_eq!(communities.len(), 2);
    }

    #[test]
    fn test_community_detector_two_nodes_with_edge() {
        let graph =
            SymbolGraph::with_edges(vec![("node_a".to_string(), "node_b".to_string(), 1.0)]);
        let communities = CommunityDetector::detect(&graph);
        assert_eq!(communities.len(), 1);
        assert_eq!(communities[0].member_ids.len(), 2);
    }

    #[test]
    fn test_community_detector_deterministic() {
        let graph = SymbolGraph::with_edges(vec![
            ("a".to_string(), "b".to_string(), 1.0),
            ("b".to_string(), "c".to_string(), 1.0),
            ("c".to_string(), "d".to_string(), 0.5),
            ("d".to_string(), "e".to_string(), 1.0),
        ]);
        let result1 = CommunityDetector::detect(&graph);
        let result2 = CommunityDetector::detect(&graph);
        assert_eq!(result1.len(), result2.len());
        for (c1, c2) in result1.iter().zip(result2.iter()) {
            assert_eq!(c1.name, c2.name);
            assert_eq!(c1.member_ids, c2.member_ids);
            assert!((c1.cohesion_score - c2.cohesion_score).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_community_detector_modularity_gain() {
        let graph = SymbolGraph::with_edges(vec![
            ("a".to_string(), "b".to_string(), 2.0),
            ("b".to_string(), "c".to_string(), 2.0),
            ("d".to_string(), "e".to_string(), 2.0),
            ("e".to_string(), "f".to_string(), 2.0),
        ]);
        let communities = CommunityDetector::detect(&graph);

        let mut a_comm_members: Option<&Vec<RecordIdType>> = None;
        let mut d_comm_members: Option<&Vec<RecordIdType>> = None;
        for comm in &communities {
            if comm.member_ids.contains(&to_record_id("a")) {
                a_comm_members = Some(&comm.member_ids);
            }
            if comm.member_ids.contains(&to_record_id("d")) {
                d_comm_members = Some(&comm.member_ids);
            }
        }

        let a_members = a_comm_members.expect("a should be in a community");
        let d_members = d_comm_members.expect("d should be in a community");

        assert_ne!(
            a_members, d_members,
            "a and d should be in different communities"
        );

        assert!(
            a_members.contains(&to_record_id("b")),
            "a and b should be in the same community"
        );
        assert!(
            a_members.contains(&to_record_id("c")),
            "a and c should be in the same community"
        );
        assert!(
            d_members.contains(&to_record_id("e")),
            "d and e should be in the same community"
        );
        assert!(
            d_members.contains(&to_record_id("f")),
            "d and f should be in the same community"
        );
    }
}
