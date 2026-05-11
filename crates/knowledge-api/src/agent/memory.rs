//! 记忆系统
//!
//! 实现 Agent 的三层记忆架构：

#![allow(clippy::significant_drop_tightening)]
//! - 短期工作记忆（滑动窗口）
//! - 长期记忆（向量存储）
//! - 事件记忆（任务历史）

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use crate::agent::react_agent::ActionType;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

// ============================================================================
// 记忆条目与类型
// ============================================================================

/// 记忆条目
///
/// 记忆系统中的基本单元，包含内容、元数据和访问统计。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// 唯一标识符
    pub id: Uuid,
    /// 记忆类型
    pub entry_type: MemoryType,
    /// 内容文本
    pub content: String,
    /// 向量嵌入（可选，用于语义搜索）
    pub embedding: Option<Vec<f32>>,
    /// 重要性评分（0.0 - 1.0），用于淘汰策略
    pub importance: f64,
    /// 创建时间
    pub created_at: chrono::DateTime<Utc>,
    /// 被访问次数
    pub access_count: u64,
    /// 附加元数据
    pub metadata: serde_json::Value,
}

impl MemoryEntry {
    /// 创建新的记忆条目
    pub fn new(entry_type: MemoryType, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            entry_type,
            content: content.into(),
            embedding: None,
            importance: 0.5,
            created_at: Utc::now(),
            access_count: 0,
            metadata: serde_json::json!({}),
        }
    }

    /// 设置重要性
    #[must_use]
    pub const fn with_importance(mut self, importance: f64) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }

    /// 添加元数据
    #[must_use]
    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metadata[key] = value;
        self
    }
}

/// 记忆类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryType {
    /// 工具观察结果
    Observation,
    /// 思考过程
    Thought,
    /// 事实信息
    Fact,
    /// 用户输入
    UserInput,
    /// 反思/总结
    Reflection,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Observation => write!(f, "Observation"),
            Self::Thought => write!(f, "Thought"),
            Self::Fact => write!(f, "Fact"),
            Self::UserInput => write!(f, "UserInput"),
            Self::Reflection => write!(f, "Reflection"),
        }
    }
}

/// 压缩策略
#[derive(Debug, Clone, Copy)]
pub enum CompressionStrategy {
    /// 基于重要性的淘汰（保留重要的）
    ImportanceBased,
    /// 基于时间的淘汰（保留最近的）
    TimeBased,
    /// 基于 LRU（最近最少使用）
    LRU,
}

/// 遗忘条件
#[derive(Debug, Clone, Default)]
pub struct ForgetCriteria {
    /// 最大年龄（秒），超过此时间的记忆可被遗忘
    pub max_age_seconds: Option<u64>,
    /// 最小重要性分数，低于此值的记忆可被遗忘
    pub min_importance: Option<f64>,
    /// 最大条目数，超出时淘汰最不重要的
    pub max_entries: Option<usize>,
    /// 只遗忘特定类型
    pub memory_types: Option<Vec<MemoryType>>,
}

// ============================================================================
// 短期工作记忆
// ============================================================================

/// 短期工作记忆（滑动窗口）
///
/// 维护当前对话或任务执行过程中的临时信息。
/// 使用固定大小的滑动窗口，当达到上限时自动压缩。
///
/// # 特性
///
/// - 固定容量（基于条目数和 token 数双重限制）
/// - 支持关键词搜索
/// - 自动压缩机制
/// - 导出为 LLM 上下文格式
pub struct WorkingMemory {
    /// 记忆队列（按时间排序）
    window: VecDeque<MemoryEntry>,
    /// 最大条目数
    max_entries: usize,
    /// 最大 token 数（估算值）
    max_tokens: usize,
    /// 当前 token 数估算
    current_tokens: usize,
}

impl Default for WorkingMemory {
    fn default() -> Self {
        Self::with_capacity(100, 8000)
    }
}

