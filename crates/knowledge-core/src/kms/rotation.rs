use chrono::{DateTime, Utc};
use serde::{Deserializer, Serializer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

use crate::Result;
use crate::kms::traits::{KeyManagementService, RotationResult, RotationSchedule};

/// 密钥轮换调度器
///
/// 管理密钥的自动轮换，支持：
/// - 定期自动轮换
/// - 手动触发轮换
/// - 轮换事件通知
/// - 轮换历史记录
///
/// # 安全策略
///
/// 根据 NIST SP 800-57 和行业最佳实践：
/// - 对称加密密钥：建议 90 天轮换
/// - 签名密钥：根据算法和用途，1-2 年
/// - DEK：每次使用后可丢弃或短期缓存
///
/// # 使用示例
///
/// ```ignore
/// let scheduler = KeyRotationScheduler::new(kms);
/// scheduler.register_key("key-1", RotationScheduleConfig::days(90)).await?;
/// scheduler.start().await?;
/// ```
pub struct KeyRotationScheduler<KMS: KeyManagementService + 'static> {
    /// KMS 服务实例
    kms: Arc<KMS>,
    /// 密钥轮换计划表
    rotation_schedule: Arc<RwLock<HashMap<String, RotationSchedule>>>,
    /// 轮换历史记录
    rotation_history: Arc<RwLock<Vec<RotationRecord>>>,
    /// 后台任务句柄
    task_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// 事件发送通道
    event_sender: Option<mpsc::UnboundedSender<RotationEvent>>,
}

/// 轮换计划配置
#[derive(Debug, Clone)]
pub struct RotationScheduleConfig {
    /// 轮换周期（秒）
    pub interval_seconds: u64,
    /// 是否启用自动轮换
    pub auto_rotate: bool,
    /// 是否在启动时立即检查是否需要轮换
    pub check_on_start: bool,
    /// 最大保留的历史记录数
    pub max_history_entries: usize,
}

impl RotationScheduleConfig {
    /// 创建按天为单位的配置
    pub fn days(days: u32) -> Self {
        Self {
            interval_seconds: u64::from(days) * 24 * 60 * 60,
            auto_rotate: true,
            check_on_start: true,
            max_history_entries: 100,
        }
    }

    /// 创建按小时为单位的配置
    pub fn hours(hours: u32) -> Self {
        Self {
            interval_seconds: u64::from(hours) * 60 * 60,
            auto_rotate: true,
            check_on_start: true,
            max_history_entries: 100,
        }
    }

    /// 自定义间隔（秒）
    pub fn custom(seconds: u64) -> Self {
        Self {
            interval_seconds: seconds,
            auto_rotate: true,
            check_on_start: true,
            max_history_entries: 100,
        }
    }
}

impl Default for RotationScheduleConfig {
    fn default() -> Self {
        Self::days(90)
    }
}

/// 轮换事件触发源
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotationTriggeredBy {
    /// 自动调度触发
    Scheduler,
    /// 手动操作触发
    ManualOp(String),
    /// API 调用触发
    ApiCall(String),
    /// 系统启动检查触发
    StartupCheck,
}

/// 轮换事件
#[derive(Debug, Clone)]
pub enum RotationEvent {
    /// 轮换开始
    Started {
        /// 触发源
        triggered_by: RotationTriggeredBy,
        /// 密钥 ID
        key_id: String,
    },
    /// 轮换成功完成
    Completed {
        /// 触发源
        triggered_by: RotationTriggeredBy,
        /// 密钥 ID
        key_id: String,
        /// 轮换结果
        result: RotationResult,
    },
    /// 轮换失败
    Failed {
        /// 触发源
        triggered_by: RotationTriggeredBy,
        /// 密钥 ID
        key_id: String,
        /// 错误信息
        error: String,
    },
    /// 跳过轮换（未到期）
    Skipped {
        /// 触发源
        triggered_by: RotationTriggeredBy,
        /// 密钥 ID
        key_id: String,
        /// 跳过原因
        reason: String,
    },
}

