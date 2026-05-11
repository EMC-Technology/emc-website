#!/bin/bash
# =============================================================================
# RustRAG++ Podman Pod 全量部署脚本
# =============================================================================
# 执行方式: sudo bash deploy-podman-pod.sh
# 前置条件: Pop!_OS 24.04 / Ubuntu 24.04, NVMe SSD, 125GB+ RAM
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
WS_DEV="/dev/nvme0n1p3"
RELAX_MNT="/media/leon/Relax"
WS_MNT="/media/leon/WorkSpace"
PROJECT_DIR="/media/leon/WorkSpace/WorkSpace/RustRAG++"
BACKUP_DIR="${WS_MNT}/relax-backup"
RUST_RAG_BASE="${RELAX_MNT}/rust-rag"

# =============================================================================
# 阶段 1: 环境准备
# =============================================================================

step_1_1_install_deps() {
    log_step "阶段1-1: 安装系统依赖"

    apt update
    apt install -y \
        btrfs-progs \
        podman \
        podman-compose \
        catatonit \
        curl \
        ntfs-3g \
        rsync

    log_info "btrfs-progs: $(mkfs.btrfs --version 2>&1 || echo 'installed')"
    log_info "podman: $(podman --version)"
}

step_1_2_fix_ntfs_rw() {
    log_step "阶段1-2: 修复 NTFS 只读问题"

    log_info "检查 NTFS 分区脏标记..."
    local ws_ro=$(mount | grep "$WS_DEV" | grep -c "ro," || true)
    local relax_ro=$(mount | grep "$RELAX_DEV" | grep -c "ro," || true)

    if [ "$ws_ro" -gt 0 ] || [ "$relax_ro" -gt 0 ]; then
        log_warn "检测到 NTFS 分区以只读模式挂载（Windows 休眠/fast startup 导致）"
        log_info "使用 ntfsfix 清除脏标记..."

        umount "$RELAX_MNT" 2>/dev/null || true
        ntfsfix -d "$RELAX_DEV"
        mount -t ntfs3 -o rw,uid=1000,gid=1000,iocharset=utf8 "$RELAX_DEV" "$RELAX_MNT"
        log_info "Relax 分区已重新挂载为读写模式"

        local ws_busy=$(lsof +D "$WS_MNT" 2>/dev/null | wc -l || true)
        if [ "$ws_busy" -gt 0 ]; then
            log_warn "WorkSpace 分区正在使用中（${ws_busy} 个进程），跳过卸载"
            log_warn "将使用根分区临时空间进行备份"
        else
            umount "$WS_MNT" 2>/dev/null || true
            ntfsfix -d "$WS_DEV"
            mount -t ntfs3 -o rw,uid=1000,gid=1000,iocharset=utf8 "$WS_DEV" "$WS_MNT"
            log_info "WorkSpace 分区已重新挂载为读写模式"
        fi
    else
        log_info "NTFS 分区已是读写模式"
    fi
}

step_1_3_cleanup_windows_residuals() {
    log_step "阶段1-3: 清理 Windows 残留文件"

    rm -rf "${RELAX_MNT}/System Volume Information" 2>/dev/null && log_info "已删除 System Volume Information" || log_warn "System Volume Information 不存在"
    rm -rf "${RELAX_MNT}/\$RECYCLE.BIN" 2>/dev/null && log_info "已删除 \$RECYCLE.BIN" || log_warn "\$RECYCLE.BIN 不存在"
    rm -f "${RELAX_MNT}/pagefile.sys" 2>/dev/null && log_info "已删除 pagefile.sys" || log_warn "pagefile.sys 不存在"
    rm -rf "${RELAX_MNT}/.Trash-1000" 2>/dev/null && log_info "已删除 .Trash-1000" || true
}