impl WorkingMemory {
    /// 创建指定容量的工作记忆
    ///
    /// # Arguments
    ///
    /// * `max_entries` - 最大条目数
    /// * `max_tokens` - 最大 token 数估算
    #[must_use]
    pub fn with_capacity(max_entries: usize, max_tokens: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(max_entries),
            max_entries,
            max_tokens,
            current_tokens: 0,
        }
    }

    /// 添加记忆条目
    ///
    /// 如果已达到容量上限，会先进行压缩。
    ///
    /// # Arguments
    ///
    /// * `entry` - 要添加的记忆条目
    ///
    /// # Errors
    ///
    /// 压缩过程可能返回错误。
    /// # Errors
    ///
    /// 压缩过程可能返回错误。
    pub fn add(&mut self, entry: MemoryEntry) -> crate::Result<()> {
        let estimated_tokens = self.estimate_tokens(&entry.content);

        // 检查是否需要压缩
        if self.window.len() >= self.max_entries
            || self.current_tokens + estimated_tokens > self.max_tokens
        {
            debug!(
                entries = self.window.len(),
                tokens = self.current_tokens,
                "工作记忆接近上限，开始压缩"
            );
            self.compress(CompressionStrategy::ImportanceBased)?;
        }

        // 添加新条目
        self.window.push_back(entry);
        self.current_tokens += estimated_tokens;

        Ok(())
    }

    /// 获取最近 N 条记忆
    ///
    /// # Arguments
    ///
    /// * `n` - 要获取的条目数
    ///
    /// # Returns
    ///
    /// 返回最新的 n 条记忆（从旧到新排序）
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<MemoryEntry> {
        let start = if self.window.len() > n {
            self.window.len() - n
        } else {
            0
        };

        self.window.range(start..).cloned().collect()
    }

    /// 获取所有记忆
    #[must_use]
    pub fn all(&self) -> Vec<MemoryEntry> {
        self.window.iter().cloned().collect()
    }

    /// 搜索相关记忆（关键词匹配）
    ///
    /// 在所有记忆内容中搜索包含指定查询的条目。
    ///
    /// # Arguments
    ///
    /// * `query` - 搜索关键词
    ///
    /// # Returns
    ///
    /// 返回匹配的记忆列表（按时间倒序）
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<MemoryEntry> {
        let query_lower = query.to_lowercase();

        self.window
            .iter()
            .filter(|entry| entry.content.to_lowercase().contains(&query_lower))
            .cloned()
            .collect()
    }

    /// 压缩记忆
    ///
    /// 当接近容量上限时调用，根据策略淘汰低价值记忆。
    ///
    /// # Arguments
    ///
    /// * `strategy` - 压缩策略
    ///
    /// # Returns
    ///
    /// 返回被移除的条目数量
    ///
    /// # Errors
    ///
    /// 当前实现不会返回错误，保留 `Result` 以兼容未来扩展。
    ///
    /// # Panics
    ///
    /// 记忆重要性比较中 `partial_cmp` 返回 `None` 时使用 `Ordering::Equal`。
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    pub fn compress(&mut self, strategy: CompressionStrategy) -> crate::Result<usize> {
        let original_len = self.window.len();

        // 目标：释放约 30% 的空间
        let target_size = (self.max_entries as f64 * 0.7) as usize;

        match strategy {
            CompressionStrategy::ImportanceBased => {
                // 将 VecDeque 转为 vec 排序
                let mut entries: Vec<_> = self.window.drain(..).collect();

                // 按重要性升序排序（淘汰不重要的）
                entries.sort_by(|a, b| a.importance.total_cmp(&b.importance));

                // 保留最重要的 target_size 条
                entries.reverse();
                entries.truncate(target_size);

                // 重新放回 deque（保持时间顺序）
                entries.sort_by_key(|a| a.created_at);
                self.window = entries.into_iter().collect();
            }
            CompressionStrategy::TimeBased => {
                // 直接截断到目标大小（保留最新的）
                while self.window.len() > target_size {
                    self.window.pop_front();
                }
            }
            CompressionStrategy::LRU => {
                // 按访问次数排序，淘汰最少访问的
                let mut entries: Vec<_> = self.window.drain(..).collect();
                entries.sort_by_key(|e| e.access_count);
                entries.truncate(target_size);
                entries.sort_by_key(|a| a.created_at);
                self.window = entries.into_iter().collect();
            }
        }

        // 重新计算 token 数
        self.current_tokens = 0;
        for entry in &self.window {
            self.current_tokens += self.estimate_tokens(&entry.content);
        }

        let removed = original_len.saturating_sub(self.window.len());
        info!(
            strategy = ?strategy,
            removed,
            remaining = self.window.len(),
            "记忆压缩完成"
        );

        Ok(removed)
    }

    /// 导出为 LLM 上下文格式
    ///
    /// 将记忆转换为适合发送给 LLM 的文本格式。
    ///
    /// # Arguments
    ///
    /// * `max_tokens` - 最大 token 数限制
    ///
    /// # Returns
    ///
    /// 返回格式化的上下文字符串
    #[must_use]
    pub fn to_llm_context(&self, max_tokens: usize) -> String {
        let mut context = String::from("## 工作记忆\n\n");
        let mut tokens_used = 0;

        for entry in &self.window {
            let entry_text = format!(
                "- [{}] {}: {}\n",
                entry.entry_type,
                entry.created_at.format("%H:%M:%S"),
                entry.content
            );
            let entry_tokens = self.estimate_tokens(&entry_text);

            if tokens_used + entry_tokens > max_tokens {
                context.push_str("... (更多记忆被省略)\n");
                break;
            }

            context.push_str(&entry_text);
            tokens_used += entry_tokens;
        }

        context
    }

    /// 清空所有记忆
    pub fn clear(&mut self) {
        self.window.clear();
        self.current_tokens = 0;
    }

    /// 获取当前条目数
    #[must_use]
    pub fn len(&self) -> usize {
        self.window.len()
    }

    /// 是否为空
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.window.is_empty()
    }

    /// 估算文本的 token 数（粗略估算：每 4 个字符约 1 个 token）
    #[allow(clippy::unused_self)]
    fn estimate_tokens(&self, text: &str) -> usize {
        (text.len() / 4).max(1)
    }
}

