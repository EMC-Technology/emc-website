set positional-arguments

default: (lint)

# 安装开发依赖
dev-setup:
    cargo install just
    cargo install cargo-deny
    cargo install cargo-audit
    cargo install cargo-outdated

# 构建整个 workspace
build:
    cargo build --workspace

# 构建 release 版本
build-release:
    cargo build --release --locked -p knowledge-api

# 运行所有测试
test:
    cargo test --workspace

# 运行带数据库的集成测试
test-integration:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Starting SurrealDB..."
    surreal start --bind 0.0.0.0:8000 --user root --pass root memory &
    SURREAL_PID=$!
    sleep 3
    trap "kill $SURREAL_PID 2>/dev/null" EXIT
    SURREALDB_URL=ws://localhost:8000 cargo test --workspace
    echo "Tests completed."

# 运行格式检查
fmt:
    cargo fmt --all -- --check

# 自动格式化
fmt-fix:
    cargo fmt --all

# 运行 clippy 检查
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# 运行完整 lint（fmt + clippy + deny）
lint: fmt clippy deny

# 运行 cargo-deny 依赖审查
deny:
    cargo deny check

# 运行 cargo-audit 安全扫描
audit:
    cargo audit

# 检查过时依赖
outdated:
    cargo outdated

# 运行开发服务器
dev-run: build
    cargo run -p knowledge-api

# 构建 Docker 镜像
docker-build:
    docker build -t knowledge-api:latest .

# 启动所有服务（Docker Compose）
docker-up:
    docker-compose up -d

# 启动含监控的服务
docker-up-monitoring:
    docker-compose --profile monitoring up -d

# 停止所有服务
docker-down:
    docker-compose down

# 查看 API 日志
docker-logs:
    docker-compose logs -f api

# 构建 WASM 前端
wasm-build:
    cd crates/knowledge-frontend && trunk build --release

# 运行 WASM 前端开发服务器
wasm-dev:
    cd crates/knowledge-frontend && trunk serve

# 生成文档
doc:
    cargo doc --workspace --no-deps

# 打开文档
doc-open:
    cargo doc --workspace --no-deps --open

# 干运行发布（验证 crates.io 发布准备）
publish-dry-run:
    cargo publish -p error-core --dry-run
    cargo publish -p knowledge-core --dry-run
    cargo publish -p knowledge-parser --dry-run
    cargo publish -p knowledge-extractor --dry-run
    cargo publish -p knowledge-evaluator --dry-run
    cargo publish -p knowledge-api --dry-run

# 清理构建产物
clean:
    cargo clean

# 检查所有 crate 的元数据完整性
check-meta:
    #!/usr/bin/env bash
    set -euo pipefail
    for crate in error-core knowledge-core knowledge-parser knowledge-extractor knowledge-evaluator knowledge-api knowledge-frontend; do
        echo "=== $crate ==="
        cargo publish -p "$crate" --dry-run 2>&1 | tail -1 || true
    done
