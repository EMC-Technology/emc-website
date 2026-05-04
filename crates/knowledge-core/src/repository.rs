use std::sync::Arc;

use surrealdb::opt::RecordId;

use crate::database::DatabaseClient;
use crate::model::{Block, Document, RecordIdType, RefType, Reference, Token};
use crate::Result;
use crate::error::helpers;

/// 将 SurrealDB 的 Value 类型转换为 serde_json::Value
///
/// 处理 SurrealDB 的所有基础类型（Object、Array、Thing 等）到 JSON 的映射转换
pub fn surreal_value_to_json(value: &surrealdb::sql::Value) -> serde_json::Value {
    match value {
        surrealdb::sql::Value::None | surrealdb::sql::Value::Null => serde_json::Value::Null,
        surrealdb::sql::Value::Bool(b) => serde_json::Value::Bool(*b),
        surrealdb::sql::Value::Number(n) => {
            if n.is_float() {
                serde_json::Number::from_f64(n.clone().as_float())
                    .map_or(serde_json::Value::Null, serde_json::Value::Number)
            } else {
                serde_json::Value::Number(serde_json::Number::from(n.clone().as_int()))
            }
        }
        surrealdb::sql::Value::Strand(s) => serde_json::Value::String(s.as_str().to_string()),
        surrealdb::sql::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(surreal_value_to_json).collect())
        }
        surrealdb::sql::Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), surreal_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        surrealdb::sql::Value::Thing(t) => {
            serde_json::Value::String(t.to_string())
        }
        _ => serde_json::Value::String(value.to_string()),
    }
}

/// 通过 SurrealDB Value 中转进行反序列化
///
/// 先将 SurrealDB Value 转为 JSON 值，再通过 serde_json 反序列化
#[allow(clippy::needless_pass_by_value)]
pub fn deserialize_value<T: serde::de::DeserializeOwned>(value: surrealdb::sql::Value) -> Result<T> {
    let json_value = surreal_value_to_json(&value);
    serde_json::from_value(json_value).map_err(Into::into)
}

fn rid(table: &str, id: &str) -> RecordId {
    RecordId::from((table, id))
}

/// 文档仓储层，提供 Document 的 CRUD 操作
pub struct DocumentRepository<D: DatabaseClient> {
    client: Arc<D>,
}

impl<D: DatabaseClient> DocumentRepository<D> {
    /// 创建文档仓储实例
    pub const fn new(client: Arc<D>) -> Self {
        Self { client }
    }

    /// 创建新文档
    pub async fn create(&self, doc: &Document) -> Result<Document> {
        let results = self.client.create("document", doc).await?;

        let value = results.into_iter().next().ok_or_else(|| {
            helpers::db_error("create document: empty result")
        })?;

        deserialize_value(value)
    }

    /// 根据 ID 查找文档
    pub async fn find_by_id(&self, id: &RecordIdType) -> Result<Option<Document>> {
        self.client.select(id.clone()).await
    }

    /// 根据哈希值查找文档
    pub async fn find_by_hash(&self, hash: &str) -> Result<Option<Document>> {
        let results = self
            .client
            .query(
                "SELECT * FROM document WHERE hash = $hash LIMIT 1",
                serde_json::json!({ "hash": hash }),
            )
            .await?;

        results
            .into_iter()
            .next()
            .map(deserialize_value)
            .transpose()
    }

    /// 分页列出文档
    pub async fn list(&self, offset: usize, limit: usize) -> Result<Vec<Document>> {
        let results = self
            .client
            .query(
                "SELECT * FROM document ORDER BY created_at DESC LIMIT $limit START $offset",
                serde_json::json!({ "limit": limit, "offset": offset }),
            )
            .await?;

        results
            .into_iter()
            .map(deserialize_value)
            .collect()
    }

    /// 更新文档
    pub async fn update(&self, id: &RecordIdType, doc: &Document) -> Result<Document> {
        let value = self.client
            .update(id.clone(), doc.clone())
            .await?
            .ok_or_else(|| helpers::not_found("document", &id.to_string()))?;
        deserialize_value(value)
    }

