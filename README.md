<!-- ARC_PROJECT_HEADER_START -->

# arc-admin

RBAC 管理后台：Angular + Angular Material 前端，Rust (Axum + SQLX) 后端。
<!-- ARC_PROJECT_HEADER_END -->

## 目录结构

```
.
├── AGENTS.md                  # Codex 全局公约（会话启动自动加载）
├── frontend/                  # Angular 22 + Material M3 前端
│   └── src/app/{core,features,layout,pages}
├── backend/                   # Rust + Axum + SQLX 后端
│   ├── src/lib.rs             # 应用路由与可测试的 AppState
│   ├── src/main.rs            # 配置、连接池、迁移与监听入口
│   ├── migrations/            # SQLX 迁移（不可变，只增不改）
│   └── .env.example
├── docs/openapi.yaml          # API 契约（前后端唯一事实来源）
├── scripts/                   # 业务项目初始化命令及测试
├── FRAMEWORK_VERSION          # 模板版本标识
├── .arc-flow/                 # 可复用工作流 schema v2 配置
├── codex-audit-pipeline/      # Codex 工作流工具（自包含）
│   ├── .codex/                # 审计规则 / 模板 / 报告产物
│   ├── hooks/                 # pre-commit（仅启动 arc-flow）
│   └── tools/arc-flow/        # Rust 工作流 CLI
└── .github/                   # CI 与 Dependabot 配置
```

<!-- ARC_TEMPLATE_USAGE_START -->

## 从模板创建业务项目

从 GitHub Template 创建独立仓库后，在新仓库根目录执行一次初始化：

```bash
./scripts/init-project.sh \
  --slug stock-analysis \
  --title 股票分析系统 \
  --short-name 投研平台 \
  --database stock_analysis \
  --permission-prefix stock
```

命令要求 Git 工作区干净，并会拒绝在框架源仓库或已初始化项目中执行。它会生成唯一的本地开发 JWT 密钥和 `backend/.env`，更新运行时产品配置与 README，并写入 `.arc-project.json`。本地 `.env` 受 `.gitignore` 保护，不会进入版本库。

查看所有参数：

```bash
./scripts/init-project.sh --help
```

<!-- ARC_TEMPLATE_USAGE_END -->

## 快速开始

```bash
# 一次性准备
cd frontend && npm ci && cd ..
[[ -f backend/.env ]] || cp backend/.env.example backend/.env
docker pull postgres:16-alpine
git config core.hooksPath codex-audit-pipeline/hooks
cargo flow doctor

# 前后端一键启动（Ctrl+C 会同时停止两个服务）
./start.sh

# 也可以分别启动
# 前端（新终端）
cd frontend && npm start                  # http://localhost:4200

# 首次初始化管理员（密码至少 16 字符，不会写入迁移或日志）
BOOTSTRAP_ADMIN_PASSWORD='replace-with-a-strong-password' \
  cargo run --manifest-path backend/Cargo.toml --bin bootstrap_admin

# 后端（新终端，先创建 DATABASE_URL 指向的本地数据库）
cd backend && cargo run                    # http://localhost:8080/api/v1/healthz
```

首次运行或环境变化后，可从仓库根执行依赖体检（不会连接数据库）：

```bash
cargo flow doctor
```

## Codex 工作流

所有命令从仓库根执行。`cargo flow` 是本仓库在 `.cargo/config.toml` 中定义的别名，会从源码启动 `arc-flow`、自动定位 `.arc-flow/flow.toml`，并将结构化报告写入 `codex-audit-pipeline/.codex/reports/`。

```bash
# 开始编码前：确认变更会触发哪些组件
cargo flow scope

# 编码过程中：只验证工作区变更命中的组件
cargo flow verify

# 只验证指定组件，适合定向排查
cargo flow verify --components backend
cargo flow verify --components frontend

# 检查配置及环境覆盖后的最终值
cargo flow config check
cargo flow config print --resolved

# 提交或创建 PR 前：忽略变更范围，执行所有组件
cargo flow verify --all
```

`verify` 固定先执行凭据扫描和架构审计，门禁通过后才运行配置中的 lint、compile、test、build。后端测试优先使用 `TEST_DATABASE_URL`；未配置时自动启动一次性 PostgreSQL 容器，结束后清理，不使用开发库或生产库。

详细资料：

- [开发指南](docs/development.md)：本项目的本地开发、测试和提交步骤；
- [架构说明](docs/architecture.md)：前后端分层和依赖边界；
- [业务模块扩展指南](docs/business-extension.md)：新业务域的目录、权限、路由和后端接入清单；
- [项目公约](AGENTS.md)：Codex、Reviewer、Tester 和 Git 安全约束；
- [arc-flow 操作手册](codex-audit-pipeline/README.md)：安装、命令、预设、CI、报告和故障排查；
- [schema v2 配置参考](codex-audit-pipeline/docs/configuration.md)：`flow.toml` 与 `audit.toml` 的字段级说明。

## API 契约

[docs/openapi.yaml](docs/openapi.yaml) 是前后端联调的唯一事实来源：

- 后端按契约实现路由与 DTO（`/api/v1/...`，服务端口由 `backend/.env` 的 `PORT` 控制，默认 8080）；
- 前端通过 `HttpClient`、Bearer interceptor 和 DTO 映射直接调用契约接口；开发服务器把 `/api/v1` 代理到后端；
- 数据库 schema 与基础 RBAC 数据由 `backend/migrations/` 管理；历史演示管理员会被迁移禁用，真实管理员必须显式初始化；
- `backend/tests/openapi_contract.rs` 会在 CI 中校验 OpenAPI 路径、HTTP 方法及关键响应必填字段没有漂移。

## 运行时产品配置

前端在启动时读取 `frontend/public/config.js`。修改这些公开配置后无需重新构建：

```javascript
window.__ARC_ADMIN_CONFIG__ = {
  appName: "RBAC 管理中心",
  appShortName: "RBAC",
  appSlug: "arc-admin",
  apiBaseUrl: "/api/v1",
  themeStorageKey: "arc-admin-theme",
};
```

配置会同步控制浏览器标题、登录页、侧栏、顶栏、API 地址和主题存储键。该文件会被浏览器直接读取，禁止写入 API 密钥、JWT 密钥、数据库凭据或其他秘密。
