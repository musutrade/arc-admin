import {
  ChangeDetectionStrategy,
  Component,
  OnInit,
  computed,
  inject,
  signal,
} from '@angular/core';
import { DatePipe } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';
import { firstValueFrom } from 'rxjs';
import { DataService } from '../../core/data.service';
import { Role, StatCard, User, UserStatus } from '../../core/models';
import { AuthService } from '../../core/auth.service';
import { apiErrorMessage } from '../../core/api-error';
import { ConfirmDialog } from '../../core/confirm.dialog';
import { PasswordDialog } from './password.dialog';
import { RoleSelectionDialog } from './role-selection.dialog';
import { UserEditorDialog, UserEditorResult } from './user-editor.dialog';

const STATUS_META: Record<UserStatus, { cls: string; label: string }> = {
  active: { cls: 'st-active', label: '启用' },
  inactive: { cls: 'st-inactive', label: '停用' },
  suspended: { cls: 'st-suspended', label: '已暂停' },
};

@Component({
  selector: 'app-users',
  imports: [
    MatIconModule,
    MatCheckboxModule,
    MatProgressSpinnerModule,
    MatDialogModule,
    MatSnackBarModule,
    DatePipe,
  ],
  templateUrl: './users.html',
  styleUrl: './users.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class UsersPage implements OnInit {
  readonly users = signal<User[]>([]);
  readonly stats = signal<StatCard[]>([]);
  readonly loading = signal(true);
  readonly error = signal<string | null>(null);
  readonly search = signal('');
  readonly roleFilter = signal('all');
  readonly statusFilter = signal('all');
  readonly selected = signal<Set<string>>(new Set());
  readonly page = signal(1);
  readonly pageSize = 10;
  readonly busy = signal(false);

  private readonly data = inject(DataService);
  private readonly auth = inject(AuthService);
  private readonly dialog = inject(MatDialog);
  private readonly snackBar = inject(MatSnackBar);

  readonly canWrite = computed(() => this.auth.hasPermission('user:write'));
  readonly canResetPassword = computed(() => this.auth.hasPermission('user:admin:reset_password'));
  readonly canDeactivate = computed(() => this.auth.hasPermission('user:admin:deactivate'));
  readonly canManageStatus = computed(() => this.canWrite() && this.canDeactivate());
  readonly canManageRoles = computed(() => this.auth.hasPermission('user:roles:write'));
  readonly canGrantSuperAdmin = computed(() => this.auth.hasPermission('user:super_admin:grant'));

  /** 状态展示元数据暴露给模板 */
  readonly statusMeta = STATUS_META;

  /** 角色筛选选项(从数据提取) */
  readonly roleOptions = computed<string[]>(() => {
    const set = new Set<string>();
    this.users().forEach((u) => u.roles.forEach((r) => set.add(r)));
    return ['all', ...set];
  });

  readonly filteredUsers = computed<User[]>(() => {
    const term = this.search().trim().toLowerCase();
    const role = this.roleFilter();
    const status = this.statusFilter();
    return this.users().filter((u) => {
      const matchTerm =
        !term || u.name.toLowerCase().includes(term) || u.email.toLowerCase().includes(term);
      const matchRole = role === 'all' || u.roles.includes(role);
      const matchStatus = status === 'all' || u.status === status;
      return matchTerm && matchRole && matchStatus;
    });
  });

  /** 总页数 */
  readonly totalPages = computed(() =>
    Math.max(1, Math.ceil(this.filteredUsers().length / this.pageSize)),
  );

  /** 当前页数据 */
  readonly pagedUsers = computed(() => {
    const start = (this.page() - 1) * this.pageSize;
    return this.filteredUsers().slice(start, start + this.pageSize);
  });

  readonly selectedCount = computed(() => this.selected().size);

  readonly allChecked = computed(() => {
    const rows = this.filteredUsers();
    return rows.length > 0 && rows.every((u) => this.selected().has(u.id));
  });

  readonly someChecked = computed(() => {
    const rows = this.filteredUsers();
    return rows.some((u) => this.selected().has(u.id)) && !this.allChecked();
  });

  /** 当前页首条序号(1-based) */
  readonly pageStart = computed(() => (this.page() - 1) * this.pageSize + 1);

  /** 当前页末条序号 */
  readonly pageEnd = computed(() =>
    Math.min(this.page() * this.pageSize, this.filteredUsers().length),
  );

  /** 页码数组(少于 7 页全部展示,否则首尾+省略) */
  readonly pageNumbers = computed<number[]>(() => {
    const total = this.totalPages();
    if (total <= 7) {
      return Array.from({ length: total }, (_, i) => i + 1);
    }
    const cur = this.page();
    const pages = new Set<number>([1, total, cur - 1, cur, cur + 1]);
    return [...pages].filter((p) => p >= 1 && p <= total).sort((a, b) => a - b);
  });

  ngOnInit(): void {
    void this.loadData();
  }

  private async loadData(): Promise<void> {
    this.loading.set(true);
    this.error.set(null);
    try {
      const statsRequest = this.auth.hasPermission('dashboard:analytics:read')
        ? this.data.getUserStats()
        : Promise.resolve([]);
      const [users, stats] = await Promise.all([this.data.getUsers(), statsRequest]);
      this.users.set(users);
      this.stats.set(stats);
      this.selected.set(new Set());
      this.goToPage(this.page());
    } catch (error) {
      this.error.set(apiErrorMessage(error, '用户数据加载失败，请稍后重试'));
    } finally {
      this.loading.set(false);
    }
  }

  searchUsers(value: string): void {
    this.search.set(value);
    this.page.set(1);
  }

  applyRoleFilter(value: string): void {
    this.roleFilter.set(value);
    this.page.set(1);
  }

  applyStatusFilter(value: string): void {
    this.statusFilter.set(value);
    this.page.set(1);
  }

  resetFilters(): void {
    this.search.set('');
    this.roleFilter.set('all');
    this.statusFilter.set('all');
    this.page.set(1);
  }

  goToPage(p: number): void {
    this.page.set(Math.min(Math.max(1, p), this.totalPages()));
  }

  toggleRow(id: string, checked: boolean): void {
    this.selected.update((s) => {
      const next = new Set(s);
      if (checked) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });
  }

  toggleAll(checked: boolean): void {
    this.selected.update((s) => {
      const next = new Set(s);
      this.filteredUsers().forEach((u) => {
        if (checked) {
          next.add(u.id);
        } else {
          next.delete(u.id);
        }
      });
      return next;
    });
  }

  async onCreate(): Promise<void> {
    await this.openEditor();
  }

  async onEdit(user: User): Promise<void> {
    await this.openEditor(user);
  }

  async onResetPassword(user: User): Promise<void> {
    const password: string | undefined = await firstValueFrom(
      this.dialog.open(PasswordDialog, { data: user.username }).afterClosed(),
    );
    if (!password) {
      return;
    }
    await this.runMutation(
      () => this.data.updateUser(user.id, { password }),
      `已重置 ${user.name} 的密码`,
    );
  }

  async onToggleStatus(user: User): Promise<void> {
    const activating = user.status !== 'active';
    const action = activating ? '启用' : '停用';
    const confirmed = await this.confirm(
      `${action}用户`,
      activating
        ? `确定重新启用 ${user.name} 吗？启用后该账号可以正常登录。`
        : `确定停用 ${user.name} 吗？停用后该账号将无法登录。`,
      `${action}用户`,
      !activating,
    );
    if (!confirmed) {
      return;
    }
    await this.runMutation(
      () => this.data.updateUser(user.id, { status: activating ? 'active' : 'inactive' }),
      `已${action} ${user.name}`,
    );
  }

  isCurrentUser(user: User): boolean {
    return String(this.auth.currentUser()?.id ?? '') === user.id;
  }

  async onDelete(user: User): Promise<void> {
    const confirmed = await this.confirm(
      '删除用户',
      `确定删除 ${user.name} 吗？删除后该账号将立即停用。`,
      '删除用户',
    );
    if (!confirmed) {
      return;
    }
    await this.runMutation(() => this.data.deleteUser(user.id), `已删除 ${user.name}`);
  }

  async deleteSelected(): Promise<void> {
    const ids = [...this.selected()];
    if (
      ids.length === 0 ||
      !(await this.confirm('删除所选用户', `确定删除选中的 ${ids.length} 个账号吗？`, '删除用户'))
    ) {
      return;
    }
    await this.runMutation(
      () => Promise.all(ids.map((id) => this.data.deleteUser(id))).then(() => undefined),
      `已删除 ${ids.length} 个用户`,
    );
  }

  async changeSelectedRoles(): Promise<void> {
    const ids = [...this.selected()];
    if (ids.length === 0) {
      return;
    }
    try {
      const roles = this.filterGrantableRoles(await this.data.getRoles());
      const roleIds: string[] | undefined = await firstValueFrom(
        this.dialog.open(RoleSelectionDialog, { data: roles }).afterClosed(),
      );
      if (!roleIds) {
        return;
      }
      await this.runMutation(
        () =>
          Promise.all(ids.map((id) => this.data.assignUserRoles(id, roleIds))).then(
            () => undefined,
          ),
        `已更新 ${ids.length} 个用户的角色`,
      );
    } catch (error) {
      this.showError(error, '角色数据加载失败');
    }
  }

  private async openEditor(user?: User): Promise<void> {
    try {
      const canManageRoles = this.canManageRoles();
      const canManageStatus = this.canManageStatus();
      const canResetPassword = this.canResetPassword();
      const roles = canManageRoles ? this.filterGrantableRoles(await this.data.getRoles()) : [];
      const result: UserEditorResult | undefined = await firstValueFrom(
        this.dialog
          .open(UserEditorDialog, {
            data: { user, roles, canManageRoles, canManageStatus, canResetPassword },
          })
          .afterClosed(),
      );
      if (!result) {
        return;
      }
      if (user) {
        await this.runMutation(async () => {
          await this.data.updateUser(user.id, {
            displayName: result.displayName,
            email: result.email || null,
            ...(canManageStatus ? { status: result.status } : {}),
            ...(canResetPassword && result.password ? { password: result.password } : {}),
          });
          if (canManageRoles) {
            await this.data.assignUserRoles(user.id, result.roleIds);
          }
        }, `已更新 ${result.displayName}`);
      } else {
        await this.runMutation(
          () =>
            this.data.createUser({
              username: result.username,
              password: result.password,
              displayName: result.displayName,
              email: result.email || null,
              ...(canManageStatus ? { status: result.status } : {}),
              ...(canManageRoles ? { roleIds: result.roleIds.map(Number) } : {}),
            }),
          `已创建 ${result.displayName}`,
        );
      }
    } catch (error) {
      this.showError(error, '用户编辑器加载失败');
    }
  }

  private async runMutation(action: () => Promise<unknown>, success: string): Promise<void> {
    this.busy.set(true);
    try {
      await action();
      await this.auth.refreshSession();
      this.snackBar.open(success, '关闭', { duration: 3000 });
      await this.loadData();
    } catch (error) {
      this.showError(error, '操作失败，请稍后重试');
    } finally {
      this.busy.set(false);
    }
  }

  private filterGrantableRoles(roles: Role[]): Role[] {
    return this.canGrantSuperAdmin() ? roles : roles.filter((role) => role.code !== 'super_admin');
  }

  private confirm(
    title: string,
    message: string,
    confirmLabel: string,
    danger = true,
  ): Promise<boolean> {
    return firstValueFrom(
      this.dialog
        .open(ConfirmDialog, {
          data: { title, message, confirmLabel, danger },
        })
        .afterClosed(),
    ).then(Boolean);
  }

  private showError(error: unknown, fallback: string): void {
    this.snackBar.open(apiErrorMessage(error, fallback), '关闭', { duration: 5000 });
  }

  initials(name: string): string {
    return name
      .split(' ')
      .map((n) => n[0])
      .slice(0, 2)
      .join('')
      .toUpperCase();
  }
}