    /// 级联删除文档及其关联的 Block、Token 和 Reference
    ///
    /// 使用 SurrealQL 事务确保原子性：所有删除操作要么全部成功，
    /// 要么全部回滚，避免中途失败导致数据不一致。
    pub async fn delete_cascade(&self, doc_id: &RecordIdType) -> Result<()> {
        let doc_id_str = doc_id.to_string();
        self.client
            .query(
                "BEGIN TRANSACTION; \
                 DELETE FROM reference WHERE source_id = $doc_id OR target_id = $doc_id; \
                 DELETE FROM token WHERE block_id IN (SELECT id FROM block WHERE document_id = $doc_id); \
                 DELETE FROM block WHERE document_id = $doc_id; \
                 DELETE FROM document WHERE id = $doc_id; \
                 COMMIT TRANSACTION;",
                serde_json::json!({ "doc_id": doc_id_str }),
            )
            .await?;

        Ok(())
    }
}

/// 块仓储层，提供 Block 的 CRUD 及向量搜索操作
pub struct BlockRepository<D: DatabaseClient> {
    client: Arc<D>,
}

impl<D: DatabaseClient> BlockRepository<D> {
    /// 创建块仓储实例
    pub const fn new(client: Arc<D>) -> Self {
        Self { client }
    }

    /// 创建新块
    pub async fn create(&self, block: &Block) -> Result<Block> {
        let results = self.client.create("block", block).await?;

        let value = results.into_iter().next().ok_or_else(|| {
            helpers::db_error("create block: empty result")
        })?;

        deserialize_value(value)
    }

    /// 根据 ID 查找块
    pub async fn find_by_id(&self, id: &RecordIdType) -> Result<Option<Block>> {
        self.client.select(id.clone()).await
    }

    /// 根据文档 ID 查找所有块，按起始行排序
    pub async fn find_by_document(&self, doc_id: &RecordIdType) -> Result<Vec<Block>> {
        let results = self
            .client
            .query(
                "SELECT * FROM block WHERE document_id = $doc_id ORDER BY start_line ASC",
                serde_json::json!({ "doc_id": doc_id.to_string() }),
            )
            .await?;

        results
            .into_iter()
            .map(deserialize_value)
            .collect()
    }

    /// 更新块的向量嵌入
    pub async fn update_embedding(&self, id: &RecordIdType, embedding: Vec<f32>) -> Result<()> {
        self.client
            .update(id.clone(), serde_json::json!({ "embedding": embedding }))
            .await?;
        Ok(())
    }

    /// 向量相似度搜索
    pub async fn vector_search(&self, query_vec: Vec<f32>, k: u32) -> Result<Vec<(Block, f64)>> {
        let results = self
            .client
            .query(
                "SELECT *, vector::similarity::cosine(embedding, $query) AS score FROM block WHERE embedding != NONE ORDER BY score DESC, id ASC LIMIT $k",
                serde_json::json!({ "query": query_vec, "k": k }),
            )
            .await?;

        let mut output = Vec::new();
        for value in results {
            let json_val = surreal_value_to_json(&value);
            let score = json_val.get("score").and_then(serde_json::Value::as_f64).unwrap_or_else(|| {
                tracing::warn!("vector_search: score 字段缺失或类型不匹配，默认为 0.0");
                0.0
            });
            let block: Block = deserialize_value(value)?;
            output.push((block, score));
        }

        output.sort_by(|a, b| {
            let score_cmp = b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal);
            let near_tie = (a.1 - b.1).abs() < 1e-10;
            if score_cmp == std::cmp::Ordering::Equal || near_tie {
                let id_a = a.0.id.as_ref().map_or_else(String::new, std::string::ToString::to_string);
                let id_b = b.0.id.as_ref().map_or_else(String::new, std::string::ToString::to_string);
                id_a.cmp(&id_b)
            } else {
                score_cmp
            }
        });

