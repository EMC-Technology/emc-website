# RustRAG++ & SurrealDB 部署方案：Podman Pod 原生容器化

> ADR 编号: ADR-011  
> 状态: 已批准  
> 日期: 2026-05-10  
> 决策者: 项目架构组

---

## 1. 上下文与动机

本项目当前使用 Docker Compose 进行部署（见 `docker-compose.yml`），但存在以下问题：

1. **Docker 依赖守护进程**：Docker Daemon 存在安全风险和资源开销
2. **开发→生产路径断裂**：Docker Compose 编排无法直接迁移到 Kubernetes
3. **本地环境不一致**：开发环境与生产环境差异大
4. **扩容困难**：从单机到集群缺乏清晰的迁移路径

目标：在本地 Relax 分区（`/media/leon/Relax`）建立 Podman Pod 环境，实现：

- 开发阶段：SurrealDB 容器化 + API 本地编译调试
- 测试阶段：完整 Pod 内集成测试
- 生产阶段：API 容器化 + `podman generate kube` 导出可移植 YAML
- 扩容阶段：YAML 直接部署到 Kubernetes 集群

---

## 2. 环境基线

| 资源 | 详情 |
|------|------|
| OS | Pop!_OS 24.04 LTS (Ubuntu 系) |
| 内核 | 6.18.7 |
| CPU | 36 核 |
| 内存 | 125GB |
| 根分区 | `/dev/nvme0n1p2` 279G ext4 |
| WorkSpace | `/dev/nvme0n1p3` 411G NTFS |
| Relax | `/dev/nvme0n1p4` 262G **btrfs** (zstd:3 压缩) ✅ 已转换 |

---

## 3. 关键决策：Relax 分区文件系统转换

### 3.1 问题

Relax 分区当前为 NTFS 格式。Podman 的 overlay 存储驱动依赖 overlayfs，**overlayfs 不支持 NTFS**。

### 3.2 方案选择

| 方案 | 描述 | 优势 | 劣势 |
|------|------|------|------|
| **A: NTFS → btrfs** ⭐ | 格式化转换为 btrfs | overlayfs 原生支持、子卷快照、透明压缩 | 需备份数据、一次性操作 |
| B: Loopback ext4 | 在 NTFS 上创建 ext4 虚拟磁盘文件 | 不改分区格式 | 双层文件系统、性能损耗 15-25% |
| C: 混合方案 | Podman 存放根分区、数据卷放 Relax | 无需格式化 | 容器层和数据分离、迁移复杂 |

### 3.3 决策

选择 **方案 A：NTFS → btrfs**。

理由：
- btrfs 子卷快照支持整个环境秒级复制/回滚
- 透明压缩（zstd）节省 30-50% 磁盘空间
- `btrfs send/receive` 支持增量复制到其他节点
- 一次性格式化，长期受益

### 3.4 转换步骤

```bash
# 1. 将 Relax 有效数据（约 27G，排除 pagefile.sys）备份到 WorkSpace
mkdir -p /media/leon/WorkSpace/relax-backup
rsync -aHAX --progress \
  --exclude='pagefile.sys' \
  --exclude='$RECYCLE.BIN' \
  --exclude='System Volume Information' \
  /media/leon/Relax/ /media/leon/WorkSpace/relax-backup/

# 2. 卸载并格式化
sudo umount /media/leon/Relax
sudo mkfs.btrfs -L Relax /dev/nvme0n1p4

# 3. 创建 btrfs 子卷结构
sudo mount /dev/nvme0n1p4 /mnt
sudo btrfs subvolume create /mnt/rust-rag
sudo btrfs subvolume create /mnt/rust-rag-data
sudo umount /mnt

# 4. 挂载并恢复数据
sudo mount -o subvol=rust-rag,compress=zstd,noatime /dev/nvme0n1p4 /media/leon/Relax
rsync -aHAX /media/leon/WorkSpace/relax-backup/ /media/leon/Relax/

# 5. 更新 /etc/fstab
# UUID=<new-uuid>  /media/leon/Relax  btrfs  defaults,subvol=rust-rag,compress=zstd,noatime  0  0

# 6. 清理备份
# rm -rf /media/leon/WorkSpace/relax-backup
```

---

## 4. Podman Pod 架构

### 4.1 目录结构

