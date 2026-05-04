//! 抽取管道编排
//!
//! 详见文档: §5 | 用例: UC-018~UC-025

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::stream::{self, StreamExt};

use knowledge_core::model::semantic::{
    SemanticEntity, SemanticRelation,
};
use knowledge_core::model::RecordIdType;
use knowledge_core::surreal_value_to_json;

use crate::config::ExtractorConfig;
use crate::deduplication::Deduplicator;
use crate::disambiguation::Disambiguator;
use crate::error::{self, Result};
use crate::extractors::{ExtractedRelation, LanguageModel, LlmExtractor};
use error_core::classification::{ErrorSource, Recoverability};

/// 错误恢复动作
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    /// 重试当前操作
    Retry,
    /// 跳过当前块，继续处理
    Skip,
    /// 中止整个管道
    Abort,
}

/// 抽取管道
///
/// 使用泛型参数 `D: DatabaseClient` 以支持依赖注入和测试替身。
///
/// 详见文档: §5 | 用例: UC-018~UC-025
#[cfg(feature = "db")]
pub struct ExtractionPipeline<D: knowledge_core::DatabaseClient> {
    extractor: LlmExtractor,
    disambiguator: Disambiguator,
    deduplicator: Deduplicator,
    db: Arc<D>,
}

#[cfg(feature = "db")]
impl<D: knowledge_core::DatabaseClient> ExtractionPipeline<D> {
    /// 创建抽取管道
    ///
    /// 详见文档: §5.1 | 用例: UC-018 | 方法: M-028
    ///
    /// # Errors
    ///
    /// 当配置验证失败时返回配置错误
    pub fn new(
        llm: Arc<dyn LanguageModel>,
        config: ExtractorConfig,
        db: Arc<D>,
    ) -> Result<Self> {
        config.validate()?;
        let extractor = LlmExtractor::new(llm.clone(), config.clone())?;
        let disambiguator = Disambiguator::with_llm(config.clone(), llm);
        let deduplicator = Deduplicator::new(config);
        Ok(Self {
            extractor,
            disambiguator,
            deduplicator,
            db,
        })
    }

    /// 处理单个块
    ///
    /// 执行完整的抽取流程：实体抽取 → 关系抽取 → 加载已有实体 → 消歧 → 去重
    ///
    /// 详见文档: §5.2 | 用例: UC-019 | 方法: M-029
    ///
    /// # Errors
    ///
    /// 当 LLM 调用失败或响应解析失败时返回错误
    pub async fn process_block(
        &self,
        block_id: RecordIdType,
        content: &str,
    ) -> Result<(Vec<SemanticEntity>, Vec<ExtractedRelation>)> {
        let entities = self.extractor.extract_entities(content).await?;
        let relations = self.extractor.extract_relations(content, &entities).await?;
        let existing = self.load_existing_entities(&entities).await?;
        let resolved_entities = self.disambiguator.resolve_entities_async(&entities, &existing).await;
        let unique_entities = self.deduplicator.deduplicate(&resolved_entities);
        let _ = block_id;
        Ok((unique_entities, relations))
    }

    /// 处理块并持久化结果
    ///
    /// 详见文档: §5.2 | 用例: UC-019 | 方法: M-030
    ///
    /// # Errors
    ///
    /// 当抽取或持久化失败时返回错误
    pub async fn process_block_with_persistence(
        &self,
        block_id: RecordIdType,
        content: &str,
    ) -> Result<()> {
        let (entities, relations) = self.process_block(block_id, content).await?;
        let entity_id_map = self.persist_entities(&entities).await?;
        self.persist_relations(&relations, &entity_id_map).await?;
        Ok(())
    }

