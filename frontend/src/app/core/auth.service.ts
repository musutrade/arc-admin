import { HttpClient } from '@angular/common/http';
import { Injectable, inject, signal } from '@angular/core';
import { firstValueFrom } from 'rxjs';
import { API_BASE_URL } from './runtime-config';
import { ApiUser, ChangePasswordRequest, LoginResponse, PermissionCodes } from './api.models';
import { AuthTokenStore } from './auth-token.store';

@Injectable({ providedIn: 'root' })
export class AuthService {
  private readonly http = inject(HttpClient);
  private readonly apiBaseUrl = inject(API_BASE_URL);
  private readonly tokenStore = inject(AuthTokenStore);
  private readonly userState = signal<ApiUser | null>(null);
  private readonly permissionState = signal<ReadonlySet<string>>(new Set());
  private sessionCheck: Promise<boolean> | null = null;

  readonly currentUser = this.userState.asReadonly();
  readonly permissions = this.permissionState.asReadonly();

  async login(username: string, password: string, remember: boolean): Promise<void> {
    const response = await firstValueFrom(
      this.http.post<LoginResponse>(`${this.apiBaseUrl}/auth/login`, { username, password }),
    );
    this.tokenStore.set(response.accessToken, remember);
    this.userState.set(response.user);
    try {
      await this.loadPermissions();
    } catch (error) {
      this.clearSession();
      throw error;
    }
  }

  ensureSession(): Promise<boolean> {
    if (!this.tokenStore.token()) {
      return Promise.resolve(false);
    }
    if (this.userState()) {
      return this.refreshPermissions()
        .then(() => true)
        .catch(() => {
          this.clearSession();
          return false;
        });
    }
    if (!this.sessionCheck) {
      this.sessionCheck = Promise.all([
        firstValueFrom(this.http.get<ApiUser>(`${this.apiBaseUrl}/auth/me`)),
        firstValueFrom(this.http.get<PermissionCodes>(`${this.apiBaseUrl}/auth/me/permissions`)),
      ])
        .then(([user, permissions]) => {
          this.userState.set(user);
          this.permissionState.set(new Set(permissions.codes));
          return true;
        })
        .catch(() => {
          this.clearSession();
          return false;
        })
        .finally(() => {
          this.sessionCheck = null;
        });
    }
    return this.sessionCheck;
  }

  hasPermission(code: string): boolean {
    return this.permissionState().has(code);
  }

  hasAllPermissions(codes: readonly string[]): boolean {
    return codes.every((code) => this.permissionState().has(code));
  }

  async changePassword(request: ChangePasswordRequest): Promise<void> {
    await firstValueFrom(this.http.put<void>(`${this.apiBaseUrl}/auth/me/password`, request));
    this.clearSession();
  }

  async refreshSession(): Promise<void> {
    const [user, permissions] = await Promise.all([
      firstValueFrom(this.http.get<ApiUser>(`${this.apiBaseUrl}/auth/me`)),
      firstValueFrom(this.http.get<PermissionCodes>(`${this.apiBaseUrl}/auth/me/permissions`)),
    ]);
    this.userState.set(user);
    this.permissionState.set(new Set(permissions.codes));
  }

  logout(): void {
    this.clearSession();
  }

  handleUnauthorized(): void {
    this.clearSession();
  }

  async refreshPermissions(): Promise<void> {
    const response = await firstValueFrom(
      this.http.get<PermissionCodes>(`${this.apiBaseUrl}/auth/me/permissions`),
    );
    this.permissionState.set(new Set(response.codes));
  }

  private loadPermissions(): Promise<void> {
    return this.refreshPermissions();
  }

  private clearSession(): void {
    this.tokenStore.clear();
    this.userState.set(null);
    this.permissionState.set(new Set());
  }
}
