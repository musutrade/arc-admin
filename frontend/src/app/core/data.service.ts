import { HttpClient, HttpParams } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { firstValueFrom } from 'rxjs';
import { API_BASE_URL } from './api.config';
import {
  ApiPage,
  ApiPermissionGroup,
  ApiRole,
  ApiUser,
  CreateRoleRequest,
  CreateUserRequest,
  DashboardStats,
  RolePermissions,
  UpdateRoleRequest,
  UpdateUserRequest,
} from './api.models';
import { PermissionGroup, Role, RolePermissionRow, StatCard, User } from './models';

const AVATAR_COLORS = ['#165dff', '#00b42a', '#ff7d00', '#f53f3f', '#722ed1'];

@Injectable({ providedIn: 'root' })
export class DataService {
  private readonly http = inject(HttpClient);
  private readonly apiBaseUrl = inject(API_BASE_URL);

  async getUsers(): Promise<User[]> {
    const pageSize = 100;
    const first = await this.getUserPage(1, pageSize);
    const pageCount = Math.ceil(first.total / pageSize);
    const remaining = await Promise.all(
      Array.from({ length: Math.max(0, pageCount - 1) }, (_, index) =>
        this.getUserPage(index + 2, pageSize),
      ),
    );
    return [first, ...remaining].flatMap((page) => page.items.map(mapUser));
  }

  async getUserStats(): Promise<StatCard[]> {
    const stats = await firstValueFrom(
      this.http.get<DashboardStats>(`${this.apiBaseUrl}/dashboard/stats`),
    );
    return [
      { label: '用户总数', value: String(stats.totalUsers), icon: 'group' },
      { label: '启用用户', value: String(stats.activeUsers), icon: 'verified_user' },
      { label: '角色总数', value: String(stats.totalRoles), icon: 'badge' },
      { label: '已暂停用户', value: String(stats.suspendedUsers), icon: 'person_off' },
    ];
  }

  async getPermissionGroups(): Promise<PermissionGroup[]> {
    const groups = await firstValueFrom(
      this.http.get<ApiPermissionGroup[]>(`${this.apiBaseUrl}/permissions/groups`),
    );
    return groups.map((group) => ({
      id: String(group.id),
      code: group.code,
      name: group.name,
      icon: group.icon ?? 'folder',
      permissions: group.permissions.map((permission) => ({
        id: String(permission.id),
        code: permission.code,
        name: permission.name,
        type: permission.type,
        description: permission.description ?? '',
      })),
    }));
  }

  async getRoles(): Promise<Role[]> {
    const roles = await firstValueFrom(this.http.get<ApiRole[]>(`${this.apiBaseUrl}/roles`));
    return roles.map(mapRole);
  }

  async getRolePermissionRows(): Promise<RolePermissionRow[]> {
    const roles = await this.getRoles();
    return roles.map((role) => ({
      roleId: role.id,
      roleCode: role.code,
      roleName: role.name,
      usersAssigned: role.members,
      active: role.isActive,
      groupIds: role.permissionGroupIds,
    }));
  }

  async getAssignedPermissionIds(roleId: string): Promise<string[]> {
    const response = await firstValueFrom(
      this.http.get<RolePermissions>(`${this.apiBaseUrl}/roles/${roleId}/permissions`),
    );
    return response.permissionIds.map(String);
  }

  async createUser(request: CreateUserRequest): Promise<User> {
    const user = await firstValueFrom(this.http.post<ApiUser>(`${this.apiBaseUrl}/users`, request));
    return mapUser(user);
  }

  async updateUser(id: string, request: UpdateUserRequest): Promise<User> {
    const user = await firstValueFrom(
      this.http.put<ApiUser>(`${this.apiBaseUrl}/users/${id}`, request),
    );
    return mapUser(user);
  }

  async deleteUser(id: string): Promise<void> {
    await firstValueFrom(this.http.delete<void>(`${this.apiBaseUrl}/users/${id}`));
  }

  async assignUserRoles(id: string, roleIds: string[]): Promise<void> {
    await firstValueFrom(
      this.http.put<void>(`${this.apiBaseUrl}/users/${id}/roles`, {
        roleIds: roleIds.map(Number),
      }),
    );
  }

  async createRole(request: CreateRoleRequest): Promise<Role> {
    const role = await firstValueFrom(this.http.post<ApiRole>(`${this.apiBaseUrl}/roles`, request));
    return mapRole(role);
  }

  async updateRole(id: string, request: UpdateRoleRequest): Promise<Role> {
    const role = await firstValueFrom(
      this.http.put<ApiRole>(`${this.apiBaseUrl}/roles/${id}`, request),
    );
    return mapRole(role);
  }

  async deleteRole(id: string): Promise<void> {
    await firstValueFrom(this.http.delete<void>(`${this.apiBaseUrl}/roles/${id}`));
  }

  async assignRolePermissions(roleId: string, permissionIds: string[]): Promise<void> {
    await firstValueFrom(
      this.http.put<void>(`${this.apiBaseUrl}/roles/${roleId}/permissions`, {
        permissionIds: permissionIds.map(Number),
      }),
    );
  }

  private getUserPage(page: number, pageSize: number): Promise<ApiPage<ApiUser>> {
    const params = new HttpParams().set('page', page).set('pageSize', pageSize);
    return firstValueFrom(this.http.get<ApiPage<ApiUser>>(`${this.apiBaseUrl}/users`, { params }));
  }
}

function mapUser(user: ApiUser): User {
  return {
    id: String(user.id),
    username: user.username,
    name: user.displayName,
    email: user.email ?? '',
    roles: user.roles,
    status: user.status,
    lastLogin: user.lastLoginAt,
    createdAt: user.createdAt,
    avatarColor: AVATAR_COLORS[user.id % AVATAR_COLORS.length],
  };
}

function mapRole(role: ApiRole): Role {
  return {
    id: String(role.id),
    code: role.code,
    name: role.name,
    category: role.category,
    icon: role.icon ?? 'badge',
    color: role.color,
    description: role.description ?? '',
    members: role.members,
    permissionGroupIds: role.permissionGroupIds.map(String),
    isActive: role.isActive,
  };
}
