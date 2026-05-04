use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use knowledge_core::model::{Community, Document, RecordIdType};

use crate::pipeline::ParseStage;

/// 确定性决胜容差（Deterministic Resolution Epsilon）
///
/// 当两个浮点数的差值小于此阈值时，视为相等。
/// 平局时使用社区 ID 的字典序作为确定性决胜规则，
/// 确保相同输入在不同平台/编译优化级别下产生相同结果。
const DETERMINISTIC_EPSILON: f64 = 1e-10;

/// 符号图，用于社区检测
///
/// 表示一个加权无向图，节点为符号 ID，边为符号间的加权关系。
/// 节点按字典序排列以保证确定性。
pub struct SymbolGraph {
    nodes: Vec<String>,
    node_set: std::collections::HashSet<String>,
    edges: Vec<(String, String, f64)>,
}

impl SymbolGraph {
    /// 创建空符号图
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            node_set: std::collections::HashSet::new(),
            edges: Vec::new(),
        }
    }

    /// 从边列表创建符号图
    ///
    /// 节点自动从边端点提取并按字典序排列，保证确定性。
    #[must_use]
    pub fn with_edges(edges: Vec<(String, String, f64)>) -> Self {
        let mut node_set = HashSet::new();
        for (from, to, _) in &edges {
            node_set.insert(from.clone());
            node_set.insert(to.clone());
        }
        let mut nodes: Vec<String> = node_set.into_iter().collect();
        nodes.sort();
        Self {
            node_set: nodes.iter().cloned().collect(),
            nodes,
            edges,
        }
    }

    /// 添加节点（若不存在）
    pub fn add_node(&mut self, node_id: impl Into<String>) {
        let id = node_id.into();
        if self.node_set.insert(id.clone()) {
            self.nodes.push(id);
        }
    }

    /// 添加加权无向边
    ///
    /// 两端点若不存在则自动添加。
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
    /// 返回所有节点 ID 的切片
    #[must_use]
    pub fn nodes(&self) -> &[String] {
        &self.nodes
    }

    /// 返回所有边的切片
    #[must_use]
    pub fn edges(&self) -> &[(String, String, f64)] {
        &self.edges
    }
}

/// Leiden 社区检测算法
///
/// 实现确定性 Leiden 算法，检测符号图中的社区结构。
/// 完全不依赖随机数生成器，符合本项目"0 随机性"设计哲学。
///
/// 算法三阶段：
/// 1. **局部移动**：将节点移至使模块度增益最大的邻居社区
/// 2. **细化**：检查分裂社区是否改善模块度
/// 3. **聚合**：将社区聚合为超节点并递归执行
///
/// 模块度公式：`Q = (1/2m) * Σ[A_ij - γ·k_i·k_j/(2m)] · δ(c_i, c_j)`
///
/// # 确定性保证
///
/// 传统 Leiden 实现使用随机 shuffle 打破节点遍历顺序的偏差。
/// 本实现使用**确定性旋转**（deterministic rotation）替代：
/// 每次迭代将节点处理顺序旋转 `iteration` 个位置。
/// 这既避免了固定顺序的偏差，又保证了字节级确定性。
pub struct CommunityDetector;

/// 对切片执行确定性旋转
///
/// 将切片元素向左旋转 `shift` 个位置。
/// 例如：`[0, 1, 2, 3, 4]` 旋转 2 位得到 `[2, 3, 4, 0, 1]`。
/// 此操作是确定性的：相同输入永远产生相同输出。
fn deterministic_rotate<T>(slice: &mut [T], shift: usize) {
    if slice.is_empty() {
        return;
    }
    let len = slice.len();
    let shift = shift % len;
    if shift == 0 {
        return;
    }
    slice.rotate_left(shift);
}

impl CommunityDetector {
    /// 使用 Leiden 算法检测社区
    ///
    /// 返回按社区 ID 排序的社区列表，每个社区包含名称、凝聚分数和成员 ID。
    /// 结果对相同输入图具有确定性。
    #[must_use]
    pub fn detect(graph: &SymbolGraph) -> Vec<Community> {
        if graph.nodes.is_empty() {
            return Vec::new();
        }

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
        let final_community = Self::leiden(&adj, &degree, total_weight, resolution);

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
            &mut community,
            &mut community_nodes,
            &mut community_total_degree,
        );

        let refined = Self::refinement_phase(
            adj,
            degree,
            total_weight,
            resolution,
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
            deterministic_rotate(&mut order, iterations as usize);
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
            order.sort_unstable();

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

/// 社区检测流水线阶段
///
/// 此阶段将文档直接传递，不进行修改。
/// 实际的社区检测在解析完成后由 `CommunityDetector::detect` 对符号图执行。
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

    #[test]
    fn test_deterministic_rotate() {
        let mut v = vec![0, 1, 2, 3, 4];
        deterministic_rotate(&mut v, 2);
        assert_eq!(v, vec![2, 3, 4, 0, 1]);

        deterministic_rotate(&mut v, 0);
        assert_eq!(v, vec![2, 3, 4, 0, 1]);

        let mut v2 = vec![0, 1, 2];
        deterministic_rotate(&mut v2, 3);
        assert_eq!(v2, vec![0, 1, 2]);

        let mut empty: Vec<i32> = vec![];
        deterministic_rotate(&mut empty, 5);
        assert!(empty.is_empty());
    }
}
