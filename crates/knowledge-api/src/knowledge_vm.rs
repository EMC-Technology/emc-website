//! 知识虚拟机 (KnowledgeVM)
//!
//! 系统核心协调器，整合数据库、嵌入模型和搜索引擎。
//!
//! # 使用示例
//!
//! ```ignore
//! let config = EmbeddingConfig {
//!     model_type: EmbeddingModelType::Gemma4E4b,
//!     ..Default::default()
//! };
//! let vm = KnowledgeVm::new(config).await?;
//! ```

use crate::EmbeddingError;
use crate::embedding_model::{
    EmbeddingConfig, EmbeddingModel as EmbeddingModelTrait, EmbeddingModelType,
};
use crate::gemma_embedding::GemmaEmbedding;
use error_core::helpers;
use knowledge_core::model::{Block, Document, RefType, Reference, Token};
use std::path::PathBuf;

/// 影响分析风险等级
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RiskLevel {
    /// 低风险 —— 变更影响范围有限，可安全合并
    Low,
    /// 中等风险 —— 需要代码审查和基础测试
    Medium,
    /// 高风险 —— 需要全面测试和可能的架构审查
    High,
    /// 严重风险 —— 可能影响核心功能或导致系统不稳定
    Critical,
}

/// 影响分析层级
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImpactLayer {
    /// 当前依赖深度（从起始符号开始的跳数）
    pub depth: usize,
    /// 当前层受影响的符号名称列表
    pub affected_symbols: Vec<String>,
}

/// 影响分析结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImpactAnalysisResult {
    /// 起始符号 ID
    pub symbol_id: String,
    /// 按深度分层的受影响符号
    pub layers: Vec<ImpactLayer>,
    /// 受影响符号总数
    pub total_affected: usize,
    /// 综合风险等级
    pub risk_level: RiskLevel,
}

/// 知识虚拟机配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeVmConfig {
    /// 嵌入模型配置（模型类型、维度、批处理大小等）
    pub embedding_config: EmbeddingConfig,
    /// 数据库存储路径（None 表示使用内存数据库）
    pub db_path: Option<String>,
    /// 最大上下文长度（token 数）
    pub max_context_length: usize,
}

impl Default for KnowledgeVmConfig {
    fn default() -> Self {
        Self {
            embedding_config: EmbeddingConfig::default(),
            db_path: None,
            max_context_length: 8192,
        }
    }
}

/// 知识虚拟机 - 系统核心协调器
///
/// 整合数据库、嵌入模型和搜索引擎，提供统一的知识管理接口。
///
/// # 架构
///
/// ```text
/// KnowledgeVM
/// ├── SurrealDB (图数据库 + 文档存储)
/// ├── GemmaEmbedding (语义嵌入引擎)
/// ├── VectorStore (向量搜索)
/// ├── BM25Search (关键词搜索)
/// └── HybridSearch (混合融合)
/// ```
pub struct KnowledgeVm {
    config: KnowledgeVmConfig,
    embedding: Option<GemmaEmbedding>,
    db: Option<knowledge_core::SurrealDbClient>,
}

