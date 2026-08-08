import { Injectable, inject, signal } from '@angular/core';
import { Api } from '../generated/api/api';
import { changeCurrentUserPassword } from '../generated/api/fn/auth/change-current-user-password';
import { getCurrentUserPermissions } from '../generated/api/fn/auth/get-current-user-permissions';
import { getCurrentUser } from '../generated/api/fn/auth/get-current-user';
import { login as loginRequest } from '../generated/api/fn/auth/login';
import { logout as logoutRequest } from '../generated/api/fn/auth/logout';
import { ChangePasswordRequest } from '../generated/api/models/change-password-request';
import { UserResponse } from '../generated/api/models/user-response';

@Injectable({ providedIn: 'root' })
export class AuthService {
  private readonly api = inject(Api);
  private readonly userState = signal<UserResponse | null>(null);
  private readonly permissionState = signal<ReadonlySet<string>>(new Set());
  private sessionCheck: Promise<boolean> | null = null;

  readonly currentUser = this.userState.asReadonly();
  readonly permissions = this.permissionState.asReadonly();

  async login(username: string, password: string, remember: boolean): Promise<void> {
    const response = await this.api.invoke(loginRequest, {
      body: { username, password, remember },
    });
    this.userState.set(response.user);
    try {
      await this.loadPermissions();
    } catch (error) {
      await this.logout().catch(() => undefined);
      throw error;
    }
  }

  ensureSession(): Promise<boolean> {
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
        this.api.invoke(getCurrentUser),
        this.api.invoke(getCurrentUserPermissions),
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
    await this.api.invoke(changeCurrentUserPassword, { body: request });
    this.clearSession();
  }

  async refreshSession(): Promise<void> {
    const [user, permissions] = await Promise.all([
      this.api.invoke(getCurrentUser),
      this.api.invoke(getCurrentUserPermissions),
    ]);
    this.userState.set(user);
    this.permissionState.set(new Set(permissions.codes));
  }

  async logout(): Promise<void> {
    try {
      await this.api.invoke(logoutRequest);
    } finally {
      this.clearSession();
    }
  }

  handleUnauthorized(): void {
    this.clearSession();
  }

  async refreshPermissions(): Promise<void> {
    const response = await this.api.invoke(getCurrentUserPermissions);
    this.permissionState.set(new Set(response.codes));
  }

  private loadPermissions(): Promise<void> {
    return this.refreshPermissions();
  }

  private clearSession(): void {
    this.userState.set(null);
    this.permissionState.set(new Set());
  }
}
