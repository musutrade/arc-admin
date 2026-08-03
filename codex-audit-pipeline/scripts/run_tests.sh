#!/bin/bash
# 测试执行器：lint → cargo check → 两端测试 → 生成 test_result.md
# 协议：文件【最后一行】固定为 TEST_SUMMARY: PASS / FAIL / SKIP（不截断，保证可解析）
# 用法：
#   RUN_RUST=false RUN_ANGULAR=true bash codex-audit-pipeline/scripts/run_tests.sh
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/common.sh
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

mkdir -p "$REPORT_DIR/logs"
RAW_LOG_DIR="$REPORT_DIR/logs"

REPORT_FILE="$REPORT_DIR/test_result.md"
: > "$REPORT_FILE"

cd "$PROJECT_ROOT"

BACKEND_DIR="${BACKEND_DIR:-backend}"
FRONTEND_DIR="${FRONTEND_DIR:-frontend}"
RUN_RUST="${RUN_RUST:-true}"
RUN_ANGULAR="${RUN_ANGULAR:-true}"

echo "=== 测试执行报告 ===" >> "$REPORT_FILE"
echo "执行时间: $(date '+%Y-%m-%d %H:%M:%S')" >> "$REPORT_FILE"
echo "范围: RUN_RUST=$RUN_RUST RUN_ANGULAR=$RUN_ANGULAR" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

# ------------------------------------------------------------
# 阶段 1：Lint 门控（先检查；失败则自动修复后复检，并在报告中提示人工复核 diff）
# ------------------------------------------------------------
echo "🔧 [1/5] Lint 检查..."

if [ "$RUN_RUST" = true ] && [ -f "$BACKEND_DIR/Cargo.toml" ]; then
  echo "  >> Rust Clippy..."
  if ! cargo clippy --manifest-path="$BACKEND_DIR/Cargo.toml" -- -D warnings > "$RAW_LOG_DIR/clippy_check.log" 2>&1; then
    echo "  >> Clippy 未通过，尝试自动修复..."
    cargo clippy --manifest-path="$BACKEND_DIR/Cargo.toml" --fix --allow-dirty --allow-staged > "$RAW_LOG_DIR/clippy_fix.log" 2>&1 || true
    if cargo clippy --manifest-path="$BACKEND_DIR/Cargo.toml" -- -D warnings > "$RAW_LOG_DIR/clippy_check2.log" 2>&1; then
      echo "  ✅ Clippy 自动修复成功（⚠️ 请人工复核 git diff 后再提交）"
    else
      echo "### 未修复的 Clippy 错误:" >> "$REPORT_FILE"
      grep -E "^error" "$RAW_LOG_DIR/clippy_check2.log" | head -n 8 >> "$REPORT_FILE"
      echo "TEST_SUMMARY: FAIL (LINT_ERROR)" >> "$REPORT_FILE"
      echo "❌ 阶段 1 失败: Clippy 未通过"
      exit 1
    fi
  fi
fi

if [ "$RUN_ANGULAR" = true ] && [ -d "$FRONTEND_DIR/src" ]; then
  echo "  >> Angular ESLint..."
  if ! (cd "$FRONTEND_DIR" && npx eslint src --max-warnings=0) > "$RAW_LOG_DIR/eslint_check.log" 2>&1; then
    echo "  >> ESLint 未通过，尝试自动修复..."
    (cd "$FRONTEND_DIR" && npx eslint src --fix) > "$RAW_LOG_DIR/eslint_fix.log" 2>&1 || true
    if (cd "$FRONTEND_DIR" && npx eslint src --max-warnings=0) > "$RAW_LOG_DIR/eslint_check2.log" 2>&1; then
      echo "  ✅ ESLint 自动修复成功（⚠️ 请人工复核 git diff 后再提交）"
    else
      echo "### 未修复的 ESLint 错误:" >> "$REPORT_FILE"
      grep -E "error" "$RAW_LOG_DIR/eslint_check2.log" | head -n 8 >> "$REPORT_FILE"
      echo "TEST_SUMMARY: FAIL (LINT_ERROR)" >> "$REPORT_FILE"
      echo "❌ 阶段 1 失败: ESLint 未通过"
      exit 1
    fi
  fi
fi

echo "✅ Lint 通过"

# ------------------------------------------------------------
# 阶段 2：编译验证
# ------------------------------------------------------------
if [ "$RUN_RUST" = true ] && [ -f "$BACKEND_DIR/Cargo.toml" ]; then
  echo "🔍 [2/5] cargo check..."
  if ! cargo check --manifest-path="$BACKEND_DIR/Cargo.toml" > "$RAW_LOG_DIR/check_output.log" 2>&1; then
    echo "### 编译失败详情:" >> "$REPORT_FILE"
    grep -E "^error" "$RAW_LOG_DIR/check_output.log" | head -n 8 >> "$REPORT_FILE"
    echo "TEST_SUMMARY: FAIL (COMPILE_ERROR)" >> "$REPORT_FILE"
    echo "❌ 阶段 2 失败: cargo check 未通过"
    exit 1
  fi
  echo "  ✅ 编译通过"
