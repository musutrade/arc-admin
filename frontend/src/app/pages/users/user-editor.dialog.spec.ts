import { provideZonelessChangeDetection } from '@angular/core';
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { MAT_DIALOG_DATA, MatDialogRef } from '@angular/material/dialog';
import { UserEditorDialog, UserEditorData } from './user-editor.dialog';

const user = {
  id: '7',
  username: 'support',
  name: '支持人员',
  email: 'support@example.com',
  roles: ['二线支持'],
  status: 'active' as const,
  lastLogin: null,
  createdAt: '2026-08-08T00:00:00Z',
  avatarColor: '#165dff',
};

describe('UserEditorDialog', () => {
  async function create(data: UserEditorData): Promise<ComponentFixture<UserEditorDialog>> {
    TestBed.resetTestingModule();
    await TestBed.configureTestingModule({
      imports: [UserEditorDialog],
      providers: [
        provideZonelessChangeDetection(),
        { provide: MAT_DIALOG_DATA, useValue: data },
        { provide: MatDialogRef, useValue: { close: vi.fn() } },
      ],
    }).compileComponents();
    const fixture = TestBed.createComponent(UserEditorDialog);
    await fixture.whenStable();
    return fixture;
  }

  it('hides privileged controls when the operator lacks their permissions', async () => {
    const fixture = await create({
      user,
      roles: [],
      canResetPassword: false,
      canManageStatus: false,
      canManageRoles: false,
    });
    const text = fixture.nativeElement.textContent as string;
    expect(text).not.toContain('新密码');
    expect(text).not.toContain('状态');
    expect(text).not.toContain('角色');
  });

  it('shows privileged controls when all permissions are present', async () => {
    const fixture = await create({
      user,
      roles: [],
      canResetPassword: true,
      canManageStatus: true,
      canManageRoles: true,
    });
    const text = fixture.nativeElement.textContent as string;
    expect(text).toContain('新密码');
    expect(text).toContain('状态');
    expect(text).toContain('角色');
  });
});