```
/media/leon/Relax/rust-rag/
├── podman-storage/          ← Podman graphroot (overlay)
├── data/
│   ├── surrealdb/           ← RocksDB 持久化数据
│   ├── knowledge-api/       ← API 密钥、日志
│   ├── prometheus/          ← TSDB
│   └── grafana/             ← 仪表板
├── config/
│   ├── surrealdb/env        ← 环境变量
│   ├── knowledge-api/       ← config.yaml
│   ├── prometheus/          ← prometheus.yml
│   └── grafana/             ← provisioning
├── secrets/                 ← 敏感凭据
└── kube/                    ← 导出的 K8s YAML
```

### 4.2 Pod 网络拓扑

```
┌─ Pod: rust-rag (共享网络命名空间) ──────────────┐
│                                                  │
│  ┌──────────────┐    localhost:8000              │
│  │  surrealdb   │◄──────────┐                   │
│  │  :8000       │           │                   │
│  │  RocksDB     │           │                   │
│  └──────────────┘           │                   │
│                             │                   │
│  ┌──────────────┐           │                   │
│  │ knowledge-api│───────────┘                   │
│  │ :3000 (HTTP) │                               │
│  │ :3001 (Met)  │──┐                           │
│  └──────────────┘  │                           │
│                    │ localhost:3001/metrics      │
│  ┌──────────────┐  │                           │
│  │ prometheus   │◄─┘                           │
│  │ :9090        │                               │
│  └──────────────┘                               │
│                                                  │
│  ┌──────────────┐                               │
│  │ grafana      │                               │
│  │ :3000→映射3002│                               │
│  └──────────────┘                               │
└──────────────────────────────────────────────────┘

宿主机端口映射:
  3000 → knowledge-api HTTP
  3001 → knowledge-api Metrics
  8000 → SurrealDB
  9090 → Prometheus
  3002 → Grafana
```

---

## 5. 实施步骤

### 5.1 安装 Podman

```bash
sudo apt update
sudo apt install -y podman podman-compose catatonit

# 配置 rootless 存储
mkdir -p ~/.config/containers
cat > ~/.config/containers/storage.conf << 'EOF'
[storage]
driver = "overlay"
graphroot = "/media/leon/Relax/rust-rag/podman-storage"
runroot = "/run/user/1000/containers"
EOF
```

### 5.2 创建目录结构

```bash
BASE="/media/leon/Relax/rust-rag"
mkdir -p "$BASE"/{data/{surrealdb,knowledge-api,prometheus,grafana},config/{surrealdb,knowledge-api,prometheus,grafana},kube,secrets}
```

### 5.3 创建 Pod

```bash
podman pod create \
  --name rust-rag \
  -p 3000:3000 \
  -p 3001:3001 \
  -p 8000:8000 \
  -p 9090:9090 \
  -p 3002:3000
```

### 5.4 启动 SurrealDB

```bash
podman run -d \
  --pod rust-rag \
  --name surrealdb \
  --restart unless-stopped \
  -e SURREAL_USER=leon \
  -e SURREAL_PASS="${SURREALDB_PASSWORD}" \
  -e SURREAL_LOG=info \
  -v /media/leon/Relax/rust-rag/data/surrealdb:/data:Z \
  surrealdb/surrealdb:v2.1.4 \
  start --bind 0.0.0.0:8000 --user leon --pass "${SURREALDB_PASSWORD}" rocksdb:///data
```

### 5.5 开发阶段：API 本地编译 + 连接 Pod 内 SurrealDB

```bash
cd /media/leon/WorkSpace/WorkSpace/RustRAG++

# 配置连接
# config.toml 中 database.addr = "ws://localhost:8000"

# 初始化 Schema
surreal sql \
  --endpoint http://localhost:8000 \
  --user leon --pass "${SURREALDB_PASSWORD}" \
  --ns knowledge --db knowledge \
  --file crates/knowledge-core/schema.surql

# 编译运行
cargo run -p knowledge-api
```

### 5.6 商业化就绪：API 移入 Pod

```bash
# 编译 release
cargo build --release --locked -p knowledge-api

# 构建容器镜像
podman build \
  -t knowledge-api:latest \
  -f Containerfile.api \
  /media/leon/WorkSpace/WorkSpace/RustRAG++

# 运行 API 容器
podman run -d \
  --pod rust-rag \
  --name knowledge-api \
  --restart unless-stopped \
  -e RUST_LOG=info \
  -e RUST_BACKTRACE=1 \
  -e CONFIG_PATH=/app/config/config.yaml \
  -e KNOWLEDGE_DB_USERNAME=root \
  -e KNOWLEDGE_DB_PASSWORD="${SURREALDB_PASSWORD}" \
  -e KNOWLEDGE_JWT_SECRET="${JWT_SECRET}" \
  -v /media/leon/Relax/rust-rag/config/knowledge-api:/app/config:Z \
  -v /media/leon/Relax/rust-rag/data/knowledge-api:/app/data:Z \
  knowledge-api:latest
```

