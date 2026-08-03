# RBAC Control Center — Angular 22 + Angular Material

根据 `stitch_arco_design_dashboard` 设计稿(Arco Design 风格 RBAC 后台)生成的完整前端项目。

## 技术栈

- **Angular 22.1**(Standalone Components, signal-based 状态管理)
- **Angular Material 22.1**(M3 主题系统,通过 `--mat-sys-*` token 覆盖为 Arco 蓝)
- 字体:Ant Design 中后台系统字体栈(系统优先)+ Material Symbols Outlined
- 样式:SCSS + CSS 变量(light / dark 双主题)
- **Zoneless** 变更检测(`provideZonelessChangeDetection`)+ 全组件 `OnPush`

## 快速开始

```bash
npm install
npm start          # http://localhost:4200
```

生产构建:`npm run build`(输出到 `dist/arc-admin`)

单元测试:`npm test`(Vitest + jsdom,覆盖 DataService、authGuard、用户筛选逻辑)

## 演示账号

| 用户名 | 密码 |
| ------ | ---- |
| admin  | admin123 |

## 页面清单

| 路由 | 页面 | 设计稿来源 |
| ---- | ---- | ---- |
| `/login` | 登录页(网格背景 + 表单校验) | `rbac_admin` |
| `/permissions` | 权限管理(层级分组表格,可展开/搜索/类型过滤) | `arco_style_1` |
| `/users` | 用户管理(表格 + 批量操作 + 统计卡片) | `arco_style_3` |
| `/roles` | 角色管理(卡片网格 / 列表视图切换) | `arco_style_4` |
| `/role-permissions` | 分配权限(角色表格 + `mat-dialog` 权限模态框) | `arco_style_5` |
| `/403` `/404` `/500` | 错误页 | `403/404/500_arco_style` |

- 未登录访问主页面会被 `authGuard` 重定向到 `/login`。
- 顶栏 `contrast` 按钮切换暗色模式,偏好持久化到 `localStorage('arc-theme')`,并支持 `[data-theme="dark"]` 手动覆盖。
- 登录勾选"Remember me"时认证状态持久化到 `localStorage`,否则仅当前会话(`sessionStorage`)有效。
- 除登录页外的路由均懒加载,并按需分包(`loadComponent`);路由切换带 View Transitions。

## 主题令牌

`src/styles.scss` 内通过覆盖 Material 22 M3 的 `--mat-sys-*` 变量实现 Arco 设计系统:

- Primary `#165dff`(Arco Blue),语义色 success `#00B42A` / warning `#FF7D00` / danger `#F53F3F`
- 圆角:4px(小)/ 8px(中)/ 12px(大)
- 字体族:Inter + 中文字体回退
- 暗色模式:表面色 `#131315` 系列、容器层级 `#0e0e10 → #333335`

## Mock 数据

`src/app/core/mock-data.ts` 提供全部演示数据(10 用户 / 5 组 15 权限 / 6 角色 / 权限分配映射),
经 `DataService` 以 Promise + 延迟模拟真实接口,后续可无缝替换为 HTTP 调用。

## 目录结构

```
src/app/
├── core/            # 模型、mock 数据、DataService、ThemeService
├── layout/          # 主布局(侧边栏 + 顶栏 + 主题切换)
└── pages/
    ├── login/       # 登录页
    ├── permissions/ # 权限管理
    ├── users/       # 用户管理
    ├── roles/       # 角色管理
    ├── role-permissions/  # 分配权限 + 对话框
    └── errors/      # 403 / 404 / 500
```
