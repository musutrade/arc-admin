# arc-flow

`arc-flow` 是可复用的 Rust 开发工作流与架构门禁 CLI。源码位于 `codex-audit-pipeline/tools/arc-flow/`；独立二进制是标准入口，本仓库额外提供 `cargo flow` 别名。

它负责 changed paths、secret scan、audit、doctor、外部命令编排、测试结果计数和临时服务生命周期。Git hook 只保留启动器，流程判断不依赖 Shell 脚本。

## 安装与初始化

从源码安装独立二进制：

```bash
cargo install --path codex-audit-pipeline/tools/arc-flow
arc-flow presets
arc-flow --project-root /path/to/new-project init --preset rust-api
arc-flow --project-root /path/to/new-project config check
```

内置预设：

| 预设 | 用途 |
| --- | --- |
| `generic` | Git 项目和最小 whitespace check |
| `rust-api` | 单 Rust crate 的 fmt、Clippy、check、test |
| `angular-only` | Angular/npm 的 lint、test、build |
| `angular-rust-postgres` | Angular + Rust + 临时 PostgreSQL |

`init` 写入 `.arc-flow/flow.toml`、`.arc-flow/audit.toml` 和忽略报告目录的 `.arc-flow/.gitignore`，不会覆盖已有配置，除非显式传入 `--force`。

## 命令

| 命令 | 用途 |
| --- | --- |
| `arc-flow presets` | 列出内置项目预设 |
| `arc-flow init --preset <name>` | 在空项目中生成 schema v2 配置 |
| `arc-flow doctor` | 执行配置声明的环境检查 |
| `arc-flow scope` | 列出变更和动态选择的 components |
| `arc-flow secrets` | 扫描高置信凭据模式，只输出文件名 |
| `arc-flow audit` | 执行配置的正则架构规则 |
| `arc-flow verify` | 执行默认 profile |
| `arc-flow verify --profile ci --all` | 执行任意已声明 profile 和全部 components |
| `arc-flow hook` | 对暂存区执行配置的 hook profile |
| `arc-flow step <id>` | 通过 secrets/audit 后单独运行一个步骤 |
| `arc-flow config check` | 校验 schema、引用和环境覆盖 |
| `arc-flow config print --resolved` | 输出最终生效配置 |
| `arc-flow config migrate` | 将 schema v1 配置转换成 v2 |
| `arc-flow parse-logs` | 提取 JSON Lines ERROR trace 上下文 |

本仓库以下命令等价：

```bash
cargo flow scope
cargo flow verify --components backend,frontend
cargo flow verify --profile full --all
```

## Schema v2

项目根由 `.arc-flow/flow.toml` 标识，不再要求固定的 `backend/`、`frontend/` 或工具源码目录。component 和 profile 都是配置中的小写字符串 ID。

```toml
version = 2

[project]
name = "example"
default_profile = "full"
hook_profile = "hook"

[paths]
reports = ".arc-flow/reports"
audit_config = ".arc-flow/audit.toml"

[paths.aliases.api]
path = "services/api"
env = "API_DIR"

[[scope.rules]]
patterns = ["services/api/**"]
components = ["api"]

[[steps]]
id = "api.clippy"
label = "API Clippy"
component = "api"
profiles = ["full", "hook", "ci"]
program = "cargo"
args = ["clippy", "--manifest-path", "{api}/Cargo.toml"]
cwd = "{root}"
log = "api_clippy.log"
timeout_secs = 180
```

可用占位符包括 `{root}`、`{reports}`、`{audit_config}` 和任意 `[paths.aliases.*]`。命令使用 `program + args[]`，不经过 Shell 字符串解析。

配置优先级为：内置安全约束 < `flow.toml` < 环境变量 < CLI `--project-root` / `--config`。`REPORT_DIR`、`AUDITOR_CONFIG` 和步骤或服务声明的 `*_env` 字段可覆盖配置值。

## 扩展点

### Doctor

`[[doctor.checks]]` 支持 `command`、`path`、`glob`、`env`、`env-or-file`、`git-config`、`git-remotes`、`version` 和 `service`。`required = false` 的失败表现为 warning，其余为 failure。

### 测试解析器

`[parsers.<id>]` 定义一个或多个正则、计数 capture group 和最低结果数。步骤通过 `parser = "<id>"` 引用，因此 Jest、Pytest、Go test 等文本输出无需修改 Rust 代码即可增加零测试保护。

### 临时服务

`[services.<id>]` 支持：

- `kind = "environment"`：从已有环境变量读取连接值并注入步骤；
- `kind = "docker"`：声明镜像、端口、环境、健康检查、连接串和目标环境变量。

Docker provider 可用于 PostgreSQL、MySQL、Redis 等服务。容器使用随机宿主端口，验证结束或异常退出时自动删除。

步骤可用 `services = ["test-postgres", "test-redis"]` 组合多个服务，并以 `remove_env = ["DATABASE_URL"]` 删除继承的运行时变量；当前后端测试会先移除 `DATABASE_URL`，再由 service 注入隔离的 `TEST_DATABASE_URL`。

### 项目策略

`[policy].required_steps` 声明该项目不可缺失的基础步骤。策略属于项目配置，不再编译进通用引擎；新增 component、profile、普通命令、路径别名、正则 parser 或 Docker service 都不需要修改 Rust 源码。

## 固定安全门禁

以下行为不由项目配置关闭：

1. 外部步骤前必须依次通过 secret scan 和 audit；
2. 配置、报告和路径别名不得逃出项目根；
3. 禁止 `sh -c`、`bash -lc` 等 Shell 命令字符串；
4. 未知引用、重复 ID、非法 glob/正则/占位符和越界超时直接失败；
5. service 容器必须声明健康检查并在结束时清理。

## v1 迁移

```bash
arc-flow --project-root /path/to/project config migrate \
  --input codex-audit-pipeline/.codex/flow.toml \
  --output .arc-flow/flow.toml
```

迁移器保留源文件，把原有 backend/frontend、doctor、PostgreSQL、parser、scope 和 steps 转成 v2。目标文件已存在时需要显式 `--force`。

## 验证与报告

`verify` 固定按以下顺序执行：

1. working tree 或 staged snapshot secret scan；
2. auditor 全量架构扫描；
3. 按配置顺序执行选中 component/profile 的步骤。

报告目录由 `[paths].reports` 决定，当前项目仍使用 `codex-audit-pipeline/.codex/reports/`，包含 `review_context.json`、`scope.json`、`secret_scan.json`、`test_result.json`、`test_result.md` 和 `logs/*.log`。

## Git Hook

当前仓库使用：

```bash
git config core.hooksPath codex-audit-pipeline/hooks
```

`pre-commit` 执行 `cargo flow hook`。hook profile 不运行数据库集成测试或 production build；交付前使用 `cargo flow verify --all`。

## 当前审计规则

`codex-audit-pipeline/.codex/audit.toml` 仍约束 arc-admin 的 SQL 写入层、Handler/Service、Angular Component/Service 和代码模板。`arch_rules.allowed_patterns` 可声明逐行例外，不再存在写死的 model trait 放行逻辑。新项目由预设生成空规则文件，再按自身架构增加规则。
