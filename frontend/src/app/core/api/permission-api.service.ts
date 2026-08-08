import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { firstValueFrom } from 'rxjs';
import { ApiPermissionGroup } from '../api.models';
import { PermissionGroup } from '../models';
import { API_BASE_URL } from '../runtime-config';

@Injectable({
  providedIn: 'root',
})
export class PermissionApiService {
  private readonly http = inject(HttpClient);
  private readonly apiBaseUrl = inject(API_BASE_URL);

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
}
