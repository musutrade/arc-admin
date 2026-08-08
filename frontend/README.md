# RBAC Control Center — Angular 22 + Angular Material

面向 RBAC 管理的 Angular 前端，使用真实后端 API、JWT 会话和权限感知路由。

## 技术栈

- **Angular 22.1**(Standalone Components, signal-based 状态管理)
- **Angular Material 22.1**(M3 主题系统,通过语义 token 映射 Material 变量)
- 字体:Ant Design 中后台系统字体栈(系统优先)+ Material Symbols Outlined
- 样式:SCSS + CSS 变量(light / dark 双主题)
- **Zoneless** 变更检测(`provideZonelessChangeDetection`)+ 全组件 `OnPush`

## 快速开始

```bash
npm ci
npm start          # http://localhost:4200
```

开发服务器通过 `proxy.conf.json` 把 `/api/v1` 转发到 `http://127.0.0.1:8080`，因此需要先启动后端并显式初始化管理员。仓库不提供固定密码演示账号。

```bash
npm run lint
npm run format:check
npm run test:ci       # Vitest + jsdom
npm run e2e           # Playwright:桌面 Chromium + Pixel 7
npm run build         # 输出到 dist/arc-admin
```

## API 与认证

- `AuthService` 登录后保存 Bearer token，并从 `/auth/me/permissions` 获取权限码；权限加载失败会清除整个部分会话。
- `authInterceptor` 为 API 请求附加 token，收到 401 后清理会话并跳转登录页。
- 路由守卫会向服务端复核现有 token；页面和按钮按权限码显示，但后端授权始终是最终安全边界。
- `core/api` 下按资源拆分的 API Service 将 OpenAPI 的数值 ID、nullable 字段和分页响应映射到视图模型。
- 路由守卫与侧栏共用 `app.navigation.ts` 的权限要求；新业务统一放入 `features/<domain>`。
- 默认 API 根路径是 `/api/v1`。部署时覆盖静态 `config.js` 的 `window.__ARC_ADMIN_CONFIG__.apiBaseUrl` 即可切换地址，无需重新构建。

## 页面清单

| 路由                 | 页面                                           | 设计稿来源               |
| -------------------- | ---------------------------------------------- | ------------------------ |
| `/login`             | 登录页(表单校验 + 响应式布局)                  | `rbac_admin`             |
| `/permissions`       | 权限目录(层级分组表格、搜索、类型过滤，只读)   | `arco_style_1`           |
| `/users`             | 用户增删改、密码重置、角色分配、筛选和批量操作 | `arco_style_3`           |
| `/roles`             | 角色增删改和权限分配                           | `arco_style_4`           |
| `/role-permissions`  | 角色权限矩阵和 `mat-dialog` 分配流程           | `arco_style_5`           |
| `/403` `/404` `/500` | 错误页                                         | `403/404/500_arco_style` |

- 未登录访问主页面会被 `authGuard` 重定向到 `/login`。
- 顶栏 `contrast` 按钮切换暗色模式,偏好持久化到 `localStorage('arc-theme')`,并支持 `[data-theme="dark"]` 手动覆盖。
- 登录勾选"Remember me"时 token 持久化到 `localStorage`,否则仅当前会话(`sessionStorage`)有效。
- 除登录页外的路由均懒加载,并按需分包(`loadComponent`);路由切换带 View Transitions。

## 主题令牌

主题基础值集中在 `src/styles/_tokens.scss`,`src/styles.scss` 只负责装配 Material M3 主题和全局组件样式。组件应优先使用 `--ui-*` 语义 token,不要直接声明色值或依赖具体色阶。

| Token 分组                                         | 用途                  | 示例                        |
| -------------------------------------------------- | --------------------- | --------------------------- |
| `--ui-color-surface-*`                             | 页面、面板和弱化背景  | `--ui-color-surface-panel`  |
| `--ui-color-text-*`                                | 主、次、辅助文本      | `--ui-color-text-secondary` |
| `--ui-color-{success,warning,error,info,feature}*` | 状态及其弱背景、边框  | `--ui-color-error-soft`     |
| `--ui-radius-*`                                    | 2px 至 8px 的统一圆角 | `--ui-radius-lg`            |
| `--ui-shadow-*` / `--ui-focus-ring`                | 层级和键盘焦点        | `--ui-shadow-md`            |
| `--ui-duration-*` / `--ui-ease-*`                  | 交互动效              | `--ui-duration-fast`        |

- `:root` 装配亮色 token,`[data-theme='dark']` 只覆盖颜色层;间距、圆角和动效保持一致。
- `_tokens.scss` 同时映射 Angular Material 的 `--mat-sys-*` 变量,因此 Material 组件与自定义组件共享同一语义体系。
- 新增状态样式时先组合现有 token;只有确实新增设计语义时才同时补齐 light/dark 两组定义。
- 所有可聚焦控件必须保留全局 `:focus-visible` 或组件内 `--ui-focus-ring`,动效需服从 `prefers-reduced-motion`。

## 目录结构

```
src/app/
├── app.navigation.ts # 共享路由权限与导航配置
├── core/
│   └── api/           # 模板内置的 RBAC 平台资源客户端
├── features/          # 新业务域，每个域自带页面、数据访问、模型与测试
├── layout/            # 主布局(侧边栏 + 顶栏 + 主题切换)
└── pages/             # 模板内置的 RBAC 平台页面
    ├── login/
    ├── permissions/
    ├── users/
    ├── roles/
    ├── role-permissions/
    ├── audit-logs/
    └── errors/
```
