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
import { DataService } from '../../core/data.service';
import { Permission, PermissionGroup, PermissionType } from '../../core/models';

const TYPE_META: Record<PermissionType, { icon: string; cls: string }> = {
  menu: { icon: 'table_chart', cls: 'badge-menu' },
  button: { icon: 'smart_button', cls: 'badge-button' },
  api: { icon: 'api', cls: 'badge-api' },
};

interface FilteredGroup extends PermissionGroup {
  collapsed: boolean;
}

@Component({
  selector: 'app-permissions',
  imports: [MatIconModule, MatProgressSpinnerModule],
  templateUrl: './permissions.html',
  styleUrl: './permissions.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class PermissionsPage implements OnInit {
  readonly groups = signal<PermissionGroup[]>([]);
  readonly loading = signal(true);
  readonly error = signal<string | null>(null);
  readonly search = signal('');
  readonly typeFilter = signal<'all' | PermissionType>('all');
  readonly collapsed = signal<Set<string>>(new Set());

  /** 类型元数据暴露给模板 */
  readonly typeMeta = TYPE_META;

  readonly typeOptions: ('all' | PermissionType)[] = ['all', 'menu', 'button', 'api'];

  private readonly data = inject(DataService);

  readonly filteredGroups = computed<FilteredGroup[]>(() => {
    const term = this.search().trim().toLowerCase();
    const type = this.typeFilter();
    const collapsedIds = this.collapsed();
    return this.groups()
      .map((g) => ({
        ...g,
        collapsed: collapsedIds.has(g.id),
        permissions: g.permissions.filter((p) => {
          const matchType = type === 'all' || p.type === type;
          const matchTerm =
            !term ||
            p.name.toLowerCase().includes(term) ||
            p.code.toLowerCase().includes(term) ||
            p.description.toLowerCase().includes(term);
          return matchType && matchTerm;
        }),
      }))
      .filter((g) => g.permissions.length > 0);
  });

  /** 当前筛选结果中的权限总数 */
  readonly totalCount = computed(() =>
    this.filteredGroups().reduce((acc, g) => acc + g.permissions.length, 0),
  );

  ngOnInit(): void {
    this.loadGroups();
  }

  private loadGroups(): void {
    this.data
      .getPermissionGroups()
      .then((groups) => {
        this.groups.set(groups);
        this.loading.set(false);
      })
      .catch(() => {
        this.error.set('权限数据加载失败,请稍后重试');
        this.loading.set(false);
      });
  }

  toggleGroup(id: string): void {
    this.collapsed.update((s) => {
      const next = new Set(s);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  searchPermissions(value: string): void {
    this.search.set(value);
  }

  applyTypeFilter(value: string): void {
    this.typeFilter.set(value as 'all' | PermissionType);
  }

  onEdit(p: Permission): void {
    console.log('edit', p.code);
  }

  onDelete(p: Permission): void {
    console.log('delete', p.code);
  }
}
