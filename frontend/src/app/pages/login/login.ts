import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { Router } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MOCK_CREDENTIALS } from '../../core/mock-data';

@Component({
  selector: 'app-login',
  imports: [ReactiveFormsModule, MatIconModule, MatCheckboxModule, MatProgressSpinnerModule],
  templateUrl: './login.html',
  styleUrl: './login.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class LoginPage {
  readonly hidePassword = signal(true);
  readonly loading = signal(false);
  readonly error = signal<string | null>(null);

  private readonly fb = inject(FormBuilder);
  private readonly router = inject(Router);

  readonly form = this.fb.nonNullable.group({
    username: ['', [Validators.required]],
    password: ['', [Validators.required]],
    remember: [true],
  });

  togglePassword(): void {
    this.hidePassword.update((v) => !v);
  }

  onSubmit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }
    this.loading.set(true);
    this.error.set(null);

    const { username, password, remember } = this.form.getRawValue();

    // Mock 登录校验
    setTimeout(() => {
      if (
        username === MOCK_CREDENTIALS.username &&
        password === MOCK_CREDENTIALS.password
      ) {
        // 勾选"记住我"时持久化到 localStorage,否则仅本次会话有效
        const storage = remember ? localStorage : sessionStorage;
        storage.setItem('arc-auth', 'mock-token');
        this.router.navigate(['/']);
      } else {
        this.error.set('用户名或密码错误,请使用 admin / admin123');
        this.loading.set(false);
      }
    }, 900);
  }
}