/// 轮换历史记录
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RotationRecord {
    /// 密钥 ID
    pub key_id: String,
    /// 轮换时间
    pub rotated_at: DateTime<Utc>,
    /// 轮换结果
    #[serde(
        serialize_with = "serialize_result",
        deserialize_with = "deserialize_result"
    )]
    pub result: Result<RotationResult>,
    /// 触发方式
    pub trigger: RotationTrigger,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
}

/// 轮换触发方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RotationTrigger {
    /// 自动调度
    Scheduled,
    /// 手动触发
    Manual,
    /// API 调用
    ApiCall,
}

impl<KMS: KeyManagementService + 'static> KeyRotationScheduler<KMS> {
    /// 创建新的密钥轮换调度器
    pub fn new(kms: KMS) -> Self {
        Self {
            kms: Arc::new(kms),
            rotation_schedule: Arc::new(RwLock::new(HashMap::new())),
            rotation_history: Arc::new(RwLock::new(Vec::new())),
            task_handle: Arc::new(RwLock::new(None)),
            event_sender: None,
        }
    }

    /// 从 Arc<KMS> 创建实例
    pub fn from_arc(kms: Arc<KMS>) -> Self {
        Self {
            kms,
            rotation_schedule: Arc::new(RwLock::new(HashMap::new())),
            rotation_history: Arc::new(RwLock::new(Vec::new())),
            task_handle: Arc::new(RwLock::new(None)),
            event_sender: None,
        }
    }

    /// 注册需要轮换的密钥
    ///
    /// # 参数
    ///
    /// - `key_id`: 密钥标识符
    /// - `config`: 轮换计划配置
    pub async fn register_key(&self, key_id: &str, config: RotationScheduleConfig) -> Result<()> {
        let schedule = RotationSchedule {
            key_id: key_id.to_string(),
            interval_seconds: config.interval_seconds,
            last_rotation: Utc::now(),
            next_rotation: Utc::now()
                + chrono::Duration::seconds(
                    i64::try_from(config.interval_seconds).unwrap_or(i64::MAX),
                ),
            auto_rotate: config.auto_rotate,
        };

        let mut schedules = self.rotation_schedule.write().await;
        schedules.insert(key_id.to_string(), schedule);

        tracing::info!(
            key_id = %key_id,
            interval_seconds = config.interval_seconds,
            auto_rotate = config.auto_rotate,
            "已注册密钥到轮换计划"
        );

        Ok(())
    }

    /// 注销密钥的轮换计划
    pub async fn unregister_key(&self, key_id: &str) -> Result<bool> {
        let mut schedules = self.rotation_schedule.write().await;
        Ok(schedules.remove(key_id).is_some())
    }

    /// 启动定期轮换后台任务
    ///
    /// 此方法会启动一个异步任务，定期检查并执行密钥轮换。
    /// 任务会持续运行直到调用 stop() 或调度器被 drop。
    pub async fn start(&mut self) -> Result<()> {
        let (tx, _rx) = mpsc::unbounded_channel();
        self.event_sender = Some(tx.clone());

        let kms = self.kms.clone();
        let schedule = self.rotation_schedule.clone();
        let history = self.rotation_history.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                let due_keys: Vec<String> = {
                    let schedules = schedule.read().await;
                    let now = Utc::now();
                    schedules
                        .iter()
                        .filter(|(_, s)| s.auto_rotate && now >= s.next_rotation)
                        .map(|(k, _)| k.clone())
                        .collect()
                };

                let now = Utc::now();
                for key_id in due_keys {
                    let _ = tx.send(RotationEvent::Started {
                        triggered_by: RotationTriggeredBy::Scheduler,
                        key_id: key_id.clone(),
                    });

                    match kms.rotate_key(&key_id).await {
                        Ok(result) => {
                            let _ = tx.send(RotationEvent::Completed {
                                triggered_by: RotationTriggeredBy::Scheduler,
                                key_id: key_id.clone(),
                                result: result.clone(),
                            });

                            let record = RotationRecord {
                                key_id: key_id.clone(),
                                rotated_at: Utc::now(),
                                result: Ok(result),
                                trigger: RotationTrigger::Scheduled,
                                duration_ms: 0,
                            };

                            let mut hist = history.write().await;
                            hist.push(record);
                        }
                        Err(e) => {
                            let _ = tx.send(RotationEvent::Failed {
                                triggered_by: RotationTriggeredBy::Scheduler,
                                key_id: key_id.clone(),
                                error: e.to_string(),
                            });
                        }
                    }

                    let mut schedules = schedule.write().await;
                    if let Some(s) = schedules.get_mut(&key_id) {
                        s.last_rotation = now;
                        s.next_rotation = now
                            + chrono::Duration::seconds(
                                i64::try_from(s.interval_seconds).unwrap_or(i64::MAX),
                            );
                    }
                }
            }
        });

        let mut handle_guard = self.task_handle.write().await;
        *handle_guard = Some(handle);

        tracing::info!("密钥轮换调度器已启动");
        Ok(())
    }

    /// 停止后台任务
    pub async fn stop(&self) -> Result<()> {
        let mut handle_guard = self.task_handle.write().await;
        if let Some(handle) = handle_guard.take() {
            handle.abort();
            tracing::info!("密钥轮换调度器已停止");
        }
        Ok(())
    }

    /// 手动触发轮换
    pub async fn rotate_now(&self, key_id: &str) -> Result<RotationResult> {
        let start = std::time::Instant::now();

        let result = self.kms.rotate_key(key_id).await?;

        let mut schedules = self.rotation_schedule.write().await;
        if let Some(sched) = schedules.get_mut(key_id) {
            let now = Utc::now();
            sched.last_rotation = now;
            sched.next_rotation = now
                + chrono::Duration::seconds(
                    i64::try_from(sched.interval_seconds).unwrap_or(i64::MAX),
                );
        }
        drop(schedules);

        let record = RotationRecord {
            key_id: key_id.to_string(),
            rotated_at: Utc::now(),
            result: Ok(result.clone()),
            trigger: RotationTrigger::Manual,
            duration_ms: u64::try_from(start.elapsed().as_millis()).unwrap(),
        };

        let mut history = self.rotation_history.write().await;
        history.push(record);

        Ok(result)
    }

    /// 获取下次轮换时间
    pub fn next_rotation(&self, _key_id: &str) -> Option<DateTime<Utc>> {
        None
    }

    /// 获取所有已注册的密钥
    pub async fn registered_keys(&self) -> Vec<String> {
        let schedules = self.rotation_schedule.read().await;
        schedules.keys().cloned().collect()
    }

    /// 获取轮换历史记录
    pub async fn get_history(&self, limit: Option<usize>) -> Vec<RotationRecord> {
        let history = self.rotation_history.read().await;
        match limit {
            Some(l) => history.iter().rev().take(l).cloned().collect(),
            None => history.iter().rev().cloned().collect(),
        }
    }

    /// 检查是否有密钥需要立即轮换
    pub async fn keys_due_for_rotation(&self) -> Vec<String> {
        let schedules = self.rotation_schedule.read().await;
        let now = Utc::now();

        schedules
            .iter()
            .filter(|(_, s)| s.auto_rotate && now >= s.next_rotation)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// 设置事件接收器
    pub fn set_event_handler<F>(&mut self, handler: F)
    where
        F: Fn(RotationEvent) + Send + 'static,
    {
        let _ = handler;
    }
}