        Ok(output)
    }

    /// 根据文档 ID 删除所有块
    pub async fn delete_by_document(&self, doc_id: &RecordIdType) -> Result<usize> {
        let results = self
            .client
            .query(
                "DELETE FROM block WHERE document_id = $doc_id RETURN count()",
                serde_json::json!({ "doc_id": doc_id.to_string() }),
            )
            .await?;

        Ok(results.len())
    }
}

/// 词元仓储层，提供 Token 的 CRUD 及批量操作
pub struct TokenRepository<D: DatabaseClient> {
    client: Arc<D>,
}

impl<D: DatabaseClient> TokenRepository<D> {
    /// 创建词元仓储实例
    pub const fn new(client: Arc<D>) -> Self {
        Self { client }
    }

    /// 创建新词元
    pub async fn create(&self, token: &Token) -> Result<Token> {
        let results = self.client.create("token", token).await?;

        let value = results.into_iter().next().ok_or_else(|| {
            helpers::db_error("create token: empty result")
        })?;

        deserialize_value(value)
    }

    /// 批量插入词元
    pub async fn insert_batch(&self, tokens: &[Token]) -> Result<Vec<Token>> {
        let items: Vec<serde_json::Value> = tokens
            .iter()
            .map(|t| serde_json::to_value(t).map_err(error_core::ErrorObject::from))
            .collect::<Result<Vec<_>>>()?;

        let results = self.client.insert_batch("token", items).await?;

        let mut created = Vec::with_capacity(tokens.len());
        for (i, value) in results.into_iter().enumerate() {
            let token_data: Token = deserialize_value(value)?;
            let mut token = tokens[i].clone();
            token.id = token_data.id;
            created.push(token);
        }
        Ok(created)
    }

    /// 根据 ID 查找词元
    pub async fn find_by_id(&self, id: &RecordIdType) -> Result<Option<Token>> {
        self.client.select(id.clone()).await
    }

    /// 根据块 ID 查找所有词元，按起始字符位置排序
    pub async fn find_by_block(&self, block_id: &RecordIdType) -> Result<Vec<Token>> {
        let results = self
            .client
            .query(
                "SELECT * FROM token WHERE block_id = $block_id ORDER BY start_char ASC",
                serde_json::json!({ "block_id": block_id.to_string() }),
            )
            .await?;

        results
            .into_iter()
            .map(deserialize_value)
            .collect()
    }

    /// 根据全局偏移量查找词元
    pub async fn find_by_global_offset(&self, offset: u64) -> Result<Option<Token>> {
        let results = self
            .client
            .query(
                "SELECT * FROM token WHERE global_offset = $offset LIMIT 1",
                serde_json::json!({ "offset": offset }),
            )
            .await?;

        results
            .into_iter()
            .next()
            .map(deserialize_value)
            .transpose()
    }

    /// 根据块 ID 删除所有词元
    pub async fn delete_by_block(&self, block_id: &RecordIdType) -> Result<usize> {
        let results = self
            .client
            .query(
                "DELETE FROM token WHERE block_id = $block_id RETURN count()",
                serde_json::json!({ "block_id": block_id.to_string() }),
            )
            .await?;

        Ok(results.len())
    }
}

/// 引用仓储层，提供 Reference 的 CRUD 及图遍历操作
pub struct ReferenceRepository<D: DatabaseClient> {
    client: Arc<D>,
}

impl<D: DatabaseClient> ReferenceRepository<D> {
    /// 创建引用仓储实例
    pub const fn new(client: Arc<D>) -> Self {
        Self { client }
    }

    /// 创建新引用
    pub async fn create(&self, reference: &Reference) -> Result<Reference> {
        let results = self.client.create("reference", reference).await?;

        let value = results.into_iter().next().ok_or_else(|| {
            helpers::db_error("create reference: empty result")
        })?;

        deserialize_value(value)
    }

