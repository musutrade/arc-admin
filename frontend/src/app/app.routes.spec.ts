import { provideZonelessChangeDetection } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import {
  ActivatedRouteSnapshot,
  Router,
  RouterStateSnapshot,
  UrlTree,
  provideRouter,
} from '@angular/router';
import { authGuard } from './app.routes';

function makeRoute(path: string): ActivatedRouteSnapshot {
  return { routeConfig: { path } } as unknown as ActivatedRouteSnapshot;
}

describe('authGuard', () => {
  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [provideZonelessChangeDetection(), provideRouter([])],
    });
    localStorage.clear();
    sessionStorage.clear();
  });

  function run(path: string): unknown {
    return TestBed.runInInjectionContext(() =>
      authGuard(makeRoute(path), {} as RouterStateSnapshot),
    );
  }

  it('redirects to /login when unauthenticated', () => {
    const result = run('users');
    expect(result instanceof UrlTree).toBe(true);
    const router = TestBed.inject(Router);
    expect(router.serializeUrl(result as UrlTree)).toBe('/login');
  });

  it('allows access when authenticated via localStorage', () => {
    localStorage.setItem('arc-auth', 'mock-token');
    expect(run('users')).toBe(true);
  });

  it('allows access when authenticated via sessionStorage', () => {
    sessionStorage.setItem('arc-auth', 'mock-token');
    expect(run('roles')).toBe(true);
  });

  it('always allows public error pages without authentication', () => {
    expect(run('403')).toBe(true);
    expect(run('404')).toBe(true);
    expect(run('500')).toBe(true);
  });
});