// ============================================================================
// 长期记忆
// ============================================================================

/// 向量存储 trait（抽象接口）
#[async_trait::async_trait]
pub trait VectorStore: Send + Sync {
    /// 存储向量及其关联数据
    async fn store(
        &self,
        id: &Uuid,
        vector: &[f32],
        payload: &serde_json::Value,
    ) -> crate::Result<()>;

    /// 语义相似度搜索
    async fn search(&self, query_vector: &[f32], top_k: usize) -> crate::Result<Vec<(Uuid, f64)>>;

    /// 删除条目
    async fn delete(&self, id: &Uuid) -> crate::Result<()>;
}

/// 摘要器 trait（用于反思和整合）
#[async_trait::async_trait]
pub trait Summarizer: Send + Sync {
    /// 对一组记忆条目生成摘要
    async fn summarize(&self, entries: &[MemoryEntry]) -> crate::Result<String>;
}

/// 长期记忆（基于向量数据库）
///
/// 提供持久化的语义记忆存储和检索能力。
/// 通过向量嵌入实现语义相似度搜索。
pub struct LongTermMemory {
    /// 向量存储后端
    store: Arc<dyn VectorStore>,
    /// 集合名称（命名空间）
    collection: String,
    /// 可选的摘要器（用于定期整合）
    summarizer: Option<Arc<dyn Summarizer>>,
    /// 内存索引（加速查找）
    index: RwLock<HashMap<Uuid, MemoryEntry>>,
}

impl LongTermMemory {
    /// 创建新的长期记忆实例
    pub fn new(store: Arc<dyn VectorStore>, collection: impl Into<String>) -> Self {
        Self {
            store,
            collection: collection.into(),
            summarizer: None,
            index: RwLock::new(HashMap::new()),
        }
    }

    /// 设置摘要器
    #[must_use]
    pub fn with_summarizer(mut self, summarizer: Arc<dyn Summarizer>) -> Self {
        self.summarizer = Some(summarizer);
        self
    }

