import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { Role } from '../../core/models';

export interface RoleEditorResult {
  code: string;
  name: string;
  category: string;
  icon: string;
  color: Role['color'];
  description: string;
  isActive: boolean;
}

@Component({
  selector: 'app-role-editor-dialog',
  imports: [ReactiveFormsModule, MatDialogModule, MatIconModule],
  templateUrl: './role-editor.dialog.html',
  styleUrl: '../../core/editor-dialog.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class RoleEditorDialog {
  readonly role = inject<Role | null>(MAT_DIALOG_DATA);
  private readonly dialogRef = inject(MatDialogRef<RoleEditorDialog>);
  private readonly fb = inject(FormBuilder);

  readonly form = this.fb.nonNullable.group({
    code: [
      { value: this.role?.code ?? '', disabled: Boolean(this.role) },
      [Validators.required, Validators.pattern(/^[a-z][a-z0-9_]{2,63}$/)],
    ],
    name: [this.role?.name ?? '', [Validators.required, Validators.maxLength(128)]],
    category: [this.role?.category ?? 'general', [Validators.required]],
    icon: [this.role?.icon ?? 'badge'],
    color: [this.role?.color ?? ('neutral' as Role['color'])],
    description: [this.role?.description ?? ''],
    isActive: [this.role?.isActive ?? true],
  });

  submit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }
    this.dialogRef.close(this.form.getRawValue() satisfies RoleEditorResult);
  }
}
