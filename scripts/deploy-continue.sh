#!/bin/bash
# =============================================================================
# RustRAG++ Podman Pod 续接部署脚本
# =============================================================================
# 从当前状态继续：btrfs 已格式化，子卷已创建，但挂载/fstab/Podman/SurrealDB 未完成
# 执行方式: sudo bash deploy-continue.sh
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
RUST_RAG_BASE="${RELAX_MNT}/rust-rag"

SDB_USER="leon"
SDB_PASS="781013"

# =============================================================================
# Step 1: 修复挂载 — rw + rust-rag 子卷
# =============================================================================

step_fix_mount() {
    log_step "Step 1: 修复 Relax 分区挂载"

    log_info "当前挂载状态:"
    mount | grep nvme0n1p4 || true

    log_info "卸载当前只读挂载..."
    umount "${RELAX_MNT}" 2>/dev/null || {
        log_warn "卸载失败，尝试 lazy unmount..."
        umount -l "${RELAX_MNT}" 2>/dev/null || true
        sleep 2
    }

    log_info "以 rw + rust-rag 子卷重新挂载..."
    mount -t btrfs -o subvol=rust-rag,compress=zstd,noatime,rw "${RELAX_DEV}" "${RELAX_MNT}"

    log_info "验证挂载:"
    mount | grep nvme0n1p4
    df -hT "${RELAX_MNT}"
    touch "${RELAX_MNT}/.write-test" && rm "${RELAX_MNT}/.write-test"
    log_info "读写验证通过 ✓"
}

# =============================================================================
# Step 2: 更新 fstab
# =============================================================================

step_update_fstab() {
    log_step "Step 2: 更新 fstab 持久化挂载"

    local uuid
    uuid=$(blkid -s UUID -o value "${RELAX_DEV}")
    if [ -z "$uuid" ]; then
        log_error "无法获取 ${RELAX_DEV} 的 UUID"
        log_info "尝试 lsblk..."
        lsblk -f "${RELAX_DEV}"
        exit 1
    fi

    log_info "UUID: ${uuid}"

    local fstab_entry="UUID=${uuid}  ${RELAX_MNT}  btrfs  defaults,subvol=rust-rag,compress=zstd,noatime  0  0"

    if grep -q "${RELAX_MNT}" /etc/fstab; then
        log_warn "fstab 中已存在 ${RELAX_MNT} 条目，更新..."
        sed -i "\|${RELAX_MNT}|d" /etc/fstab
    fi

    echo "$fstab_entry" >> /etc/fstab
    log_info "fstab 已更新:"
    cat /etc/fstab
}

# =============================================================================
# Step 3: 创建目录结构
# =============================================================================

step_create_dirs() {
    log_step "Step 3: 创建 RustRAG 目录结构"

    mkdir -p "${RUST_RAG_BASE}"/{\
podman-storage,\
data/{surrealdb,knowledge-api,prometheus,grafana,backups},\
config/{surrealdb,knowledge-api,prometheus,grafana},\
secrets,\
kube,\
scripts,\
logs}

    chown -R leon:leon "${RUST_RAG_BASE}"
    log_info "目录结构:"
    find "${RUST_RAG_BASE}" -type d | sort
}

# =============================================================================
# Step 4: 配置 Podman 存储
# =============================================================================

step_configure_podman() {
    log_step "Step 4: 配置 Podman 存储与镜像仓库"

    mkdir -p /etc/containers
    cat > /etc/containers/registries.conf << 'REGEOF'
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
REGEOF
    log_info "镜像仓库配置已写入（含国内镜像加速）: /etc/containers/registries.conf"

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

    log_info "Podman 存储配置:"
    cat "${user_home}/.config/containers/storage.conf"

    log_info "验证 Podman..."
    su - leon -c 'podman info --format "{{.Store.GraphDriverName}} {{.Store.GraphRoot}}"' 2>/dev/null || \
        log_warn "Podman 验证需要 leon 用户重新登录后生效"
}

# =============================================================================
# Step 5: 生成凭据与配置
# =============================================================================

