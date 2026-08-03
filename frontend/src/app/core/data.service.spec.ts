import { TestBed } from '@angular/core/testing';
import { DataService } from './data.service';

describe('DataService', () => {
  let service: DataService;

  beforeEach(() => {
    TestBed.configureTestingModule({});
    service = TestBed.inject(DataService);
  });

  it('returns mock users', async () => {
    const users = await service.getUsers();
    expect(users.length).toBeGreaterThan(0);
    expect(users[0].id).toBeDefined();
    expect(users[0].roles.length).toBeGreaterThan(0);
  });

  it('returns mock stats', async () => {
    const stats = await service.getUserStats();
    expect(stats.length).toBe(4);
    expect(stats.every((s) => s.label && s.value)).toBe(true);
  });

  it('returns permission groups with nested permissions', async () => {
    const groups = await service.getPermissionGroups();
    expect(groups.length).toBeGreaterThan(0);
    expect(groups.every((g) => g.permissions.length > 0)).toBe(true);
  });

  it('returns roles', async () => {
    const roles = await service.getRoles();
    expect(roles.length).toBeGreaterThan(0);
    expect(roles.every((r) => r.id && r.name)).toBe(true);
  });

  it('returns role permission rows', async () => {
    const rows = await service.getRolePermissionRows();
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.every((r) => r.roleId && r.roleName)).toBe(true);
  });

  it('returns assigned permission ids for a known role', async () => {
    const ids = await service.getAssignedPermissionIds('r-001');
    expect(ids.length).toBeGreaterThan(0);
  });

  it('returns an empty list for an unknown role', async () => {
    const ids = await service.getAssignedPermissionIds('unknown-role');
    expect(ids).toEqual([]);
  });
});
