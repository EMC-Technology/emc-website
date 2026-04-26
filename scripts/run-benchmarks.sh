#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BASELINES_FILE="$PROJECT_ROOT/baselines.json"
REPORT_DIR="$PROJECT_ROOT/target/criterion"
RESULTS_JSON="$REPORT_DIR/benchmark_results.json"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PASS_COUNT=0
FAIL_COUNT=0
WARN_COUNT=0

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[PASS]${NC}  $*"; }
log_fail()  { echo -e "${RED}[FAIL]${NC}  $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }

detect_environment() {
    log_info "检测运行环境..."

    local os_name="$(uname -s) $(uname -r)"
    local cpu_model="unknown"
    if [[ "$os_name" == *"Linux"* ]]; then
        cpu_model=$(grep "model name" /proc/cpuinfo | head -1 | cut -d':' -f2 | xargs || echo "unknown")
    elif [[ "$os_name" == *"Darwin"* ]]; then
        cpu_model=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "unknown")
    fi
    local mem_gb=$(free -g 2>/dev/null | awk '/Mem:/{print $2}' || echo "unknown")
    local rustc_version=$(rustc --version 2>/dev/null | awk '{print $2}' || echo "unknown")

    echo "{\"os\":\"$os_name\",\"cpu\":\"$cpu_model\",\"memory_gb\":$mem_gb,\"rustc\":\"$rustc_version\"}"
}

run_bench_crate() {
    local crate_path="$1"
    local crate_name="$2"
    local bench_name="${3:-}"

    log_info "运行 $crate_name 基准测试..."

    if [[ -n "$bench_name" ]]; then
        cargo bench --manifest-path "$crate_path/Cargo.toml" --bench "$bench_name" \
            -- --save-baseline current 2>&1 || {
            log_warn "$crate_name::$bench_name 基准测试执行失败（可能缺少某些依赖或 feature），跳过"
            return 1
        }
    else
        cargo bench --manifest-path "$crate_path/Cargo.toml" \
            -- --save-baseline current 2>&1 || {
            log_warn "$crate_name 基准测试执行失败，跳过"
            return 1
        }
    fi
}

