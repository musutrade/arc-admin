# RBAC Control Center — Angular 22 + Angular Material

面向 RBAC 管理的 Angular 前端，使用真实后端 API、JWT 会话和权限感知路由。

## 技术栈

- **Angular 22.1**(Standalone Components, signal-based 状态管理)
- **Angular Material 22.1**(M3 主题系统,通过 `--mat-sys-*` token 覆盖为 Arco 蓝)
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
- `DataService` 将 OpenAPI 的数值 ID、nullable 字段和分页响应映射到视图模型。
- 默认 API 根路径是 `/api/v1`。部署时覆盖静态 `config.js` 的 `window.__ARC_ADMIN_CONFIG__.apiBaseUrl` 即可切换地址，无需重新构建。

## 页面清单

| 路由 | 页面 | 设计稿来源 |
| ---- | ---- | ---- |
| `/login` | 登录页(网格背景 + 表单校验) | `rbac_admin` |
| `/permissions` | 权限目录(层级分组表格、搜索、类型过滤，只读) | `arco_style_1` |
| `/users` | 用户增删改、密码重置、角色分配、筛选和批量操作 | `arco_style_3` |
| `/roles` | 角色增删改和权限分配 | `arco_style_4` |
| `/role-permissions` | 角色权限矩阵和 `mat-dialog` 分配流程 | `arco_style_5` |
| `/403` `/404` `/500` | 错误页 | `403/404/500_arco_style` |

- 未登录访问主页面会被 `authGuard` 重定向到 `/login`。
- 顶栏 `contrast` 按钮切换暗色模式,偏好持久化到 `localStorage('arc-theme')`,并支持 `[data-theme="dark"]` 手动覆盖。
- 登录勾选"Remember me"时 token 持久化到 `localStorage`,否则仅当前会话(`sessionStorage`)有效。
- 除登录页外的路由均懒加载,并按需分包(`loadComponent`);路由切换带 View Transitions。

## 主题令牌

`src/styles.scss` 内通过覆盖 Material 22 M3 的 `--mat-sys-*` 变量实现 Arco 设计系统:

- Primary `#165dff`(Arco Blue),语义色 success `#00B42A` / warning `#FF7D00` / danger `#F53F3F`
- 圆角:4px(小)/ 8px(中)/ 12px(大)
- 字体族:Inter + 中文字体回退
- 暗色模式:表面色 `#131315` 系列、容器层级 `#0e0e10 → #333335`

## 目录结构

```
src/app/
├── core/            # API DTO、认证、interceptor、DataService、ThemeService
├── layout/          # 主布局(侧边栏 + 顶栏 + 主题切换)
└── pages/
    ├── login/       # 登录页
    ├── permissions/ # 权限管理
    ├── users/       # 用户管理
    ├── roles/       # 角色管理
    ├── role-permissions/  # 分配权限 + 对话框
    └── errors/      # 403 / 404 / 500
```
