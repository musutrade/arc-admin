import { HttpClient, HttpParams } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { firstValueFrom } from 'rxjs';
import { ApiPage, ApiUser, CreateUserRequest, UpdateUserRequest } from '../api.models';
import { User } from '../models';
import { API_BASE_URL } from '../runtime-config';

const AVATAR_COLORS = ['#165dff', '#00b42a', '#ff7d00', '#f53f3f', '#722ed1'];

@Injectable({
  providedIn: 'root',
})
export class UserApiService {
  private readonly http = inject(HttpClient);
  private readonly apiBaseUrl = inject(API_BASE_URL);

  async getUsers(): Promise<User[]> {
    const pageSize = 100;
    const first = await this.getPage(1, pageSize);
    const pageCount = Math.ceil(first.total / pageSize);
    const remaining = await Promise.all(
      Array.from({ length: Math.max(0, pageCount - 1) }, (_, index) =>
        this.getPage(index + 2, pageSize),
      ),
    );
    return [first, ...remaining].flatMap((page) => page.items.map(mapUser));
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

  private getPage(page: number, pageSize: number): Promise<ApiPage<ApiUser>> {
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
