import { inject } from '@angular/core';
import { CanActivateFn, Router, Routes } from '@angular/router';
import { LayoutComponent } from './layout/layout';
import { LoginPage } from './pages/login/login';
import { AuthService } from './core/auth.service';

/** 无需登录即可访问的错误页 */
const PUBLIC_PATHS = new Set(['403', '404', '500']);

/** 未登录跳转到 /login;错误页始终放行 */
export const authGuard: CanActivateFn = (route) => {
  const router = inject(Router);
  const auth = inject(AuthService);
  const path = route.routeConfig?.path ?? '';
  if (PUBLIC_PATHS.has(path)) {
    return true;
  }
  return auth
    .ensureSession()
    .then((authenticated) => (authenticated ? true : router.createUrlTree(['/login'])));
};

export const permissionGuard: CanActivateFn = (route) => {
  const router = inject(Router);
  const auth = inject(AuthService);
  const permission = route.data['permission'] as string | undefined;
  const permissions = (route.data['permissions'] as string[] | undefined) ?? [];
  return auth.ensureSession().then((authenticated) => {
    if (!authenticated) {
      return router.createUrlTree(['/login']);
    }
    const allowed = permission
      ? auth.hasPermission(permission)
      : auth.hasAllPermissions(permissions);
    return allowed ? true : router.createUrlTree(['/403']);
  });
};

export const routes: Routes = [
  // 主落地页保持 eager,其余页面懒加载
  { path: 'login', component: LoginPage },
  {
    path: '',
    component: LayoutComponent,
    children: [
      { path: '', redirectTo: 'permissions', pathMatch: 'full' },
      {
        path: 'permissions',
        canActivate: [authGuard, permissionGuard],
        data: { permission: 'permission:directory:read' },
        loadComponent: () =>
          import('./pages/permissions/permissions').then((m) => m.PermissionsPage),
      },
      {
        path: 'users',
        canActivate: [authGuard, permissionGuard],
        data: { permission: 'user:directory:read' },
        loadComponent: () => import('./pages/users/users').then((m) => m.UsersPage),
      },
      {
        path: 'roles',
        canActivate: [authGuard, permissionGuard],
        data: { permission: 'role:directory:read' },
        loadComponent: () => import('./pages/roles/roles').then((m) => m.RolesPage),
      },
      {
        path: 'role-permissions',
        canActivate: [authGuard, permissionGuard],
        data: {
          permissions: [
            'role:permissions:write',
            'role:directory:read',
            'permission:directory:read',
          ],
        },
        loadComponent: () =>
          import('./pages/role-permissions/role-permissions').then((m) => m.RolePermissionsPage),
      },
      {
        path: 'audit-logs',
        canActivate: [authGuard, permissionGuard],
        data: { permission: 'audit:logs:read' },
        loadComponent: () => import('./pages/audit-logs/audit-logs').then((m) => m.AuditLogs),
      },
      {
        path: '403',
        loadComponent: () => import('./pages/errors/error-403').then((m) => m.Error403Page),
      },
      {
        path: '404',
        loadComponent: () => import('./pages/errors/error-404').then((m) => m.Error404Page),
      },
      {
        path: '500',
        loadComponent: () => import('./pages/errors/error-500').then((m) => m.Error500Page),
      },
    ],
  },
  { path: '**', redirectTo: '404' },
];