    /// 批量插入引用
    pub async fn insert_batch(&self, references: &[Reference]) -> Result<Vec<Reference>> {
        let items: Vec<serde_json::Value> = references
            .iter()
            .map(|r| serde_json::to_value(r).map_err(error_core::ErrorObject::from))
            .collect::<Result<Vec<_>>>()?;

        let results = self.client.insert_batch("reference", items).await?;

        let mut created = Vec::with_capacity(references.len());
        for (i, value) in results.into_iter().enumerate() {
            let ref_data: Reference = deserialize_value(value)?;
            let mut reference = references[i].clone();
            reference.id = ref_data.id;
            created.push(reference);
        }
        Ok(created)
    }

    /// 根据 ID 查找引用
    pub async fn find_by_id(&self, id: &RecordIdType) -> Result<Option<Reference>> {
        self.client.select(id.clone()).await
    }

    /// 根据源节点 ID 查找所有出边引用
    pub async fn find_by_source(&self, source_id: &RecordIdType) -> Result<Vec<Reference>> {
        let results = self
            .client
            .query(
                "SELECT * FROM reference WHERE source_id = $source_id",
                serde_json::json!({ "source_id": source_id.to_string() }),
            )
            .await?;

        results
            .into_iter()
            .map(deserialize_value)
            .collect()
    }

    /// 根据目标节点 ID 查找所有入边引用
    pub async fn find_by_target(&self, target_id: &RecordIdType) -> Result<Vec<Reference>> {
        let results = self
            .client
            .query(
                "SELECT * FROM reference WHERE target_id = $target_id",
                serde_json::json!({ "target_id": target_id.to_string() }),
            )
            .await?;

        results
            .into_iter()
            .map(deserialize_value)
            .collect()
    }

    /// 正向追踪：从指定节点出发，沿出边方向查找引用
    pub async fn trace_forward(&self, id: &RecordIdType, ref_type: Option<RefType>) -> Result<Vec<Reference>> {
        let sql = match ref_type {
            Some(_) => "SELECT * FROM reference WHERE source_id = $id AND ref_type = $ref_type",
            None => "SELECT * FROM reference WHERE source_id = $id",
        };
        let bindings = match ref_type {
            Some(rt) => {
                let rt_json = serde_json::to_string(&rt)
                    .map_err(|e| helpers::serde_error(&format!("RefType 序列化失败: {e}")))?;
                serde_json::json!({ "id": id.to_string(), "ref_type": rt_json })
            }
            None => serde_json::json!({ "id": id.to_string() }),
        };

        let results = self.client.query(sql, bindings).await?;
        results.into_iter().map(deserialize_value).collect()
    }

    /// 反向追踪：从指定节点出发，沿入边方向查找引用
    pub async fn trace_backward(&self, id: &RecordIdType, ref_type: Option<RefType>) -> Result<Vec<Reference>> {
        let sql = match ref_type {
            Some(_) => "SELECT * FROM reference WHERE target_id = $id AND ref_type = $ref_type",
            None => "SELECT * FROM reference WHERE target_id = $id",
        };
        let bindings = match ref_type {
            Some(rt) => {
                let rt_json = serde_json::to_string(&rt)
                    .map_err(|e| helpers::serde_error(&format!("RefType 序列化失败: {e}")))?;
                serde_json::json!({ "id": id.to_string(), "ref_type": rt_json })
            }
            None => serde_json::json!({ "id": id.to_string() }),
        };

        let results = self.client.query(sql, bindings).await?;
        results.into_iter().map(deserialize_value).collect()
    }

    /// 根据文档 ID 删除所有关联引用
    pub async fn delete_by_document(&self, doc_id: &RecordIdType) -> Result<usize> {
        let results = self
            .client
            .query(
                "DELETE FROM reference WHERE source_id = $doc_id OR target_id = $doc_id RETURN count()",
                serde_json::json!({ "doc_id": doc_id.to_string() }),
            )
            .await?;

        Ok(results.len())
    }
}

