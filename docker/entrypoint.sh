#!/bin/bash
# =============================================================================
# 知识系统入口脚本
# =============================================================================

set -euo pipefail

echo "=========================================="
echo "  文本全结构化知识系统"
echo "=========================================="

# 检查配置
if [ -z "${CONFIG_PATH:-}" ]; then
    echo "[WARN] CONFIG_PATH not set, using default config"
    CONFIG_PATH="/app/config/config.yaml"
fi

# 生成加密密钥（如果不存在）
KEY_FILE="/app/data/key.aes"
if [ ! -f "$KEY_FILE" ]; then
    echo "[INFO] Generating encryption key..."
    mkdir -p /app/data
    openssl rand -hex 32 > "$KEY_FILE"
    chmod 600 "$KEY_FILE"
    echo "[INFO] Encryption key generated at $KEY_FILE"
fi

# 等待 SurrealDB 就绪
DB_ADDR="${CONFIG__DATABASE__ADDR:-ws://surrealdb:8000}"
echo "[INFO] Waiting for SurrealDB at $DB_ADDR..."

MAX_RETRIES=30
RETRY_COUNT=0

while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if curl -sf "$DB_ADDR/version" > /dev/null 2>&1; then
        echo "[INFO] SurrealDB is ready!"
        break
    fi
    
    RETRY_COUNT=$((RETRY_COUNT + 1))
    echo "[INFO] Waiting for SurrealDB... ($RETRY_COUNT/$MAX_RETRIES)"
    sleep 2
done

if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
    echo "[ERROR] SurrealDB is not available after $MAX_RETRIES attempts"
    exit 1
fi

# 启动应用
echo "[INFO] Starting Knowledge API..."
exec knowledge-api "$@"
