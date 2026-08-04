# 架构说明

## 系统边界

- `frontend/`：Angular 22 standalone 应用，负责界面、路由和前端交互状态。
- `backend/`：Axum API，负责认证、授权、业务规则和持久化。
- `docs/openapi.yaml`：前后端 HTTP 契约的事实来源。
- `codex-audit-pipeline/`：`arc-flow` Rust CLI，统一负责范围路由、静态审计、安全扫描、测试编排、报告和 Git hook profile。

## 后端分层

请求按 `Router -> Handler -> Service -> Repository -> PostgreSQL` 流动：

- Handler 解析 HTTP 输入并映射状态码，不直接写 SQL。
- Service 编排业务规则和事务边界，不直接写 SQL。
- Repository 是运行时 SQL 读写的唯一入口。
- Model 定义 API 和数据库数据结构，不包含业务流程。
- migrations、tests 和 seed 可执行数据库写操作，但不得连接生产库。

`backend/src/lib.rs` 暴露可复用的 `build_router`，生产进程和集成测试使用同一套路由。`backend/src/main.rs` 仅负责读取配置、连接数据库、执行迁移并启动监听。

## 配置与安全

| 变量 | 说明 |
| --- | --- |
| `DATABASE_URL` | 必填 PostgreSQL 连接串 |
| `PORT` | 监听端口，默认 8080，范围 1-65535 |
| `APP_ENV` | `development`、`test` 或 `production` |
| `JWT_SECRET` | 生产环境必填且至少 32 字符 |
| `TOKEN_TTL_SECS` | 正整数，默认 86400 |
| `CORS_ALLOWED_ORIGINS` | 逗号分隔 origin；生产环境必填且禁止 `*` |

凭据只存放在进程环境或已忽略的 `backend/.env`，不得进入 Git remote、日志、报告或文档。

## 契约与测试

- `API_ROUTE_CONTRACT` 和 `backend/tests/openapi_contract.rs` 校验 OpenAPI 的路径与 HTTP 方法。
- `backend/tests/api_flow.rs` 使用真实迁移和 PostgreSQL，覆盖未认证访问、登录、角色读取、用户创建/停用/删除。
- Angular 测试使用 Vitest/jsdom；生产构建是完整门禁的一部分。
- arc-flow 对 SQL 写入位置、分层依赖和旧模板模式做确定性扫描，并管理外部命令超时与一次性 PostgreSQL 生命周期。

路由契约测试只解决路径级漂移；请求/响应 schema 的完整自动生成可在 API 规模扩大后引入 `utoipa`。
