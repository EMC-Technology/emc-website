//! 嵌入计算工作线程池
//!
//! 使用 work-stealing 模式实现多 Worker 并发消费：
//! 每个 Worker 持有独立的 mpsc::Receiver，调度器通过轮询
//! 将任务分发到最空闲的 Worker，避免共享 Receiver 的持锁 await 问题。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use error_core::helpers;
use tokio::sync::mpsc;

use crate::embedding_service::EmbeddingService;

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
    pub const fn new(
        service: Arc<EmbeddingService>,
        rx: mpsc::Receiver<PoolEmbeddingTask>,
    ) -> Self {
        Self { service, rx }
    }

    /// 启动工作线程的事件循环，持续接收并处理嵌入计算任务
    pub async fn run(&mut self) {
        while let Some(task) = self.rx.recv().await {
            let result = self.service.embed_block("worker_block", &task.text).await;
            let _ =
                task.callback
                    .send(result.map(|r| {
                        Arc::try_unwrap(r.embedding).unwrap_or_else(|arc| (*arc).clone())
                    }));
        }
    }
}

/// 嵌入计算工作线程池
///
/// 每个 Worker 持有独立的 `mpsc::Receiver`，调度器通过原子计数器
/// 轮询分发任务，避免共享 Receiver 的持锁 await 问题。
pub struct EmbeddingWorkerPool {
    /// 各 Worker 的任务发送通道
    txs: Vec<mpsc::Sender<PoolEmbeddingTask>>,
    /// 轮询计数器
    next_worker: AtomicUsize,
}

impl EmbeddingWorkerPool {
    /// 创建指定工作线程数的嵌入计算线程池
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(service: Arc<EmbeddingService>, workers: usize) -> Self {
        let mut txs = Vec::with_capacity(workers);

        for _ in 0..workers {
            let (tx, rx) = mpsc::channel::<PoolEmbeddingTask>(64);
            txs.push(tx);

            let service = Arc::clone(&service);
            tokio::spawn(async move {
                let mut worker = EmbeddingWorker::new(service, rx);
                worker.run().await;
            });
        }

        Self {
            txs,
            next_worker: AtomicUsize::new(0),
        }
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

        let worker_count = self.txs.len();
        let idx = self.next_worker.fetch_add(1, Ordering::Relaxed) % worker_count;
        self.txs[idx]
            .send(task)
            .await
            .map_err(|_| helpers::io_error("工作线程池已关闭"))?;
        callback_rx
            .await
            .map_err(|_| helpers::io_error("工作线程响应超时"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding_service::EmbeddingModel;

    #[tokio::test]
    async fn test_worker_pool_embed_success() {
        let service =
            Arc::new(EmbeddingService::new(EmbeddingModel::LocalBgeLarge, 64, 1, 100).unwrap());
        let pool = EmbeddingWorkerPool::new(service, 2);

        let result = pool.embed("hello world".to_string()).await;
        assert!(result.is_ok(), "嵌入计算应成功: {result:?}");
        let embedding = result.unwrap();
        assert_eq!(embedding.len(), 64);
    }

    #[tokio::test]
    async fn test_worker_pool_multiple_tasks() {
        let service =
            Arc::new(EmbeddingService::new(EmbeddingModel::LocalBgeLarge, 64, 1, 100).unwrap());
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
        let service =
            Arc::new(EmbeddingService::new(EmbeddingModel::LocalBgeLarge, 32, 1, 100).unwrap());
        let pool = EmbeddingWorkerPool::new(service, 1);

        let result = pool.embed("single worker test".to_string()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }
}
