import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';

@Component({
  selector: 'app-password-dialog',
  imports: [ReactiveFormsModule, MatDialogModule, MatIconModule],
  template: `
    <form class="editor-dialog compact-dialog" [formGroup]="form" (ngSubmit)="submit()">
      <div class="dialog-title-row">
        <mat-icon>lock_reset</mat-icon>
        <h2>Reset Password</h2>
      </div>
      <p class="dialog-message">Set a new password for {{ username }}.</p>
      <div class="form-field">
        <label for="new-password">New password</label>
        <input
          id="new-password"
          type="password"
          formControlName="password"
          autocomplete="new-password"
        />
        @if (form.controls.password.touched && form.controls.password.invalid) {
          <small>Password must contain at least 8 characters.</small>
        }
      </div>
      <div class="dialog-actions">
        <button type="button" class="btn-outline" mat-dialog-close>Cancel</button>
        <button type="submit" class="btn-primary">Reset Password</button>
      </div>
    </form>
  `,
  styleUrl: '../../core/editor-dialog.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class PasswordDialog {
  readonly username = inject<string>(MAT_DIALOG_DATA);
  private readonly dialogRef = inject(MatDialogRef<PasswordDialog>);
  private readonly fb = inject(FormBuilder);
  readonly form = this.fb.nonNullable.group({
    password: ['', [Validators.required, Validators.minLength(8)]],
  });

  submit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }
    this.dialogRef.close(this.form.getRawValue().password);
  }
}