    /// 存储长期记忆
    ///
    /// # Arguments
    ///
    /// * `entry` - 要存储的记忆条目
    ///
    /// # Returns
    ///
    /// 返回存储后的条目 ID
    ///
    /// # Errors
    ///
    /// 向量存储失败时返回错误。
    pub async fn store(&self, entry: &MemoryEntry) -> crate::Result<Uuid> {
        let id = entry.id;

        // 如果有嵌入向量则使用，否则使用零向量（实际应用中应计算嵌入）
        let vector = entry.embedding.clone().unwrap_or_else(|| vec![0.0; 768]);

        // 构建负载
        let payload = serde_json::json!({
            "content": entry.content,
            "entry_type": entry.entry_type.to_string(),
            "importance": entry.importance,
            "created_at": entry.created_at.to_rfc3339(),
            "metadata": entry.metadata,
        });

        // 存储到向量数据库
        self.store
            .store(&id, &vector, &payload)
            .await
            .map_err(|e| error_core::helpers::agent_memory_error(&format!("向量存储失败: {e}")))?;

        // 更新内存索引
        {
            let mut index = self.index.write().await;
            index.insert(id, entry.clone());
        }

        info!(id = %id, collection = %self.collection, "长期记忆已存储");

        Ok(id)
    }

    /// 检索相关记忆（语义搜索）
    ///
    /// 根据查询向量返回最相关的记忆条目。
    ///
    /// # Arguments
    ///
    /// * `query` - 查询文本（将被向量化）
    /// * `top_k` - 返回的最大结果数
    ///
    /// # Returns
    ///
    /// 返回相似度排序的记忆列表
    pub fn recall(&self, query: &str, top_k: usize) -> Vec<MemoryEntry> {
        debug!(query, top_k, "检索长期记忆");

        let Ok(index) = self.index.try_read() else {
            warn!("长期记忆索引锁获取失败");
            return Vec::new();
        };

        let query_lower = query.to_lowercase();
        let mut results: Vec<MemoryEntry> = index
            .values()
            .filter(|entry| entry.content.to_lowercase().contains(&query_lower))
            .cloned()
            .collect();

        results.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);

        results
    }

    /// 遗忘（删除过期或不重要的记忆）
    ///
    /// # Arguments
    ///
    /// * `criteria` - 遗忘条件
    ///
    /// # Returns
    ///
    /// 返回被删除的条目数量
    ///
    /// # Errors
    ///
    /// 删除操作失败时返回错误。
    #[allow(clippy::cast_possible_wrap)]
    pub async fn forget(&self, criteria: &ForgetCriteria) -> crate::Result<u64> {
        let mut deleted_count = 0u64;
        let now = Utc::now();

        let mut to_delete = Vec::new();

        {
            let index = self.index.read().await;
            for (id, entry) in index.iter() {
                let should_forget =
                    // 检查年龄
                    criteria.max_age_seconds.is_some_and(|max_age| {
                        let age = now.signed_duration_since(entry.created_at);
                        age.num_seconds() > max_age as i64
                    })
                    // 检查重要性
                    || criteria.min_importance.is_some_and(|min_imp| {
                        entry.importance < min_imp
                    })
                    // 检查类型
                    || criteria.memory_types.as_ref().is_some_and(|types| {
                        !types.contains(&entry.entry_type)
                    });

                if should_forget {
                    to_delete.push(*id);
                }
            }
        }

        // 执行删除
        for id in to_delete {
            if self.store.delete(&id).await.is_ok() {
                let mut index = self.index.write().await;
                index.remove(&id);
                deleted_count += 1;
            }
        }

        if deleted_count > 0 {
            info!(count = deleted_count, "长期记忆清理完成");
        }

        Ok(deleted_count)
    }

    /// 反思与整合（定期总结零散记忆）
    ///
    /// 分析当前存储的记忆，提取关键模式和洞察，
    /// 生成更高层次的摘要性记忆。
    ///
    /// # Returns
    ///
    /// 返回新生成的摘要记忆条目
    ///
    /// # Errors
    ///
    /// 摘要生成失败时返回错误。
    pub async fn reflect(&self) -> crate::Result<Vec<MemoryEntry>> {
        let Some(summarizer) = &self.summarizer else {
            debug!("未配置摘要器，跳过反思");
            return Ok(Vec::new());
        };

        // 获取最近的观察和思考
        let recent_entries = {
            let index = self.index.read().await;
            let mut entries: Vec<_> = index.values().cloned().collect();

            // 按时间倒序排序，取最近 50 条
            entries.sort_by_key(|b| std::cmp::Reverse(b.created_at));
            entries.truncate(50);
            entries
        };

        if recent_entries.is_empty() {
            return Ok(Vec::new());
        }

        // 使用摘要器生成总结
        let summary = summarizer
            .summarize(&recent_entries)
            .await
            .map_err(|e| error_core::helpers::agent_memory_error(&format!("摘要生成失败: {e}")))?;

        // 创建新的反思记忆
        let reflection_entry = MemoryEntry::new(MemoryType::Reflection, summary)
            .with_importance(0.9)
            .with_metadata("source", serde_json::json!("auto-reflection"));

        // 存储反思结果
        let _id = self.store(&reflection_entry).await?;

        info!("反思完成，生成新的摘要记忆");

        Ok(vec![reflection_entry])
    }

    /// 获取存储的条目总数
    pub async fn count(&self) -> usize {
        self.index.read().await.len()
    }
}