### 5.7 导出可移植 YAML

```bash
podman generate kube rust-rag > /media/leon/Relax/rust-rag/kube/rust-rag-pod.yaml
```

---

## 6. 监控栈（可选）

### 6.1 Prometheus

```bash
podman run -d \
  --pod rust-rag \
  --name prometheus \
  --restart unless-stopped \
  -v /media/leon/Relax/rust-rag/config/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml:Z \
  -v /media/leon/Relax/rust-rag/data/prometheus:/prometheus:Z \
  prom/prometheus:v2.51.0 \
  --config.file=/etc/prometheus/prometheus.yml \
  --storage.tsdb.path=/prometheus
```

### 6.2 Grafana

```bash
podman run -d \
  --pod rust-rag \
  --name grafana \
  --restart unless-stopped \
  -e GF_SECURITY_ADMIN_PASSWORD="${GRAFANA_ADMIN_PASSWORD}" \
  -e GF_USERS_ALLOW_SIGN_UP=false \
  -v /media/leon/Relax/rust-rag/data/grafana:/var/lib/grafana:Z \
  -v /media/leon/Relax/rust-rag/config/grafana/provisioning:/etc/grafana/provisioning:Z \
  grafana/grafana:10.4.0
```

---

## 7. 备份策略

### 7.1 btrfs 快照（秒级）

```bash
# 创建快照
sudo btrfs subvolume snapshot \
  /media/leon/Relax/rust-rag \
  /media/leon/Relax/snapshots/rust-rag-$(date +%Y%m%d)

# 回滚
sudo btrfs subvolume delete /media/leon/Relax/rust-rag
sudo btrfs subvolume snapshot \
  /media/leon/Relax/snapshots/rust-rag-20260510 \
  /media/leon/Relax/rust-rag
```

### 7.2 SurrealDB 逻辑备份

```bash
# 每日导出
surreal export \
  --endpoint http://localhost:8000 \
  --user root --pass "${SURREALDB_PASSWORD}" \
  --ns knowledge --db knowledge \
  /media/leon/Relax/rust-rag/data/backups/knowledge_$(date +%Y%m%d).surql
```

### 7.3 增量复制到其他节点

```bash
sudo btrfs send /media/leon/Relax/snapshots/rust-rag-20260510 | \
  ssh other-machine "sudo btrfs receive /media/leon/Relax/"
```

---

## 8. 安全加固

| 层级 | 措施 | 优先级 |
|------|------|--------|
| 存储 | btrfs 子卷隔离 + 透明压缩 | 高 |
| 网络 | Pod 内 localhost 通信，仅映射必要端口 | 高 |
| 认证 | SurrealDB `--user leon/--pass`，API 使用同名账户 | 高 |
| TLS | 生产环境 `wss://` + Let's Encrypt | 高 |
| Rootless | Podman 以普通用户运行 | 高 |
| 密钥 | 凭据从 Vault/Secrets Manager 注入 | 高 |
| 文件权限 | `env` 文件 600，`key.aes` 文件 600 | 中 |

---

## 9. 扩容路径

```
单机 Pod (podman play kube)
    ↓
多机复制 (btrfs send/receive)
    ↓
K8s Deployment + StatefulSet (kubectl apply)
    ↓
云原生 (Helm Chart + Operator)
```

---

## 10. 优劣势总结

### 优势

- **开发→生产一致性**：同一 Pod 定义，零修改迁移
- **一键可移植**：`podman generate kube` / `podman play kube`
- **渐进式集成**：先容器化 DB，后容器化 API
- **无守护进程**：Podman 无 daemon，更安全更轻量
- **Rootless 安全**：容器以普通用户运行
- **btrfs 快照**：秒级快照/回滚
- **Pod 网络简化**：容器间 localhost 直连
- **扩容路径清晰**：单机 Pod → K8s，YAML 直接复用

### 劣势

- **NTFS 不兼容**：必须转 btrfs/ext4
- **Podman 学习曲线**：与 Docker 有少量差异
- **容器内编译慢**：开发阶段 API 不容器化可规避
- **btrfs 需维护**：需定期 balance 和 scrub
- **Rootless 限制**：无法绑定 <1024 端口（不影响功能）
