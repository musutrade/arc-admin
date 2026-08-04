import { provideHttpClient } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';
import { TestBed } from '@angular/core/testing';
import { API_BASE_URL } from './api.config';
import { ApiUser, LoginResponse } from './api.models';
import { AuthTokenStore } from './auth-token.store';
import { AuthService } from './auth.service';

const user: ApiUser = {
  id: 1,
  username: 'admin',
  displayName: 'Administrator',
  email: 'admin@example.test',
  status: 'active',
  roles: ['Super Admin'],
  lastLoginAt: null,
  createdAt: '2026-08-01T00:00:00Z',
};

const loginResponse: LoginResponse = {
  accessToken: 'access-token',
  tokenType: 'Bearer',
  expiresIn: 3600,
  user,
};

describe('AuthService', () => {
  let service: AuthService;
  let http: HttpTestingController;
  let tokenStore: {
    token: ReturnType<typeof vi.fn>;
    set: ReturnType<typeof vi.fn>;
    clear: ReturnType<typeof vi.fn>;
  };

  beforeEach(() => {
    tokenStore = {
      token: vi.fn(() => null),
      set: vi.fn(),
      clear: vi.fn(),
    };
    TestBed.configureTestingModule({
      providers: [
        provideHttpClient(),
        provideHttpClientTesting(),
        { provide: API_BASE_URL, useValue: '/api/v1' },
        { provide: AuthTokenStore, useValue: tokenStore },
      ],
    });
    service = TestBed.inject(AuthService);
    http = TestBed.inject(HttpTestingController);
  });

  afterEach(() => http.verify());

  it('stores the token and loads permissions after login', async () => {
    const result = service.login('admin', 'secure-password', true);
    http.expectOne('/api/v1/auth/login').flush(loginResponse);
    await Promise.resolve();
    http.expectOne('/api/v1/auth/me/permissions').flush({ codes: ['user:directory:read'] });
    await result;

    expect(tokenStore.set).toHaveBeenCalledWith('access-token', true);
    expect(service.currentUser()).toEqual(user);
    expect(service.hasPermission('user:directory:read')).toBe(true);
  });

  it('clears the partial session when permission loading fails', async () => {
    const result = service.login('admin', 'secure-password', false);
    http.expectOne('/api/v1/auth/login').flush(loginResponse);
    await Promise.resolve();
    http
      .expectOne('/api/v1/auth/me/permissions')
      .flush({ message: 'failed' }, { status: 500, statusText: 'Server Error' });

    await expect(result).rejects.toBeTruthy();
    expect(tokenStore.clear).toHaveBeenCalledOnce();
    expect(service.currentUser()).toBeNull();
    expect(service.permissions().size).toBe(0);
  });
});
