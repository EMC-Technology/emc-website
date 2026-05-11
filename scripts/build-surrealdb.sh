#!/bin/bash
# =============================================================================
# SurrealDB 编译与部署脚本 — 全部安装到 Relax 分区
# =============================================================================
# 执行方式: sudo bash build-surrealdb.sh
# 重要: 必须用 sudo 运行整个脚本，避免中途 sudo 密码提示卡住
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

SURREAL_SRC="/home/leon/下载/surrealdb-main/surrealdb-main"
SDB_USER="leon"
SDB_PASS="781013"
PROJECT_DIR="/media/leon/WorkSpace/WorkSpace/RustRAG++"
RUST_RAG_BASE="/media/leon/Relax/rust-rag"
REAL_USER="leon"

# =============================================================================
# Step 0: 前置检查
# =============================================================================

log_step "Step 0: 前置检查"

if [ "$(id -u)" -ne 0 ]; then
    log_error "此脚本必须以 root 运行: sudo bash $0"
    exit 1
fi

log_info "修复 Relax 分区只读挂载..."
systemctl daemon-reload
mount -o remount,rw /media/leon/Relax 2>/dev/null || true
chown -R "${REAL_USER}:${REAL_USER}" /media/leon/Relax
su - "${REAL_USER}" -c "touch /media/leon/Relax/.write-test && rm /media/leon/Relax/.write-test" && log_info "Relax 读写正常 ✓" || {
    log_error "Relax 分区仍然只读！尝试卸载重挂..."
    umount /media/leon/Relax 2>/dev/null || true
    mount -t btrfs -o subvol=rust-rag,compress=zstd,noatime,rw /dev/nvme0n1p4 /media/leon/Relax
    chown -R "${REAL_USER}:${REAL_USER}" /media/leon/Relax
    su - "${REAL_USER}" -c "touch /media/leon/Relax/.write-test && rm /media/leon/Relax/.write-test" && log_info "Relax 读写正常 ✓" || {
        log_error "无法挂载 Relax 为读写模式，退出"
        exit 1
    }
}

# =============================================================================
# Step 1: 安装编译依赖
# =============================================================================

log_step "Step 1: 安装编译依赖"

apt install -y \
    build-essential \
    cmake \
    libclang-dev \
    protobuf-compiler \
    pkg-config \
    libssl-dev

protoc --version
cmake --version | head -1

# =============================================================================
# Step 2: 编译 SurrealDB（以 leon 用户执行 cargo）
# =============================================================================

log_step "Step 2: 编译 SurrealDB（36核并行，预计 5-15 分钟）"

cd "${SURREAL_SRC}"

log_info "源码版本:"
head -5 Cargo.toml
echo ""

log_info "开始编译（以 leon 用户身份）..."
su - "${REAL_USER}" -c "cd ${SURREAL_SRC} && cargo build --release --features storage-rocksdb,storage-mem,http,cli --no-default-features"

if [ ! -f "${SURREAL_SRC}/target/release/surreal" ]; then
    log_error "编译失败：二进制不存在"
    exit 1
fi

log_info "编译成功:"
ls -lh "${SURREAL_SRC}/target/release/surreal"

# =============================================================================
# Step 3: 安装到 Relax 分区
# =============================================================================

log_step "Step 3: 安装 SurrealDB 到 Relax 分区"

mkdir -p "${RUST_RAG_BASE}/bin"
cp "${SURREAL_SRC}/target/release/surreal" "${RUST_RAG_BASE}/bin/surreal"
chmod +x "${RUST_RAG_BASE}/bin/surreal"
chown "${REAL_USER}:${REAL_USER}" "${RUST_RAG_BASE}/bin/surreal"

log_info "二进制已安装: ${RUST_RAG_BASE}/bin/surreal"
log_info "版本: $(${RUST_RAG_BASE}/bin/surreal version)"

ln -sf "${RUST_RAG_BASE}/bin/surreal" /usr/local/bin/surreal
log_info "符号链接: /usr/local/bin/surreal → ${RUST_RAG_BASE}/bin/surreal"

# =============================================================================
# Step 4: 创建 Relax 分区上的目录结构
# =============================================================================

