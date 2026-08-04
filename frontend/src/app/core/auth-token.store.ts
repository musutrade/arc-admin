import { Injectable, signal } from '@angular/core';

const TOKEN_KEY = 'arc-auth';

@Injectable({ providedIn: 'root' })
export class AuthTokenStore {
  private readonly value = signal<string | null>(this.read());
  readonly token = this.value.asReadonly();

  set(token: string, remember: boolean): void {
    localStorage.removeItem(TOKEN_KEY);
    sessionStorage.removeItem(TOKEN_KEY);
    (remember ? localStorage : sessionStorage).setItem(TOKEN_KEY, token);
    this.value.set(token);
  }

  clear(): void {
    localStorage.removeItem(TOKEN_KEY);
    sessionStorage.removeItem(TOKEN_KEY);
    this.value.set(null);
  }

  private read(): string | null {
    if (typeof localStorage === 'undefined' || typeof sessionStorage === 'undefined') {
      return null;
    }
    return localStorage.getItem(TOKEN_KEY) ?? sessionStorage.getItem(TOKEN_KEY);
  }
}
