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

用户-角色和角色-权限写入由 Service 开启事务并把同一个连接传给 Repository，主记录与关联表要么同时成功，要么同时回滚。内置 `super_admin` 不能被停用或清空权限，最后一个有效超级管理员不能被停用、删除或移除角色。

每个受保护请求都会从数据库重新读取账号有效状态和权限码，因此停用账号或修改角色权限后，旧 JWT 会立即失效或按新权限执行。Handler 使用类型化权限提取器声明所需权限，前端隐藏按钮仅用于交互体验，不作为安全边界。

## 配置与安全

| 变量 | 说明 |
| --- | --- |
| `DATABASE_URL` | 必填 PostgreSQL 连接串 |
| `PORT` | 监听端口，默认 8080，范围 1-65535 |
| `APP_ENV` | `development`、`test` 或 `production` |
| `JWT_SECRET` | 生产环境必填且至少 32 字符 |
| `TOKEN_TTL_SECS` | 正整数，默认 86400 |
| `CORS_ALLOWED_ORIGINS` | 逗号分隔 origin；生产环境必填且禁止 `*` |

管理员账号不再由固定密码迁移创建。`bootstrap_admin` 使用进程环境传入的至少 16 字符密码，并在事务中创建或激活账号及绑定 `super_admin`。

凭据只存放在进程环境或已忽略的 `backend/.env`，不得进入 Git remote、日志、报告或文档。

## 契约与测试

- `API_ROUTE_CONTRACT`、`API_SCHEMA_REQUIRED_FIELDS` 和 `backend/tests/openapi_contract.rs` 校验 OpenAPI 的路径、HTTP 方法及关键响应必填字段。
- `backend/tests/api_flow.rs` 使用真实迁移和 PostgreSQL，覆盖安全初始化、默认密码失效、权限拒绝、旧 token 失效、事务回滚和超级管理员保护。
- `/healthz` 仅表示进程存活；`/readyz` 检查 PostgreSQL，可用于负载均衡就绪探针。服务收到 Ctrl+C 或 SIGTERM 后执行优雅停机。
- Angular 单测使用 Vitest/jsdom；Playwright 使用拦截 API 的确定性数据，在桌面 Chromium 和 Pixel 7 视口覆盖登录、权限导航、用户创建及未认证重定向。
- arc-flow 对 SQL 写入位置、分层依赖和旧模板模式做确定性扫描，并管理外部命令超时与一次性 PostgreSQL 生命周期。

当前响应必填字段由轻量常量检查，尚未覆盖所有请求字段、枚举和格式。接口规模明显扩大后可引入 `utoipa`，由后端类型生成 OpenAPI 并让前端生成 DTO。