step_1_4_backup_relax() {
    log_step "阶段1-4: 备份 Relax 有效数据到 WorkSpace"

    if [ -d "$BACKUP_DIR" ] && [ "$(ls -A $BACKUP_DIR 2>/dev/null)" ]; then
        log_warn "备份目录 $BACKUP_DIR 已存在且非空"
        read -p "是否跳过备份？(y/n): " skip_backup
        if [ "$skip_backup" = "y" ]; then
            log_info "跳过备份"
            return 0
        fi
    fi

    mkdir -p "$BACKUP_DIR"

    log_info "开始备份（rsync -rltD，兼容 NTFS）..."
    rsync -rltD --info=progress2 \
        --exclude='.Trash-1000' \
        "${RELAX_MNT}/" "${BACKUP_DIR}/" || [ $? -eq 23 ]

    local src_size=$(du -sh "${RELAX_MNT}" 2>/dev/null | cut -f1)
    local dst_size=$(du -sh "${BACKUP_DIR}" 2>/dev/null | cut -f1)
    log_info "源: ${src_size}, 备份: ${dst_size}"

    local src_count=$(find "${RELAX_MNT}" -type f 2>/dev/null | wc -l)
    local dst_count=$(find "${BACKUP_DIR}" -type f 2>/dev/null | wc -l)
    if [ "$src_count" -eq "$dst_count" ]; then
        log_info "文件数验证通过: ${src_count} 个文件"
    else
        log_warn "文件数不一致: 源 ${src_count}, 备份 ${dst_count}"
        log_warn "请检查是否有文件遗漏"
    fi
    log_info "备份完成: ${BACKUP_DIR}"
}

step_1_5_format_btrfs() {
    log_step "阶段1-5: Relax 分区 NTFS → btrfs 格式化"

    log_warn "⚠️  此操作将永久删除 ${RELAX_DEV} 上的所有数据！"
    log_warn "⚠️  请确认已成功完成备份！"
    read -p "确认格式化 ${RELAX_DEV} 为 btrfs？输入 YES 继续: " confirm
    if [ "$confirm" != "YES" ]; then
        log_error "用户取消格式化"
        exit 1
    fi

    umount "$RELAX_MNT" 2>/dev/null || true
    log_info "正在格式化 ${RELAX_DEV} 为 btrfs..."
    mkfs.btrfs -f -L Relax "$RELAX_DEV"
    log_info "btrfs 格式化完成"
}

step_1_6_create_subvolumes() {
    log_step "阶段1-6: 创建 btrfs 子卷结构并恢复数据"

    mkdir -p "$RELAX_MNT"
    mount -t btrfs -o compress=zstd,noatime "$RELAX_DEV" "$RELAX_MNT"

    log_info "创建 btrfs 子卷..."
    btrfs subvolume create "${RELAX_MNT}/rust-rag"
    btrfs subvolume create "${RELAX_MNT}/snapshots"
    btrfs subvolume create "${RELAX_MNT}/original-data"

    log_info "恢复备份数据到 original-data 子卷..."
    if [ -d "$BACKUP_DIR" ] && [ "$(ls -A $BACKUP_DIR 2>/dev/null)" ]; then
        rsync -rltD --info=progress2 "${BACKUP_DIR}/" "${RELAX_MNT}/original-data/" || [ $? -eq 23 ]
        log_info "数据恢复完成"
    else
        log_warn "未找到备份数据，跳过恢复"
    fi

    chown -R leon:leon "${RELAX_MNT}"

    umount "$RELAX_MNT"

    log_info "使用 rust-rag 子卷重新挂载..."
    mount -t btrfs -o subvol=rust-rag,compress=zstd,noatime,uid=1000,gid=1000 "$RELAX_DEV" "$RELAX_MNT"

    log_info "验证挂载..."
    df -hT "$RELAX_MNT"
    btrfs subvolume show "${RELAX_MNT}" 2>/dev/null || log_warn "子卷信息不可用（可能需要重新挂载）"
}

step_1_7_update_fstab() {
    log_step "阶段1-7: 更新 fstab 持久化挂载"

    local uuid=$(blkid -s UUID -o value "$RELAX_DEV")
    if [ -z "$uuid" ]; then
        log_error "无法获取 ${RELAX_DEV} 的 UUID"
        exit 1
    fi

    local fstab_entry="UUID=${uuid}  ${RELAX_MNT}  btrfs  defaults,subvol=rust-rag,compress=zstd,noatime  0  0"

    if grep -q "$RELAX_MNT" /etc/fstab; then
        log_warn "fstab 中已存在 ${RELAX_MNT} 的挂载条目，将更新"
        sed -i "\|${RELAX_MNT}|d" /etc/fstab
    fi

    echo "$fstab_entry" >> /etc/fstab
    log_info "已添加 fstab 条目: $fstab_entry"
    log_info "fstab 内容:"
    cat /etc/fstab
}

