import { provideZonelessChangeDetection } from '@angular/core';
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { AuthService } from '../../core/auth.service';
import { DataService } from '../../core/data.service';
import { User } from '../../core/models';

const USERS: User[] = Array.from({ length: 12 }, (_, index) => ({
  id: String(index + 1),
  username: index === 0 ? 'sarah' : `user${index + 1}`,
  name: index === 0 ? 'Sarah Jenkins' : `User ${index + 1}`,
  email: index === 0 ? 'sarah@example.com' : `user${index + 1}@example.com`,
  roles: [index === 0 ? 'Auditor' : index % 2 === 0 ? 'Viewer' : 'Editor'],
  status: index === 1 ? 'suspended' : index % 3 === 0 ? 'inactive' : 'active',
  lastLogin: '2026-08-01T00:00:00Z',
  createdAt: '2026-01-01T00:00:00Z',
  avatarColor: '#165dff',
}));
import { UsersPage } from './users';

const dataServiceStub: Partial<DataService> = {
  getUsers: () => Promise.resolve(USERS),
  getUserStats: () => Promise.resolve([]),
};
const authServiceStub: Partial<AuthService> = {
  hasPermission: () => false,
};

describe('UsersPage', () => {
  let fixture: ComponentFixture<UsersPage>;
  let page: UsersPage;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [UsersPage],
      providers: [
        provideZonelessChangeDetection(),
        { provide: DataService, useValue: dataServiceStub },
        { provide: AuthService, useValue: authServiceStub },
      ],
    }).compileComponents();
    fixture = TestBed.createComponent(UsersPage);
    page = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('loads users and finishes loading on init', () => {
    expect(page.users().length).toBe(USERS.length);
    expect(page.loading()).toBe(false);
  });

  it('derives unique role options from users', () => {
    const roles = page.roleOptions();
    expect(roles[0]).toBe('all');
    expect(roles).toContain('Auditor');
  });

  it('filters users by search term', () => {
    page.searchUsers('sarah');
    expect(page.filteredUsers().map((u) => u.name)).toEqual(['Sarah Jenkins']);
  });

  it('filters users by role', () => {
    page.applyRoleFilter('Auditor');
    expect(page.filteredUsers().length).toBeGreaterThan(0);
    expect(page.filteredUsers().every((u) => u.roles.includes('Auditor'))).toBe(true);
  });

  it('filters users by status', () => {
    page.applyStatusFilter('suspended');
    expect(page.filteredUsers().length).toBeGreaterThan(0);
    expect(page.filteredUsers().every((u) => u.status === 'suspended')).toBe(true);
  });

  it('resets filters and returns to the first page', () => {
    page.searchUsers('zzz');
    page.applyRoleFilter('Auditor');
    page.goToPage(2);
    page.resetFilters();
    expect(page.search()).toBe('');
    expect(page.roleFilter()).toBe('all');
    expect(page.statusFilter()).toBe('all');
    expect(page.page()).toBe(1);
    expect(page.filteredUsers().length).toBe(USERS.length);
  });

  it('paginates with correct totals and page bounds', () => {
    const expectedPages = Math.ceil(USERS.length / page.pageSize);
    expect(page.totalPages()).toBe(expectedPages);
    page.goToPage(99);
    expect(page.page()).toBe(expectedPages);
    page.goToPage(-5);
    expect(page.page()).toBe(1);
    expect(page.pagedUsers().length).toBe(page.pageSize);
  });

  it('tracks row selection', () => {
    const first = USERS[0];
    page.toggleRow(first.id, true);
    expect(page.selected().has(first.id)).toBe(true);
    expect(page.selectedCount()).toBe(1);
    page.toggleRow(first.id, false);
    expect(page.selected().has(first.id)).toBe(false);
  });
});
