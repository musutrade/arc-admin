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
PIPELINE_DIR="$(CDPATH= cd -- "$CALLER_DIR" && pwd)"
case "$PIPELINE_DIR" in
  */scripts|*/hooks) PIPELINE_DIR="$(dirname "$PIPELINE_DIR")" ;;
esac

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
elif [ -d "$PIPELINE_DIR/frontend" ] && [ -d "$PIPELINE_DIR/backend" ]; then
  PROJECT_ROOT="$PIPELINE_DIR"
else
  PROJECT_ROOT="$(detect_project_root "$PIPELINE_DIR")"
fi
if [ -z "$PROJECT_ROOT" ]; then
  echo "⚠️ 未找到项目根目录（缺少 frontend/ 与 backend/），回退到当前目录: $PWD" >&2
  PROJECT_ROOT="$PWD"
fi

REPORT_DIR="${REPORT_DIR:-$PIPELINE_DIR/.codex/reports}"
AUDITOR_BIN="${AUDITOR_BIN:-$PIPELINE_DIR/tools/auditor/target/release/auditor}"
AUDITOR_CONFIG="${AUDITOR_CONFIG:-$PIPELINE_DIR/.codex/audit.toml}"