// ============================================================================
// 事件记忆
// ============================================================================

/// 事件存储 trait
#[async_trait::async_trait]
pub trait EpisodeStore: Send + Sync {
    /// 存储一个完整的事件记录
    async fn save_episode(&self, episode: &Episode) -> crate::Result<Uuid>;

    /// 根据 ID 获取事件
    async fn get_episode(&self, id: &Uuid) -> crate::Result<Option<Episode>>;

    /// 搜索相似的事件
    async fn find_similar(&self, query: &str, limit: usize) -> crate::Result<Vec<Episode>>;

    /// 获取最近的事件
    async fn list_recent(&self, limit: usize) -> crate::Result<Vec<Episode>>;
}

/// 任务结果类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskOutcome {
    /// 成功完成
    Success,
    /// 失败
    Failure(String),
    /// 部分成功 —— 子任务部分完成，返回成功率
    PartialSuccess {
        /// 成功完成的子任务比例 (0.0 - 1.0)
        success_rate: f64,
    },
}

/// 完整的事件记录（一次完整的任务执行过程）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    /// 事件 ID
    pub id: Uuid,
    /// 任务描述
    pub task_description: String,
    /// 执行结果
    pub outcome: TaskOutcome,
    /// 完整的 `ReAct` 步骤序列
    pub steps: Vec<super::ReActStep>,
    /// 学到的经验教训
    pub lessons_learned: Vec<String>,
    /// 总耗时（毫秒）
    pub duration_ms: u64,
    /// 创建时间
    pub created_at: chrono::DateTime<Utc>,
}

/// 执行模式（从多个成功案例中提取）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPattern {
    /// 模式 ID
    pub id: Uuid,
    /// 模式描述
    pub description: String,
    /// 典型的步骤序列
    pub typical_steps: Vec<String>,
    /// 成功率
    pub success_rate: f64,
    /// 平均耗时（毫秒）
    pub avg_duration_ms: u64,
    /// 关键工具使用模式
    pub tool_usage_patterns: HashMap<String, u32>,
}

/// 事件记忆（任务执行历史）
///
/// 记录完整的任务执行过程，支持经验回顾和模式提取。
pub struct EpisodicMemory {
    /// 事件存储后端
    store: Box<dyn EpisodeStore>,
}

impl EpisodicMemory {
    /// 创建新的事件记忆实例
    #[must_use]
    pub fn new(store: Box<dyn EpisodeStore>) -> Self {
        Self { store }
    }

    /// 记录一个完整的任务执行过程
    ///
    /// # Arguments
    ///
    /// * `episode` - 要记录的事件
    ///
    /// # Returns
    ///
    /// 返回保存后的事件 ID
    ///
    /// # Errors
    ///
    /// 保存事件失败时返回错误。
    pub async fn record_episode(&self, episode: &Episode) -> crate::Result<Uuid> {
        let id =
            self.store.save_episode(episode).await.map_err(|e| {
                error_core::helpers::agent_memory_error(&format!("保存事件失败: {e}"))
            })?;

        info!(
            episode_id = %id,
            task = %episode.task_description,
            outcome = ?episode.outcome,
            "事件已记录"
        );

        Ok(id)
    }

