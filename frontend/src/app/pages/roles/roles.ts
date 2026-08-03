import { ChangeDetectionStrategy, Component, OnInit, inject, signal } from '@angular/core';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { DataService } from '../../core/data.service';
import { Role } from '../../core/models';

@Component({
  selector: 'app-roles',
  imports: [MatIconModule, MatProgressSpinnerModule],
  templateUrl: './roles.html',
  styleUrl: './roles.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class RolesPage implements OnInit {
  readonly roles = signal<Role[]>([]);
  readonly loading = signal(true);
  readonly error = signal<string | null>(null);
  readonly view = signal<'grid' | 'list'>('grid');

  private readonly data = inject(DataService);

  ngOnInit(): void {
    this.loadRoles();
  }

  private loadRoles(): void {
    this.data
      .getRoles()
      .then((roles) => {
        this.roles.set(roles);
        this.loading.set(false);
      })
      .catch(() => {
        this.error.set('角色数据加载失败,请稍后重试');
        this.loading.set(false);
      });
  }

  setView(v: 'grid' | 'list'): void {
    this.view.set(v);
  }

  onCreateRole(): void {
    console.log('create role');
  }

  onEditPermissions(r: Role): void {
    console.log('edit permissions for', r.name);
  }
}