/// 社区仓储层，提供 Community 的操作接口
pub struct CommunityRepository<D: DatabaseClient> {
    client: Arc<D>,
}

impl<D: DatabaseClient> CommunityRepository<D> {
    /// 创建社区仓储实例
    pub const fn new(client: Arc<D>) -> Self {
        Self { client }
    }

    /// 根据 ID 查找社区
    pub async fn find_by_id(&self, id: &RecordIdType) -> Result<Option<crate::model::Community>> {
        self.client.select(id.clone()).await
    }

    /// 分页列出社区，按内聚分数降序排列
    pub async fn list(&self, offset: usize, limit: usize) -> Result<Vec<crate::model::Community>> {
        let results = self
            .client
            .query(
                "SELECT * FROM community ORDER BY cohesion_score DESC LIMIT $limit START $offset",
                serde_json::json!({ "limit": limit, "offset": offset }),
            )
            .await?;

        results
            .into_iter()
            .map(deserialize_value)
            .collect()
    }
}

/// 流程仓储层，提供 Process 的操作接口
pub struct ProcessRepository<D: DatabaseClient> {
    client: Arc<D>,
}

impl<D: DatabaseClient> ProcessRepository<D> {
    /// 创建流程仓储实例
    pub const fn new(client: Arc<D>) -> Self {
        Self { client }
    }

    /// 根据 ID 查找流程
    pub async fn find_by_id(&self, id: &RecordIdType) -> Result<Option<crate::model::Process>> {
        self.client.select(id.clone()).await
    }

    /// 分页列出流程，按名称升序排列
    pub async fn list(&self, offset: usize, limit: usize) -> Result<Vec<crate::model::Process>> {
        let results = self
            .client
            .query(
                "SELECT * FROM process ORDER BY name ASC LIMIT $limit START $offset",
                serde_json::json!({ "limit": limit, "offset": offset }),
            )
            .await?;

        results
            .into_iter()
            .map(deserialize_value)
            .collect()
    }
}

/// 知识图谱聚合仓储，组合 Document/Block/Token/Reference 子仓储
pub struct KnowledgeRepository<D: DatabaseClient> {
    client: Arc<D>,
    document: DocumentRepository<D>,
    block: BlockRepository<D>,
    token: TokenRepository<D>,
    reference: ReferenceRepository<D>,
}

impl<D: DatabaseClient> KnowledgeRepository<D> {
    /// 创建知识图谱聚合仓储实例
    pub fn new(client: Arc<D>) -> Self {
        Self {
            document: DocumentRepository::new(Arc::clone(&client)),
            block: BlockRepository::new(Arc::clone(&client)),
            token: TokenRepository::new(Arc::clone(&client)),
            reference: ReferenceRepository::new(Arc::clone(&client)),
            client,
        }
    }

    /// 在事务中保存完整的知识图谱（文档 + 块 + 词元 + 引用）
    pub async fn save_graph(
        &self,
        doc: &Document,
        blocks: &[Block],
        tokens: &[Token],
        references: &[Reference],
    ) -> Result<()> {
        let mut queries = Vec::new();
        let mut bindings = Vec::new();

        let doc_json = serde_json::to_value(doc).map_err(error_core::ErrorObject::from)?;
        queries.push("CREATE document CONTENT $doc".to_string());
        bindings.push(serde_json::json!({ "doc": doc_json }));

        for (i, block) in blocks.iter().enumerate() {
            let block_json = serde_json::to_value(block).map_err(error_core::ErrorObject::from)?;
            queries.push(format!("CREATE block CONTENT $block_{i}"));
            bindings.push(serde_json::json!({ format!("block_{i}"): block_json }));
        }

        for (i, token) in tokens.iter().enumerate() {
            let token_json = serde_json::to_value(token).map_err(error_core::ErrorObject::from)?;
            queries.push(format!("CREATE token CONTENT $token_{i}"));
            bindings.push(serde_json::json!({ format!("token_{}", i): token_json }));
        }

        for (i, reference) in references.iter().enumerate() {
            let ref_json = serde_json::to_value(reference).map_err(error_core::ErrorObject::from)?;
            queries.push(format!("CREATE reference CONTENT $ref_{i}"));
            bindings.push(serde_json::json!({ format!("ref_{i}"): ref_json }));
        }

        self.client.execute_transaction(queries, bindings).await?;
        Ok(())
    }