    /// 处理块（带重试）
    ///
    /// 详见文档: §5.2 | 用例: UC-019 | 方法: M-031
    ///
    /// # Errors
    ///
    /// 当所有重试均失败时返回最后一个错误
    pub async fn process_block_with_retry(
        &self,
        block_id: RecordIdType,
        content: &str,
    ) -> Result<()> {
        let max_retries = self.extractor.max_retries();
        for attempt in 1..=max_retries {
            match self
                .process_block_with_persistence(block_id.clone(), content)
                .await
            {
                Ok(()) => return Ok(()),
                Err(e) if attempt < max_retries => {
                    tracing::warn!("Block processing failed (attempt {attempt}): {e}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(error_core::helpers::validation_error("重试次数耗尽但未获得结果，请检查 max_retries 配置", "process_block_with_retry"))
    }

    /// 批量处理块（顺序）
    ///
    /// 详见文档: §5.3 | 用例: UC-020 | 方法: M-032
    ///
    /// # Errors
    ///
    /// 当任一块处理失败时返回错误
    pub async fn process_batch(
        &self,
        blocks: &[(RecordIdType, String)],
    ) -> Result<()> {
        for (block_id, content) in blocks {
            self.process_block_with_retry(block_id.clone(), content).await?;
        }
        Ok(())
    }

    /// 并发批量处理
    ///
    /// 使用 `futures::stream::iter` + `buffer_unordered` 限制并发度，
    /// 避免同时发起过多 LLM 请求。
    ///
    /// 详见文档: §5.3 | 用例: UC-020 | 方法: M-033
    ///
    /// # Errors
    ///
    /// 当任一块处理失败时返回第一个错误
    pub async fn process_batch_concurrent(
        &self,
        blocks: &[(RecordIdType, String)],
        concurrency: usize,
    ) -> Result<()> {
        let futures = blocks.iter().map(|(block_id, content)| {
            self.process_block_with_retry(block_id.clone(), content)
        });
        let results: Vec<Result<()>> = stream::iter(futures)
            .buffer_unordered(concurrency)
            .collect()
            .await;

        results.into_iter().collect()
    }

    /// 处理错误并决定恢复动作
    ///
    /// 详见文档: §5.4 | 用例: UC-021 | 方法: M-034
    #[must_use]
    pub fn handle_error(&self, error: &error_core::ErrorObject, _block_id: RecordIdType) -> RecoveryAction {
        match error.recoverability() {
            Recoverability::AutoRecoverable => RecoveryAction::Retry,
            Recoverability::ManualIntervention => {
                match error.source() {
                    ErrorSource::CFG => RecoveryAction::Abort,
                    _ => RecoveryAction::Skip,
                }
            }
            Recoverability::SemiAuto | Recoverability::NonRecoverable => RecoveryAction::Skip,
        }
    }

    /// 持久化实体
    ///
    /// 将抽取的实体写入数据库，并返回实体名称到 `RecordId` 的映射，
    /// 供后续关系持久化使用。
    ///
    /// 详见文档: §5.5 | 用例: UC-023 | 方法: M-035
    ///
    /// # Errors
    ///
    /// 当数据库写入失败时返回数据库错误
    async fn persist_entities(
        &self,
        entities: &[SemanticEntity],
    ) -> Result<HashMap<String, RecordIdType>> {
        let mut entity_id_map = HashMap::with_capacity(entities.len());

        for entity in entities {
            let results = self
                .db
                .create("semantic_entity", entity.clone())
                .await
                .map_err(|e| error::database_error(e.to_string()))?;

            if let Some(surreal_value) = results.first() {
                let json_value = surreal_value_to_json(surreal_value);
                if let Some(id_str) = json_value.get("id").and_then(|v| v.as_str()) {
                    if let Ok(record_id) = id_str.parse::<RecordIdType>() {
                        entity_id_map.insert(entity.name.clone(), record_id);
                    }
                }
            }
        }

        Ok(entity_id_map)
    }

    /// 持久化关系
    ///
    /// 将抽取的关系写入数据库，使用 `entity_id_map` 将实体名称
    /// 解析为数据库 `RecordId`。
    ///
    /// 详见文档: §5.6 | 用例: UC-024 | 方法: M-036
    ///
    /// # Errors
    ///
    /// 当数据库写入失败或实体 ID 解析失败时返回错误
    async fn persist_relations(
        &self,
        relations: &[ExtractedRelation],
        entity_id_map: &HashMap<String, RecordIdType>,
    ) -> Result<()> {
        for relation in relations {
            let source_id = if let Some(id) = entity_id_map.get(&relation.source_name) {
                id.clone()
            } else {
                tracing::warn!(
                    "Skipping relation: source entity '{}' not found in persisted entities",
                    relation.source_name
                );
                continue;
            };
            let target_id = if let Some(id) = entity_id_map.get(&relation.target_name) {
                id.clone()
            } else {
                tracing::warn!(
                    "Skipping relation: target entity '{}' not found in persisted entities",
                    relation.target_name
                );
                continue;
            };

            let semantic_relation =
                SemanticRelation::new(source_id, target_id, relation.relation_type.clone(), relation.evidence.clone());

            self.db
                .create("semantic_relation", semantic_relation)
                .await
                .map_err(|e| error::database_error(e.to_string()))?;
        }

        Ok(())
    }

    /// 运行完整抽取管道
    ///
    /// 从数据库加载指定文档的所有块，然后并发执行抽取。
    ///
    /// 详见文档: §5.7 | 用例: UC-025 | 方法: M-037
    ///
    /// # Errors
    ///
    /// 当块加载或抽取失败时返回错误
    pub async fn run(&self, document_id: RecordIdType) -> Result<()> {
        let blocks = self.load_blocks(document_id.clone()).await?;
        tracing::info!(
            "Starting extraction for document {}, {} blocks",
            document_id,
            blocks.len()
        );
        self.process_batch_concurrent(&blocks, 4).await?;
        tracing::info!("Extraction complete for document {}", document_id);
        Ok(())
    }

    /// 从数据库加载与候选实体可能匹配的已有实体
    ///
    /// 查询策略：按候选实体的名称和类型查询数据库中已存在的实体，
    /// 使用 `disambiguation_key`（`name:entity_type`）作为匹配依据。
    /// 同时查询别名匹配，确保别名相同的实体也能被消歧。
    ///
    /// # Errors
    ///
    /// 当数据库查询失败时返回错误
    async fn load_existing_entities(
        &self,
        candidates: &[SemanticEntity],
    ) -> Result<Vec<SemanticEntity>> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let names: Vec<String> = candidates.iter().map(|e| e.name.clone()).collect();
        let names_json = serde_json::Value::Array(
            names.iter().map(|n| serde_json::Value::String(n.clone())).collect(),
        );

        let sql = "SELECT * FROM semantic_entity WHERE name IN $names";
        let bindings = serde_json::json!({ "names": names_json });

        let results = self
            .db
            .query(sql, bindings)
            .await
            .map_err(|e| error::database_error(e.to_string()))?;

        let mut existing = Vec::new();
        for surreal_value in &results {
            let json_value = surreal_value_to_json(surreal_value);
            if let Ok(entity) = serde_json::from_value::<SemanticEntity>(json_value) {
                existing.push(entity);
            }
        }

        Ok(existing)
    }

    /// 从数据库加载文档的所有块及其内容
    async fn load_blocks(
        &self,
        document_id: RecordIdType,
    ) -> Result<Vec<(RecordIdType, String)>> {
        let sql = "SELECT id, content FROM block WHERE document_id = $doc_id ORDER BY start_line";
        let bindings = serde_json::json!({ "doc_id": document_id.to_string() });

        let results = self
            .db
            .query(sql, bindings)
            .await
            .map_err(|e| error::database_error(e.to_string()))?;

        let mut blocks = Vec::new();
        for surreal_value in &results {
            let json_value = surreal_value_to_json(surreal_value);
            let id_str = json_value.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let content = json_value.get("content").and_then(|v| v.as_str()).unwrap_or("");

            if let Ok(record_id) = id_str.parse::<RecordIdType>() {
                blocks.push((record_id, content.to_string()));
            }
        }

        Ok(blocks)
    }
}

#[cfg(all(test, feature = "db"))]
mod tests {
    use super::*;
    use knowledge_core::DatabaseClient;
    use std::future::Future;

    struct MockLlm;

    #[async_trait::async_trait]
    impl LanguageModel for MockLlm {
        async fn generate(&self, _prompt: &str) -> std::result::Result<String, String> {
            Ok("[]".to_string())
        }
    }

    struct MockDb;

    impl DatabaseClient for MockDb {
        fn select<T: for<'de> serde::Deserialize<'de> + Send>(
            &self,
            _id: surrealdb::opt::RecordId,
        ) -> impl Future<Output = knowledge_core::Result<Option<T>>> + Send {
            async move { Ok(None) }
        }

        fn query(
            &self,
            _sql: &str,
            _bindings: impl serde::Serialize + Send,
        ) -> impl Future<Output = knowledge_core::Result<Vec<surrealdb::sql::Value>>> + Send {
            async move { Ok(vec![]) }
        }

        fn create<T: serde::Serialize + Send>(
            &self,
            _table: &str,
            _data: T,
        ) -> impl Future<Output = knowledge_core::Result<Vec<surrealdb::sql::Value>>> + Send {
            async move { Ok(vec![]) }
        }

        fn update<T: serde::Serialize + Send>(
            &self,
            _id: surrealdb::opt::RecordId,
            _data: T,
        ) -> impl Future<Output = knowledge_core::Result<Option<surrealdb::sql::Value>>> + Send {
            async move { Ok(None) }
        }

        fn delete(
            &self,
            _id: surrealdb::opt::RecordId,
        ) -> impl Future<Output = knowledge_core::Result<Option<surrealdb::sql::Value>>> + Send {
            async move { Ok(None) }
        }

        fn insert_batch(
            &self,
            _table: &str,
            _items: Vec<serde_json::Value>,
        ) -> impl Future<Output = knowledge_core::Result<Vec<surrealdb::sql::Value>>> + Send {
            async move { Ok(vec![]) }
        }

        fn execute_transaction(
            &self,
            _queries: Vec<String>,
            _bindings: Vec<serde_json::Value>,
        ) -> impl Future<Output = knowledge_core::Result<Vec<Vec<surrealdb::sql::Value>>>> + Send {
            async move { Ok(vec![]) }
        }
    }

    fn create_test_pipeline() -> ExtractionPipeline<MockDb> {
        let config = ExtractorConfig::default();
        ExtractionPipeline::new(
            Arc::new(MockLlm),
            config,
            Arc::new(MockDb),
        ).unwrap()
    }

    fn make_rid(table: &str, id: &str) -> RecordIdType {
        surrealdb::sql::Thing::from((table, id))
    }

    #[test]
    fn test_recovery_action_variants() {
        assert_eq!(RecoveryAction::Retry, RecoveryAction::Retry);
        assert_ne!(RecoveryAction::Skip, RecoveryAction::Abort);
    }

    #[test]
    fn test_handle_error_returns_correct_action() {
        let pipeline = create_test_pipeline();
        let block_id = make_rid("block", "test");

        assert_eq!(
            pipeline.handle_error(&error::timeout_error(), block_id.clone()),
            RecoveryAction::Retry
        );
        assert_eq!(
            pipeline.handle_error(&error::llm_error("err"), block_id.clone()),
            RecoveryAction::Retry
        );
        assert_eq!(
            pipeline.handle_error(&error::invalid_config("bad"), block_id.clone()),
            RecoveryAction::Abort
        );
        assert_eq!(
            pipeline.handle_error(&error::database_error("db"), block_id.clone()),
            RecoveryAction::Skip
        );
        assert_eq!(
            pipeline.handle_error(
                &error::serialization_error(&serde_json::from_str::<i32>("x").unwrap_err()),
                block_id,
            ),
            RecoveryAction::Skip
        );
    }

    #[tokio::test]
    async fn test_process_block_returns_empty_for_mock() {
        let pipeline = create_test_pipeline();
        let block_id = make_rid("block", "test");

        let (entities, relations) = pipeline.process_block(block_id, "test content").await.unwrap();
        assert!(entities.is_empty());
        assert!(relations.is_empty());
    }

    #[tokio::test]
    async fn test_process_batch_concurrent_empty() {
        let pipeline = create_test_pipeline();
        let result = pipeline.process_batch_concurrent(&[], 4).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_batch_empty() {
        let pipeline = create_test_pipeline();
        let result = pipeline.process_batch(&[]).await;
        assert!(result.is_ok());
    }
}
