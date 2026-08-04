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
  roles: ['Viewer'],
  lastLoginAt: null,
  createdAt: '2026-08-01T00:00:00Z',
};

const apiRole: ApiRole = {
  id: 3,
  code: 'viewer',
  name: 'Viewer',
  category: 'Read Only',
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
      expect.objectContaining({ id: '7', username: 'sarah', name: 'Sarah Chen' }),
    ]);
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

    await expect(result).resolves.toHaveLength(4);
  });

  it('maps permission groups and nullable fields', async () => {
    const result = service.getPermissionGroups();
    const group: ApiPermissionGroup = {
      id: 2,
      code: 'identity',
      name: 'Identity',
      icon: null,
      permissions: [
        {
          id: 9,
          code: 'user:directory:read',
          name: 'View Users',
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
      expect.objectContaining({ roleId: '3', roleName: 'Viewer', usersAssigned: 4 }),
    ]);
  });

  it('sends numeric permission ids when assigning a role', async () => {
    const result = service.assignRolePermissions('3', ['7', '9']);
    const request = http.expectOne('/api/v1/roles/3/permissions');
    expect(request.request.method).toBe('PUT');
    expect(request.request.body).toEqual({ permissionIds: [7, 9] });
    request.flush(null);
    await result;
  });
});
