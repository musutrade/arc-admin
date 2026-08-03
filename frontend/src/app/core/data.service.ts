import { Injectable } from '@angular/core';
import {
  MOCK_ASSIGNED_MAP,
  MOCK_PERMISSION_GROUPS,
  MOCK_ROLE_PERMISSION_ROWS,
  MOCK_ROLES,
  MOCK_USER_STATS,
  MOCK_USERS,
} from './mock-data';
import { PermissionGroup, Role, RolePermissionRow, StatCard, User } from './models';

/** 模拟网络延迟 */
function delay<T>(value: T, ms = 200): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), ms));
}

/** Mock 数据服务:所有方法模拟异步接口,后续可无缝替换为真实 HTTP */
@Injectable({ providedIn: 'root' })
export class DataService {
  getUsers(): Promise<User[]> {
    return delay(MOCK_USERS);
  }

  getUserStats(): Promise<StatCard[]> {
    return delay(MOCK_USER_STATS, 120);
  }

  getPermissionGroups(): Promise<PermissionGroup[]> {
    return delay(MOCK_PERMISSION_GROUPS);
  }

  getRoles(): Promise<Role[]> {
    return delay(MOCK_ROLES);
  }

  getRolePermissionRows(): Promise<RolePermissionRow[]> {
    return delay(MOCK_ROLE_PERMISSION_ROWS);
  }

  getAssignedPermissionIds(roleId: string): Promise<string[]> {
    return delay(MOCK_ASSIGNED_MAP[roleId] ?? []);
  }
}