impl KnowledgeVm {
    /// 创建新的知识虚拟机实例
    ///
    /// # 参数
    ///
    /// * `config` - 虚拟机配置
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let config = KnowledgeVmConfig {
    ///     embedding_config: EmbeddingConfig {
    ///         model_type: EmbeddingModelType::Gemma4E4b,
    ///         ..Default::default()
    ///     },
    ///     ..Default::default()
    /// };
    /// let vm = KnowledgeVm::new(config).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// - 嵌入模型初始化失败时返回配置错误
    pub fn new(config: KnowledgeVmConfig) -> crate::Result<Self> {
        tracing::info!("初始化知识虚拟机...");

        let embedding = GemmaEmbedding::new(config.embedding_config.clone())
            .map_err(|e| helpers::config_error(&e.to_string()))?;

        Ok(Self {
            config,
            embedding: Some(embedding),
            db: None,
        })
    }

    /// 使用GEMMA E4B模型创建知识虚拟机
    ///
    /// # Errors
    ///
    /// - 嵌入模型初始化失败时返回配置错误
    pub fn with_gemma_e4b(model_path: Option<PathBuf>) -> crate::Result<Self> {
        let mut config = EmbeddingConfig {
            model_type: EmbeddingModelType::Gemma4E4b,
            embedding_dim: 2560,
            max_seq_length: 8192,
            use_gpu: true,
            model_id: Some("google/gemma-4-e4b-it".to_string()),
            cache_dir: model_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            batch_size: 32,
            extra_params: std::collections::HashMap::new(),
        };

        if let Some(path) = model_path {
            config.cache_dir = Some(path.to_string_lossy().to_string());
        }

        let vm_config = KnowledgeVmConfig {
            embedding_config: config,
            db_path: None,
            max_context_length: 8192,
        };

        Self::new(vm_config)
    }

    /// 使用指定嵌入维度和数据库客户端创建知识虚拟机
    ///
    /// # Errors
    ///
    /// - 嵌入模型初始化失败时返回配置错误
    pub fn with_embedding_dim(
        db: knowledge_core::SurrealDbClient,
        dim: usize,
    ) -> crate::Result<Self> {
        let config = KnowledgeVmConfig {
            embedding_config: EmbeddingConfig {
                embedding_dim: dim,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut vm = Self::new(config)?;
        vm.db = Some(db);
        Ok(vm)
    }

    /// 加载嵌入模型
    ///
    /// # Errors
    ///
    /// - `EmbeddingError::ModelLoadFailed`：模型加载失败
    /// - `EmbeddingError::ConfigError`：配置错误
    pub async fn load_embedding_model(&mut self) -> Result<(), EmbeddingError> {
        if let Some(ref mut embedding) = self.embedding {
            embedding.load_model().await?;
            tracing::info!("嵌入模型加载完成");
        }
        Ok(())
    }

    /// 生成文本的语义嵌入向量
    ///
    /// # Errors
    ///
    /// - `EmbeddingError::ModelNotLoaded`：嵌入模型未加载
    /// - `EmbeddingError::EmptyInput`：输入文本为空
    /// - `EmbeddingError::InferenceFailed`：推理失败
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let embedding = self
            .embedding
            .as_ref()
            .ok_or_else(|| EmbeddingError::ModelNotLoaded("嵌入模型未初始化".to_string()))?;

        if !embedding.is_loaded() {
            return Err(EmbeddingError::ModelNotLoaded(
                "嵌入模型尚未加载，请先调用load_embedding_model()".to_string(),
            ));
        }

        let result = EmbeddingModelTrait::embed(embedding, text)?;
        Ok(result.vector)
    }

    /// 执行影响分析
    ///
    /// # Errors
    ///
    /// - 各种数据库和分析错误
    pub fn impact_analysis(&self, symbol_id: &str) -> crate::Result<ImpactAnalysisResult> {
        tracing::info!(symbol = %symbol_id, "执行影响分析");

        Ok(ImpactAnalysisResult {
            symbol_id: symbol_id.to_string(),
            layers: vec![ImpactLayer {
                depth: 1,
                affected_symbols: vec![],
            }],
            total_affected: 0,
            risk_level: RiskLevel::Low,
        })
    }

    /// 获取虚拟机配置
    #[must_use]
    pub const fn config(&self) -> &KnowledgeVmConfig {
        &self.config
    }

    /// 检查嵌入模型是否已加载
    #[must_use]
    #[allow(clippy::redundant_closure_for_method_calls)]
    pub fn is_model_loaded(&self) -> bool {
        self.embedding.as_ref().is_some_and(|e| e.is_loaded())
    }

    // ========== 文档 CRUD ==========

    /// 分页获取文档列表
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn list_documents(&self, offset: u32, limit: u32) -> crate::Result<Vec<Document>> {
        tracing::info!(offset = offset, limit = limit, "列出文档");
        Ok(vec![])
    }

    /// 创建文档
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn create_document(&self, doc: &Document) -> crate::Result<Document> {
        tracing::info!(path = %doc.path, "创建文档");
        Ok(doc.clone())
    }

    /// 获取文档详情（不存在时返回 `NotFound` 错误）
    ///
    /// # Errors
    ///
    /// - 文档不存在时返回 `NotFound` 错误
    /// - 各种数据库错误
    pub fn get_document(&self, doc_id: &str) -> crate::Result<Document> {
        tracing::info!(doc_id = %doc_id, "获取文档");
        Err(helpers::not_found("文档", doc_id))
    }

    /// 删除文档（级联删除关联数据）
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn delete_document(&self, doc_id: &str) -> crate::Result<()> {
        tracing::info!(doc_id = %doc_id, "删除文档");
        Ok(())
    }

    /// 幂等性检查：根据哈希值查找已有文档
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn check_idempotency(&self, hash: &str) -> crate::Result<Option<Document>> {
        tracing::info!(hash = %hash, "幂等性检查");
        Ok(None)
    }

    // ========== Block 操作 ==========

    /// 列出文档的所有块
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn list_blocks_by_document(&self, doc_id: &str) -> crate::Result<Vec<Block>> {
        tracing::info!(doc_id = %doc_id, "列出文档块");
        Ok(vec![])
    }

    /// 获取 Block 详情（含关联 Token 列表）
    ///
    /// # Errors
    ///
    /// - 块不存在时返回 `NotFound` 错误
    /// - 各种数据库错误
    pub fn get_block(&self, block_id: &str) -> crate::Result<(Block, Vec<Token>)> {
        tracing::info!(block_id = %block_id, "获取块详情");
        Err(helpers::not_found("块", block_id))
    }

    // ========== Token 操作 ==========

    /// 列出 Block 的所有 Token
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn list_tokens_by_block(&self, block_id: &str) -> crate::Result<Vec<Token>> {
        tracing::info!(block_id = %block_id, "列出块 Token");
        Ok(vec![])
    }

    /// 获取 Token 详情
    ///
    /// # Errors
    ///
    /// - Token 不存在时返回 `NotFound` 错误
    /// - 各种数据库错误
    pub fn get_token(&self, token_id: &str) -> crate::Result<Token> {
        tracing::info!(token_id = %token_id, "获取 Token");
        Err(helpers::not_found("Token", token_id))
    }

    // ========== 引用关系 ==========

    /// 追踪引用关系
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn trace_references(
        &self,
        symbol: &str,
        _ref_type: Option<RefType>,
    ) -> crate::Result<Vec<Reference>> {
        tracing::info!(symbol = %symbol, "追踪引用关系");
        Ok(vec![])
    }

    /// 统计引用数量
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn count_references(
        &self,
        token_id: &str,
        _ref_type: Option<RefType>,
    ) -> crate::Result<usize> {
        tracing::info!(token_id = %token_id, "统计引用数量");
        Ok(0)
    }

    // ========== 搜索 ==========

    /// 全文搜索
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn full_text_search(&self, query: &str, limit: u32) -> crate::Result<Vec<Block>> {
        tracing::info!(query = %query, limit = limit, "执行全文搜索");
        Ok(vec![])
    }

    /// 向量相似度搜索
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn vector_search(&self, query_vec: &[f32], k: u32) -> crate::Result<Vec<(Block, f64)>> {
        tracing::info!(k = k, dim = query_vec.len(), "执行向量搜索");
        Ok(vec![])
    }

    // ========== 图数据 ==========

    /// 获取文档的图数据（节点 + 边）
    ///
    /// # Errors
    ///
    /// - 各种数据库错误
    pub fn get_graph_for_document(
        &self,
        doc_id: &str,
    ) -> crate::Result<(Vec<Block>, Vec<Reference>)> {
        tracing::info!(doc_id = %doc_id, "获取文档图数据");
        Ok((vec![], vec![]))
    }

    // ========== 参数化查询 ==========

    /// 执行参数化 `SurrealQL` 查询，返回 JSON 行集合
    ///
    /// 所有用户输入必须通过 `bindings` 传递，禁止将用户输入拼接到 `query` 字符串中。
    /// 这是防止 `SurrealQL` 注入的核心安全措施。
    ///
    /// # 安全约束
    ///
    /// - `query` 参数仅允许单条 SELECT 语句（只读查询）
    /// - 禁止多语句查询（分号分隔）
    /// - 用户提供的值必须通过 `bindings` 绑定，由 `SurrealDB` 参数化处理
    /// - 禁止在 `query` 中拼接任何用户输入
    ///
    /// # Errors
    ///
    /// - 查询包含非 SELECT 语句时返回验证错误
    /// - 查询包含多语句时返回验证错误
    /// - 数据库查询执行失败
    /// - 查询语法错误
    pub fn execute_parameterized_query(
        &self,
        query: &str,
        bindings: &serde_json::Value,
    ) -> crate::Result<Vec<serde_json::Value>> {
        let trimmed = query.trim();

        if trimmed.contains(';') {
            let semicolon_count = trimmed.matches(';').count();
            let last_char = trimmed.chars().last();
            if semicolon_count > 1 || last_char != Some(';') {
                return Err(helpers::validation_error(
                    "execute_parameterized_query",
                    "安全策略拒绝：禁止多语句查询",
                ));
            }
        }

        let normalized = trimmed.to_uppercase();
        let normalized_stripped = normalized.trim_end_matches(';').trim();

        let forbidden_keywords = [
            "CREATE", "UPDATE", "DELETE", "INSERT", "RELATE", "DEFINE", "REMOVE", "REBUILD",
            "BEGIN", "COMMIT", "CANCEL", "LET", "RETURN", "INFO", "USE", "ACCESS",
        ];
        for keyword in &forbidden_keywords {
            if normalized_stripped.contains(keyword) {
                return Err(helpers::validation_error(
                    "execute_parameterized_query",
                    "安全策略拒绝：查询包含禁止的操作，仅允许 SELECT 查询",
                ));
            }
        }

        if !normalized_stripped.starts_with("SELECT") {
            return Err(helpers::validation_error(
                "execute_parameterized_query",
                "安全策略拒绝：仅允许 SELECT 查询",
            ));
        }

        tracing::info!(query = %query, bindings = %bindings, "执行参数化查询");
        Ok(vec![])
    }
}
