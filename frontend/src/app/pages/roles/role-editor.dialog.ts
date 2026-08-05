import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatSelectModule } from '@angular/material/select';
import { Role } from '../../core/models';

interface RoleIconOption {
  value: string;
  label: string;
}

interface RoleIconGroup {
  label: string;
  options: readonly RoleIconOption[];
}

const ROLE_ICON_GROUPS: readonly RoleIconGroup[] = [
  {
    label: 'People',
    options: [
      { value: 'badge', label: 'Role badge' },
      { value: 'person', label: 'Person' },
      { value: 'group', label: 'Team' },
      { value: 'groups', label: 'Organization' },
      { value: 'supervisor_account', label: 'Supervisor' },
      { value: 'manage_accounts', label: 'Account manager' },
      { value: 'engineering', label: 'Engineer' },
      { value: 'support_agent', label: 'Support' },
    ],
  },
  {
    label: 'Access and security',
    options: [
      { value: 'admin_panel_settings', label: 'Administrator' },
      { value: 'shield', label: 'Security' },
      { value: 'verified_user', label: 'Verified user' },
      { value: 'lock', label: 'Restricted access' },
      { value: 'key', label: 'Access key' },
      { value: 'policy', label: 'Policy' },
      { value: 'gavel', label: 'Compliance' },
      { value: 'visibility', label: 'Read only' },
    ],
  },
  {
    label: 'Organization',
    options: [
      { value: 'business', label: 'Business' },
      { value: 'corporate_fare', label: 'Company' },
      { value: 'account_balance', label: 'Institution' },
      { value: 'apartment', label: 'Department' },
      { value: 'store', label: 'Branch' },
      { value: 'public', label: 'Global' },
      { value: 'hub', label: 'Network' },
      { value: 'folder', label: 'Group' },
    ],
  },
  {
    label: 'Work and workflow',
    options: [
      { value: 'work', label: 'Work' },
      { value: 'assignment', label: 'Assignment' },
      { value: 'task_alt', label: 'Tasks' },
      { value: 'approval', label: 'Approval' },
      { value: 'fact_check', label: 'Review' },
      { value: 'dashboard', label: 'Dashboard' },
      { value: 'settings', label: 'Settings' },
      { value: 'tune', label: 'Configuration' },
    ],
  },
];

const ROLE_ICON_LABELS = new Map(
  ROLE_ICON_GROUPS.flatMap((group) => group.options.map((option) => [option.value, option.label])),
);

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
  imports: [ReactiveFormsModule, MatDialogModule, MatIconModule, MatSelectModule],
  templateUrl: './role-editor.dialog.html',
  styleUrl: '../../core/editor-dialog.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class RoleEditorDialog {
  readonly role = inject<Role | null>(MAT_DIALOG_DATA);
  private readonly dialogRef = inject(MatDialogRef<RoleEditorDialog>);
  private readonly fb = inject(FormBuilder);
  readonly iconGroups = ROLE_ICON_GROUPS;

  readonly form = this.fb.nonNullable.group({
    code: [
      { value: this.role?.code ?? '', disabled: Boolean(this.role) },
      [Validators.required, Validators.pattern(/^[a-z][a-z0-9_]{2,63}$/)],
    ],
    name: [this.role?.name ?? '', [Validators.required, Validators.maxLength(128)]],
    category: [this.role?.category ?? 'general', [Validators.required]],
    icon: [this.role?.icon ?? 'badge', [Validators.required]],
    color: [this.role?.color ?? ('neutral' as Role['color'])],
    description: [this.role?.description ?? ''],
    isActive: [this.role?.isActive ?? true],
  });

  iconLabel(icon: string): string {
    return ROLE_ICON_LABELS.get(icon) ?? 'Current icon';
  }

  isKnownIcon(icon: string): boolean {
    return ROLE_ICON_LABELS.has(icon);
  }

  submit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }
    this.dialogRef.close(this.form.getRawValue() satisfies RoleEditorResult);
  }
}