    /// 回顾类似任务的执行经验
    ///
    /// 当面临新任务时，可以回顾历史上类似任务的执行方式，
    /// 从中学习成功的经验和失败的教训。
    ///
    /// # Arguments
    ///
    /// * `task_description` - 当前任务的描述
    ///
    /// # Returns
    ///
    /// 返回相似的历史事件列表
    pub async fn recall_similar(&self, task_description: &str) -> Vec<Episode> {
        debug!(task = %task_description, "回顾类似事件");

        match self.store.find_similar(task_description, 5).await {
            Ok(episodes) => episodes,
            Err(e) => {
                warn!(error = %e, "查找类似事件失败");
                Vec::new()
            }
        }
    }

    /// 提取成功模式
    ///
    /// 分析历史事件数据，识别成功的执行模式。
    /// 这些模式可以用来指导未来的任务执行。
    ///
    /// # Returns
    ///
    /// 返回识别出的执行模式列表
    ///
    /// # Errors
    ///
    /// 获取事件列表失败时返回错误。
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_possible_truncation)]
    pub async fn extract_patterns(&self) -> crate::Result<Vec<ExecutionPattern>> {
        // 获取最近的成功事件
        let recent_episodes = self.store.list_recent(100).await.map_err(|e| {
            error_core::helpers::agent_memory_error(&format!("获取事件列表失败: {e}"))
        })?;

        // 过滤出成功的事件
        let successful: Vec<&Episode> = recent_episodes
            .iter()
            .filter(|e| matches!(e.outcome, TaskOutcome::Success))
            .collect();

        if successful.is_empty() {
            return Ok(Vec::new());
        }

        // 分析工具使用频率
        let mut tool_usage: HashMap<String, u32> = HashMap::new();
        let mut total_duration = 0u64;

        for episode in &successful {
            total_duration += episode.duration_ms;

            for step in &episode.steps {
                if let Some(ref action) = step.action
                    && action.action_type == ActionType::UseTool
                {
                    *tool_usage.entry(action.tool_name.clone()).or_insert(0) += 1;
                }
            }
        }

        // 创建模式
        let pattern = ExecutionPattern {
            id: Uuid::new_v4(),
            description: format!("基于 {} 个成功案例提取的模式", successful.len()),
            typical_steps: vec![
                "收集信息".to_string(),
                "分析问题".to_string(),
                "执行操作".to_string(),
                "验证结果".to_string(),
            ],
            success_rate: successful.len() as f64 / recent_episodes.len() as f64,
            avg_duration_ms: total_duration / successful.len() as u64,
            tool_usage_patterns: tool_usage,
        };

        info!(
            patterns_found = 1,
            based_on = successful.len(),
            "成功模式提取完成"
        );

        Ok(vec![pattern])
    }
}

// ============================================================================
// 统一记忆系统
// ============================================================================

/// 记忆系统（短期 + 长期 + 事件）
///
/// 整合三种记忆类型的统一入口点。
#[derive(Default)]
pub struct MemorySystem {
    /// 短期工作记忆
    pub short_term: WorkingMemory,
    /// 长期记忆（可选，如果不需要持久化可以为 None）
    pub long_term: Option<LongTermMemory>,
    /// 事件记忆（可选）
    pub episodic: Option<EpisodicMemory>,
}

impl MemorySystem {
    /// 创建只包含短期记忆的系统
    #[must_use]
    pub fn short_term_only() -> Self {
        Self::default()
    }

    /// 创建完整的三层记忆系统
    #[must_use]
    pub fn full_system(long_term: LongTermMemory, episodic: EpisodicMemory) -> Self {
        Self {
            short_term: WorkingMemory::default(),
            long_term: Some(long_term),
            episodic: Some(episodic),
        }
    }

    /// 添加观察到记忆
    ///
    /// # Errors
    ///
    /// 记忆添加或存储失败时返回错误。
    pub async fn add_observation(
        &mut self,
        task_id: Uuid,
        observation: impl Into<String>,
    ) -> crate::Result<()> {
        let observation = observation.into();
        let entry = MemoryEntry::new(MemoryType::Observation, observation.clone())
            .with_metadata("task_id", serde_json::json!(task_id));

        self.short_term.add(entry)?;

        if let Some(ref lt) = self.long_term {
            let entry = MemoryEntry::new(MemoryType::Observation, observation)
                .with_metadata("task_id", serde_json::json!(task_id));
            lt.store(&entry).await?;
        }

        Ok(())
    }

