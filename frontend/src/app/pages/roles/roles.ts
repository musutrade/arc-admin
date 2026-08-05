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
import { MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';
import { firstValueFrom } from 'rxjs';
import { apiErrorMessage } from '../../core/api-error';
import { AuthService } from '../../core/auth.service';
import { ConfirmDialog } from '../../core/confirm.dialog';
import { DataService } from '../../core/data.service';
import { Role, RolePermissionRow } from '../../core/models';
import { AssignPermissionsDialog } from '../role-permissions/assign-permissions.dialog';
import { RoleEditorDialog, RoleEditorResult } from './role-editor.dialog';

@Component({
  selector: 'app-roles',
  imports: [MatIconModule, MatProgressSpinnerModule, MatDialogModule, MatSnackBarModule],
  templateUrl: './roles.html',
  styleUrl: './roles.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class RolesPage implements OnInit {
  readonly roles = signal<Role[]>([]);
  readonly loading = signal(true);
  readonly error = signal<string | null>(null);
  readonly view = signal<'grid' | 'list'>('grid');
  readonly busy = signal(false);

  private readonly data = inject(DataService);
  private readonly auth = inject(AuthService);
  private readonly dialog = inject(MatDialog);
  private readonly snackBar = inject(MatSnackBar);

  readonly canWrite = computed(() => this.auth.hasPermission('role:write'));
  readonly canAssign = computed(() => this.auth.hasPermission('role:permissions:write'));

  ngOnInit(): void {
    void this.loadRoles();
  }

  private async loadRoles(): Promise<void> {
    this.loading.set(true);
    this.error.set(null);
    try {
      this.roles.set(await this.data.getRoles());
    } catch (error) {
      this.error.set(apiErrorMessage(error, '角色数据加载失败，请稍后重试'));
    } finally {
      this.loading.set(false);
    }
  }

  setView(v: 'grid' | 'list'): void {
    this.view.set(v);
  }

  async onCreateRole(): Promise<void> {
    await this.openRoleEditor(null);
  }

  async onEditRole(role: Role): Promise<void> {
    await this.openRoleEditor(role);
  }

  async onToggleRoleStatus(role: Role): Promise<void> {
    const activating = !role.isActive;
    const action = activating ? '启用' : '停用';
    const message = activating
      ? role.members > 0
        ? `确定启用 ${role.name} 吗？启用后，已绑定的 ${role.members} 名成员将重新获得该角色授予的权限。`
        : `确定启用 ${role.name} 吗？该角色当前没有成员，启用后可以重新授予权限。`
      : `确定停用 ${role.name} 吗？该角色已分配给 ${role.members} 名成员。停用后，成员将立即失去该角色授予的权限，但绑定会保留。如果这是你当前使用的角色，你可能立即失去后续管理权限。`;
    const confirmed: boolean | undefined = await firstValueFrom(
      this.dialog
        .open(ConfirmDialog, {
          data: {
            title: `${action}角色`,
            message,
            confirmLabel: `${action}角色`,
            danger: !activating,
          },
        })
        .afterClosed(),
    );
    if (!confirmed) {
      return;
    }
    await this.runMutation(
      () => this.data.updateRole(role.id, { isActive: activating }),
      `已${action} ${role.name}`,
    );
  }

  async onDeleteRole(role: Role): Promise<void> {
    const confirmed: boolean | undefined = await firstValueFrom(
      this.dialog
        .open(ConfirmDialog, {
          data: {
            title: '删除角色',
            message: `确定删除 ${role.name} 吗？请先迁移已分配的用户。`,
            confirmLabel: '删除角色',
            danger: true,
          },
        })
        .afterClosed(),
    );
    if (!confirmed) {
      return;
    }
    await this.runMutation(() => this.data.deleteRole(role.id), `已删除 ${role.name}`);
  }

  async onEditPermissions(role: Role): Promise<void> {
    try {
      const [groups, assigned] = await Promise.all([
        this.data.getPermissionGroups(),
        this.data.getAssignedPermissionIds(role.id),
      ]);
      const row: RolePermissionRow = {
        roleId: role.id,
        roleCode: role.code,
        roleName: role.name,
        usersAssigned: role.members,
        active: role.isActive,
        groupIds: role.permissionGroupIds,
      };
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
      if (permissionIds) {
        await this.runMutation(
          () => this.data.assignRolePermissions(role.id, permissionIds),
          `已更新 ${role.name} 的权限`,
        );
      }
    } catch (error) {
      this.showError(error, '权限数据加载失败');
    }
  }

  private async openRoleEditor(role: Role | null): Promise<void> {
    const result: RoleEditorResult | undefined = await firstValueFrom(
      this.dialog.open(RoleEditorDialog, { data: role }).afterClosed(),
    );
    if (!result) {
      return;
    }
    if (role) {
      await this.runMutation(
        () =>
          this.data.updateRole(role.id, {
            name: result.name,
            category: result.category,
            icon: result.icon || null,
            color: result.color,
            description: result.description || null,
            isActive: result.isActive,
          }),
        `已更新 ${result.name}`,
      );
    } else {
      await this.runMutation(
        () =>
          this.data.createRole({
            code: result.code,
            name: result.name,
            category: result.category,
            icon: result.icon || null,
            color: result.color,
            description: result.description || null,
          }),
        `已创建 ${result.name}`,
      );
    }
  }

  private async runMutation(action: () => Promise<unknown>, success: string): Promise<void> {
    this.busy.set(true);
    try {
      await action();
      this.snackBar.open(success, '关闭', { duration: 3000 });
      await this.loadRoles();
    } catch (error) {
      this.showError(error, '操作失败，请稍后重试');
    } finally {
      this.busy.set(false);
    }
  }

  private showError(error: unknown, fallback: string): void {
    this.snackBar.open(apiErrorMessage(error, fallback), '关闭', { duration: 5000 });
  }
}
