import { Injectable, signal } from '@angular/core';

const STORAGE_KEY = 'arc-theme';

/** 主题服务:light / dark 手动切换,持久化到 localStorage */
@Injectable({ providedIn: 'root' })
export class ThemeService {
  readonly isDark = signal(false);

  constructor() {
    const saved = typeof localStorage !== 'undefined' ? localStorage.getItem(STORAGE_KEY) : null;
    this.isDark.set(saved === 'dark');
    this.apply();
  }

  toggle(): void {
    this.isDark.update((v) => !v);
    this.apply();
  }

  setDark(dark: boolean): void {
    this.isDark.set(dark);
    this.apply();
  }

  private apply(): void {
    const root = document.documentElement;
    if (this.isDark()) {
      root.setAttribute('data-theme', 'dark');
      localStorage.setItem(STORAGE_KEY, 'dark');
    } else {
      root.removeAttribute('data-theme');
      localStorage.setItem(STORAGE_KEY, 'light');
    }
  }
}
