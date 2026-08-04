import { ChangeDetectionStrategy, Component, DestroyRef, inject, signal } from '@angular/core';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { NavigationEnd, Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { filter } from 'rxjs';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { ThemeService } from '../core/theme.service';
import { AuthService } from '../core/auth.service';

@Component({
  selector: 'app-layout',
  imports: [RouterOutlet, RouterLink, RouterLinkActive, MatIconModule, MatMenuModule],
  templateUrl: './layout.html',
  styleUrl: './layout.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class LayoutComponent {
  readonly collapsed = signal(false);
  readonly sidebarOpen = signal(false);
  /** User Management 子菜单展开状态 */
  readonly userSubOpen = signal(false);
  /** 当前是否处于 /users 相关路由(高亮 User Management 父项) */
  readonly usersActive = signal(false);
  private readonly theme = inject(ThemeService);
  private readonly router = inject(Router);
  private readonly destroyRef = inject(DestroyRef);
  readonly auth = inject(AuthService);
  readonly isDark = this.theme.isDark;

  constructor() {
    const onUsersRoute = () => this.router.url.startsWith('/users');
    this.usersActive.set(onUsersRoute());
    this.userSubOpen.set(onUsersRoute());
    this.router.events
      .pipe(
        filter((event): event is NavigationEnd => event instanceof NavigationEnd),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe((event) => {
        const isUsers = event.url.startsWith('/users');
        this.usersActive.set(isUsers);
        if (isUsers) {
          this.userSubOpen.set(true);
        }
      });
  }

  toggleTheme(): void {
    this.theme.toggle();
  }

  toggleSidebar(): void {
    this.collapsed.update((v) => !v);
  }

  toggleUserMenu(): void {
    this.userSubOpen.update((v) => !v);
  }

  closeUserMenu(): void {
    this.userSubOpen.set(false);
  }

  can(permission: string): boolean {
    return this.auth.hasPermission(permission);
  }

  logout(): void {
    this.auth.logout();
    this.router.navigate(['/login']);
  }
}