log_step "Step 4: 创建数据/配置/日志目录（全部在 Relax 上）"

mkdir -p "${RUST_RAG_BASE}"/{\
data/{surrealdb,knowledge-api,prometheus,grafana,backups},\
config/{surrealdb,knowledge-api,prometheus,grafana},\
secrets,\
kube,\
scripts,\
logs/surrealdb,\
bin}

chown -R "${REAL_USER}:${REAL_USER}" "${RUST_RAG_BASE}"

# =============================================================================
# Step 5: 写入配置文件
# =============================================================================

log_step "Step 5: 写入配置文件"

cat > "${RUST_RAG_BASE}/config/surrealdb/env" << EOF
SURREAL_USER=${SDB_USER}
SURREAL_PASS=${SDB_PASS}
SURREAL_PATH=rocksdb://${RUST_RAG_BASE}/data/surrealdb
SURREAL_BIND=0.0.0.0:8000
SURREAL_LOG=info
EOF
chmod 600 "${RUST_RAG_BASE}/config/surrealdb/env"

jwt_secret=$(cat "${RUST_RAG_BASE}/secrets/jwt_secret" 2>/dev/null || openssl rand -base64 32)
echo -n "${jwt_secret}" > "${RUST_RAG_BASE}/secrets/jwt_secret"
echo -n "${SDB_PASS}" > "${RUST_RAG_BASE}/secrets/surrealdb_password"
[ ! -f "${RUST_RAG_BASE}/secrets/key.aes" ] && openssl rand -hex 32 > "${RUST_RAG_BASE}/secrets/key.aes"
chmod 600 "${RUST_RAG_BASE}/secrets"/*

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
enc_key_path = "${RUST_RAG_BASE}/secrets/key.aes"
jwt_secret = "${jwt_secret}"
jwt_expiry_hours = 24
audit_log_path = "${RUST_RAG_BASE}/logs/audit.log"

[parser]
max_file_size = 1073741824
chunk_size = 1000
chunk_overlap = 100
enable_code_parsing = true
embedding_dim = 2560
EOF

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
enc_key_path = "${RUST_RAG_BASE}/secrets/key.aes"
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

chown -R "${REAL_USER}:${REAL_USER}" "${RUST_RAG_BASE}"
chown "${REAL_USER}:${REAL_USER}" "${PROJECT_DIR}/config.toml"

log_info "配置文件已写入"

# =============================================================================
# Step 6: 创建 systemd 服务（指向 Relax 路径）
# =============================================================================

log_step "Step 6: 创建 systemd 服务"

id surrealdb &>/dev/null || useradd --system --no-create-home --shell /usr/sbin/nologin surrealdb

chown -R surrealdb:surrealdb \
    "${RUST_RAG_BASE}/data/surrealdb" \
    "${RUST_RAG_BASE}/logs/surrealdb" \
    "${RUST_RAG_BASE}/config/surrealdb"

cat > /etc/systemd/system/surrealdb.service << SVCEOF
[Unit]
Description=SurrealDB Multi-Model Database (RustRAG++)
After=network-online.target
Wants=network-online.target
StartLimitIntervalSec=60

[Service]
Type=simple
User=${REAL_USER}
Group=${REAL_USER}
EnvironmentFile=${RUST_RAG_BASE}/config/surrealdb/env

ExecStart=${RUST_RAG_BASE}/bin/surreal start \\
  --bind \${SURREAL_BIND} \\
  --log \${SURREAL_LOG} \\
  --username \${SURREAL_USER} \\
  --password \${SURREAL_PASS} \\
  \${SURREAL_PATH}

KillSignal=SIGTERM
TimeoutStopSec=30
LimitNOFILE=65536

Restart=on-failure
RestartSec=5
StartLimitBurst=5

StandardOutput=journal
StandardError=journal
SyslogIdentifier=surrealdb

[Install]
WantedBy=multi-user.target
SVCEOF

systemctl daemon-reload
systemctl enable surrealdb
systemctl start surrealdb

log_info "等待 SurrealDB 启动..."
retries=0
while [ $retries -lt 15 ]; do
    if curl -sf http://localhost:8000/version > /dev/null 2>&1; then
        log_info "SurrealDB 已就绪: $(curl -sf http://localhost:8000/version 2>/dev/null)"
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

# =============================================================================
# Step 7: 初始化 Schema
# =============================================================================

log_step "Step 7: 初始化 Schema"

surreal sql \
    --endpoint http://localhost:8000 \
    --username "${SDB_USER}" --password "${SDB_PASS}" \
    --namespace knowledge --database knowledge \
    < "${PROJECT_DIR}/crates/knowledge-core/schema.surql"

log_info "Schema 初始化完成 ✓"

# =============================================================================
# Step 8: 创建维护脚本
# =============================================================================

log_step "Step 8: 创建维护脚本"

cat > "${RUST_RAG_BASE}/scripts/surrealdb-control.sh" << 'EOF'
#!/bin/bash
set -euo pipefail
case "${1:-}" in
    start)   sudo systemctl start surrealdb   ;;
    stop)    sudo systemctl stop surrealdb    ;;
    restart) sudo systemctl restart surrealdb ;;
    status)  sudo systemctl status surrealdb --no-pager ;;
    logs)    sudo journalctl -u surrealdb -f  ;;
    *)       echo "Usage: $0 {start|stop|restart|status|logs}" ;;
esac
EOF

cat > "${RUST_RAG_BASE}/scripts/backup.sh" << 'EOF'
#!/bin/bash
set -euo pipefail
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/media/leon/Relax/rust-rag/data/backups"
SDB_PASS=$(cat /media/leon/Relax/rust-rag/secrets/surrealdb_password)
mkdir -p "${BACKUP_DIR}"
surreal export \
    --endpoint http://localhost:8000 \
    --username leon --password "${SDB_PASS}" \
    --namespace knowledge --database knowledge \
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

chmod +x "${RUST_RAG_BASE}/scripts/"*.sh
chown -R "${REAL_USER}:${REAL_USER}" "${RUST_RAG_BASE}/scripts"

# =============================================================================
# Step 9: 验证
# =============================================================================

log_step "Step 9: 验证"

echo ""
echo "=== SurrealDB 版本 ==="
surreal version

echo "=== 服务状态 ==="
systemctl status surrealdb --no-pager | head -10

echo "=== HTTP 接口 ==="
curl -sf http://localhost:8000/version && echo ""

echo "=== Relax 分区布局 ==="
echo "bin:       ${RUST_RAG_BASE}/bin/surreal"
echo "data:      ${RUST_RAG_BASE}/data/surrealdb/"
echo "config:    ${RUST_RAG_BASE}/config/surrealdb/env"
echo "logs:      ${RUST_RAG_BASE}/logs/surrealdb/"
echo "secrets:   ${RUST_RAG_BASE}/secrets/"
echo "scripts:   ${RUST_RAG_BASE}/scripts/"

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║           SurrealDB 编译部署完成（Relax 分区）               ║"
echo "╠══════════════════════════════════════════════════════════════╣"
echo "║                                                              ║"
echo "║  全部文件位于 /media/leon/Relax/rust-rag/                   ║"
echo "║  btrfs 快照可一键捕获整个环境                               ║"
echo "║                                                              ║"
echo "║  SurrealDB:    http://localhost:8000                         ║"
echo "║  用户: leon    密码: 781013                                  ║"
echo "║  命名空间: knowledge  数据库: knowledge                     ║"
echo "║                                                              ║"
echo "║  服务管理:                                                   ║"
echo "║    bash ${RUST_RAG_BASE}/scripts/surrealdb-control.sh status ║"
echo "║    bash ${RUST_RAG_BASE}/scripts/surrealdb-control.sh logs   ║"
echo "║                                                              ║"
echo "║  备份/快照:                                                  ║"
echo "║    bash ${RUST_RAG_BASE}/scripts/backup.sh                  ║"
echo "║    bash ${RUST_RAG_BASE}/scripts/snapshot.sh                ║"
echo "║                                                              ║"
echo "║  开发模式:                                                   ║"
echo "║    cd ${PROJECT_DIR}                ║"
echo "║    cargo run -p knowledge-api                                ║"
echo "╚══════════════════════════════════════════════════════════════╝"
