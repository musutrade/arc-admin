import { provideZonelessChangeDetection } from '@angular/core';
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { DataService } from '../../core/data.service';
import { MOCK_USERS } from '../../core/mock-data';
import { UsersPage } from './users';

const dataServiceStub: Partial<DataService> = {
  getUsers: () => Promise.resolve(MOCK_USERS),
  getUserStats: () => Promise.resolve([]),
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
      ],
    }).compileComponents();
    fixture = TestBed.createComponent(UsersPage);
    page = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('loads users and finishes loading on init', () => {
    expect(page.users().length).toBe(MOCK_USERS.length);
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
    expect(page.filteredUsers().length).toBe(MOCK_USERS.length);
  });

  it('paginates with correct totals and page bounds', () => {
    const expectedPages = Math.ceil(MOCK_USERS.length / page.pageSize);
    expect(page.totalPages()).toBe(expectedPages);
    page.goToPage(99);
    expect(page.page()).toBe(expectedPages);
    page.goToPage(-5);
    expect(page.page()).toBe(1);
    expect(page.pagedUsers().length).toBe(page.pageSize);
  });

  it('tracks row selection', () => {
    const first = MOCK_USERS[0];
    page.toggleRow(first.id, true);
    expect(page.selected().has(first.id)).toBe(true);
    expect(page.selectedCount()).toBe(1);
    page.toggleRow(first.id, false);
    expect(page.selected().has(first.id)).toBe(false);
  });
});