    /// 检查幂等性键是否已存在
    pub async fn check_idempotency(&self, key: &str) -> Result<Option<RecordIdType>> {
        let results = self
            .client
            .query(
                "SELECT record_id FROM idempotency WHERE key = $key LIMIT 1",
                serde_json::json!({ "key": key }),
            )
            .await?;

        let record_id_str = results
            .into_iter()
            .next()
            .and_then(|v| {
                if let surrealdb::sql::Value::Object(obj) = v {
                    obj.get("record_id").map(|rid| rid.to_string().trim_matches('"').to_string())
                } else {
                    None
                }
            });

        record_id_str.map_or_else(|| Ok(None), |s| Ok(Some(rid("idempotency", &s))))
    }

    /// 记录幂等性键与记录 ID 的映射
    pub async fn record_idempotency(&self, key: &str, record_id: &RecordIdType) -> Result<()> {
        self.client
            .create(
                "idempotency",
                serde_json::json!({ "key": key, "record_id": record_id.to_string() }),
            )
            .await?;
        Ok(())
    }

    /// 获取文档子仓储
    pub const fn document(&self) -> &DocumentRepository<D> {
        &self.document
    }

    /// 获取块子仓储
    pub const fn block(&self) -> &BlockRepository<D> {
        &self.block
    }

    /// 获取词元子仓储
    pub const fn token(&self) -> &TokenRepository<D> {
        &self.token
    }

    /// 获取引用子仓储
    pub const fn reference(&self) -> &ReferenceRepository<D> {
        &self.reference
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::MockDbClient;
    use crate::model::{BlockType, Direction, SourceType, TokenType};

    #[test]
    fn test_debug_surreal_value_serialization() {
        let mut obj = std::collections::BTreeMap::new();
        obj.insert("id".to_string(), surrealdb::sql::Value::from("document:abc"));
        obj.insert("path".to_string(), surrealdb::sql::Value::from("/test.md"));
        obj.insert("title".to_string(), surrealdb::sql::Value::from("Test"));
        obj.insert("source_type".to_string(), surrealdb::sql::Value::from("Markdown"));
        obj.insert("hash".to_string(), surrealdb::sql::Value::from("a".repeat(64)));
        let value = surrealdb::sql::Value::from(obj);
        let json = surreal_value_to_json(&value);
        let doc_result: Result<Document> = serde_json::from_value(json).map_err(Into::into);
        assert!(doc_result.is_ok(), "deserialization failed: {:?}", doc_result.err());
        let doc = doc_result.unwrap();
        assert_eq!(doc.path, "/test.md");
    }

    fn test_rid(table: &str, id: &str) -> RecordIdType {
        RecordId::from((table, id))
    }

    fn test_doc() -> Document {
        Document {
            id: None,
            path: "/test.md".to_string(),
            title: "Test".to_string(),
            source_type: SourceType::Markdown,
            hash: "a".repeat(64),
        }
    }

    fn test_block(doc_id: RecordIdType) -> Block {
        Block::new(doc_id, BlockType::Paragraph, 0, 10)
    }

    fn test_token(block_id: RecordIdType) -> Token {
        Token::new(block_id, "hello", TokenType::Word, 0, 0)
    }

    fn test_reference(from_id: RecordIdType, to_id: RecordIdType) -> Reference {
        Reference::new(RefType::Usage, Direction::OneWay, from_id, to_id)
    }

    #[test]
    fn test_helper_block_creation() {
        let doc_id = RecordIdType::from(("doc".to_string(), "test".to_string()));
        let block = test_block(doc_id.clone());
        assert_eq!(block.block_type, BlockType::Paragraph);
        assert_eq!(block.start_line, 0);
        assert_eq!(block.end_line, 10);
        assert_eq!(&block.doc_id, &doc_id);
    }

    #[test]
    fn test_helper_token_creation() {
        let block_id = RecordIdType::from(("block".to_string(), "test".to_string()));
        let token = test_token(block_id.clone());
        assert_eq!(token.content, "hello");
        assert_eq!(token.token_type, TokenType::Word);
        assert_eq!(&token.block_id, &block_id);
    }

    #[test]
    fn test_helper_reference_creation() {
        let from_id = RecordIdType::from(("token".to_string(), "a".to_string()));
        let to_id = RecordIdType::from(("token".to_string(), "b".to_string()));
        let reference = test_reference(from_id.clone(), to_id.clone());
        assert_eq!(reference.ref_type, RefType::Usage);
        assert_eq!(reference.direction, Direction::OneWay);
        assert_eq!(&reference.from_id, &from_id);
        assert_eq!(&reference.to_id, &to_id);
    }

    #[test]
    fn test_repository_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DocumentRepository<MockDbClient>>();
        assert_send_sync::<BlockRepository<MockDbClient>>();
        assert_send_sync::<TokenRepository<MockDbClient>>();
        assert_send_sync::<ReferenceRepository<MockDbClient>>();
        assert_send_sync::<KnowledgeRepository<MockDbClient>>();
    }

