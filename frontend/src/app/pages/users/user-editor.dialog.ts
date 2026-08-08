import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { Role, User, UserStatus } from '../../core/models';

export interface UserEditorData {
  user?: User;
  roles: Role[];
  canResetPassword: boolean;
  canManageStatus: boolean;
  canManageRoles: boolean;
}

export interface UserEditorResult {
  username: string;
  password: string;
  displayName: string;
  email: string;
  status: UserStatus;
  roleIds: string[];
}

@Component({
  selector: 'app-user-editor-dialog',
  imports: [ReactiveFormsModule, MatDialogModule, MatIconModule],
  templateUrl: './user-editor.dialog.html',
  styleUrl: '../../core/editor-dialog.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class UserEditorDialog {
  readonly data = inject<UserEditorData>(MAT_DIALOG_DATA);
  private readonly dialogRef = inject(MatDialogRef<UserEditorDialog>);
  private readonly fb = inject(FormBuilder);

  readonly form = this.fb.nonNullable.group({
    username: [
      { value: this.data.user?.username ?? '', disabled: Boolean(this.data.user) },
      [Validators.required, Validators.minLength(3), Validators.maxLength(64)],
    ],
    password: [
      '',
      this.data.user ? [Validators.minLength(8)] : [Validators.required, Validators.minLength(8)],
    ],
    displayName: [this.data.user?.name ?? '', [Validators.required, Validators.maxLength(128)]],
    email: [this.data.user?.email ?? '', [Validators.email]],
    status: [this.data.user?.status ?? ('active' as UserStatus), [Validators.required]],
    roleIds: [
      this.data.roles
        .filter((role) => this.data.user?.roles.includes(role.name))
        .map((role) => role.id),
    ],
  });

  submit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }
    this.dialogRef.close(this.form.getRawValue() satisfies UserEditorResult);
  }
}
