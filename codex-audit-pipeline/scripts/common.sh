#!/bin/sh
# scripts/common.sh —— 路径与工具定位（POSIX sh / bash 通用）
#
# 调用方必须传入自身所在目录（scripts/ 或 hooks/），例如：
#   bash: source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"
#   sh  : . "$HOOKS_DIR/../scripts/common.sh" "$HOOKS_DIR"
#
# 输出变量：
#   PIPELINE_DIR      codex-audit-pipeline 目录
#   PROJECT_ROOT      仓库根（自动向上查找含 frontend/ + backend/ 的目录）
#   REPORT_DIR        报告输出目录（默认 $PIPELINE_DIR/.codex/reports）
#   AUDITOR_BIN       auditor 可执行文件
#   AUDITOR_CONFIG    audit.toml 路径

CALLER_DIR="$1"
CALLER_DIR="$(CDPATH='' cd -- "$CALLER_DIR" && pwd)"

detect_project_root() {
  dir="$1"
  while [ "$dir" != "/" ] && [ -n "$dir" ]; do
    if [ -d "$dir/frontend" ] && [ -d "$dir/backend" ]; then
      printf '%s\n' "$dir"
      return 0
    fi
    dir="$(dirname "$dir")"
  done
}

if [ -n "${PROJECT_ROOT:-}" ]; then
  :
else
  PROJECT_ROOT="$(detect_project_root "$CALLER_DIR")"
fi
if [ -z "$PROJECT_ROOT" ]; then
  echo "⚠️ 未找到项目根目录（缺少 frontend/ 与 backend/），回退到当前目录: $PWD" >&2
  PROJECT_ROOT="$PWD"
fi

# pipeline 目录：优先按仓库根下的固定位置，其次按调用方所在目录推导
# （git 钩子执行时 $0 可能指向 .git/hooks/...，不能依赖 $0 推导）
if [ -f "$PROJECT_ROOT/codex-audit-pipeline/.codex/audit.toml" ]; then
  PIPELINE_DIR="$PROJECT_ROOT/codex-audit-pipeline"
else
  PIPELINE_DIR="$(dirname "$CALLER_DIR")"
  case "$PIPELINE_DIR" in
    */scripts|*/hooks) PIPELINE_DIR="$(dirname "$PIPELINE_DIR")" ;;
  esac
fi

REPORT_DIR="${REPORT_DIR:-$PIPELINE_DIR/.codex/reports}"
AUDITOR_BIN="${AUDITOR_BIN:-$PIPELINE_DIR/tools/auditor/target/release/auditor}"
AUDITOR_CONFIG="${AUDITOR_CONFIG:-$PIPELINE_DIR/.codex/audit.toml}"