step_1_8_create_directory_structure() {
    log_step "阶段1-8: 创建 RustRAG 目录结构"

    mkdir -p "${RUST_RAG_BASE}"/{\
podman-storage,\
data/{surrealdb,knowledge-api,prometheus,grafana,backups},\
config/{surrealdb,knowledge-api,prometheus,grafana},\
secrets,\
kube,\
scripts}

    chown -R leon:leon "${RUST_RAG_BASE}"
    log_info "目录结构创建完成: ${RUST_RAG_BASE}"
    find "${RUST_RAG_BASE}" -type d | sort
}

step_1_9_configure_podman() {
    log_step "阶段1-9: 配置 Podman 存储"

    local user_home=$(eval echo ~leon)
    mkdir -p "${user_home}/.config/containers"

    cat > "${user_home}/.config/containers/storage.conf" << 'STORAGE_EOF'
[storage]
driver = "overlay"
graphroot = "/media/leon/Relax/rust-rag/podman-storage"
runroot = "/run/user/1000/containers"

[storage.options]
remap_uids = ""
remap_gids = ""
STORAGE_EOF

    chown -R leon:leon "${user_home}/.config/containers"
    log_info "Podman 存储配置已写入: ${user_home}/.config/containers/storage.conf"
}

# =============================================================================
# 阶段 2: 开发/测试环境
# =============================================================================

