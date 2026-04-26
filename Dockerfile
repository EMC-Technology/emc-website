# =============================================================================
# 文本全结构化知识系统 - 生产级 Docker 镜像
# =============================================================================
# 阶段1: 构建
# -----------------------------------------------------------------------------
FROM rust:1.85-slim AS builder

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# 创建工作目录
WORKDIR /app

# 复制 Cargo 配置和工作区清单（利用 Docker 缓存层）
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ .cargo/
COPY crates/error-core/Cargo.toml crates/error-core/Cargo.toml
COPY crates/knowledge-core/Cargo.toml crates/knowledge-core/Cargo.toml
COPY crates/knowledge-parser/Cargo.toml crates/knowledge-parser/Cargo.toml
COPY crates/knowledge-api/Cargo.toml crates/knowledge-api/Cargo.toml
COPY crates/knowledge-frontend/Cargo.toml crates/knowledge-frontend/Cargo.toml

# 创建 dummy 源文件以缓存依赖编译
RUN mkdir -p crates/error-core/src && echo "" > crates/error-core/src/lib.rs && \
    mkdir -p crates/knowledge-core/src && echo "" > crates/knowledge-core/src/lib.rs && \
    mkdir -p crates/knowledge-parser/src && echo "" > crates/knowledge-parser/src/lib.rs && \
    mkdir -p crates/knowledge-api/src && echo "" > crates/knowledge-api/src/lib.rs && \
    mkdir -p crates/knowledge-frontend/src && echo "" > crates/knowledge-frontend/src/lib.rs

# 构建依赖（此层会被 Docker 缓存，仅当 Cargo.toml/Cargo.lock 变化时重建）
RUN cargo build --release --locked --workspace 2>/dev/null || true

# 复制真实源码并构建
COPY crates/ ./crates/
RUN cargo build --release --locked -p knowledge-api

# =============================================================================
# 阶段2: 运行
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runner

# 安装运行时依赖（包含 curl 用于健康检查）
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 创建非 root 用户
RUN groupadd -r appgroup && useradd -r -g appgroup appuser

# 创建工作目录
WORKDIR /app

# 复制构建产物
COPY --from=builder /app/target/release/knowledge-api /usr/local/bin/
COPY --from=builder /app/crates/knowledge-api/schema.sql /app/schema.sql

# 创建配置目录和数据目录
RUN mkdir -p /app/data /app/logs /app/config

# 复制配置模板
COPY --chown=appuser:appgroup docker/config.docker.yaml /app/config/config.yaml

# 复制入口脚本
COPY --chown=appuser:appgroup docker/entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

# 设置环境变量
ENV RUST_LOG=info \
    RUST_BACKTRACE=1 \
    CONFIG_PATH=/app/config/config.yaml

# 切换到非 root 用户
USER appuser

# 暴露端口
EXPOSE 3000 3001

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# 启动
ENTRYPOINT ["/app/entrypoint.sh"]
