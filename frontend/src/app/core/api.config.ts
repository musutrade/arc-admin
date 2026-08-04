import { InjectionToken } from '@angular/core';

declare global {
  interface Window {
    __ARC_ADMIN_CONFIG__?: {
      apiBaseUrl?: string;
    };
  }
}

export const API_BASE_URL = new InjectionToken<string>('API_BASE_URL', {
  providedIn: 'root',
  factory: () => (window.__ARC_ADMIN_CONFIG__?.apiBaseUrl ?? '/api/v1').replace(/\/$/, ''),
});
