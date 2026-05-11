/// HNSWLIB 内存向量索引适配器（开发/测试环境）
///
/// 基于内存的 HNSW 图索引实现，无需外部依赖。
use crate::error::{Error, Result};
use crate::math::{cosine_similarity, dot_product, euclidean_distance};
use crate::vector_store::store::{CollectionInfo, CollectionStatus, VectorPoint, VectorStore};
use crate::vector_store::types::*;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

/// HNSW 索引配置
#[derive(Debug, Clone)]
pub struct HnswConfig {
    /// 最大连接数
    pub max_connections: usize,
    /// EF 构造参数
    pub ef_construction: usize,
    /// EF 搜索参数
    pub ef_search: usize,
    /// 距离度量
    pub distance: DistanceMetric,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            max_connections: 16,
            ef_construction: 200,
            ef_search: 50,
            distance: DistanceMetric::Cosine,
        }
    }
}

/// 内存集合
#[derive(Debug, Clone)]
struct InMemoryCollection {
    points: HashMap<Uuid, VectorPoint>,
    distance: DistanceMetric,
    dimension: usize,
}

/// HNSWLIB 内存向量索引适配器
#[derive(Debug)]
pub struct HnswlibAdapter {
    collections: RwLock<HashMap<String, InMemoryCollection>>,
    config: HnswConfig,
}

impl HnswlibAdapter {
    /// 创建新的 HNSWLIB 适配器
    pub fn new(config: HnswConfig) -> Self {
        Self {
            collections: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// 暴力搜索（内存实现）
    fn brute_force_search(
        &self,
        collection: &InMemoryCollection,
        query: &[f32],
        options: &SearchOptions,
    ) -> Vec<SearchResult> {
        let mut results: Vec<(Uuid, f64)> = collection
            .points
            .iter()
            .map(|(id, p)| {
                let score = match collection.distance {
                    DistanceMetric::Cosine => cosine_similarity(query, &p.vector),
                    DistanceMetric::Euclidean => euclidean_distance(query, &p.vector),
                    DistanceMetric::DotProduct => dot_product(query, &p.vector),
                };
                (*id, score)
            })
            .collect();

        results.sort_by(|a, b| {
            b.1.total_cmp(&a.1)
                .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
        });
        results.truncate(options.top_k);

        results
            .into_iter()
            .map(|(id, score)| {
                let point = collection
                    .points
                    .get(&id)
                    .ok_or_else(|| format!("向量点 {id} 在搜索结果中但不在集合中"))
                    .expect("搜索结果中的 ID 必定存在于集合中; 此不变量由 brute_force_search 保证");
                SearchResult {
                    id,
                    score,
                    payload: point.payload.clone(),
                    vector: None,
                    metadata: None,
                }
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl VectorStore for HnswlibAdapter {
    async fn init_collection(
        &self,
        name: &str,
        dimension: usize,
        distance: DistanceMetric,
    ) -> Result<()> {
        debug!(collection = name, dim = dimension, "init collection");
        let mut cols = self.collections.write().await;
        if !cols.contains_key(name) {
            cols.insert(
                name.to_string(),
                InMemoryCollection {
                    points: HashMap::new(),
                    distance,
                    dimension,
                },
            );
        }
        Ok(())
    }

    async fn upsert(&self, collection: &str, points: Vec<VectorPoint>) -> Result<Vec<Uuid>> {
        if points.is_empty() {
            return Ok(vec![]);
        }
        let mut cols = self.collections.write().await;
        let col = cols
            .get_mut(collection)
            .ok_or_else(|| Error::new("NOT_FOUND").context(collection))?;
        let ids: Vec<Uuid> = points.iter().map(|p| p.id).collect();
        for p in &points {
            col.points.insert(p.id, p.clone());
        }
        Ok(ids)
    }

    async fn similarity_search(
        &self,
        collection: &str,
        query_vector: &[f32],
        options: SearchOptions,
    ) -> Result<Vec<SearchResult>> {
        let cols = self.collections.read().await;
        let col = cols
            .get(collection)
            .ok_or_else(|| Error::new("NOT_FOUND").context(collection))?;
        Ok(self.brute_force_search(col, query_vector, &options))
    }

    async fn hybrid_search(
        &self,
        collection: &str,
        query: &HybridQuery,
    ) -> Result<Vec<SearchResult>> {
        if let Some(ref v) = query.vector {
            self.similarity_search(collection, v, query.options.clone())
                .await
        } else {
            Ok(vec![])
        }
    }

    async fn get_by_ids(&self, collection: &str, ids: &[Uuid]) -> Result<Vec<SearchResult>> {
        let cols = self.collections.read().await;
        match cols.get(collection) {
            Some(col) => Ok(ids
                .iter()
                .filter_map(|id| {
                    col.points.get(id).map(|p| SearchResult {
                        id: *id,
                        score: 1.0,
                        payload: p.payload.clone(),
                        vector: None,
                        metadata: extract_metadata(&p.payload),
                    })
                })
                .collect()),
            None => Err(Error::new("NOT_FOUND").context(collection)),
        }
    }

    async fn delete(&self, collection: &str, ids: &[Uuid]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut cols = self.collections.write().await;
        if let Some(col) = cols.get_mut(collection) {
            for id in ids {
                col.points.remove(id);
            }
        }
        Ok(())
    }

    async fn collection_info(&self, collection: &str) -> Result<CollectionInfo> {
        let cols = self.collections.read().await;
        match cols.get(collection) {
            Some(col) => Ok(CollectionInfo {
                name: collection.to_string(),
                vectors_count: col.points.len() as u64,
                dimension: col.dimension,
                status: CollectionStatus::Green,
            }),
            None => Err(Error::new("NOT_FOUND").context(collection)),
        }
    }

    async fn close(&self) -> Result<()> {
        let mut cols = self.collections.write().await;
        cols.clear();
        Ok(())
    }
}

fn extract_metadata(payload: &serde_json::Value) -> std::collections::HashMap<String, String> {
    let mut metadata = std::collections::HashMap::new();
    if let Some(obj) = payload.as_object() {
        for (k, v) in obj {
            if v.is_string() {
                metadata.insert(k.clone(), v.as_str().unwrap_or_default().to_string());
            }
        }
    }
    metadata
}
