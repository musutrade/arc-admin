import { HttpErrorResponse } from '@angular/common/http';
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { MatDialogRef } from '@angular/material/dialog';
import { AuthService } from './auth.service';
import { ChangePasswordDialog } from './change-password.dialog';

describe('ChangePasswordDialog', () => {
  let component: ChangePasswordDialog;
  let fixture: ComponentFixture<ChangePasswordDialog>;
  let auth: {
    changePassword: ReturnType<typeof vi.fn>;
    issueStepUp: ReturnType<typeof vi.fn>;
  };
  let dialogRef: { close: ReturnType<typeof vi.fn> };

  beforeEach(() => {
    auth = { changePassword: vi.fn(), issueStepUp: vi.fn() };
    dialogRef = { close: vi.fn() };
    TestBed.configureTestingModule({
      providers: [
        { provide: AuthService, useValue: auth },
        { provide: MatDialogRef, useValue: dialogRef },
      ],
    });
    fixture = TestBed.createComponent(ChangePasswordDialog);
    component = fixture.componentInstance;
  });

  it('rejects a confirmation that does not match the new password', async () => {
    component.passwordModel.set({
      currentPassword: 'current-password',
      newPassword: 'updated-password',
      confirmPassword: 'different-password',
      totpCode: '123456',
    });
    await fixture.whenStable();

    expect(component.passwordForm.confirmPassword().invalid()).toBe(true);
    expect(component.passwordForm.confirmPassword().errors()[0].message).toBe(
      '两次输入的新密码不一致',
    );
  });

  it('submits valid passwords and closes the dialog', async () => {
    auth.changePassword.mockResolvedValue(undefined);
    auth.issueStepUp.mockResolvedValue({ token: 'step-up-token' });
    component.passwordModel.set({
      currentPassword: 'current-password',
      newPassword: 'updated-password',
      confirmPassword: 'updated-password',
      totpCode: '123456',
    });

    component.submitPassword();

    await vi.waitFor(() => {
      expect(auth.issueStepUp).toHaveBeenCalledWith(
        'auth.password.change',
        'current-password',
        '123456',
      );
      expect(auth.changePassword).toHaveBeenCalledWith(
        {
          currentPassword: 'current-password',
          newPassword: 'updated-password',
        },
        'step-up-token',
      );
      expect(dialogRef.close).toHaveBeenCalledWith(true);
    });
  });

  it('shows the API error and keeps the dialog open', async () => {
    auth.changePassword.mockRejectedValue(
      new HttpErrorResponse({
        status: 422,
        error: { error: { message: '当前密码不正确' } },
      }),
    );
    auth.issueStepUp.mockResolvedValue({ token: 'step-up-token' });
    component.passwordModel.set({
      currentPassword: 'incorrect-password',
      newPassword: 'updated-password',
      confirmPassword: 'updated-password',
      totpCode: '123456',
    });

    component.submitPassword();

    await vi.waitFor(() => {
      expect(component.errorMessage()).toBe('当前密码不正确');
      expect(component.submitting()).toBe(false);
    });
    expect(dialogRef.close).not.toHaveBeenCalled();
  });
});
