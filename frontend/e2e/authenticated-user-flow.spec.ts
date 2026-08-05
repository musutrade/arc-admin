import { expect, test } from '@playwright/test';
import type {
  ApiRole,
  ApiUser,
  ChangePasswordRequest,
  CreateUserRequest,
} from '../src/app/core/api.models';

const administrator: ApiUser = {
  id: 1,
  username: 'admin',
  displayName: '管理员',
  email: 'admin@example.test',
  status: 'active',
  roles: ['超级管理员'],
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
    name: '超级管理员',
    category: '系统核心',
    icon: 'shield',
    color: 'primary',
    description: '拥有所有系统权限',
    isActive: true,
    members: 1,
    permissionGroupIds: [1],
  },
  {
    id: 2,
    code: 'viewer',
    name: '查看者',
    category: '只读',
    icon: 'visibility',
    color: 'success',
    description: '仅可查看系统内容',
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
  let passwordChangeRequest: ChangePasswordRequest | null = null;

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
    } else if (path === '/api/v1/auth/me/password' && request.method() === 'PUT') {
      passwordChangeRequest = request.postDataJSON() as ChangePasswordRequest;
      await route.fulfill({ status: 204 });
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
        roles: ['查看者'],
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
  await page.getByLabel('用户名').fill('admin');
  await page.getByLabel('密码', { exact: true }).fill('safe-password');
  await page.getByRole('button', { name: '登录' }).click();
  await expect(page.getByRole('heading', { name: '权限目录' })).toBeVisible();

  await page.getByRole('button', { name: '账户菜单' }).click();
  await expect(page.getByRole('menuitem', { name: '修改密码' })).toBeVisible();
  await expect(page.getByRole('menuitem', { name: '退出登录' })).toBeVisible();
  if (process.env['VISUAL_REVIEW']) {
    await page.screenshot({ path: testInfo.outputPath('account-menu.png') });
  }
  await page.getByRole('menuitem', { name: '修改密码' }).click();

  const changePasswordDialog = page.getByRole('dialog');
  await expect(changePasswordDialog.getByRole('heading', { name: '修改密码' })).toBeVisible();
  await changePasswordDialog.locator('.editor-dialog').evaluate(async (element) => {
    await Promise.allSettled(
      element.getAnimations({ subtree: true }).map((animation) => animation.finished),
    );
  });
  await changePasswordDialog.getByLabel('当前密码', { exact: true }).fill('safe-password');
  await changePasswordDialog.getByLabel('新密码', { exact: true }).fill('updated-safe-password');
  await changePasswordDialog.getByLabel('确认新密码', { exact: true }).fill('different-password');
  await changePasswordDialog.getByLabel('确认新密码', { exact: true }).blur();
  await expect(changePasswordDialog.getByText('两次输入的新密码不一致')).toBeVisible();
  await changePasswordDialog
    .getByLabel('确认新密码', { exact: true })
    .fill('updated-safe-password');

  if (process.env['VISUAL_REVIEW']) {
    await page.screenshot({ path: testInfo.outputPath('change-password-dialog.png') });
  }

  await changePasswordDialog.getByRole('button', { name: '保存修改' }).click();
  await expect(changePasswordDialog).toBeHidden();
  expect(passwordChangeRequest).toEqual({
    currentPassword: 'safe-password',
    newPassword: 'updated-safe-password',
  });
  await expect(page.getByText('密码修改成功', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '关闭' }).click();

  const mobileMenu = page.getByRole('button', { name: '打开菜单' });
  if (await mobileMenu.isVisible()) {
    await mobileMenu.click();
  }
  await page.getByRole('button', { name: '用户管理' }).click();
  await page.getByRole('link', { name: '用户列表' }).click();
  await expect(page.getByRole('heading', { name: '用户管理' })).toBeVisible();
  await page.getByRole('button', { name: '新增用户' }).click();

  const dialog = page.getByRole('dialog');
  const editor = dialog.locator('.editor-dialog');
  await expect(dialog).toBeVisible();
  await dialog.evaluate(async (element) => {
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

  await page.getByLabel('用户名').fill('new_user');
  await page.getByLabel('密码').fill('new-user-password');
  await page.getByLabel('显示名称').fill('新用户');
  await page.getByLabel('邮箱').fill('new.user@example.test');
  await page.getByLabel('角色').selectOption({ label: '查看者' });
  await page.getByRole('button', { name: '保存用户' }).click();

  await expect(page.getByText('新用户', { exact: true })).toBeVisible();
  expect(createdRequest).toMatchObject({
    username: 'new_user',
    displayName: '新用户',
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

  const closeSnackBar = page.getByRole('button', { name: '关闭' });
  if (await closeSnackBar.isVisible()) {
    await closeSnackBar.click();
    await expect(snackBar).toBeHidden();
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
  await page.getByRole('link', { name: '角色管理' }).click();
  await expect(page.getByRole('heading', { name: '角色管理' })).toBeVisible();
  await page.getByRole('button', { name: '列表视图' }).click();

  const roleTable = page.locator('.roles-table');
  const roleTableScroll = page.locator('.role-table-scroll');
  await expect(roleTable).toBeVisible();
  await expect(roleTable.getByRole('columnheader', { name: '状态' })).toBeVisible();
  const viewerRow = roleTable.getByRole('row').filter({ hasText: '查看者' });
  await expect(viewerRow).toContainText('viewer');
  await expect(viewerRow.getByText('启用', { exact: true })).toBeVisible();
  const editViewer = viewerRow.getByRole('button', { name: '编辑角色 查看者' });
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
    await expect(page.getByText('已创建 新用户', { exact: true })).toBeHidden();
    await page.screenshot({ path: testInfo.outputPath('roles-list-light.png'), fullPage: true });
  }

  await editViewer.click();
  const roleDialog = page.getByRole('dialog');
  await expect(roleDialog.getByRole('heading', { name: '编辑角色' })).toBeVisible();
  const iconSelect = roleDialog.getByRole('combobox', { name: '角色图标' });
  await expect(iconSelect).toContainText('只读');
  await iconSelect.click();
  await expect(page.getByRole('option', { name: '管理员', exact: true })).toBeVisible();
  if (process.env['VISUAL_REVIEW']) {
    await page.screenshot({ path: testInfo.outputPath('role-icon-picker.png'), fullPage: true });
  }
  await page.getByRole('option', { name: '审核', exact: true }).click();
  await expect(iconSelect).toContainText('审核');
  await roleDialog.getByRole('button', { name: '取消' }).click();
});

test('redirects an unauthenticated deep link to login', async ({ page }) => {
  await page.goto('/users');
  await expect(page).toHaveURL(/\/login$/);
  await expect(page.getByRole('button', { name: '登录' })).toBeVisible();
});