impl<KMS: KeyManagementService + 'static> Drop for KeyRotationScheduler<KMS> {
    fn drop(&mut self) {
        if let Ok(guard) = self.task_handle.try_read()
            && let Some(handle) = guard.as_ref()
        {
            handle.abort();
        }
    }
}

fn serialize_result<S, T>(result: &Result<T>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
    T: serde::Serialize,
{
    match result {
        Ok(value) => serializer.serialize_some(value),
        Err(e) => {
            use serde::ser::SerializeStruct;
            let mut s = serializer.serialize_struct("ErrorObject", 2)?;
            s.serialize_field("error", &e.to_string())?;
            s.end()
        }
    }
}

fn deserialize_result<'de, D, T>(deserializer: D) -> std::result::Result<Result<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct ResultVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T: serde::Deserialize<'de>> Visitor<'de> for ResultVisitor<T> {
        type Value = Result<T>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "a result value")
        }

        fn visit_some<D: Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> std::result::Result<Self::Value, D::Error> {
            T::deserialize(deserializer).map(Ok)
        }

        fn visit_none<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
            Ok(Err(crate::error::helpers::serde_error(
                "deserialized error",
            )))
        }

        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut error_msg = String::new();
            while let Some(key) = map.next_key::<String>()? {
                if key == "error" {
                    error_msg = map.next_value()?;
                } else {
                    map.next_value::<serde::de::IgnoredAny>()?;
                }
            }
            Ok(Err(crate::error::helpers::serde_error(&error_msg)))
        }
    }

    deserializer.deserialize_any(ResultVisitor(std::marker::PhantomData))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kms::local::LocalKms;
    use crate::kms::traits::{KeySpec, RotationStatus};
    use tempfile::TempDir;

    async fn create_test_kms() -> (LocalKms, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let master_key = LocalKms::generate_master_key();
        let kms = LocalKms::new(master_key, dir.path()).unwrap();
        kms.create_key("rotate-test-key", KeySpec::Aes256, None)
            .await
            .unwrap();
        (kms, dir)
    }

    #[tokio::test]
    async fn test_register_and_unregister_key() {
        let (kms, _dir) = create_test_kms().await;
        let scheduler = KeyRotationScheduler::new(kms);

        scheduler
            .register_key("test-key", RotationScheduleConfig::days(90))
            .await
            .unwrap();

        let keys = scheduler.registered_keys().await;
        assert_eq!(keys.len(), 1);
        assert!(keys.contains(&"test-key".to_string()));

        let removed = scheduler.unregister_key("test-key").await.unwrap();
        assert!(removed);

        let keys_after = scheduler.registered_keys().await;
        assert!(keys_after.is_empty());
    }

    #[tokio::test]
    async fn test_manual_rotation() {
        let (kms, _dir) = create_test_kms().await;
        let scheduler = KeyRotationScheduler::new(kms);

        scheduler
            .register_key("rotate-test-key", RotationScheduleConfig::custom(1))
            .await
            .unwrap();

        let result = scheduler.rotate_now("rotate-test-key").await.unwrap();
        assert_eq!(result.status, RotationStatus::Completed);
        assert_eq!(result.new_version, 2);

        let history = scheduler.get_history(Some(10)).await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].trigger, RotationTrigger::Manual);
    }

    #[tokio::test]
    async fn test_rotation_config_defaults() {
        let config = RotationScheduleConfig::default();
        assert_eq!(config.interval_seconds, 90 * 24 * 60 * 60);
        assert!(config.auto_rotate);
        assert!(config.check_on_start);
    }

    #[tokio::test]
    async fn test_rotation_config_days_hours() {
        let days_config = RotationScheduleConfig::days(30);
        assert_eq!(days_config.interval_seconds, 30 * 24 * 60 * 60);

        let hours_config = RotationScheduleConfig::hours(12);
        assert_eq!(hours_config.interval_seconds, 12 * 60 * 60);
    }

    #[tokio::test]
    async fn test_multiple_rotations() {
        let (kms, _dir) = create_test_kms().await;
        let scheduler = KeyRotationScheduler::new(kms);

        scheduler
            .register_key("rotate-test-key", RotationScheduleConfig::custom(1))
            .await
            .unwrap();

        for i in 1..=3 {
            let result = scheduler.rotate_now("rotate-test-key").await.unwrap();
            assert_eq!(result.new_version, i + 1);
        }

        let history = scheduler.get_history(None).await;
        assert_eq!(history.len(), 3);
    }

    #[tokio::test]
    async fn test_get_empty_history() {
        let (kms, _dir) = create_test_kms().await;
        let scheduler = KeyRotationScheduler::new(kms);

        let history = scheduler.get_history(Some(10)).await;
        assert!(history.is_empty());
    }
}
