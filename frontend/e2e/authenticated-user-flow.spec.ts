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

test('logs in, uses permission-aware navigation, and creates a user', async ({
  page,
}, testInfo) => {
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

  const dialog = page.getByRole('dialog');
  const editor = dialog.locator('.editor-dialog');
  await expect(dialog).toBeVisible();
  await editor.evaluate(async (element) => {
    await Promise.allSettled(
      element.getAnimations({ subtree: true }).map((animation) => animation.finished),
    );
  });
  const dialogBox = await dialog.boundingBox();
  const viewport = page.viewportSize();
  expect(dialogBox).not.toBeNull();
  expect(viewport).not.toBeNull();
  expect(dialogBox!.width).toBeLessThanOrEqual(viewport!.width - 16);
  expect(dialogBox!.height).toBeLessThanOrEqual(viewport!.height - 16);
  await expect
    .poll(() => editor.evaluate((element) => element.scrollWidth <= element.clientWidth))
    .toBe(true);
  if (viewport!.width >= 800) {
    expect(dialogBox!.width).toBeGreaterThanOrEqual(720);
  }

  if (process.env['VISUAL_REVIEW']) {
    await page.screenshot({ path: testInfo.outputPath('add-user-dialog.png'), fullPage: true });
  }

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
  const snackBar = page.locator('mat-snack-bar-container');
  await expect(snackBar).toBeVisible();
  await snackBar.evaluate(async (element) => {
    await Promise.allSettled(
      element.getAnimations({ subtree: true }).map(({ finished }) => finished),
    );
  });
  await expect
    .poll(() =>
      snackBar
        .locator('.mdc-snackbar__surface')
        .evaluate((element) => getComputedStyle(element).backgroundColor),
    )
    .not.toBe('rgba(0, 0, 0, 0)');
  const snackBarBox = await snackBar.boundingBox();
  expect(snackBarBox).not.toBeNull();
  expect(snackBarBox!.y).toBeGreaterThanOrEqual(64);
  expect(snackBarBox!.width).toBeLessThanOrEqual(viewport!.width - 16);
  expect(snackBarBox!.x + snackBarBox!.width / 2).toBeCloseTo(viewport!.width / 2, 0);

  if (process.env['VISUAL_REVIEW']) {
    await page.screenshot({ path: testInfo.outputPath('users-snackbar.png'), fullPage: true });
  }

  const closeSnackBar = page.getByRole('button', { name: 'Close' });
  if (await closeSnackBar.isVisible()) {
    await closeSnackBar.click();
  }

  const rootToken = (name: string) =>
    page.evaluate((tokenName) => {
      return getComputedStyle(document.documentElement).getPropertyValue(tokenName).trim();
    }, name);

  await expect.poll(() => rootToken('--ui-color-surface-page')).toBe('#f6f8fc');
  await expect
    .poll(() =>
      page.evaluate(
        () => document.documentElement.scrollWidth <= document.documentElement.clientWidth,
      ),
    )
    .toBe(true);

  if (process.env['VISUAL_REVIEW']) {
    await page.screenshot({ path: testInfo.outputPath('users-light.png'), fullPage: true });
  }

  await page.getByRole('button', { name: '切换到暗色模式' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect.poll(() => rootToken('--ui-color-surface-page')).toBe('#141414');

  if (process.env['VISUAL_REVIEW']) {
    await page.screenshot({ path: testInfo.outputPath('users-dark.png'), fullPage: true });
  }

  await page.getByRole('button', { name: '切换到亮色模式' }).click();
  await expect.poll(() => rootToken('--ui-color-surface-page')).toBe('#f6f8fc');
  if (await mobileMenu.isVisible()) {
    await mobileMenu.click();
  }
  await page.getByRole('link', { name: 'Role Permissions' }).click();
  await expect(page.getByRole('heading', { name: 'Role Management' })).toBeVisible();
  await page.getByRole('button', { name: '列表视图' }).click();

  const roleTable = page.locator('.roles-table');
  const roleTableScroll = page.locator('.role-table-scroll');
  await expect(roleTable).toBeVisible();
  await expect(roleTable.getByRole('columnheader', { name: 'Status' })).toBeVisible();
  const viewerRow = roleTable.getByRole('row').filter({ hasText: 'Viewer' });
  await expect(viewerRow).toContainText('viewer');
  await expect(viewerRow.getByText('Active', { exact: true })).toBeVisible();
  const editViewer = viewerRow.getByRole('button', { name: 'Edit Viewer' });
  await expect(editViewer).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(
        () => document.documentElement.scrollWidth <= document.documentElement.clientWidth,
      ),
    )
    .toBe(true);
  if (viewport!.width < 800) {
    await expect
      .poll(() => roleTableScroll.evaluate((element) => element.scrollWidth > element.clientWidth))
      .toBe(true);
  }

  if (process.env['VISUAL_REVIEW']) {
    await expect(page.getByText('New User created', { exact: true })).toBeHidden();
    await page.screenshot({ path: testInfo.outputPath('roles-list-light.png'), fullPage: true });
  }

  await editViewer.click();
  const roleDialog = page.getByRole('dialog');
  await expect(roleDialog.getByRole('heading', { name: 'Edit Role' })).toBeVisible();
  const iconSelect = roleDialog.getByRole('combobox', { name: 'Role icon' });
  await expect(iconSelect).toContainText('Read only');
  await iconSelect.click();
  await expect(page.getByRole('option', { name: 'Administrator', exact: true })).toBeVisible();
  if (process.env['VISUAL_REVIEW']) {
    await page.screenshot({ path: testInfo.outputPath('role-icon-picker.png'), fullPage: true });
  }
  await page.getByRole('option', { name: 'Review', exact: true }).click();
  await expect(iconSelect).toContainText('Review');
  await roleDialog.getByRole('button', { name: 'Cancel' }).click();
});

test('redirects an unauthenticated deep link to login', async ({ page }) => {
  await page.goto('/users');
  await expect(page).toHaveURL(/\/login$/);
  await expect(page.getByRole('button', { name: 'Login' })).toBeVisible();
});
