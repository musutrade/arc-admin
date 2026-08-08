import { provideHttpClient } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';
import { TestBed } from '@angular/core/testing';
import { API_BASE_URL } from './api.config';
import { ApiPermissionGroup, ApiRole, ApiUser } from './api.models';
import { DataService } from './data.service';

const apiUser: ApiUser = {
  id: 7,
  username: 'sarah',
  displayName: 'Sarah Chen',
  email: 'sarah@example.com',
  status: 'active',
  roles: ['查看者'],
  lastLoginAt: null,
  createdAt: '2026-08-01T00:00:00Z',
};

const apiRole: ApiRole = {
  id: 3,
  code: 'viewer',
  name: '查看者',
  category: '只读',
  icon: null,
  color: 'success',
  description: null,
  isActive: true,
  members: 4,
  permissionGroupIds: [1],
};

describe('DataService', () => {
  let service: DataService;
  let http: HttpTestingController;

  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [
        provideHttpClient(),
        provideHttpClientTesting(),
        { provide: API_BASE_URL, useValue: '/api/v1' },
      ],
    });
    service = TestBed.inject(DataService);
    http = TestBed.inject(HttpTestingController);
  });

  afterEach(() => http.verify());

  it('maps paged API users into view users', async () => {
    const result = service.getUsers();
    const request = http.expectOne(
      (candidate) => candidate.url === '/api/v1/users' && candidate.params.get('page') === '1',
    );
    request.flush({ items: [apiUser], total: 1, page: 1, pageSize: 100 });

    await expect(result).resolves.toEqual([
      expect.objectContaining({
        id: '7',
        username: 'sarah',
        name: 'Sarah Chen',
        lastLogin: null,
      }),
    ]);
  });

  it('updates a user status through the existing user endpoint', async () => {
    const result = service.updateUser('7', { status: 'inactive' });
    const request = http.expectOne('/api/v1/users/7');
    expect(request.request.method).toBe('PUT');
    expect(request.request.body).toEqual({ status: 'inactive' });
    request.flush({ ...apiUser, status: 'inactive' });

    await expect(result).resolves.toEqual(expect.objectContaining({ id: '7', status: 'inactive' }));
  });

  it('maps dashboard statistics into cards', async () => {
    const result = service.getUserStats();
    http.expectOne('/api/v1/dashboard/stats').flush({
      totalUsers: 10,
      activeUsers: 8,
      totalRoles: 6,
      totalPermissions: 19,
      suspendedUsers: 1,
    });

    await expect(result).resolves.toEqual([
      expect.objectContaining({ label: '用户总数', value: '10' }),
      expect.objectContaining({ label: '启用用户', value: '8' }),
      expect.objectContaining({ label: '角色总数', value: '6' }),
      expect.objectContaining({ label: '已暂停用户', value: '1' }),
    ]);
  });

  it('maps permission groups and nullable fields', async () => {
    const result = service.getPermissionGroups();
    const group: ApiPermissionGroup = {
      id: 2,
      code: 'identity',
      name: '身份与访问模块',
      icon: null,
      permissions: [
        {
          id: 9,
          code: 'user:directory:read',
          name: '查看用户目录',
          type: 'menu',
          description: null,
        },
      ],
    };
    http.expectOne('/api/v1/permissions/groups').flush([group]);

    await expect(result).resolves.toEqual([
      expect.objectContaining({ id: '2', code: 'identity', icon: 'folder' }),
    ]);
  });

  it('maps roles and role permission rows', async () => {
    const roles = service.getRoles();
    http.expectOne('/api/v1/roles').flush([apiRole]);
    await expect(roles).resolves.toEqual([
      expect.objectContaining({ id: '3', code: 'viewer', isActive: true }),
    ]);

    const rows = service.getRolePermissionRows();
    http.expectOne('/api/v1/roles').flush([apiRole]);
    await expect(rows).resolves.toEqual([
      expect.objectContaining({ roleId: '3', roleName: '查看者', usersAssigned: 4 }),
    ]);
  });

  it('updates a role status through the existing role endpoint', async () => {
    const result = service.updateRole('3', { isActive: false });
    const request = http.expectOne('/api/v1/roles/3');
    expect(request.request.method).toBe('PUT');
    expect(request.request.body).toEqual({ isActive: false });
    request.flush({ ...apiRole, isActive: false });

    await expect(result).resolves.toEqual(expect.objectContaining({ id: '3', isActive: false }));
  });

  it('sends numeric permission ids when assigning a role', async () => {
    const result = service.assignRolePermissions('3', ['7', '9']);
    const request = http.expectOne('/api/v1/roles/3/permissions');
    expect(request.request.method).toBe('PUT');
    expect(request.request.body).toEqual({ permissionIds: [7, 9] });
    request.flush(null);
    await result;
  });

  it('loads filtered audit logs with contract pagination', async () => {
    const result = service.getAuditLogs(2, 20, 'admin', 'user.update');
    const request = http.expectOne(
      (candidate) =>
        candidate.url === '/api/v1/audit-logs' &&
        candidate.params.get('page') === '2' &&
        candidate.params.get('keyword') === 'admin' &&
        candidate.params.get('action') === 'user.update',
    );
    request.flush({ items: [], total: 0, page: 2, pageSize: 20 });
    await expect(result).resolves.toEqual({ items: [], total: 0, page: 2, pageSize: 20 });
  });
});
