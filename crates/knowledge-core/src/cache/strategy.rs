use std::time::Duration;

/// 缓存策略枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStrategy {
    /// Look-Aside (旁路缓存): 应用层管理缓存
    ///
    /// 适用场景：读多写少、数据一致性要求不高
    /// 特点：简单、高性能、需手动失效
    LookAside,

    /// Write-Through (直写): 写入时同步更新缓存
    ///
    /// 适用场景：强一致性要求、读写均衡
    /// 特点：数据一致、写入延迟较高
    WriteThrough,

    /// Write-Behind (回写): 写入时异步更新缓存
    ///
    /// 适用场景：高吞吐写入、允许短暂不一致
    /// 特点：写入快、最终一致性
    WriteBehind,

    /// Refresh-Ahead (预刷新): TTL 过期前主动刷新
    ///
    /// 适用场景：热点数据、访问模式可预测
    /// 特点：减少冷启动、提高命中率
    RefreshAhead,
}

impl std::fmt::Display for CacheStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LookAside => write!(f, "look_aside"),
            Self::WriteThrough => write!(f, "write_through"),
            Self::WriteBehind => write!(f, "write_behind"),
            Self::RefreshAhead => write!(f, "refresh_ahead"),
        }
    }
}

/// 预定义的缓存策略配置
#[derive(Debug, Clone)]
pub struct CacheStrategyConfig {
    /// L1 TTL (本地内存缓存过期时间)
    pub l1_ttl: Duration,
    /// L2 TTL (Redis 过期时间)
    pub l2_ttl: Duration,
    /// 缓存策略
    pub strategy: CacheStrategy,
    /// 目标命中率（用于监控告警）
    pub target_hit_rate: f64,
}

/// 预定义的业务场景缓存策略
pub const CACHE_STRATEGIES: &[(&str, CacheStrategyConfig)] = &[
    (
        "embedding_result",
        CacheStrategyConfig {
            l1_ttl: Duration::from_secs(86400),   // 24h
            l2_ttl: Duration::from_secs(172800),  // 48h
            strategy: CacheStrategy::LookAside,
            target_hit_rate: 0.95,
        },
    ),
    (
        "knowledge_node",
        CacheStrategyConfig {
            l1_ttl: Duration::from_secs(3600),     // 1h
            l2_ttl: Duration::from_secs(7200),     // 2h
            strategy: CacheStrategy::WriteThrough,
            target_hit_rate: 0.80,
        },
    ),
    (
        "permission_check",
        CacheStrategyConfig {
            l1_ttl: Duration::from_secs(300),      // 5min
            l2_ttl: Duration::from_secs(600),      // 10min
            strategy: CacheStrategy::RefreshAhead,
            target_hit_rate: 0.99,
        },
    ),
    (
        "write_heavy_operation",
        CacheStrategyConfig {
            l1_ttl: Duration::from_secs(60),
            l2_ttl: Duration::from_secs(120),
            strategy: CacheStrategy::WriteBehind,
            target_hit_rate: 0.70,
        },
    ),
];

/// 根据业务场景名称获取缓存策略配置
///
/// # 参数
/// - `scenario_name`: 业务场景标识符
///
/// # 返回
/// 返回匹配的策略配置，未找到时返回 None
#[must_use]
pub fn get_strategy(scenario_name: &str) -> Option<&'static CacheStrategyConfig> {
    CACHE_STRATEGIES
        .iter()
        .find(|(name, _)| *name == scenario_name)
        .map(|(_, config)| config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_display() {
        assert_eq!(CacheStrategy::LookAside.to_string(), "look_aside");
        assert_eq!(CacheStrategy::WriteThrough.to_string(), "write_through");
        assert_eq!(CacheStrategy::WriteBehind.to_string(), "write_behind");
        assert_eq!(CacheStrategy::RefreshAhead.to_string(), "refresh_ahead");
    }

    #[test]
    fn test_get_strategy_existing() {
        let config = get_strategy("embedding_result").unwrap();
        assert_eq!(config.strategy, CacheStrategy::LookAside);
        assert_eq!(config.l1_ttl, Duration::from_secs(86400));
        assert!((config.target_hit_rate - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_strategy_nonexistent() {
        assert!(get_strategy("nonexistent_scenario").is_none());
    }

    #[test]
    fn test_all_strategies_covered() {
        let strategies: Vec<&CacheStrategy> = CACHE_STRATEGIES
            .iter()
            .map(|(_, c)| &c.strategy)
            .collect();

        assert!(strategies.contains(&&CacheStrategy::LookAside));
        assert!(strategies.contains(&&CacheStrategy::WriteThrough));
        assert!(strategies.contains(&&CacheStrategy::WriteBehind));
        assert!(strategies.contains(&&CacheStrategy::RefreshAhead));
    }
}