fi

# ------------------------------------------------------------
# 阶段 3：Rust 测试
# ------------------------------------------------------------
RUST_OK=true
if [ "$RUN_RUST" = true ] && [ -f "$BACKEND_DIR/Cargo.toml" ]; then
  echo "🔍 [3/5] cargo test (超时 60s)..."
  RUST_LOG="$RAW_LOG_DIR/rust_test_raw.log"
  timeout 60 cargo test --manifest-path="$BACKEND_DIR/Cargo.toml" -- --nocapture > "$RUST_LOG" 2>&1
  RUST_EXIT=$?

  if [ "$RUST_EXIT" -ne 0 ]; then
    RUST_OK=false
    echo "❌ Rust 测试: 失败" >> "$REPORT_FILE"
    echo "### 失败详情:" >> "$REPORT_FILE"
    if [ -x "$AUDITOR_BIN" ]; then
      "$AUDITOR_BIN" parse-logs --input "$RUST_LOG" --output "$REPORT_DIR/error_context.json" 2>/dev/null
      if [ -s "$REPORT_DIR/error_context.json" ]; then
        echo "结构化错误上下文 (前 10 行):" >> "$REPORT_FILE"
        head -n 10 "$REPORT_DIR/error_context.json" >> "$REPORT_FILE"
      fi
    else
      grep -E "FAILED|assert|panicked|^error" "$RUST_LOG" 2>/dev/null | head -n 10 >> "$REPORT_FILE"
    fi
  else
    echo "  ✅ Rust 测试: 通过"
  fi
fi

# ------------------------------------------------------------
# 阶段 4：Angular 测试
# ------------------------------------------------------------
ANG_OK=true
if [ "$RUN_ANGULAR" = true ] && [ -d "$FRONTEND_DIR/src" ]; then
  echo "🔍 [4/5] Angular 测试 (超时 120s)..."
  NG_LOG="$RAW_LOG_DIR/ng_test_raw.log"

  if command -v google-chrome >/dev/null 2>&1 || command -v chromium >/dev/null 2>&1 || command -v chrome >/dev/null 2>&1; then
    BROWSER_ARGS="--browsers=ChromeHeadless"
  else
    # 无 Chrome 时依赖项目已配置 @angular/build:unit-test (Vitest/jsdom)；
    # 不要向 karma 传 --browsers=jsdom（karma 不支持）
    BROWSER_ARGS=""
  fi

  # shellcheck disable=SC2086
  ( cd "$FRONTEND_DIR" && timeout 120 npx ng test --watch=false $BROWSER_ARGS ) > "$NG_LOG" 2>&1
  NG_EXIT=$?

  if [ "$NG_EXIT" -eq 0 ]; then
    echo "  ✅ Angular 测试: 通过"
  else
    ANG_OK=false
    echo "❌ Angular 测试: 失败" >> "$REPORT_FILE"
    echo "### 失败详情:" >> "$REPORT_FILE"
    grep -E "FAILED|Expected|Received|Error:" "$NG_LOG" 2>/dev/null | head -n 10 >> "$REPORT_FILE"
  fi
fi

# ------------------------------------------------------------
# 阶段 5：汇总（最后一行必须是 TEST_SUMMARY，禁止截断）
# ------------------------------------------------------------
echo "" >> "$REPORT_FILE"
if [ "$RUN_RUST" = true ] && [ "$RUN_ANGULAR" = true ]; then
  if [ "$RUST_OK" = true ] && [ "$ANG_OK" = true ]; then
    echo "TEST_SUMMARY: PASS" >> "$REPORT_FILE"
  else
    echo "TEST_SUMMARY: FAIL" >> "$REPORT_FILE"
  fi
elif [ "$RUN_RUST" = true ]; then
  [ "$RUST_OK" = true ] && echo "TEST_SUMMARY: PASS" >> "$REPORT_FILE" || echo "TEST_SUMMARY: FAIL" >> "$REPORT_FILE"
elif [ "$RUN_ANGULAR" = true ]; then
  [ "$ANG_OK" = true ] && echo "TEST_SUMMARY: PASS" >> "$REPORT_FILE" || echo "TEST_SUMMARY: FAIL" >> "$REPORT_FILE"
else
  echo "TEST_SUMMARY: SKIP" >> "$REPORT_FILE"
fi

SUMMARY=$(tail -n 1 "$REPORT_FILE")
echo "✅ 测试报告已生成: $REPORT_FILE"
echo "$SUMMARY"

case "$SUMMARY" in
  "TEST_SUMMARY: PASS" | "TEST_SUMMARY: SKIP") exit 0 ;;
  *) exit 1 ;;
esac
