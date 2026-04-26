//! 嵌入计算工作线程池
//!
//! 使用共享 mpsc 通道实现多 Worker 并发消费，
//! 每个 Worker 从同一通道接收任务并执行嵌入计算。

use std::sync::Arc;
use tokio::sync::mpsc;

use crate::embedding_service::EmbeddingService;
use knowledge_core::model::Block;
use error_core::helpers;

/// 嵌入计算任务（工作线程池版本）
pub struct PoolEmbeddingTask {
    /// 待嵌入的原始文本内容
    pub text: String,
    /// 用于返回嵌入结果的 oneshot 通道发送端
    pub callback: tokio::sync::oneshot::Sender<crate::Result<Vec<f32>>>,
}

/// 嵌入计算工作线程
pub struct EmbeddingWorker {
    /// 嵌入计算服务
    service: Arc<EmbeddingService>,
    /// 任务接收通道
    rx: mpsc::Receiver<PoolEmbeddingTask>,
}

impl EmbeddingWorker {
    /// 创建新的嵌入计算工作线程
    #[must_use]
    pub const fn new(service: Arc<EmbeddingService>, rx: mpsc::Receiver<PoolEmbeddingTask>) -> Self {
        Self { service, rx }
    }

    /// 启动工作线程的事件循环，持续接收并处理嵌入计算任务
    pub async fn run(&mut self) {
        while let Some(task) = self.rx.recv().await {
            let block = Block {
                id: None,
                doc_id: surrealdb::sql::Thing::from(("block".to_string(), "worker".to_string())),
                block_type: knowledge_core::model::BlockType::Paragraph,
                start_line: 0,
                end_line: 0,
                embedding: None,
                idempotency_key: Some(task.text.clone()),
            };
            let result = self.service.embed_block(&block).await;
            let _ = task.callback.send(result.map(|r| r.embedding));
        }
    }
}

/// 嵌入计算工作线程池
///
/// 所有 Worker 共享同一个 mpsc 通道接收端，
/// 通过 `mpsc::Receiver::recv()` 的竞争消费实现负载均衡。
pub struct EmbeddingWorkerPool {
    /// 任务发送通道
    tx: mpsc::Sender<PoolEmbeddingTask>,
}

impl EmbeddingWorkerPool {
    /// 创建指定工作线程数的嵌入计算线程池
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(service: Arc<EmbeddingService>, workers: usize) -> Self {
        let (tx, rx) = mpsc::channel::<PoolEmbeddingTask>(256);

        let rx = Arc::new(tokio::sync::Mutex::new(rx));

        for _ in 0..workers {
            let service = Arc::clone(&service);
            let rx = Arc::clone(&rx);
            tokio::spawn(async move {
                loop {
                    let task = {
                        let mut guard = rx.lock().await;
                        guard.recv().await
                    };
                    match task {
                        Some(task) => {
                            let block = Block {
                                id: None,
                                doc_id: surrealdb::sql::Thing::from((
                                    "block".to_string(),
                                    "pool_worker".to_string(),
                                )),
                                block_type: knowledge_core::model::BlockType::Paragraph,
                                start_line: 0,
                                end_line: 0,
                                embedding: None,
                                idempotency_key: Some(task.text.clone()),
                            };
                            let result = service.embed_block(&block).await;
                            let _ = task.callback.send(result.map(|r| r.embedding));
                        }
                        None => break,
                    }
                }
            });
        }

        Self { tx }
    }

    /// # Errors
    ///
    /// 当工作线程池已关闭或响应超时时返回错误。
    pub async fn embed(&self, text: String) -> crate::Result<Vec<f32>> {
        let (callback_tx, callback_rx) = tokio::sync::oneshot::channel();
        let task = PoolEmbeddingTask {
            text,
            callback: callback_tx,
        };
        self.tx.send(task).await.map_err(|_| helpers::internal_error("工作线程池已关闭"))?;
        callback_rx.await.map_err(|_| helpers::internal_error("工作线程响应超时"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding_service::EmbeddingModel;

    #[tokio::test]
    async fn test_worker_pool_embed_success() {
        let service = Arc::new(
            EmbeddingService::new(EmbeddingModel::LocalBgeLarge, 64, 1, 100)
                .unwrap(),
        );
        let pool = EmbeddingWorkerPool::new(service, 2);

        let result = pool.embed("hello world".to_string()).await;
        assert!(result.is_ok(), "嵌入计算应成功: {result:?}");
        let embedding = result.unwrap();
        assert_eq!(embedding.len(), 64);
    }

    #[tokio::test]
    async fn test_worker_pool_multiple_tasks() {
        let service = Arc::new(
            EmbeddingService::new(EmbeddingModel::LocalBgeLarge, 64, 1, 100)
                .unwrap(),
        );
        let pool = EmbeddingWorkerPool::new(service, 2);

        let mut results = Vec::new();
        for i in 0..5 {
            let result = pool.embed(format!("text chunk {i}")).await;
            results.push(result);
        }

        for result in &results {
            assert!(result.is_ok(), "所有任务应成功完成");
        }
    }

    #[tokio::test]
    async fn test_worker_pool_single_worker() {
        let service = Arc::new(
            EmbeddingService::new(EmbeddingModel::LocalBgeLarge, 32, 1, 100)
                .unwrap(),
        );
        let pool = EmbeddingWorkerPool::new(service, 1);

        let result = pool.embed("single worker test".to_string()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }
}
