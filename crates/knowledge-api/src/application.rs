//! 应用程序启动与生命周期管理

use knowledge_core::SurrealDbClient;
use error_core::helpers;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

use crate::auth::AuthService;
use crate::auth::LoginRateLimiter;
use crate::authz::engine::{AuthorizationEngine, DbPolicyStore, NoopAuditLogger};
use crate::config::{Config, ConfigLoader};
use crate::knowledge_vm::KnowledgeVm as KnowledgeVM;
use crate::router::build_router;
use crate::handler::AppState;
use crate::ws::WsConnectionManager;
use crate::observability;

/// 应用程序入口
///
/// 封装 Axum 服务的构建和运行逻辑。
pub struct Application {
    config: Config,
    knowledge_vm: Arc<KnowledgeVM>,
    ws_manager: Arc<WsConnectionManager>,
    shutdown_tx: broadcast::Sender<()>,
    telemetry: observability::TelemetryHandle,
}

impl Application {
    /// # Errors
    ///
    /// 配置加载失败、数据库连接失败或遥测初始化失败时返回错误。
    pub async fn build() -> crate::Result<Self> {
        let config = ConfigLoader::load_default()?;
        ConfigLoader::validate(&config)?;

        let telemetry = observability::init_telemetry("knowledge-api")?;

        info!("配置加载完成: 服务器端口={}", config.server.port);

        let db_client = SurrealDbClient::new(
            &config.database.addr,
            &config.database.namespace,
            &config.database.database,
        ).await?;

        info!("数据库连接成功: {}", config.database.addr);

        let knowledge_vm = Arc::new(
            KnowledgeVM::with_embedding_dim(db_client, config.parser.embedding_dim)
                .map_err(|e| error_core::helpers::internal_error(&e.to_string()))?
        );
        let ws_manager = Arc::new(WsConnectionManager::new());

        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config,
            knowledge_vm,
            ws_manager,
            shutdown_tx,
            telemetry,
        })
    }

    /// # Errors
    ///
    /// 地址解析失败或服务器运行出错时返回错误。
    pub async fn run(self) -> crate::Result<()> {
        let Self {
            config,
            knowledge_vm,
            ws_manager,
            shutdown_tx,
            telemetry,
        } = self;

        let auth_service = Arc::new(
            AuthService::new(&config.security.jwt_secret, config.security.jwt_expiry_hours)?
        );

        let policy_store = Arc::new(DbPolicyStore::new());
        let audit_logger = Arc::new(NoopAuditLogger);
        let authz_engine = Arc::new(AuthorizationEngine::new(
            policy_store,
            None,
            audit_logger,
        ));

        let app_state = AppState {
            vm: knowledge_vm,
            ws_manager,
            auth_service,
            rate_limiter: Arc::new(LoginRateLimiter::new()),
            authz_engine: Some(authz_engine),
        };

        let router = build_router(app_state);

        let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
            .parse()
            .map_err(|e| helpers::internal_error(&format!("地址解析失败: {e}")))?;

        info!("服务器启动中: {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router)
            .with_graceful_shutdown(Self::shutdown_signal(shutdown_tx.clone()))
            .await?;

        info!("服务器已关闭");

        telemetry.shutdown();

        Ok(())
    }

    async fn shutdown_signal(shutdown_tx: broadcast::Sender<()>) {
        #[cfg(unix)]
        {
            let mut terminate = tokio::signal::unix::signal(
                tokio::signal::unix::SignalKind::terminate(),
            )
            .map_err(|e| tracing::error!("无法注册 SIGTERM 信号: {}", e))
            .ok();
            let mut interrupt = tokio::signal::unix::signal(
                tokio::signal::unix::SignalKind::interrupt(),
            )
            .map_err(|e| tracing::error!("无法注册 SIGINT 信号: {}", e))
            .ok();

            tokio::select! {
                _ = async { if let Some(ref mut sig) = terminate { sig.recv().await; } } => info!("收到 SIGTERM 信号，准备关闭"),
                _ = async { if let Some(ref mut sig) = interrupt { sig.recv().await; } } => info!("收到 SIGINT 信号，准备关闭"),
            }
        }

        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c()
                .await
                .map_err(|e| tracing::error!("无法注册 Ctrl+C 信号: {}", e))
                .ok();
            info!("收到 Ctrl+C 信号，准备关闭");
        }

        let _ = shutdown_tx.send(());
    }
}

/// # Errors
///
/// 应用程序构建或运行失败时返回错误。
pub async fn run() -> crate::Result<()> {
    info!("启动文本全结构化知识系统 {}", crate::API_VERSION);

    let app = Application::build().await?;
    app.run().await
}
