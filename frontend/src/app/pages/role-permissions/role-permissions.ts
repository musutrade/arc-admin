import { ChangeDetectionStrategy, Component, OnInit, inject, signal } from '@angular/core';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatDialog } from '@angular/material/dialog';
import { DataService } from '../../core/data.service';
import { RolePermissionRow } from '../../core/models';
import { AssignPermissionsDialog } from './assign-permissions.dialog';

@Component({
  selector: 'app-role-permissions',
  imports: [MatIconModule, MatProgressSpinnerModule],
  templateUrl: './role-permissions.html',
  styleUrl: './role-permissions.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class RolePermissionsPage implements OnInit {
  readonly rows = signal<RolePermissionRow[]>([]);
  readonly loading = signal(true);
  readonly error = signal<string | null>(null);
  private readonly data = inject(DataService);
  private readonly dialog = inject(MatDialog);

  ngOnInit(): void {
    this.loadRows();
  }

  private loadRows(): void {
    this.data
      .getRolePermissionRows()
      .then((rows) => {
        this.rows.set(rows);
        this.loading.set(false);
      })
      .catch(() => {
        this.error.set('角色权限数据加载失败,请稍后重试');
        this.loading.set(false);
      });
  }

  async onEditPermissions(row: RolePermissionRow): Promise<void> {
    const groups = await this.data.getPermissionGroups();
    const assigned = await this.data.getAssignedPermissionIds(row.roleId);
    this.dialog.open(AssignPermissionsDialog, {
      width: '720px',
      maxWidth: '94vw',
      panelClass: 'assign-dialog-panel',
      data: { role: row, groups, assigned },
    });
  }

  onCreateRole(): void {
    console.log('add new role');
  }
}
