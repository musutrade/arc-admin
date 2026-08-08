# 会话交接文档（2026-08-04）

> 用途：保存当前进度，供下次会话直接续接。长期约定以 `AGENTS.md`、`docs/development.md`、`docs/architecture.md` 和 `docs/openapi.yaml` 为准。

## 当前状态

| 模块 | 已完成内容 |
| --- | --- |
| 工作流 | `cargo flow verify` 统一执行范围检测、secret scan、架构审计和测试；流程 Shell 已由 Rust CLI 替代 |
| 变更范围 | `cargo flow scope` 支持 working-tree、staged、base 和 all，并输出 backend/frontend/workflow components |
| 后端安全 | 固定密码管理员已禁用，改为显式 `bootstrap_admin`；每个请求复核账号状态与权限码；超级管理员有最后账号和不可变权限保护 |
| 后端数据 | 用户-角色、角色-权限写入使用事务；用户和角色列表已改为聚合查询，避免 N+1 |
| 后端测试 | 临时 PostgreSQL 集成测试覆盖默认密码拒绝、RBAC 403、旧 token 失效、事务回滚、超级管理员保护和 CRUD |
| 契约 | OpenAPI 路由、HTTP 方法和关键响应必填字段由 Rust 契约测试自动比对 |
| 运行安全 | 生产环境强制 Secure Cookie/CORS；服务端会话、CSRF、登录限流和认证审计已接通；内部错误不返回客户端；提供 liveness/readiness 和 SIGTERM 优雅停机 |
| 前端 | mock 已移除；真实 HttpClient、Cookie/CSRF interceptor、会话/权限守卫、用户/角色 CRUD 和角色授权已接通 |
| 前端测试 | ESLint、Prettier、Vitest、production build，以及桌面/Pixel 7 Playwright E2E 纳入统一门禁 |
| CI | GitHub Actions 安装 Playwright Chromium，拆分 quality/backend/frontend/dependency review，并上传 14 天测试报告 |
| 供应链 | Node/Rust 工具链固定，Dependabot 覆盖 npm、Cargo 和 GitHub Actions |

## 常用命令

```bash
cargo flow doctor
cargo flow verify
cargo flow verify --all
```

后端测试默认启动并销毁一次性 `postgres:16-alpine` 容器；也可导出指向隔离测试库的 `TEST_DATABASE_URL`。任何情况下都不得指向生产库。

## 下一阶段

1. 用户量增大后，把前端“拉取全部页再本地筛选”升级为服务端搜索、筛选、排序和分页。
2. 增加连接真实后端与临时 PostgreSQL 的少量跨端冒烟测试；现有 Playwright 使用拦截 API，重点保证 UI 流程确定性。
3. 接口规模明显增长后，引入 `utoipa` 从后端类型生成完整 OpenAPI，并由契约生成前端 DTO。
4. GitHub 仓库设置中启用 `main` 分支保护、required checks、secret scanning 与 push protection。

## 已知残余风险

- 2026-08-04 执行 `npm audit`：生产依赖为 0；开发链
  `@angular/cli@22.1.2 -> @modelcontextprotocol/sdk -> @hono/node-server` 有 3 个 moderate。
  npm 只提供降级 Angular CLI 21 的自动修复，Angular 22 暂无兼容补丁，因此保留并由 Dependabot 跟踪。