    /// 添加思考到记忆
    ///
    /// # Errors
    ///
    /// 记忆添加失败时返回错误。
    pub fn add_thought(&mut self, task_id: Uuid, thought: impl Into<String>) -> crate::Result<()> {
        let entry = MemoryEntry::new(MemoryType::Thought, thought)
            .with_importance(0.6)
            .with_metadata("task_id", serde_json::json!(task_id));

        self.short_term.add(entry)
    }

    /// 获取相关上下文（供 LLM 使用）
    ///
    /// 整合短期和长期记忆，生成统一的上下文字符串。
    pub fn get_context(&self, max_tokens: usize) -> String {
        let mut context = String::new();

        let short_term_budget = max_tokens / 2;
        context.push_str(&self.short_term.to_llm_context(short_term_budget));

        if let Some(ref long_term) = self.long_term {
            let long_term_budget = max_tokens - short_term_budget;

            let recent = self.short_term.recent(3);
            let query: String = recent
                .iter()
                .map(|e| e.content.as_str())
                .collect::<Vec<_>>()
                .join(" ");

            if !query.is_empty() {
                let mut entries = long_term.recall(&query, 10);
                entries.sort_by(|a, b| {
                    b.importance
                        .partial_cmp(&a.importance)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let mut tokens_used = 0;
                for entry in entries {
                    let entry_text = format!("【长期记忆】{}\n", entry.content);
                    let entry_tokens = (entry_text.len() / 4).max(1);

                    if tokens_used + entry_tokens > long_term_budget {
                        break;
                    }

                    context.push_str(&entry_text);
                    tokens_used += entry_tokens;
                }
            }
        }

        context
    }

    /// 清理资源
    pub fn cleanup(&mut self) {
        self.short_term.clear();
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working_memory_add_and_retrieve() {
        let mut memory = WorkingMemory::with_capacity(10, 1000);

        let entry = MemoryEntry::new(MemoryType::Observation, "测试观察");
        memory.add(entry).unwrap();

        assert_eq!(memory.len(), 1);

        let recent = memory.recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].content, "测试观察");
    }

    #[test]
    fn test_working_memory_search() {
        let mut memory = WorkingMemory::with_capacity(10, 1000);

        memory
            .add(MemoryEntry::new(
                MemoryType::Observation,
                "关于 Rust 的信息",
            ))
            .unwrap();
        memory
            .add(MemoryEntry::new(MemoryType::Thought, "关于 Python 的想法"))
            .unwrap();

        let results = memory.search("Rust");
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("Rust"));
    }

    #[test]
    fn test_working_memory_compression() {
        let mut memory = WorkingMemory::with_capacity(5, 500);

        for i in 0..6 {
            let entry = MemoryEntry::new(MemoryType::Observation, format!("观察 {i}"))
                .with_importance(f64::from(i) / 10.0);
            memory.add(entry).unwrap();
        }

        // 应该触发压缩
        assert!(memory.len() <= 5);
    }

    #[tokio::test]
    async fn test_memory_entry_creation() {
        let entry = MemoryEntry::new(MemoryType::Fact, "重要事实")
            .with_importance(0.95)
            .with_metadata("category", serde_json::json!("important"));

        assert_eq!(entry.entry_type, MemoryType::Fact);
        assert!((entry.importance - 0.95).abs() < f64::EPSILON);
        assert_eq!(entry.metadata["category"], "important");
    }

