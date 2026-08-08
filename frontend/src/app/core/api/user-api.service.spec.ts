import { provideHttpClient } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';
import { TestBed } from '@angular/core/testing';
import { ApiUser } from '../api.models';
import { API_BASE_URL } from '../runtime-config';
import { UserApiService } from './user-api.service';

const apiUser: ApiUser = {
  id: 7,
  username: 'sarah',
  displayName: 'Sarah Chen',
  email: 'sarah@example.com',
  status: 'active',
  roles: ['查看者'],
  lastLoginAt: null,
  createdAt: '2026-08-01T00:00:00Z',
};

describe('UserApiService', () => {
  let service: UserApiService;
  let http: HttpTestingController;

  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [
        provideHttpClient(),
        provideHttpClientTesting(),
        { provide: API_BASE_URL, useValue: '/api/v1' },
      ],
    });
    service = TestBed.inject(UserApiService);
    http = TestBed.inject(HttpTestingController);
  });

  afterEach(() => http.verify());

  it('maps paged API users into view users', async () => {
    const result = service.getUsers();
    const request = http.expectOne(
      (candidate) => candidate.url === '/api/v1/users' && candidate.params.get('page') === '1',
    );
    request.flush({ items: [apiUser], total: 1, page: 1, pageSize: 100 });

    await expect(result).resolves.toEqual([
      expect.objectContaining({
        id: '7',
        username: 'sarah',
        name: 'Sarah Chen',
        lastLogin: null,
      }),
    ]);
  });

  it('updates a user through the existing user endpoint', async () => {
    const result = service.updateUser('7', { status: 'inactive' });
    const request = http.expectOne('/api/v1/users/7');
    expect(request.request.method).toBe('PUT');
    expect(request.request.body).toEqual({ status: 'inactive' });
    request.flush({ ...apiUser, status: 'inactive' });

    await expect(result).resolves.toEqual(expect.objectContaining({ id: '7', status: 'inactive' }));
  });

  it('sends numeric role ids when assigning user roles', async () => {
    const result = service.assignUserRoles('7', ['2', '4']);
    const request = http.expectOne('/api/v1/users/7/roles');
    expect(request.request.method).toBe('PUT');
    expect(request.request.body).toEqual({ roleIds: [2, 4] });
    request.flush(null);
    await result;
  });
});
