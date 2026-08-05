import {
  ChangeDetectionStrategy,
  Component,
  OnInit,
  computed,
  inject,
  signal,
} from '@angular/core';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatDialog } from '@angular/material/dialog';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';
import { firstValueFrom } from 'rxjs';
import { apiErrorMessage } from '../../core/api-error';
import { AuthService } from '../../core/auth.service';
import { DataService } from '../../core/data.service';
import { RolePermissionRow } from '../../core/models';
import { AssignPermissionsDialog } from './assign-permissions.dialog';
import { RoleEditorDialog, RoleEditorResult } from '../roles/role-editor.dialog';

@Component({
  selector: 'app-role-permissions',
  imports: [MatIconModule, MatProgressSpinnerModule, MatSnackBarModule],
  templateUrl: './role-permissions.html',
  styleUrl: './role-permissions.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class RolePermissionsPage implements OnInit {
  readonly rows = signal<RolePermissionRow[]>([]);
  readonly loading = signal(true);
  readonly error = signal<string | null>(null);
  readonly busy = signal(false);
  private readonly data = inject(DataService);
  private readonly dialog = inject(MatDialog);
  private readonly auth = inject(AuthService);
  private readonly snackBar = inject(MatSnackBar);

  readonly canCreateRole = computed(() => this.auth.hasPermission('role:write'));

  ngOnInit(): void {
    void this.loadRows();
  }

  private async loadRows(): Promise<void> {
    this.loading.set(true);
    this.error.set(null);
    try {
      this.rows.set(await this.data.getRolePermissionRows());
    } catch (error) {
      this.error.set(apiErrorMessage(error, '角色权限数据加载失败,请稍后重试'));
    } finally {
      this.loading.set(false);
    }
  }

  async onEditPermissions(row: RolePermissionRow): Promise<void> {
    try {
      const [groups, assigned] = await Promise.all([
        this.data.getPermissionGroups(),
        this.data.getAssignedPermissionIds(row.roleId),
      ]);
      const permissionIds: string[] | undefined = await firstValueFrom(
        this.dialog
          .open(AssignPermissionsDialog, {
            width: '960px',
            maxWidth: 'calc(100vw - 32px)',
            maxHeight: 'calc(100dvh - 32px)',
            panelClass: 'assign-dialog-panel',
            data: { role: row, groups, assigned },
          })
          .afterClosed(),
      );
      if (!permissionIds) {
        return;
      }
      this.busy.set(true);
      await this.data.assignRolePermissions(row.roleId, permissionIds);
      this.snackBar.open(`${row.roleName} permissions updated`, 'Close', { duration: 3000 });
      await this.loadRows();
    } catch (error) {
      this.snackBar.open(apiErrorMessage(error, '权限保存失败'), 'Close', { duration: 5000 });
    } finally {
      this.busy.set(false);
    }
  }

  async onCreateRole(): Promise<void> {
    const result: RoleEditorResult | undefined = await firstValueFrom(
      this.dialog.open(RoleEditorDialog, { data: null }).afterClosed(),
    );
    if (!result) {
      return;
    }
    try {
      this.busy.set(true);
      await this.data.createRole({
        code: result.code,
        name: result.name,
        category: result.category,
        icon: result.icon || null,
        color: result.color,
        description: result.description || null,
      });
      this.snackBar.open(`${result.name} created`, 'Close', { duration: 3000 });
      await this.loadRows();
    } catch (error) {
      this.snackBar.open(apiErrorMessage(error, '角色创建失败'), 'Close', { duration: 5000 });
    } finally {
      this.busy.set(false);
    }
  }
}
