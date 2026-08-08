import { ChangeDetectionStrategy, Component, DestroyRef, inject, signal } from '@angular/core';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { NavigationEnd, Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { filter } from 'rxjs';
import { MatIconModule } from '@angular/material/icon';
import { MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatMenuModule } from '@angular/material/menu';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';
import { ThemeService } from '../core/theme.service';
import { AuthService } from '../core/auth.service';
import { ChangePasswordDialog } from '../core/change-password.dialog';
import { APP_CONFIG } from '../core/runtime-config';

@Component({
  selector: 'app-layout',
  imports: [
    RouterOutlet,
    RouterLink,
    RouterLinkActive,
    MatIconModule,
    MatDialogModule,
    MatMenuModule,
    MatSnackBarModule,
  ],
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
  private readonly dialog = inject(MatDialog);
  private readonly snackBar = inject(MatSnackBar);
  readonly auth = inject(AuthService);
  readonly appConfig = inject(APP_CONFIG);
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

  canAll(permissions: readonly string[]): boolean {
    return this.auth.hasAllPermissions(permissions);
  }

  openChangePassword(): void {
    this.dialog
      .open(ChangePasswordDialog)
      .afterClosed()
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe((changed: boolean | undefined) => {
        if (changed) {
          this.snackBar.open('密码修改成功，请重新登录', '关闭', { duration: 3000 });
          void this.router.navigate(['/login']);
        }
      });
  }

  logout(): void {
    this.auth.logout();
    this.router.navigate(['/login']);
  }
}
