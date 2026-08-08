import { provideZonelessChangeDetection } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import {
  ActivatedRouteSnapshot,
  Router,
  RouterStateSnapshot,
  UrlTree,
  provideRouter,
} from '@angular/router';
import { authGuard, permissionGuard } from './app.routes';
import { AuthService } from './core/auth.service';

function makeRoute(
  path: string,
  permission?: string,
  permissions?: string[],
): ActivatedRouteSnapshot {
  return {
    routeConfig: { path },
    data: permission ? { permission } : permissions ? { permissions } : {},
  } as unknown as ActivatedRouteSnapshot;
}

describe('route guards', () => {
  const auth = {
    ensureSession: vi.fn(() => Promise.resolve(false)),
    hasPermission: vi.fn(() => false),
    hasAllPermissions: vi.fn(() => false),
  };

  beforeEach(() => {
    auth.ensureSession.mockReset().mockResolvedValue(false);
    auth.hasPermission.mockReset().mockReturnValue(false);
    auth.hasAllPermissions.mockReset().mockReturnValue(false);
    TestBed.configureTestingModule({
      providers: [
        provideZonelessChangeDetection(),
        provideRouter([]),
        { provide: AuthService, useValue: auth },
      ],
    });
  });

  function runAuth(path: string): unknown {
    return TestBed.runInInjectionContext(() =>
      authGuard(makeRoute(path), {} as RouterStateSnapshot),
    );
  }

  function runPermission(permission: string): unknown {
    return TestBed.runInInjectionContext(() =>
      permissionGuard(makeRoute('users', permission), {} as RouterStateSnapshot),
    );
  }

  function runPermissions(permissions: string[]): unknown {
    return TestBed.runInInjectionContext(() =>
      permissionGuard(
        makeRoute('role-permissions', undefined, permissions),
        {} as RouterStateSnapshot,
      ),
    );
  }

  it('redirects to login when the server session is invalid', async () => {
    const result = await runAuth('users');
    const router = TestBed.inject(Router);
    expect(result instanceof UrlTree).toBe(true);
    expect(router.serializeUrl(result as UrlTree)).toBe('/login');
  });

  it('allows an authenticated session', async () => {
    auth.ensureSession.mockResolvedValue(true);
    await expect(runAuth('users')).resolves.toBe(true);
  });

  it('always allows public error pages', () => {
    expect(runAuth('403')).toBe(true);
    expect(runAuth('404')).toBe(true);
    expect(runAuth('500')).toBe(true);
  });

  it('redirects authenticated users without the required permission', async () => {
    auth.ensureSession.mockResolvedValue(true);
    const result = await runPermission('user:directory:read');
    const router = TestBed.inject(Router);
    expect(router.serializeUrl(result as UrlTree)).toBe('/403');
  });

  it('allows authenticated users with the required permission', async () => {
    auth.ensureSession.mockResolvedValue(true);
    auth.hasPermission.mockReturnValue(true);
    await expect(runPermission('user:directory:read')).resolves.toBe(true);
  });

  it('requires every dependency for the role permission page', async () => {
    auth.ensureSession.mockResolvedValue(true);
    const required = ['role:permissions:write', 'role:directory:read', 'permission:directory:read'];
    const router = TestBed.inject(Router);
    const denied = await runPermissions(required);
    expect(router.serializeUrl(denied as UrlTree)).toBe('/403');

    auth.hasAllPermissions.mockReturnValue(true);
    await expect(runPermissions(required)).resolves.toBe(true);
  });
});
