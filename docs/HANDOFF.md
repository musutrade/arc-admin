# 会话交接文档（2026-08-11）

> 用途：保存当前进度，供下次会话直接续接。长期约定以 `AGENTS.md`、`docs/development.md`、`docs/architecture.md` 和生成的 `docs/openapi.json` 为准。

## 当前状态

| 模块 | 已完成内容 |
| --- | --- |
| 工作流 | `cargo flow verify` 统一执行范围检测、secret scan、架构审计和测试；流程 Shell 已由 Rust CLI 替代 |
| 变更范围 | `cargo flow scope` 支持 working-tree、staged、base 和 all，并输出 backend/frontend/workflow components |
| 后端安全 | 固定密码管理员已禁用，改为显式 `bootstrap_admin`；每个请求复核账号状态与权限码；超级管理员有最后账号和不可变权限保护 |
| 后端数据 | 用户-角色、角色-权限写入使用事务；用户和角色列表已改为聚合查询，避免 N+1 |
| 后端测试 | 一次性 PostgreSQL 集成测试覆盖服务端会话、CSRF、RBAC、数据范围、审计保护、事务回滚、超级管理员保护和 CRUD |
| 契约 | Rust 类型生成 OpenAPI 3.1，并继续生成 Angular DTO/API Client；完整性由生成门禁自动比对 |
| 运行安全 | 生产环境强制 Secure Cookie/CORS；服务端会话、CSRF、登录限流和认证审计已接通；内部错误不返回客户端；提供 liveness/readiness 和 SIGTERM 优雅停机 |
| 前端 | mock 已移除；真实 HttpClient、Cookie/CSRF interceptor、会话/权限守卫、用户/角色 CRUD 和角色授权已接通 |
| 前端测试 | ESLint、Prettier、Vitest、production build、mock E2E，以及真实 Angular/Axum/PostgreSQL 跨端冒烟测试纳入统一门禁 |
| 可观测性 | JSON Lines 日志、Prometheus 应用与连接池指标、Blackbox 探测、Grafana 告警和可选 OTLP/Tempo 已接通 |
| 审计 | 登录来源指纹、数据库级只追加保护、JSONL 归档、SHA-256 清单、保留任务和恢复说明已完成 |
| 供应链 | Dependabot、RustSec、`cargo deny`、CodeQL、Trivy 镜像扫描和 SPDX SBOM 工作流已配置 |

## 常用命令

```bash
cargo flow doctor
cargo flow verify
cargo flow verify --all
```

后端测试默认启动并销毁一次性 `postgres:16-alpine` 容器；也可导出指向隔离测试库的 `TEST_DATABASE_URL`。任何情况下都不得指向生产库。

## 下一阶段

1. 在每个生产环境配置 Grafana 联系点、通知策略并执行真实告警送达演练，密钥不进入模板。
2. 在应用故障域之外配置独立 HTTP 心跳，并验证整个入口、TLS、代理和应用链路。
3. GitHub 仓库设置中启用 `main` 分支保护、required checks、secret scanning 与 push protection；私有仓库还需启用 GitHub Code Security 才能运行 CodeQL。
4. 高合规项目上线前把审计归档、SBOM 和备份放入权限独立的不可变存储。
5. 按[代码审计模板生产化待办](audit-template-production-todo.md)完成配置迁移、默认 preset、词法边界、稳健性和性能验收。

## 已知残余风险

- 漏洞数据库和镜像漏洞结果会随时间变化，本地全量门禁只校验安全工作流配置；真实结果由 GitHub Actions 的 PR 与每周任务给出。
- 模板内的 Blackbox 探测与 Tempo 都在同一 Docker 环境中，不能替代跨故障域的外部监控或独立证据存储。
- `super_admin` MFA 已实现，但生产环境仍必须按 `docs/mfa-operations.md` 注入密钥并在实际域名完成演练。