    #[tokio::test]
    async fn test_memory_entry_serialization() {
        let entry = MemoryEntry::new(MemoryType::Thought, "测试序列化");
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: MemoryEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.entry_type, MemoryType::Thought);
        assert_eq!(deserialized.content, "测试序列化");
    }

    #[tokio::test]
    async fn test_forget_criteria_default() {
        let criteria = ForgetCriteria::default();
        assert!(criteria.max_age_seconds.is_none());
        assert!(criteria.min_importance.is_none());
        assert!(criteria.max_entries.is_none());
        assert!(criteria.memory_types.is_none());
    }

    #[test]
    fn test_working_memory_to_llm_context() {
        let mut memory = WorkingMemory::with_capacity(10, 1000);

        memory
            .add(MemoryEntry::new(MemoryType::Observation, "第一条记忆"))
            .unwrap();
        memory
            .add(MemoryEntry::new(MemoryType::Thought, "第二条记忆"))
            .unwrap();

        let context = memory.to_llm_context(100);
        assert!(context.contains("## 工作记忆"));
        assert!(context.contains("第一条记忆"));
        assert!(context.contains("第二条记忆"));
    }

    #[tokio::test]
    async fn test_memory_system_integration() {
        let mut system = MemorySystem::short_term_only();

        system
            .add_observation(Uuid::new_v4(), "测试观察")
            .await
            .unwrap();

        assert_eq!(system.short_term.len(), 1);

        let context = system.get_context(100);
        assert!(context.contains("测试观察"));
    }

    #[tokio::test]
    async fn test_episode_serialization() {
        let episode = Episode {
            id: Uuid::new_v4(),
            task_description: "测试任务".to_string(),
            outcome: TaskOutcome::Success,
            steps: Vec::new(),
            lessons_learned: vec!["教训1".to_string()],
            duration_ms: 1000,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&episode).unwrap();
        let deserialized: Episode = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.task_description, "测试任务");
        assert!(matches!(deserialized.outcome, TaskOutcome::Success));
        assert_eq!(deserialized.lessons_learned.len(), 1);
    }

    struct MockVectorStore;

    #[async_trait::async_trait]
    impl VectorStore for MockVectorStore {
        async fn store(
            &self,
            _id: &Uuid,
            _vector: &[f32],
            _payload: &serde_json::Value,
        ) -> crate::Result<()> {
            Ok(())
        }

        async fn search(
            &self,
            _query_vector: &[f32],
            _top_k: usize,
        ) -> crate::Result<Vec<(Uuid, f64)>> {
            Ok(Vec::new())
        }

        async fn delete(&self, _id: &Uuid) -> crate::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_get_context_with_long_term_memory_contains_content() {
        let store = Arc::new(MockVectorStore);
        let long_term = LongTermMemory::new(store, "test_collection");

        let entry = MemoryEntry::new(MemoryType::Fact, "Rust所有权规则详解").with_importance(0.9);
        long_term.store(&entry).await.unwrap();

        let mut system = MemorySystem {
            short_term: WorkingMemory::with_capacity(10, 1000),
            long_term: Some(long_term),
            episodic: None,
        };

        system
            .add_observation(Uuid::new_v4(), "Rust所有权")
            .await
            .unwrap();

        let context = system.get_context(1000);
        assert!(context.contains("Rust所有权规则详解"));
        assert!(context.contains("【长期记忆】"));
    }

    #[tokio::test]
    async fn test_get_context_without_long_term_memory_unchanged() {
        let mut system = MemorySystem::short_term_only();

        system
            .add_observation(Uuid::new_v4(), "测试观察")
            .await
            .unwrap();

        let context = system.get_context(1000);
        assert!(context.contains("测试观察"));
        assert!(!context.contains("【长期记忆】"));
    }

    #[tokio::test]
    async fn test_get_context_long_term_memory_respects_token_budget() {
        let store = Arc::new(MockVectorStore);
        let long_term = LongTermMemory::new(store, "test_collection");

        for i in 0..5 {
            let entry = MemoryEntry::new(
                MemoryType::Fact,
                format!("长期事实编号{i}这是一段较长的内容用于消耗token预算"),
            )
            .with_importance(1.0 - f64::from(i) * 0.1);
            long_term.store(&entry).await.unwrap();
        }

        let mut system = MemorySystem {
            short_term: WorkingMemory::with_capacity(10, 1000),
            long_term: Some(long_term),
            episodic: None,
        };

        system
            .add_observation(Uuid::new_v4(), "编号0编号1编号2编号3编号4")
            .await
            .unwrap();

        let context = system.get_context(40);
        let long_term_count = context.matches("【长期记忆】").count();
        assert!(long_term_count < 5);
        assert!(long_term_count >= 1);
    }
}
