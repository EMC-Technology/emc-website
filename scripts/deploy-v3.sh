#!/bin/bash
# =============================================================================
# RustRAG++ 优化部署脚本 v3
# =============================================================================
# 策略变更：
#   - SurrealDB 从源码本地编译（v3.1.0-alpha），不走 Docker Hub
#   - 开发阶段：SurrealDB 原生 systemd 服务 + API 本地 cargo run
#   - 生产阶段：两者都容器化进 Podman Pod
#   - 消除 Docker Hub 网络依赖
# =============================================================================
# 执行方式: sudo bash deploy-v3.sh
# =============================================================================
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }
log_step()  { echo -e "\n${CYAN}========== $* ==========${NC}"; }

RELAX_DEV="/dev/nvme0n1p4"
RELAX_MNT="/media/leon/Relax"
PROJECT_DIR="/media/leon/WorkSpace/WorkSpace/RustRAG++"
SURREAL_SRC="/home/leon/下载/surrealdb-main/surrealdb-main"
RUST_RAG_BASE="${RELAX_MNT}/rust-rag"

SDB_USER="leon"
SDB_PASS="781013"

# =============================================================================
# Step 1: 修复 Relax 只读挂载
# =============================================================================

step_fix_mount() {
    log_step "Step 1: 修复 Relax 分区挂载为读写"

    local mount_opts
    mount_opts=$(mount | grep nvme0n1p4 | grep -o 'ro,' || true)
    if [ -n "$mount_opts" ]; then
        log_info "当前为只读挂载，重新挂载..."
        mount -o remount,rw "${RELAX_MNT}"
        log_info "已切换为读写模式"
    else
        log_info "已是读写模式"
    fi

    touch "${RELAX_MNT}/.write-test" && rm "${RELAX_MNT}/.write-test"
    log_info "读写验证通过 ✓"
}

# =============================================================================
# Step 2: 编译 SurrealDB（从源码）
# =============================================================================

step_compile_surrealdb() {
    log_step "Step 2: 从源码编译 SurrealDB v3.1.0-alpha"

    if [ -f /usr/local/bin/surreal ]; then
        local ver
        ver=$(/usr/local/bin/surreal version 2>/dev/null || echo "unknown")
        log_info "SurrealDB 已安装: ${ver}"
        read -p "是否重新编译？(y/n): " recompile
        if [ "$recompile" != "y" ]; then
            return 0
        fi
    fi

    if [ ! -d "${SURREAL_SRC}" ]; then
        log_error "SurrealDB 源码目录不存在: ${SURREAL_SRC}"
        exit 1
    fi

    log_info "安装编译依赖..."
    apt install -y build-essential cmake libclang-dev protobuf-compiler

    log_info "开始编译（36核并行，预计 5-15 分钟）..."
    cd "${SURREAL_SRC}"

    cargo build --release \
        --features storage-rocksdb,storage-mem,http,cli \
        --no-default-features \
        -j 36

    if [ ! -f "${SURREAL_SRC}/target/release/surreal" ]; then
        log_error "编译失败：二进制不存在"
        exit 1
    fi

    cp "${SURREAL_SRC}/target/release/surreal" /usr/local/bin/surreal
    chmod +x /usr/local/bin/surreal

    local ver
    ver=$(surreal version)
    log_info "SurrealDB 编译安装完成: ${ver}"
}

# =============================================================================
# Step 3: 创建 SurrealDB systemd 服务
# =============================================================================

