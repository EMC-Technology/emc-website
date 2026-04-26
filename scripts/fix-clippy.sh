#!/bin/bash
# =============================================================================
# fix-clippy.sh — Clippy 警告自动修复与清零计划
# =============================================================================
#
# 用途:
#   自动运行 Clippy 严格模式检查，尝试自动修复可修复的警告，
#   并输出需要手动审查的剩余警告列表。
#
# 使用方式:
#   bash scripts/fix-clippy.sh
#   或: make clippy-fix
#
# 目标: 零 Clippy warnings (World-class 标准)
# =============================================================================

set -euo pipefail

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${BOLD}══════════════════════════════════════════${NC}"
echo -e "${BOLD}  Clippy 警告清零计划 — World-class DX${NC}"
echo -e "${BOLD}══════════════════════════════════════════${NC}"
echo ""

# =============================================================================
# Phase 1: 严格模式 Clippy 检查
# =============================================================================

echo -e "${BLUE}[Phase 1] 运行 Clippy 严格模式检查...${NC}"
echo ""

CLIPPY_OUTPUT="clippy_output_$(date +%Y%m%d_%H%M%S).txt"

cargo clippy --workspace --all-targets --all-features \
    -W clippy::all \
    -W clippy::pedantic \
    -W clippy::nursery \
    -D warnings \
    2>&1 | tee "$CLIPPY_OUTPUT" || true

# 统计警告数量
WARNING_COUNT=$(grep -c "warning\[" "$CLIPPY_OUTPUT" 2>/dev/null || echo "0")
ERROR_COUNT=$(grep -c "error\[" "$CLIPPY_OUTPUT" 2>/dev/null || echo "0")
EXIT_CODE=${PIPESTATUS[0]:-0}

echo ""
echo -e "${BLUE}━━━ 检查结果 ━━━${NC}"
echo -e "  Warnings: ${YELLOW}${WARNING_COUNT}${NC}"
echo -e "  Errors:   ${RED}${ERROR_COUNT}${NC}"
echo -e "  Exit Code: ${EXIT_CODE}"

if [ "$EXIT_CODE" -eq 0 ] && [ "$WARNING_COUNT" -eq 0 ]; then
    echo ""
    echo -e "${GREEN}🎉 恭喜！零 Clippy warnings 已达成！${NC}"
    echo -e "${GREEN}   项目已达到世界级代码质量标准。${NC}"
    rm -f "$CLIPPY_OUTPUT"
    exit 0
fi

# =============================================================================
# Phase 2: 尝试自动修复
# =============================================================================

echo ""
echo -e "${BLUE}[Phase 2] 尝试自动修复...${NC}"

# 提取可自动修复的警告类型
AUTO_FIXABLE=$(grep -oE 'warning\[[^]]+\]' "$CLIPPY_OUTPUT" 2>/dev/null \
    | sort -u \
    | grep -E '(unused_imports|dead_code|redundant_clone|needless_borrow|map_flatten|clone_on_copy|explicit_iter_loop)' \
    || true)

if [ -n "$AUTO_FIXABLE" ]; then
    echo -e "  检测到可能可自动修复的警告:"
    echo "$AUTO_FIXABLE" | sed 's/^/    - /'
    echo ""

    read -p "  是否执行自动修复? [y/N] " -n 1 -r
    echo ""

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${BLUE}  执行 cargo clippy --fix ...${NC}"

        cargo clippy --fix --allow-dirty --allow-staged --workspace --all-features \
            -W clippy::all \
            -W clippy::pedantic \
            2>&1 || true

        echo ""
        echo -e "${GREEN}✅ 自动修复完成。请重新运行此脚本验证结果。${NC}"
    else
        echo -e "${YELLOW}  跳过自动修复。${NC}"
    fi
else
    echo -e "  ${YELLOW}未检测到常见的可自动修复警告。${NC}"
fi

# =============================================================================
# Phase 3: 分类输出剩余警告
# =============================================================================

echo ""
echo -e "${BLUE}[Phase 3] 剩余警告分类报告${NC}"
echo ""

# 按类别统计
echo -e "${BOLD}按 Lint 组分类:${NC}"
for group in all pedantic nursery restriction; do
    COUNT=$(grep -c "clippy::${group}" "$CLIPPY_OUTPUT" 2>/dev/null || echo "0")
    if [ "$COUNT" -gt 0 ]; then
        echo -e "  ${group}: ${RED}${COUNT}${NC}"
    fi
done

echo ""
echo -e "${BOLD}高频警告 Top 10:${NC}"
grep -oE 'warning\[[^]]+\]' "$CLIPPY_OUTPUT" 2>/dev/null \
    | sort \
    | uniq -c \
    | sort -rn \
    | head -10 \
    | while read count warning; do
        printf "  %-45s %s\n" "$warning" "${RED}${count}${NC}"
    done

echo ""
echo -e "${BOLD}涉及文件:${NC}"
grep -E '-->' "$CLIPPY_OUTPUT" 2>/dev/null \
    | sed 's/.*--> //' \
    | cut -d: -f1 \
    | sort \
    | uniq -c \
    | sort -rn \
    | head -10

# =============================================================================
# Phase 4: 手动修复建议
# =============================================================================

echo ""
echo -e "${BLUE}[Phase 4] 手动修复指南${NC}"
echo ""

cat << 'GUIDE'
常见警告类型及修复方法:

1. clippy::too_many_arguments (>7)
   → 将参数封装为结构体 (Options/Config pattern)

2. clippy::large_types_passed_by_value
   → 改为传递引用 &T

3. clippy::result_large_err
   → 在 clippy.toml 中配置 allow 或使用 Box<dyn Error>

4. clippy::missing_errors_doc
   → 为函数添加 # Errors 文档注释

5. clippy::module_name_repetitions
   → 重命名模块避免与父模块名称重复

6. clippy::unwrap_used (在非测试代码中)
   → 改用 .expect("描述") 或 ? 操作符

7. clippy::panic_in_result_fn
   → 返回 Err 而非 panic

详细文档: https://rust-lang.github.io/rust-clippy/master/index.html
GUIDE

# =============================================================================
# 完成
# =============================================================================

echo ""
echo -e "${BOLD}══════════════════════════════════════════${NC}"
echo -e "  完整输出已保存至: ${CLIPPY_OUTPUT}"
echo -e "  下一步: 根据上述分类逐项修复剩余警告"
echo -e "${BOLD}══════════════════════════════════════════${NC}"
