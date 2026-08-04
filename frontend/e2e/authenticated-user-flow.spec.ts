import { expect, test } from '@playwright/test';
import type { ApiRole, ApiUser, CreateUserRequest } from '../src/app/core/api.models';

const administrator: ApiUser = {
  id: 1,
  username: 'admin',
  displayName: 'Administrator',
  email: 'admin@example.test',
  status: 'active',
  roles: ['Super Admin'],
  lastLoginAt: null,
  createdAt: '2026-08-01T00:00:00Z',
};

const permissions = [
  'dashboard:analytics:read',
  'permission:directory:read',
  'role:directory:read',
  'role:permissions:write',
  'role:write',
  'user:admin:deactivate',
  'user:admin:reset_password',
  'user:directory:read',
  'user:write',
];

const roles: ApiRole[] = [
  {
    id: 1,
    code: 'super_admin',
    name: 'Super Admin',
    category: 'System Core',
    icon: 'shield',
    color: 'primary',
    description: 'Full access',
    isActive: true,
    members: 1,
    permissionGroupIds: [1],
  },
  {
    id: 2,
    code: 'viewer',
    name: 'Viewer',
    category: 'Read Only',
    icon: 'visibility',
    color: 'success',
    description: 'Read-only access',
    isActive: true,
    members: 0,
    permissionGroupIds: [1],
  },
];

test('logs in, uses permission-aware navigation, and creates a user', async ({ page }) => {
  let users: ApiUser[] = [administrator];
  let createdRequest: Record<string, unknown> | null = null;

  await page.route('**/api/v1/**', async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path === '/api/v1/auth/login') {
      await route.fulfill({
        json: {
          accessToken: 'e2e-token',
          tokenType: 'Bearer',
          expiresIn: 3600,
          user: administrator,
        },
      });
    } else if (path === '/api/v1/auth/me') {
      await route.fulfill({ json: administrator });
    } else if (path === '/api/v1/auth/me/permissions') {
      await route.fulfill({ json: { codes: permissions } });
    } else if (path === '/api/v1/permissions/groups') {
      await route.fulfill({ json: [] });
    } else if (path === '/api/v1/dashboard/stats') {
      await route.fulfill({
        json: {
          totalUsers: users.length,
          activeUsers: users.length,
          totalRoles: 2,
          totalPermissions: 9,
          suspendedUsers: 0,
        },
      });
    } else if (path === '/api/v1/roles') {
      await route.fulfill({ json: roles });
    } else if (path === '/api/v1/users' && request.method() === 'POST') {
      const payload = request.postDataJSON() as CreateUserRequest;
      createdRequest = payload as unknown as Record<string, unknown>;
      const created: ApiUser = {
        id: 2,
        username: payload.username,
        displayName: payload.displayName,
        email: payload.email ?? null,
        status: payload.status ?? 'active',
        roles: ['Viewer'],
        lastLoginAt: null,
        createdAt: '2026-08-04T00:00:00Z',
      };
      users = [...users, created];
      await route.fulfill({ status: 201, json: created });
    } else if (path === '/api/v1/users') {
      await route.fulfill({
        json: { items: users, total: users.length, page: 1, pageSize: 100 },
      });
    } else {
      await route.fulfill({
        status: 404,
        json: { error: { code: 'NOT_FOUND', message: path } },
      });
    }
  });

  await page.goto('/login');
  await page.getByLabel('Username').fill('admin');
  await page.getByLabel('Password').fill('safe-password');
  await page.getByRole('button', { name: 'Login' }).click();
  await expect(page.getByRole('heading', { name: 'Resource Access Control' })).toBeVisible();

  const mobileMenu = page.getByRole('button', { name: '打开菜单' });
  if (await mobileMenu.isVisible()) {
    await mobileMenu.click();
  }
  await page.getByRole('button', { name: 'User Management' }).click();
  await page.getByRole('link', { name: 'Active Users' }).click();
  await expect(page.getByRole('heading', { name: 'User Management' })).toBeVisible();
  await page.getByRole('button', { name: 'Add User' }).click();
  await page.getByLabel('Username').fill('new_user');
  await page.getByLabel('Password').fill('new-user-password');
  await page.getByLabel('Display name').fill('New User');
  await page.getByLabel('Email').fill('new.user@example.test');
  await page.getByLabel('Roles').selectOption({ label: 'Viewer' });
  await page.getByRole('button', { name: 'Save User' }).click();

  await expect(page.getByText('New User', { exact: true })).toBeVisible();
  expect(createdRequest).toMatchObject({
    username: 'new_user',
    displayName: 'New User',
    roleIds: [2],
  });
});

test('redirects an unauthenticated deep link to login', async ({ page }) => {
  await page.goto('/users');
  await expect(page).toHaveURL(/\/login$/);
  await expect(page.getByRole('button', { name: 'Login' })).toBeVisible();
});
