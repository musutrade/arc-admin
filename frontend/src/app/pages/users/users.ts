import {
  ChangeDetectionStrategy,
  Component,
  OnInit,
  computed,
  inject,
  signal,
} from '@angular/core';
import { MatIconModule } from '@angular/material/icon';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { DataService } from '../../core/data.service';
import { StatCard, User, UserStatus } from '../../core/models';

const STATUS_META: Record<UserStatus, { cls: string; label: string }> = {
  active: { cls: 'st-active', label: 'Active' },
  inactive: { cls: 'st-inactive', label: 'Inactive' },
  suspended: { cls: 'st-suspended', label: 'Suspended' },
};

@Component({
  selector: 'app-users',
  imports: [MatIconModule, MatCheckboxModule, MatProgressSpinnerModule],
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

  private readonly data = inject(DataService);

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
    this.loadUsers();
    this.loadStats();
  }

  private loadUsers(): void {
    this.data
      .getUsers()
      .then((users) => this.users.set(users))
      .catch(() => this.error.set('用户数据加载失败,请稍后重试'));
  }

  private loadStats(): void {
    this.data
      .getUserStats()
      .then((stats) => {
        this.stats.set(stats);
        this.loading.set(false);
      })
      .catch(() => {
        this.error.set('统计数据加载失败,请稍后重试');
        this.loading.set(false);
      });
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

  onEdit(u: User): void {
    console.log('edit user', u.id);
  }

  onResetPassword(u: User): void {
    console.log('reset password', u.id);
  }

  onMore(u: User): void {
    console.log('more actions', u.id);
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