step_2_1_generate_secrets() {
    log_step "阶段2-1: 生成安全凭据"

    local secrets_dir="${RUST_RAG_BASE}/secrets"

    openssl rand -base64 32 > "${secrets_dir}/surrealdb_password"
    openssl rand -base64 32 > "${secrets_dir}/jwt_secret"
    openssl rand -hex 32 > "${secrets_dir}/key.aes"
    openssl rand -base64 16 > "${secrets_dir}/grafana_admin_password"

    chmod 600 "${secrets_dir}"/*
    chown leon:leon "${secrets_dir}"/*

    log_info "凭据已生成:"
    log_info "  SurrealDB 密码: ${secrets_dir}/surrealdb_password"
    log_info "  JWT 密钥:       ${secrets_dir}/jwt_secret"
    log_info "  AES 密钥:       ${secrets_dir}/key.aes"
    log_info "  Grafana 密码:   ${secrets_dir}/grafana_admin_password"
}

step_2_2_write_configs() {
    log_step "阶段2-2: 写入配置文件"

    local sdb_pass=$(cat "${RUST_RAG_BASE}/secrets/surrealdb_password")
    local jwt_secret=$(cat "${RUST_RAG_BASE}/secrets/jwt_secret")

    cat > "${RUST_RAG_BASE}/config/surrealdb/env" << EOF
SURREAL_USER=leon
SURREAL_PASS=${sdb_pass}
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
username = "root"
password = "${sdb_pass}"

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
      foldersFromFilesStructure: false
EOF

    chown -R leon:leon "${RUST_RAG_BASE}/config"
    log_info "配置文件已写入"
}

step_2_3_create_pod() {
    log_step "阶段2-3: 创建 Podman Pod"

    su - leon -c '
        podman pod create \
            --name rust-rag \
            -p 3000:3000 \
            -p 3001:3001 \
            -p 8000:8000 \
            -p 9090:9090 \
            -p 3002:3000
    '

    log_info "Pod rust-rag 已创建"
    su - leon -c 'podman pod list'
}

step_2_4_start_surrealdb() {
    log_step "阶段2-4: 启动 SurrealDB 容器"

    local sdb_pass=$(cat "${RUST_RAG_BASE}/secrets/surrealdb_password")

    su - leon -c "
        podman run -d \
            --pod rust-rag \
            --name surrealdb \
            --restart unless-stopped \
            -e SURREAL_USER=leon \
            -e SURREAL_PASS='${sdb_pass}' \
            -e SURREAL_LOG=info \
            -v /media/leon/Relax/rust-rag/data/surrealdb:/data:Z \
            surrealdb/surrealdb:v2.1.4 \
            start --bind 0.0.0.0:8000 --user leon --pass '${sdb_pass}' rocksdb:///data
    "

    log_info "等待 SurrealDB 启动..."
    local retries=0
    while [ $retries -lt 30 ]; do
        if curl -sf http://localhost:8000/version > /dev/null 2>&1; then
            log_info "SurrealDB 已就绪: $(curl -sf http://localhost:8000/version 2>/dev/null)"
            break
        fi
        retries=$((retries + 1))
        sleep 2
    done

    if [ $retries -eq 30 ]; then
        log_error "SurrealDB 启动超时"
        su - leon -c 'podman logs surrealdb 2>&1 | tail -20'
        exit 1
    fi
}

step_2_5_init_schema() {
    log_step "阶段2-5: 初始化 SurrealDB Schema"

    local sdb_pass=$(cat "${RUST_RAG_BASE}/secrets/surrealdb_password")

    if [ ! -f "${PROJECT_DIR}/crates/knowledge-core/schema.surql" ]; then
        log_error "Schema 文件不存在: ${PROJECT_DIR}/crates/knowledge-core/schema.surql"
        exit 1
    fi

    su - leon -c "
        podman exec -i surrealdb surreal sql \
            --endpoint http://localhost:8000 \
            --user leon --pass '${sdb_pass}' \
            --ns knowledge --db knowledge \
    " < "${PROJECT_DIR}/crates/knowledge-core/schema.surql"

    log_info "Schema 初始化完成"
}

step_2_6_start_monitoring() {
    log_step "阶段2-6: 启动监控栈（可选）"

    local grafana_pass=$(cat "${RUST_RAG_BASE}/secrets/grafana_admin_password")

    su - leon -c "
        podman run -d \
            --pod rust-rag \
            --name prometheus \
            --restart unless-stopped \
            -v /media/leon/Relax/rust-rag/config/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml:Z \
            -v /media/leon/Relax/rust-rag/data/prometheus:/prometheus:Z \
            prom/prometheus:v2.51.0 \
            --config.file=/etc/prometheus/prometheus.yml \
            --storage.tsdb.path=/prometheus
    " 2>/dev/null && log_info "Prometheus 已启动" || log_warn "Prometheus 启动失败（镜像可能需要先拉取）"

    su - leon -c "
        podman run -d \
            --pod rust-rag \
            --name grafana \
            --restart unless-stopped \
            -e GF_SECURITY_ADMIN_PASSWORD='${grafana_pass}' \
            -e GF_USERS_ALLOW_SIGN_UP=false \
            -v /media/leon/Relax/rust-rag/data/grafana:/var/lib/grafana:Z \
            -v /media/leon/Relax/rust-rag/config/grafana/provisioning:/etc/grafana/provisioning:Z \
            grafana/grafana:10.4.0
    " 2>/dev/null && log_info "Grafana 已启动" || log_warn "Grafana 启动失败（镜像可能需要先拉取）"
}

step_2_7_write_dev_config() {
    log_step "阶段2-7: 写入开发环境配置"

    local sdb_pass=$(cat "${RUST_RAG_BASE}/secrets/surrealdb_password")
    local jwt_secret=$(cat "${RUST_RAG_BASE}/secrets/jwt_secret")
    local aes_key_path="${RUST_RAG_BASE}/secrets/key.aes"

    cat > "${PROJECT_DIR}/config.toml" << EOF
[database]
addr = "ws://localhost:8000"
namespace = "knowledge"
database = "knowledge"
username = "root"
password = "${sdb_pass}"

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
# 阶段 3: 商业化就绪
# =============================================================================

step_3_1_build_api_image() {
    log_step "阶段3-1: 编译 knowledge-api release 并构建容器镜像"

    if [ ! -f "${PROJECT_DIR}/target/release/knowledge-api" ]; then
        log_info "编译 release 二进制（首次编译约需 10-20 分钟）..."
        cd "$PROJECT_DIR"
        su - leon -c "cd ${PROJECT_DIR} && cargo build --release --locked -p knowledge-api"
    fi

    if [ ! -f "${PROJECT_DIR}/target/release/knowledge-api" ]; then
        log_error "编译失败: knowledge-api 二进制不存在"
        exit 1
    fi

    log_info "knowledge-api 编译成功: $(ls -lh ${PROJECT_DIR}/target/release/knowledge-api)"

    cat > "${RUST_RAG_BASE}/Containerfile.api" << 'CONTAINERFILE_EOF'
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
CONTAINERFILE_EOF

    log_info "构建容器镜像..."
    su - leon -c "
        cd ${PROJECT_DIR} && \
        podman build \
            -t knowledge-api:latest \
            -f /media/leon/Relax/rust-rag/Containerfile.api \
            .
    "

    log_info "容器镜像构建完成"
    su - leon -c 'podman images knowledge-api'
}

step_3_2_run_api_container() {
    log_step "阶段3-2: API 容器加入 Pod"

    local sdb_pass=$(cat "${RUST_RAG_BASE}/secrets/surrealdb_password")
    local jwt_secret=$(cat "${RUST_RAG_BASE}/secrets/jwt_secret")

    su - leon -c "
        podman run -d \
            --pod rust-rag \
            --name knowledge-api \
            --restart unless-stopped \
            -e RUST_LOG=info \
            -e RUST_BACKTRACE=1 \
            -e CONFIG_PATH=/app/config/config.yaml \
            -e KNOWLEDGE_DB_USERNAME=root \
            -e KNOWLEDGE_DB_PASSWORD='${sdb_pass}' \
            -e KNOWLEDGE_JWT_SECRET='${jwt_secret}' \
            -v /media/leon/Relax/rust-rag/config/knowledge-api:/app/config:Z \
            -v /media/leon/Relax/rust-rag/data/knowledge-api:/app/data:Z \
            -v /media/leon/Relax/rust-rag/secrets/key.aes:/app/data/key.aes:Z \
            knowledge-api:latest
    "

    log_info "等待 API 启动..."
    sleep 5

    if curl -sf http://localhost:3000/health > /dev/null 2>&1; then
        log_info "knowledge-api 已就绪 ✓"
    else
        log_warn "knowledge-api 健康检查未通过，查看日志:"
        su - leon -c 'podman logs knowledge-api 2>&1 | tail -20'
    fi
}

# =============================================================================
# 阶段 4: 导出可移植 YAML
# =============================================================================

step_4_1_export_kube_yaml() {
    log_step "阶段4-1: 导出 Kubernetes YAML"

    su - leon -c "podman generate kube rust-rag" > "${RUST_RAG_BASE}/kube/rust-rag-pod.yaml"
    log_info "K8s YAML 已导出: ${RUST_RAG_BASE}/kube/rust-rag-pod.yaml"
    cat "${RUST_RAG_BASE}/kube/rust-rag-pod.yaml"
}

step_4_2_write_backup_script() {
    log_step "阶段4-2: 创建备份脚本"

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

    chmod +x "${RUST_RAG_BASE}/scripts/backup.sh"
    chown leon:leon "${RUST_RAG_BASE}/scripts/backup.sh"
    log_info "备份脚本已创建: ${RUST_RAG_BASE}/scripts/backup.sh"
}

step_4_3_write_snapshot_script() {
    log_step "阶段4-3: 创建 btrfs 快照脚本"

    cat > "${RUST_RAG_BASE}/scripts/snapshot.sh" << 'SNAPSHOT_EOF'
#!/bin/bash
set -euo pipefail

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
SNAP_DIR="/media/leon/Relax/snapshots"
BTRFS_MNT="/media/leon/Relax"

mkdir -p "${SNAP_DIR}"

sudo btrfs subvolume snapshot \
    "${BTRFS_MNT}" \
    "${SNAP_DIR}/rust-rag-${TIMESTAMP}"

echo "[$(date)] Snapshot created: rust-rag-${TIMESTAMP}"
echo "To rollback:"
echo "  sudo btrfs subvolume delete ${BTRFS_MNT}"
echo "  sudo btrfs subvolume snapshot ${SNAP_DIR}/rust-rag-${TIMESTAMP} ${BTRFS_MNT}"
SNAPSHOT_EOF

    chmod +x "${RUST_RAG_BASE}/scripts/snapshot.sh"
    chown leon:leon "${RUST_RAG_BASE}/scripts/snapshot.sh"
    log_info "快照脚本已创建: ${RUST_RAG_BASE}/scripts/snapshot.sh"
}

step_4_4_write_systemd_services() {
    log_step "阶段4-4: 创建 systemd 服务文件（Podman 自动启动）"

    cat > /etc/systemd/system/podman-rust-rag.service << 'SYSTEMD_EOF'
[Unit]
Description=RustRAG++ Podman Pod
Documentation=https://github.com/EMC-Technology/emc-website
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
SYSTEMD_EOF

    systemctl daemon-reload
    systemctl enable podman-rust-rag
    log_info "systemd 服务已创建并启用: podman-rust-rag.service"
}

# =============================================================================
# 最终报告
# =============================================================================

final_report() {
    log_step "部署完成 - 最终报告"

    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║           RustRAG++ Podman Pod 部署完成                     ║"
    echo "╠══════════════════════════════════════════════════════════════╣"
    echo "║                                                              ║"

    local sdb_pass=$(cat "${RUST_RAG_BASE}/secrets/surrealdb_password" 2>/dev/null || echo "N/A")
    local jwt_secret=$(cat "${RUST_RAG_BASE}/secrets/jwt_secret" 2>/dev/null || echo "N/A")
    local grafana_pass=$(cat "${RUST_RAG_BASE}/secrets/grafana_admin_password" 2>/dev/null || echo "N/A")

    echo "║  SurrealDB:    http://localhost:8000                         ║"
    echo "║  API:          http://localhost:3000                         ║"
    echo "║  Metrics:      http://localhost:3001/metrics                 ║"
    echo "║  Prometheus:   http://localhost:9090                         ║"
    echo "║  Grafana:      http://localhost:3002 (admin/${grafana_pass})║"
    echo "║                                                              ║"
    echo "║  凭据文件: ${RUST_RAG_BASE}/secrets/                        ║"
    echo "║  K8s YAML:  ${RUST_RAG_BASE}/kube/rust-rag-pod.yaml         ║"
    echo "║  备份脚本:  ${RUST_RAG_BASE}/scripts/backup.sh              ║"
    echo "║  快照脚本:  ${RUST_RAG_BASE}/scripts/snapshot.sh            ║"
    echo "║                                                              ║"
    echo "║  开发模式:                                                   ║"
    echo "║    cd ${PROJECT_DIR}                ║"
    echo "║    cargo run -p knowledge-api                                ║"
    echo "║                                                              ║"
    echo "║  Pod 管理:                                                   ║"
    echo "║    podman pod list                                           ║"
    echo "║    podman pod start rust-rag                                 ║"
    echo "║    podman pod stop rust-rag                                  ║"
    echo "║    podman logs -f surrealdb                                  ║"
    echo "║    podman logs -f knowledge-api                              ║"
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
    echo "║     RustRAG++ Podman Pod 全量部署脚本                       ║"
    echo "║     目标: Relax 分区 btrfs + Podman Pod                    ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""

    if [ "$(id -u)" -ne 0 ]; then
        log_error "此脚本必须以 root 身份运行: sudo bash $0"
        exit 1
    fi

    # 阶段 1: 环境准备
    step_1_1_install_deps
    step_1_2_fix_ntfs_rw
    step_1_3_cleanup_windows_residuals
    step_1_4_backup_relax
    step_1_5_format_btrfs
    step_1_6_create_subvolumes
    step_1_7_update_fstab
    step_1_8_create_directory_structure
    step_1_9_configure_podman

    # 阶段 2: 开发/测试环境
    step_2_1_generate_secrets
    step_2_2_write_configs
    step_2_3_create_pod
    step_2_4_start_surrealdb
    step_2_5_init_schema
    step_2_6_start_monitoring
    step_2_7_write_dev_config

    # 阶段 3: 商业化就绪
    step_3_1_build_api_image
    step_3_2_run_api_container

    # 阶段 4: 导出与维护
    step_4_1_export_kube_yaml
    step_4_2_write_backup_script
    step_4_3_write_snapshot_script
    step_4_4_write_systemd_services

    # 报告
    final_report
}

main "$@"
