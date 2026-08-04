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

function makeRoute(path: string, permission?: string): ActivatedRouteSnapshot {
  return {
    routeConfig: { path },
    data: permission ? { permission } : {},
  } as unknown as ActivatedRouteSnapshot;
}

describe('route guards', () => {
  const auth = {
    ensureSession: vi.fn(() => Promise.resolve(false)),
    hasPermission: vi.fn(() => false),
  };

  beforeEach(() => {
    auth.ensureSession.mockReset().mockResolvedValue(false);
    auth.hasPermission.mockReset().mockReturnValue(false);
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
});
