import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { firstValueFrom } from 'rxjs';
import { ApiRole, CreateRoleRequest, RolePermissions, UpdateRoleRequest } from '../api.models';
import { Role, RolePermissionRow } from '../models';
import { API_BASE_URL } from '../runtime-config';

@Injectable({
  providedIn: 'root',
})
export class RoleApiService {
  private readonly http = inject(HttpClient);
  private readonly apiBaseUrl = inject(API_BASE_URL);

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