step_create_surrealdb_service() {
    log_step "Step 3: 创建 SurrealDB systemd 服务"

    id surrealdb &>/dev/null || {
        useradd --system --no-create-home --shell /usr/sbin/nologin surrealdb
        log_info "创建系统用户 surrealdb"
    }

    mkdir -p /var/lib/surrealdb /var/log/surrealdb
    chown -R surrealdb:surrealdb /var/lib/surrealdb /var/log/surrealdb

    cat > /etc/surrealdb/env << EOF
SURREAL_USER=${SDB_USER}
SURREAL_PASS=${SDB_PASS}
SURREAL_PATH=rocksdb:///var/lib/surrealdb/data
SURREAL_BIND=0.0.0.0:8000
SURREAL_LOG=info
EOF
    chmod 600 /etc/surrealdb/env
    chown surrealdb:surrealdb /etc/surrealdb/env

    cat > /etc/systemd/system/surrealdb.service << 'EOF'
[Unit]
Description=SurrealDB Multi-Model Database
Documentation=https://surrealdb.com/docs
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=surrealdb
Group=surrealdb
EnvironmentFile=/etc/surrealdb/env

ExecStart=/usr/local/bin/surreal start \
  --bind ${SURREAL_BIND} \
  --log ${SURREAL_LOG} \
  --user ${SURREAL_USER} \
  --pass ${SURREAL_PASS} \
  ${SURREAL_PATH}

KillSignal=SIGTERM
TimeoutStopSec=30

NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/surrealdb /var/log/surrealdb
PrivateTmp=true
LimitNOFILE=65536

Restart=on-failure
RestartSec=5
StartLimitBurst=5
StartLimitIntervalSec=60

StandardOutput=journal
StandardError=journal
SyslogIdentifier=surrealdb

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable surrealdb
    systemctl start surrealdb

    log_info "等待 SurrealDB 启动..."
    local retries=0
    while [ $retries -lt 15 ]; do
        if curl -sf http://localhost:8000/version > /dev/null 2>&1; then
            local ver
            ver=$(curl -sf http://localhost:8000/version 2>/dev/null)
            log_info "SurrealDB 已就绪: ${ver}"
            break
        fi
        retries=$((retries + 1))
        sleep 1
    done

    if [ $retries -eq 15 ]; then
        log_error "SurrealDB 启动超时"
        journalctl -u surrealdb --no-pager -n 20
        exit 1
    fi
}

# =============================================================================
# Step 4: 初始化 Schema
# =============================================================================

step_init_schema() {
    log_step "Step 4: 初始化 SurrealDB Schema"

    if [ ! -f "${PROJECT_DIR}/crates/knowledge-core/schema.surql" ]; then
        log_error "Schema 文件不存在"
        exit 1
    fi

    surreal sql \
        --endpoint http://localhost:8000 \
        --user "${SDB_USER}" --pass "${SDB_PASS}" \
        --ns knowledge --db knowledge \
        --file "${PROJECT_DIR}/crates/knowledge-core/schema.surql"

    log_info "Schema 初始化完成 ✓"
}

# =============================================================================
# Step 5: 同步 Relax 分区配置
# =============================================================================

step_sync_relax_config() {
    log_step "Step 5: 同步 Relax 分区配置"

    mkdir -p "${RUST_RAG_BASE}"/{data/{surrealdb,knowledge-api,prometheus,grafana,backups},config/{surrealdb,knowledge-api,prometheus,grafana},secrets,kube,scripts,logs}

    local jwt_secret
    jwt_secret=$(cat "${RUST_RAG_BASE}/secrets/jwt_secret" 2>/dev/null || openssl rand -base64 32)
    local aes_key_path="${RUST_RAG_BASE}/secrets/key.aes"
    [ ! -f "${aes_key_path}" ] && openssl rand -hex 32 > "${aes_key_path}"

    echo -n "${SDB_PASS}" > "${RUST_RAG_BASE}/secrets/surrealdb_password"
    echo -n "${jwt_secret}" > "${RUST_RAG_BASE}/secrets/jwt_secret"
    chmod 600 "${RUST_RAG_BASE}/secrets"/*

    cat > "${RUST_RAG_BASE}/config/surrealdb/env" << EOF
SURREAL_USER=${SDB_USER}
SURREAL_PASS=${SDB_PASS}
SURREAL_PATH=rocksdb:///data
SURREAL_BIND=0.0.0.0:8000
SURREAL_LOG=info
EOF
    chmod 600 "${RUST_RAG_BASE}/config/surrealdb/env"

    cat > "${RUST_RAG_BASE}/config/knowledge-api/config.yaml" << EOF
[database]
addr = "ws://localhost:8000"
namespace = "knowledge"
database = "knowledge"
username = "${SDB_USER}"
password = "${SDB_PASS}"

[server]
host = "0.0.0.0"
port = 3000
tls = false
max_request_size = 10485760

[security]
enc_key_path = "/app/data/key.aes"
jwt_secret = "${jwt_secret}"
jwt_expiry_hours = 24
audit_log_path = "/app/logs/audit.log"

[parser]
max_file_size = 1073741824
chunk_size = 1000
chunk_overlap = 100
enable_code_parsing = true
embedding_dim = 2560
EOF

    chown -R leon:leon "${RUST_RAG_BASE}"
    log_info "Relax 配置同步完成"
}

# =============================================================================
# Step 6: 写入开发环境 config.toml
# =============================================================================

step_write_dev_config() {
    log_step "Step 6: 写入开发环境 config.toml"

    local jwt_secret
    jwt_secret=$(cat "${RUST_RAG_BASE}/secrets/jwt_secret")
    local aes_key_path="${RUST_RAG_BASE}/secrets/key.aes"

    cat > "${PROJECT_DIR}/config.toml" << EOF
[database]
addr = "ws://localhost:8000"
namespace = "knowledge"
database = "knowledge"
username = "${SDB_USER}"
password = "${SDB_PASS}"

[server]
host = "127.0.0.1"
port = 3000
tls = false
max_request_size = 10485760

[security]
enc_key_path = "${aes_key_path}"
jwt_secret = "${jwt_secret}"
jwt_expiry_hours = 24
audit_log_path = "./audit.log"

[parser]
max_file_size = 1073741824
chunk_size = 1000
chunk_overlap = 100
enable_code_parsing = true
embedding_dim = 2560
EOF

    chown leon:leon "${PROJECT_DIR}/config.toml"
    log_info "开发配置已写入: ${PROJECT_DIR}/config.toml"
}

# =============================================================================
# Step 7: 验证开发环境
# =============================================================================

step_verify_dev_env() {
    log_step "Step 7: 验证开发环境"

    echo ""
    echo "=== SurrealDB ==="
    curl -sf http://localhost:8000/version && echo "" || log_error "SurrealDB 不可达"

    echo "=== Rust 工具链 ==="
    rustc --version
    cargo --version

    echo "=== 项目编译测试 ==="
    cd "${PROJECT_DIR}"
    cargo check -p knowledge-api 2>&1 | tail -5

    echo ""
    log_info "开发环境验证完成"
    log_info "启动开发服务器: cd ${PROJECT_DIR} && cargo run -p knowledge-api"
}

# =============================================================================
# Step 8: 创建维护脚本
# =============================================================================

step_write_scripts() {
    log_step "Step 8: 创建维护脚本"

    cat > "${RUST_RAG_BASE}/scripts/backup.sh" << 'EOF'
#!/bin/bash
set -euo pipefail
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/media/leon/Relax/rust-rag/data/backups"
SDB_PASS=$(cat /media/leon/Relax/rust-rag/secrets/surrealdb_password)
mkdir -p "${BACKUP_DIR}"
surreal export \
    --endpoint http://localhost:8000 \
    --user leon --pass "${SDB_PASS}" \
    --ns knowledge --db knowledge \
    "${BACKUP_DIR}/knowledge_${TIMESTAMP}.surql"
gzip "${BACKUP_DIR}/knowledge_${TIMESTAMP}.surql"
find "${BACKUP_DIR}" -name "knowledge_*.surql.gz" -mtime +30 -delete
echo "[$(date)] Backup completed: knowledge_${TIMESTAMP}.surql.gz"
EOF

    cat > "${RUST_RAG_BASE}/scripts/snapshot.sh" << 'EOF'
#!/bin/bash
set -euo pipefail
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
SNAP_DIR="/media/leon/Relax/snapshots"
mkdir -p "${SNAP_DIR}"
sudo btrfs subvolume snapshot /media/leon/Relax "${SNAP_DIR}/rust-rag-${TIMESTAMP}"
echo "[$(date)] Snapshot created: rust-rag-${TIMESTAMP}"
EOF

    cat > "${RUST_RAG_BASE}/scripts/surrealdb-control.sh" << 'EOF'
#!/bin/bash
set -euo pipefail
case "${1:-}" in
    start)   sudo systemctl start surrealdb   ;;
    stop)    sudo systemctl stop surrealdb    ;;
    restart) sudo systemctl restart surrealdb ;;
    status)  sudo systemctl status surrealdb  ;;
    logs)    sudo journalctl -u surrealdb -f  ;;
    *)       echo "Usage: $0 {start|stop|restart|status|logs}" ;;
esac
EOF

    chmod +x "${RUST_RAG_BASE}/scripts/"*.sh
    chown -R leon:leon "${RUST_RAG_BASE}/scripts"
    log_info "维护脚本已创建"
}

# =============================================================================
# Step 9: Podman Pod 配置（生产阶段就绪）
# =============================================================================

step_setup_podman() {
    log_step "Step 9: 配置 Podman（生产阶段就绪）"

    mkdir -p /etc/containers
    cat > /etc/containers/registries.conf << 'EOF'
unqualified-search-registries = ["docker.io"]

[[registry]]
prefix = "docker.io"
location = "docker.io"

[[registry.mirror]]
location = "docker.1ms.run"

[[registry.mirror]]
location = "docker.xuanyuan.me"

[[registry.mirror]]
location = "docker.rainbond.cc"
EOF

    local user_home
    user_home=$(eval echo ~leon)
    mkdir -p "${user_home}/.config/containers"

    cat > "${user_home}/.config/containers/storage.conf" << EOF
[storage]
driver = "overlay"
graphroot = "${RUST_RAG_BASE}/podman-storage"
runroot = "/run/user/1000/containers"

[storage.options]
mount_program = "/usr/bin/fuse-overlayfs"
EOF

    chown -R leon:leon "${user_home}/.config/containers"
    log_info "Podman 配置就绪（生产阶段使用）"
}

# =============================================================================
# 最终报告
# =============================================================================

final_report() {
    log_step "部署完成"

    local jwt_secret
    jwt_secret=$(cat "${RUST_RAG_BASE}/secrets/jwt_secret" 2>/dev/null || echo "N/A")
    local grafana_pass
    grafana_pass=$(cat "${RUST_RAG_BASE}/secrets/grafana_admin_password" 2>/dev/null || echo "N/A")

    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║           RustRAG++ 开发环境部署完成                        ║"
    echo "╠══════════════════════════════════════════════════════════════╣"
    echo "║                                                              ║"
    echo "║  架构: SurrealDB 原生服务 + API 本地编译                     ║"
    echo "║                                                              ║"
    echo "║  SurrealDB:    http://localhost:8000                         ║"
    echo "║    用户: leon  密码: 781013                                  ║"
    echo "║    版本: $(surreal version 2>/dev/null || echo 'N/A')"
    echo "║    服务: systemctl status surrealdb                          ║"
    echo "║                                                              ║"
    echo "║  开发模式:                                                   ║"
    echo "║    cd ${PROJECT_DIR}                ║"
    echo "║    cargo run -p knowledge-api                                ║"
    echo "║                                                              ║"
    echo "║  SurrealDB 管理:                                             ║"
    echo "║    bash ${RUST_RAG_BASE}/scripts/surrealdb-control.sh status ║"
    echo "║    bash ${RUST_RAG_BASE}/scripts/surrealdb-control.sh logs   ║"
    echo "║                                                              ║"
    echo "║  备份: bash ${RUST_RAG_BASE}/scripts/backup.sh       ║"
    echo "║  快照: bash ${RUST_RAG_BASE}/scripts/snapshot.sh     ║"
    echo "║                                                              ║"
    echo "║  生产阶段（Podman 容器化）:                                  ║"
    echo "║    Podman 已配置，镜像加速已就绪                             ║"
    echo "║    运行 deploy-continue.sh 即可容器化                       ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""

    systemctl status surrealdb --no-pager 2>/dev/null | head -10 || true
}

# =============================================================================
# 主流程
# =============================================================================

main() {
    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║     RustRAG++ 优化部署脚本 v3                               ║"
    echo "║     策略: 本地编译 SurrealDB + 原生 systemd 服务            ║"
    echo "║     优势: 无需 Docker Hub，开发阶段零容器依赖               ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""

    if [ "$(id -u)" -ne 0 ]; then
        log_error "此脚本必须以 root 身份运行: sudo bash $0"
        exit 1
    fi

    step_fix_mount
    step_compile_surrealdb
    step_create_surrealdb_service
    step_init_schema
    step_sync_relax_config
    step_write_dev_config
    step_verify_dev_env
    step_write_scripts
    step_setup_podman

    final_report
}

main "$@"
