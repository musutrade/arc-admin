import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormField, form, required, submit, validate } from '@angular/forms/signals';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';

export interface StepUpDialogData {
  title: string;
  message: string;
}

export interface StepUpCredentials {
  currentPassword: string;
  totpCode: string;
}

@Component({
  selector: 'app-step-up-dialog',
  imports: [FormField, MatDialogModule, MatIconModule, MatProgressSpinnerModule],
  template: `
    <form
      class="editor-dialog compact-dialog"
      (submit)="confirm(); $event.preventDefault()"
      novalidate
    >
      <div class="dialog-title-row">
        <mat-icon>verified_user</mat-icon>
        <h2>{{ data.title }}</h2>
      </div>
      <p class="dialog-message">{{ data.message }}</p>

      <div class="form-grid">
        <div class="form-field">
          <label for="step-up-current-password">当前密码</label>
          <input
            id="step-up-current-password"
            type="password"
            [formField]="stepUpForm.currentPassword"
            autocomplete="current-password"
          />
          @if (
            stepUpForm.currentPassword().touched() && stepUpForm.currentPassword().errors().length
          ) {
            <small>{{ stepUpForm.currentPassword().errors()[0].message }}</small>
          }
        </div>
        <div class="form-field">
          <label for="step-up-totp">身份验证器验证码</label>
          <div
            class="verification-input"
            [class.input-error]="stepUpForm.totpCode().touched() && stepUpForm.totpCode().invalid()"
          >
            <mat-icon>verified_user</mat-icon>
            <input
              id="step-up-totp"
              type="text"
              [formField]="stepUpForm.totpCode"
              inputmode="numeric"
              autocomplete="one-time-code"
              placeholder="000000"
            />
          </div>
          @if (stepUpForm.totpCode().touched() && stepUpForm.totpCode().errors().length) {
            <small>{{ stepUpForm.totpCode().errors()[0].message }}</small>
          }
        </div>
      </div>

      <div class="dialog-actions">
        <button type="button" class="btn-outline" mat-dialog-close [disabled]="submitting()">
          取消
        </button>
        <button
          type="submit"
          class="btn-primary"
          [disabled]="stepUpForm().invalid() || submitting()"
          [attr.aria-busy]="submitting()"
        >
          @if (submitting()) {
            <mat-progress-spinner diameter="18" mode="indeterminate" />
          }
          <span>继续</span>
        </button>
      </div>
    </form>
  `,
  styleUrls: ['./editor-dialog.scss', './step-up.dialog.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class StepUpDialog {
  readonly data = inject<StepUpDialogData>(MAT_DIALOG_DATA);
  private readonly dialogRef = inject(MatDialogRef<StepUpDialog>);
  readonly submitting = signal(false);
  readonly stepUpModel = signal({ currentPassword: '', totpCode: '' });
  readonly stepUpForm = form(this.stepUpModel, (path) => {
    required(path.currentPassword, { message: '请输入当前密码' });
    validate(path.totpCode, ({ value }) =>
      value().length > 0 && value().length !== 6
        ? { kind: 'totpLength', message: '验证码应为 6 位' }
        : undefined,
    );
  });

  confirm(): void {
    if (this.submitting()) {
      return;
    }
    submit(this.stepUpForm, async () => {
      this.submitting.set(true);
      this.dialogRef.close(this.stepUpModel());
    });
  }
}