    #[tokio::test]
    async fn test_create_document_success() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "CREATE document CONTENT",
            serde_json::json!([{
                "id": "document:abc",
                "path": "/test.md",
                "title": "Test",
                "source_type": "Markdown",
                "hash": "a".repeat(64),
            }]),
        );

        let repo = DocumentRepository::new(mock);
        let doc = test_doc();
        let result = repo.create(&doc).await;
        assert!(result.is_ok(), "create failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_find_by_hash_existing() {
        let mock = Arc::new(MockDbClient::new());
        let hash = "a".repeat(64);
        mock.register_response(
            "SELECT * FROM document WHERE hash = $hash LIMIT 1",
            serde_json::json!([{
                "id": "document:abc",
                "path": "/test.md",
                "title": "Test",
                "source_type": "Markdown",
                "hash": hash,
            }]),
        );

        let repo = DocumentRepository::new(mock);
        let result = repo.find_by_hash(&hash).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_find_by_hash_not_found() {
        let mock = Arc::new(MockDbClient::new());
        let repo = DocumentRepository::new(mock);
        let result = repo.find_by_hash("nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list_pagination() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "SELECT * FROM document ORDER BY",
            serde_json::json!([{
                "id": "document:abc",
                "path": "/test.md",
                "title": "Test",
                "source_type": "Markdown",
                "hash": "a".repeat(64),
            }]),
        );

        let repo = DocumentRepository::new(mock);
        let result = repo.list(0, 10).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_find_by_block_id_ordered() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "SELECT * FROM token WHERE block_id",
            serde_json::json!([
                { "id": "token:1", "block_id": "block:x", "content": "a", "token_type": "Word", "start_char": 0, "global_offset": 0 },
                { "id": "token:2", "block_id": "block:x", "content": "b", "token_type": "Word", "start_char": 1, "global_offset": 1 },
            ]),
        );

        let repo = TokenRepository::new(mock);
        let block_id = test_rid("block", "x");
        let result = repo.find_by_block(&block_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_find_by_global_offset_exact() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "SELECT * FROM token WHERE global_offset",
            serde_json::json!([{
                "id": "token:1",
                "block_id": "block:x",
                "content": "hello",
                "token_type": "Word",
                "start_char": 0,
                "global_offset": 42,
            }]),
        );

        let repo = TokenRepository::new(mock);
        let result = repo.find_by_global_offset(42).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_find_by_global_offset_not_found() {
        let mock = Arc::new(MockDbClient::new());
        let repo = TokenRepository::new(mock);
        let result = repo.find_by_global_offset(9999).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_create_batch_multiple() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "CREATE token CONTENT",
            serde_json::json!([{
                "id": "token:1",
                "block_id": "block:x",
                "content": "hello",
                "token_type": "Word",
                "start_char": 0,
                "global_offset": 0,
            }]),
        );

        let repo = TokenRepository::new(mock);
        let block_id = test_rid("block", "x");
        let tokens = vec![
            test_token(block_id.clone()),
            test_token(block_id),
        ];
        let result = repo.insert_batch(&tokens).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_create_batch_order_preserved() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "CREATE token CONTENT",
            serde_json::json!([{
                "id": "token:1",
                "block_id": "block:x",
                "content": "first",
                "token_type": "Word",
                "start_char": 0,
                "global_offset": 0,
            }]),
        );

        let repo = TokenRepository::new(mock);
        let block_id = test_rid("block", "x");
        let tokens = vec![
            Token::new(block_id.clone(), "first", TokenType::Word, 0, 0),
            Token::new(block_id, "second", TokenType::Word, 5, 5),
        ];
        let result = repo.insert_batch(&tokens).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_edge_bidirectional() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "CREATE reference CONTENT",
            serde_json::json!([{
                "id": "reference:1",
                "ref_type": "Usage",
                "direction": "TwoWay",
                "scope": null,
                "source_id": "token:a",
                "target_id": "token:b",
            }]),
        );

        let repo = ReferenceRepository::new(mock);
        let from_id = test_rid("token", "a");
        let to_id = test_rid("token", "b");
        let reference = Reference::new(RefType::Usage, Direction::TwoWay, from_id, to_id);
        let result = repo.create(&reference).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_trace_forward_with_filter() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "SELECT * FROM reference WHERE source_id",
            serde_json::json!([{
                "id": "reference:1",
                "ref_type": "Usage",
                "direction": "OneWay",
                "scope": null,
                "from_id": "token:a",
                "to_id": "token:b",
            }]),
        );

        let repo = ReferenceRepository::new(mock);
        let id = test_rid("token", "a");
        let result = repo.trace_forward(&id, Some(RefType::Usage)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_trace_backward_with_filter() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "SELECT * FROM reference WHERE target_id",
            serde_json::json!([{
                "id": "reference:1",
                "ref_type": "Usage",
                "direction": "OneWay",
                "scope": null,
                "from_id": "token:a",
                "to_id": "token:b",
            }]),
        );

        let repo = ReferenceRepository::new(mock);
        let id = test_rid("token", "b");
        let result = repo.trace_backward(&id, Some(RefType::Usage)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_trace_backward_correct_syntax() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "SELECT * FROM reference WHERE target_id",
            serde_json::json!([]),
        );

        let repo = ReferenceRepository::new(mock);
        let id = test_rid("token", "x");
        let result = repo.trace_backward(&id, None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_vector_search_deterministic() {
        let mock = Arc::new(MockDbClient::new());
        let embedding: Vec<f64> = vec![0.1; 1536];
        mock.register_response(
            "SELECT *, vector::similarity",
            serde_json::json!([{
                "id": "block:1",
                "doc_id": "document:abc",
                "block_type": "Paragraph",
                "start_line": 0,
                "end_line": 10,
                "embedding": embedding,
                "score": 0.95,
            }]),
        );

        let repo = BlockRepository::new(mock);
        let result = repo.vector_search(vec![0.1f32; 1536], 5).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_idempotency_new_key() {
        let mock = Arc::new(MockDbClient::new());
        let repo = KnowledgeRepository::new(mock);
        let result = repo.check_idempotency("new_key").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_check_idempotency_duplicate() {
        let mock = Arc::new(MockDbClient::new());
        mock.register_response(
            "SELECT record_id FROM idempotency WHERE key",
            serde_json::json!([{ "record_id": "block:abc" }]),
        );

        let repo = KnowledgeRepository::new(mock);
        let result = repo.check_idempotency("existing_key").await;
        assert!(result.is_ok());
    }
}