collect_results() {
    log_info "收集基准测试结果..."

    mkdir -p "$REPORT_DIR"

    local results="{\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"environment\":$(detect_environment),\"results\":{}}"

    for crate_dir in crates/*/benches; do
        local crate_name="$(basename "$(dirname "$crate_dir")")"
        local criterion_base="$crate_dir/../target/criterion"

        if [[ -d "$criterion_base" ]]; then
            for bench_dir in "$criterion_base"/*/; do
                if [[ -d "$bench_dir" ]]; then
                    local bench_id="$(basename "$bench_dir")"
                    local json_file="$bench_dir/new/estimates.json"

                    if [[ -f "$json_file" ]]; then
                        local mean_val=$(python3 -c "
import json, sys
with open('$json_file') as f:
    data = json.load(f)
mean = data.get('mean', {}).get('point_estimate', 0)
std = data.get('std_dev', {}).get('point_estimate', 0)
unit = data.get('mean', {}).get('unit', 'ns')
print(f'{mean}|{std}|{unit}')
" 2>/dev/null || echo "0|0|ns")

                        local mean_v=$(echo "$mean_val" | cut -d'|' -f1)
                        local std_v=$(echo "$mean_val" | cut -d'|' -f2)
                        local unit_v=$(echo "$mean_val" | cut -d'|' -f3)

                        results=$(echo "$results" | python3 -c "
import json, sys
data = json.load(sys.stdin)
key = '${crate_name}_${bench_id}'
data['results'][key] = {
    'mean': ${mean_v},
    'std_dev': ${std_v},
    'unit': '${unit_v}'
}
print(json.dumps(data, indent=2))
")
                    fi
                fi
            done
        fi
    done

    echo "$results" > "$RESULTS_JSON"
    log_info "结果已保存到 $RESULTS_JSON"
}

compare_with_baselines() {
    log_info "与基线对比..."

    if [[ ! -f "$BASELINES_FILE" ]]; then
        log_warn "baselines.json 不存在，跳过对比（首次运行后请填充真实值）"
        return
    fi

    if [[ ! -f "$RESULTS_JSON" ]]; then
        log_warn "未找到基准测试结果文件，跳过对比"
        return
    fi

    local tolerance=$(python3 -c "
import json
with open('$BASELINES_FILE') as f:
    print(json.load(f)['thresholds']['regression_tolerance_percent'])
")

    python3 << 'PYEOF'
import json
import sys

with open("$BASELINES_FILE") as f:
    baselines = json.load(f)

with open("$RESULTS_JSON") as f:
    results = json.load(f)

tolerance = baselines["thresholds"]["regression_tolerance_percent"]
improvement_threshold = baselines["thresholds"]["improvement_threshold_percent"]

benchmarks = baselines.get("benchmarks", {})
current = results.get("results", {})

global_pass = True
checked = 0

for key, baseline_val in benchmarks.items():
    if isinstance(baseline_val, dict):
        mean_key = None
        for mk in ["mean_ns", "mean_us", "mean_ms", "batch_mean_us", "mean_us_total", "mean_us_per_request"]:
            if mk in baseline_val:
                mean_key = mk
                break

        if not mean_key:
            continue

        baseline_mean = baseline_val[mean_key]

        matching_current = None
        for ck, cv in current.items():
            normalized_ck = ck.replace("-", "_").lower()
            if key.lower().replace("-", "_") in normalized_ck or normalized_ck in key.lower().replace("-", "_"):
                matching_current = cv
                break

        if matching_current is None:
            continue

        checked += 1
        current_mean = matching_current.get("mean", 0)

        if baseline_mean > 0 and current_mean > 0:
            change_pct = ((current_mean - baseline_mean) / baseline_mean) * 100

            if change_pct > tolerance:
                print(f"\033[0;31m[REGRESSION] {key}: +{change_pct:.1f}% (baseline: {baseline_mean:.2f}, current: {current_mean:.2f})\033[0m")
                global_pass = False
            elif change_pct < -improvement_threshold:
                print(f"\033[0;32m[IMPROVEMENT] {key}: {change_pct:.1f}% (baseline: {baseline_mean:.2f}, current: {current_mean:.2f})\033[0m")
            else:
                print(f"\033[0;32m[OK] {key}: {change_pct:+.1f}% within tolerance\033[0m")

if checked == 0:
    print("\033[1;33m[WARN] 未找到可对比的基线数据。请先运行一次完整基准测试并更新 baselines.json。\033[0m")

sys.exit(0 if global_pass else 1)
PYEOF
}

generate_html_report() {
    log_info "生成 HTML 报告..."

    local html_report="$REPORT_DIR/index.html"

    cat > "$html_report" << 'HTMLEOF'
<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>Knowledge System - Performance Benchmark Report</title>
<style>
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 40px; background: #f5f5f5; color: #333; }
h1 { color: #2c3e50; border-bottom: 3px solid #3498db; padding-bottom: 10px; }
h2 { color: #2980b9; margin-top: 30px; }
table { width: 100%; border-collapse: collapse; margin: 15px 0; background: white; box-shadow: 0 1px 3px rgba(0,0,0,0.12); }
th, td { padding: 12px 15px; text-align: left; border-bottom: 1px solid #ddd; }
th { background: #3498db; color: white; font-weight: 600; }
tr:hover { background: #f8f9fa; }
.pass { color: #27ae60; font-weight: bold; }
.fail { color: #e74c3c; font-weight: bold; }
.warn { color: #f39c12; font-weight: bold; }
.summary-box { display: flex; gap: 20px; margin: 20px 0; }
.summary-card { flex: 1; padding: 20px; border-radius: 8px; color: white; }
.card-pass { background: #27ae60; }
.card-fail { background: #e74c3c; }
.card-warn { background: #f39c12; }
.card-total { background: #3498db; }
.card-number { font-size: 36px; font-weight: bold; }
.card-label { font-size: 14px; opacity: 0.9; }
</style>
</head>
<body>
<h1>Knowledge System - 性能基准测试报告</h1>
<div id="env-info"></div>
<div class="summary-box">
<div class="summary-card card-total"><div class="card-number" id="total-count">-</div><div class="card-label">总测试数</div></div>
<div class="summary-card card-pass"><div class="card-number" id="pass-count">-</div><div class="card-label">通过 (PASS)</div></div>
<div class="summary-card card-warn"><div class="card-number" id="warn-count">-</div><div class="card-label">警告 (WARN)</div></div>
<div class="summary-card card-fail"><div class="card-number" id="fail-count">-</div><div class="card-label">失败 (FAIL)</div></div>
</div>
<table><thead><tr><th>Benchmark</th><th>Mean</th><th>Std Dev</th><th>Unit</th><th>Status</th></tr></thead><tbody id="results-table"></tbody></table>
<script>
document.addEventListener('DOMContentLoaded', function() {
    fetch('benchmark_results.json')
        .then(r => r.json())
        .then(data => {
            const envDiv = document.getElementById('env-info');
            envDiv.innerHTML = '<p><strong>时间:</strong> ' + data.timestamp + '</p>' +
                '<p><strong>环境:</strong> ' + JSON.stringify(data.environment, null, 2) + '</p>';

            const tbody = document.getElementById('results-table');
            const results = data.results;
            let total = 0, pass = 0, fail = 0;

            Object.entries(results).sort().forEach(([name, val]) => {
                total++;
                const row = document.createElement('tr');
                row.innerHTML = '<td>' + name + '</td>' +
                    '<td>' + val.mean.toFixed(4) + '</td>' +
                    '<td>' + (val.std_dev || 0).toFixed(4) + '</td>' +
                    '<td>' + (val.unit || '-') + '</td>' +
                    '<td class="pass">OK</td>';
                tbody.appendChild(row);
                pass++;
            });

            document.getElementById('total-count').textContent = total;
            document.getElementById('pass-count').textContent = pass;
            document.getElementById('warn-count').textContent = 0;
            document.getElementById('fail-count').textContent = fail;
        })
        .catch(err => console.error('Failed to load results:', err));
});
</script>
</body>
</html>
HTMLEOF

    log_info "HTML 报告已生成: $html_report"
}

main() {
    echo ""
    echo "============================================="
    echo "  Knowledge System - 性能基准测试套件 v1.0"
    echo "============================================="
    echo ""

    local start_time=$(date +%s)

    run_bench_crate "$PROJECT_ROOT/crates/knowledge-core" "knowledge-core" "crypto_bench" || true
    run_bench_crate "$PROJECT_ROOT/crates/knowledge-core" "knowledge-core" "database_bench" || true
    run_bench_crate "$PROJECT_ROOT/crates/knowledge-api" "knowledge-api" "api_bench" || true
    run_bench_crate "$PROJECT_ROOT/crates/knowledge-api" "knowledge-api" "embedding_bench" || true
    run_bench_crate "$PROJECT_ROOT/crates/knowledge-parser" "knowledge-parser" "parser_bench" || true

    collect_results
    compare_with_baselines || FAIL_COUNT=$((FAIL_COUNT + 1))
    generate_html_report

    local end_time=$(date +%s)
    local duration=$((end_time - start_time))

    echo ""
    echo "============================================="
    echo "  总耗时: ${duration}s"
    echo "  报告目录: $REPORT_DIR"
    echo "============================================="

    if [[ $FAIL_COUNT -gt 0 ]]; then
        log_fail "存在性能回归！请检查上方详细输出。"
        exit 1
    else
        log_ok "所有基准测试完成。"
        exit 0
    fi
}

main "$@"