step_generate_configs() {
    log_step "Step 5: 生成凭据与配置文件"

    local secrets_dir="${RUST_RAG_BASE}/secrets"

    echo -n "${SDB_PASS}" > "${secrets_dir}/surrealdb_password"
    openssl rand -base64 32 > "${secrets_dir}/jwt_secret"
    openssl rand -hex 32 > "${secrets_dir}/key.aes"
    openssl rand -base64 16 > "${secrets_dir}/grafana_admin_password"

    chmod 600 "${secrets_dir}"/*
    chown leon:leon "${secrets_dir}"/*

    local jwt_secret
    jwt_secret=$(cat "${secrets_dir}/jwt_secret")
    local aes_key_path="${secrets_dir}/key.aes"
    local grafana_pass
    grafana_pass=$(cat "${secrets_dir}/grafana_admin_password")

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

    cat > "${RUST_RAG_BASE}/config/prometheus/prometheus.yml" << 'EOF'
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'knowledge-api'
    metrics_path: '/metrics'
    static_configs:
      - targets: ['localhost:3001']
    scrape_interval: 10s

  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']
EOF

    mkdir -p "${RUST_RAG_BASE}/config/grafana/provisioning/datasources"
    mkdir -p "${RUST_RAG_BASE}/config/grafana/provisioning/dashboards"

    cat > "${RUST_RAG_BASE}/config/grafana/provisioning/datasources/datasource.yml" << 'EOF'
apiVersion: 1
datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://localhost:9090
    isDefault: true
EOF

    cat > "${RUST_RAG_BASE}/config/grafana/provisioning/dashboards/dashboard.yml" << 'EOF'
apiVersion: 1
providers:
  - name: 'default'
    orgId: 1
    folder: ''
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    options:
      path: /etc/grafana/provisioning/dashboards
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

    chown -R leon:leon "${RUST_RAG_BASE}/config"
    chown -R leon:leon "${RUST_RAG_BASE}/secrets"

    log_info "凭据文件:"
    ls -la "${secrets_dir}/"
    log_info "配置文件:"
    find "${RUST_RAG_BASE}/config" -type f | sort
}

# =============================================================================
# Step 6: 创建 Podman Pod
# =============================================================================

step_create_pod() {
    log_step "Step 6: 创建 Podman Pod"

    if su - leon -c 'podman pod exists rust-rag' 2>/dev/null; then
        log_warn "Pod rust-rag 已存在，先删除..."
        su - leon -c 'podman pod rm -f rust-rag' 2>/dev/null || true
    fi

    su - leon -c '
        podman pod create \
            --name rust-rag \
            -p 3000:3000 \
            -p 3001:3001 \
            -p 8000:8000 \
            -p 9090:9090 \
            -p 3002:3000
    '

    log_info "Pod 已创建:"
    su - leon -c 'podman pod list'
}

# =============================================================================
# Step 7: 启动 SurrealDB
# =============================================================================

step_start_surrealdb() {
    log_step "Step 7: 启动 SurrealDB 容器"

    if su - leon -c 'podman container exists surrealdb' 2>/dev/null; then
        log_warn "容器 surrealdb 已存在，先删除..."
        su - leon -c 'podman rm -f surrealdb' 2>/dev/null || true
    fi

    su - leon -c "
        podman run -d \
            --pod rust-rag \
            --name surrealdb \
            --restart unless-stopped \
            -e SURREAL_USER=${SDB_USER} \
            -e SURREAL_PASS=${SDB_PASS} \
            -e SURREAL_LOG=info \
            -v ${RUST_RAG_BASE}/data/surrealdb:/data:Z \
            surrealdb/surrealdb:v2.1.4 \
            start --bind 0.0.0.0:8000 --user ${SDB_USER} --pass ${SDB_PASS} rocksdb:///data
    "

    log_info "等待 SurrealDB 启动..."
    local retries=0
    while [ $retries -lt 30 ]; do
        if curl -sf http://localhost:8000/version > /dev/null 2>&1; then
            local ver
            ver=$(curl -sf http://localhost:8000/version 2>/dev/null)
            log_info "SurrealDB 已就绪: ${ver}"
            break
        fi
        retries=$((retries + 1))
        log_info "等待中... ($retries/30)"
        sleep 2
    done

    if [ $retries -eq 30 ]; then
        log_error "SurrealDB 启动超时"
        su - leon -c 'podman logs surrealdb 2>&1 | tail -30'
        exit 1
    fi
}

# =============================================================================
# Step 8: 初始化 Schema
# =============================================================================

step_init_schema() {
    log_step "Step 8: 初始化 SurrealDB Schema"

    if [ ! -f "${PROJECT_DIR}/crates/knowledge-core/schema.surql" ]; then
        log_error "Schema 文件不存在: ${PROJECT_DIR}/crates/knowledge-core/schema.surql"
        exit 1
    fi

    su - leon -c "
        podman exec -i surrealdb surreal sql \
            --endpoint http://localhost:8000 \
            --user ${SDB_USER} --pass ${SDB_PASS} \
            --ns knowledge --db knowledge \
    " < "${PROJECT_DIR}/crates/knowledge-core/schema.surql"

    log_info "Schema 初始化完成 ✓"
}

# =============================================================================
# Step 9: 启动监控栈
# =============================================================================

step_start_monitoring() {
    log_step "Step 9: 启动监控栈（可选）"

    local grafana_pass
    grafana_pass=$(cat "${RUST_RAG_BASE}/secrets/grafana_admin_password")

    su - leon -c "
        podman run -d \
            --pod rust-rag \
            --name prometheus \
            --restart unless-stopped \
            -v ${RUST_RAG_BASE}/config/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml:Z \
            -v ${RUST_RAG_BASE}/data/prometheus:/prometheus:Z \
            prom/prometheus:v2.51.0 \
            --config.file=/etc/prometheus/prometheus.yml \
            --storage.tsdb.path=/prometheus
    " 2>/dev/null && log_info "Prometheus 已启动" || log_warn "Prometheus 启动失败（可能需要先拉取镜像）"

    su - leon -c "
        podman run -d \
            --pod rust-rag \
            --name grafana \
            --restart unless-stopped \
            -e GF_SECURITY_ADMIN_PASSWORD='${grafana_pass}' \
            -e GF_USERS_ALLOW_SIGN_UP=false \
            -v ${RUST_RAG_BASE}/data/grafana:/var/lib/grafana:Z \
            -v ${RUST_RAG_BASE}/config/grafana/provisioning:/etc/grafana/provisioning:Z \
            grafana/grafana:10.4.0
    " 2>/dev/null && log_info "Grafana 已启动" || log_warn "Grafana 启动失败（可能需要先拉取镜像）"
}

# =============================================================================
# Step 10: 构建并运行 API 容器
# =============================================================================

step_build_api() {
    log_step "Step 10: 编译 knowledge-api 并构建容器镜像"

    if [ ! -f "${PROJECT_DIR}/target/release/knowledge-api" ]; then
        log_info "编译 release 二进制..."
        su - leon -c "cd ${PROJECT_DIR} && cargo build --release --locked -p knowledge-api"
    fi

    if [ ! -f "${PROJECT_DIR}/target/release/knowledge-api" ]; then
        log_error "编译失败"
        exit 1
    fi

    log_info "二进制: $(ls -lh ${PROJECT_DIR}/target/release/knowledge-api)"

    cat > "${RUST_RAG_BASE}/Containerfile.api" << 'EOF'
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*
RUN groupadd -r appgroup && useradd -r -g appgroup appuser
WORKDIR /app
COPY target/release/knowledge-api /usr/local/bin/
COPY crates/knowledge-core/schema.surql /app/schema.surql
RUN mkdir -p /app/data /app/logs /app/config
USER appuser
EXPOSE 3000 3001
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1
CMD ["knowledge-api"]
EOF

    log_info "构建容器镜像..."
    su - leon -c "
        cd ${PROJECT_DIR} && \
        podman build \
            -t knowledge-api:latest \
            -f ${RUST_RAG_BASE}/Containerfile.api \
            .
    "

    log_info "镜像构建完成:"
    su - leon -c 'podman images knowledge-api'
}

step_run_api() {
    log_step "Step 11: API 容器加入 Pod"

    local jwt_secret
    jwt_secret=$(cat "${RUST_RAG_BASE}/secrets/jwt_secret")

    if su - leon -c 'podman container exists knowledge-api' 2>/dev/null; then
        su - leon -c 'podman rm -f knowledge-api' 2>/dev/null || true
    fi

    su - leon -c "
        podman run -d \
            --pod rust-rag \
            --name knowledge-api \
            --restart unless-stopped \
            -e RUST_LOG=info \
            -e RUST_BACKTRACE=1 \
            -e CONFIG_PATH=/app/config/config.yaml \
            -e KNOWLEDGE_DB_USERNAME=${SDB_USER} \
            -e KNOWLEDGE_DB_PASSWORD=${SDB_PASS} \
            -e KNOWLEDGE_JWT_SECRET='${jwt_secret}' \
            -v ${RUST_RAG_BASE}/config/knowledge-api:/app/config:Z \
            -v ${RUST_RAG_BASE}/data/knowledge-api:/app/data:Z \
            -v ${RUST_RAG_BASE}/secrets/key.aes:/app/data/key.aes:Z \
            knowledge-api:latest
    "

    log_info "等待 API 启动..."
    sleep 5

    if curl -sf http://localhost:3000/health > /dev/null 2>&1; then
        log_info "knowledge-api 已就绪 ✓"
    else
        log_warn "API 健康检查未通过，查看日志:"
        su - leon -c 'podman logs knowledge-api 2>&1 | tail -20'
    fi
}

# =============================================================================
# Step 12: 导出与维护脚本
# =============================================================================

step_export_kube() {
    log_step "Step 12: 导出 K8s YAML"

    su - leon -c "podman generate kube rust-rag" > "${RUST_RAG_BASE}/kube/rust-rag-pod.yaml" 2>/dev/null || \
        log_warn "导出失败（Pod 可能未完全运行）"
    chown leon:leon "${RUST_RAG_BASE}/kube/rust-rag-pod.yaml" 2>/dev/null || true
    log_info "K8s YAML: ${RUST_RAG_BASE}/kube/rust-rag-pod.yaml"
}

step_write_scripts() {
    log_step "Step 13: 创建维护脚本"

    cat > "${RUST_RAG_BASE}/scripts/backup.sh" << 'BACKUP_EOF'
#!/bin/bash
set -euo pipefail
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/media/leon/Relax/rust-rag/data/backups"
SDB_PASS=$(cat /media/leon/Relax/rust-rag/secrets/surrealdb_password)
mkdir -p "${BACKUP_DIR}"
podman exec surrealdb surreal export \
    --endpoint http://localhost:8000 \
    --user leon --pass "${SDB_PASS}" \
    --ns knowledge --db knowledge \
    /tmp/backup_${TIMESTAMP}.surql
podman cp surrealdb:/tmp/backup_${TIMESTAMP}.surql \
    "${BACKUP_DIR}/knowledge_${TIMESTAMP}.surql"
gzip "${BACKUP_DIR}/knowledge_${TIMESTAMP}.surql"
find "${BACKUP_DIR}" -name "knowledge_*.surql.gz" -mtime +30 -delete
echo "[$(date)] Backup completed: knowledge_${TIMESTAMP}.surql.gz"
BACKUP_EOF

    cat > "${RUST_RAG_BASE}/scripts/snapshot.sh" << 'SNAPSHOT_EOF'
#!/bin/bash
set -euo pipefail
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
SNAP_DIR="/media/leon/Relax/snapshots"
mkdir -p "${SNAP_DIR}"
sudo btrfs subvolume snapshot /media/leon/Relax "${SNAP_DIR}/rust-rag-${TIMESTAMP}"
echo "[$(date)] Snapshot created: rust-rag-${TIMESTAMP}"
SNAPSHOT_EOF

    cat > "${RUST_RAG_BASE}/scripts/pod-control.sh" << 'PODCTL_EOF'
#!/bin/bash
set -euo pipefail
case "${1:-}" in
    start)   podman pod start rust-rag   ;;
    stop)    podman pod stop rust-rag    ;;
    restart) podman pod stop rust-rag; podman pod start rust-rag ;;
    status)  podman pod list; echo "---"; podman ps -a --pod ;;
    logs)    shift; podman logs -f "${1:-surrealdb}" ;;
    *)       echo "Usage: $0 {start|stop|restart|status|logs [container]}" ;;
esac
PODCTL_EOF

    chmod +x "${RUST_RAG_BASE}/scripts/"*.sh
    chown -R leon:leon "${RUST_RAG_BASE}/scripts"
    log_info "维护脚本已创建:"
    ls -la "${RUST_RAG_BASE}/scripts/"
}

step_write_systemd() {
    log_step "Step 14: 创建 systemd 服务"

    cat > /etc/systemd/system/podman-rust-rag.service << 'EOF'
[Unit]
Description=RustRAG++ Podman Pod
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
RemainAfterExit=yes
User=leon
ExecStart=/usr/bin/podman pod start rust-rag
ExecStop=/usr/bin/podman pod stop rust-rag

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable podman-rust-rag
    log_info "systemd 服务已启用: podman-rust-rag.service"
}

# =============================================================================
# 最终报告
# =============================================================================

final_report() {
    log_step "部署完成 — 最终报告"

    local jwt_secret
    jwt_secret=$(cat "${RUST_RAG_BASE}/secrets/jwt_secret" 2>/dev/null || echo "N/A")
    local grafana_pass
    grafana_pass=$(cat "${RUST_RAG_BASE}/secrets/grafana_admin_password" 2>/dev/null || echo "N/A")

    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║           RustRAG++ Podman Pod 部署完成                     ║"
    echo "╠══════════════════════════════════════════════════════════════╣"
    echo "║                                                              ║"
    echo "║  SurrealDB:    http://localhost:8000                         ║"
    echo "║    用户: leon  密码: 781013                                  ║"
    echo "║                                                              ║"
    echo "║  API:          http://localhost:3000                         ║"
    echo "║  Metrics:      http://localhost:3001/metrics                 ║"
    echo "║  Prometheus:   http://localhost:9090                         ║"
    echo "║  Grafana:      http://localhost:3002                         ║"
    echo "║    用户: admin  密码: ${grafana_pass}"
    echo "║                                                              ║"
    echo "║  凭据目录: ${RUST_RAG_BASE}/secrets/                        ║"
    echo "║  K8s YAML: ${RUST_RAG_BASE}/kube/rust-rag-pod.yaml          ║"
    echo "║  维护脚本: ${RUST_RAG_BASE}/scripts/                        ║"
    echo "║                                                              ║"
    echo "║  开发模式:                                                   ║"
    echo "║    cd ${PROJECT_DIR}                ║"
    echo "║    cargo run -p knowledge-api                                ║"
    echo "║                                                              ║"
    echo "║  Pod 管理:                                                   ║"
    echo "║    bash ${RUST_RAG_BASE}/scripts/pod-control.sh status       ║"
    echo "║    bash ${RUST_RAG_BASE}/scripts/pod-control.sh logs api     ║"
    echo "║                                                              ║"
    echo "║  迁移到其他机器:                                             ║"
    echo "║    podman play kube rust-rag-pod.yaml                        ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""

    su - leon -c 'podman pod list' 2>/dev/null || true
    echo ""
    su - leon -c 'podman ps -a --pod' 2>/dev/null || true
}

# =============================================================================
# 主流程
# =============================================================================

main() {
    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║     RustRAG++ Podman Pod 续接部署脚本                       ║"
    echo "║     从当前状态继续: btrfs ✓ 子卷 ✓ 挂载/fstab/Podman ✗    ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""

    if [ "$(id -u)" -ne 0 ]; then
        log_error "此脚本必须以 root 身份运行: sudo bash $0"
        exit 1
    fi

    step_fix_mount
    step_update_fstab
    step_create_dirs
    step_configure_podman
    step_generate_configs
    step_create_pod
    step_start_surrealdb
    step_init_schema
    step_start_monitoring
    step_build_api
    step_run_api
    step_export_kube
    step_write_scripts
    step_write_systemd

    final_report
}

main "$@"
