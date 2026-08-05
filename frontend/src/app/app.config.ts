import {
  ApplicationConfig,
  LOCALE_ID,
  provideBrowserGlobalErrorListeners,
  provideZonelessChangeDetection,
} from '@angular/core';
import { registerLocaleData } from '@angular/common';
import localeZh from '@angular/common/locales/zh';
import { provideRouter } from '@angular/router';
import { provideAnimationsAsync } from '@angular/platform-browser/animations/async';
import { provideHttpClient, withInterceptors } from '@angular/common/http';
import { MAT_DIALOG_DEFAULT_OPTIONS, MatDialogConfig } from '@angular/material/dialog';
import { MAT_SNACK_BAR_DEFAULT_OPTIONS, MatSnackBarConfig } from '@angular/material/snack-bar';

import { routes } from './app.routes';
import { authInterceptor } from './core/auth.interceptor';

registerLocaleData(localeZh);

function dialogDefaults(): MatDialogConfig {
  return Object.assign(new MatDialogConfig(), {
    maxWidth: 'calc(100vw - 32px)',
    maxHeight: 'calc(100dvh - 32px)',
  });
}

function snackBarDefaults(): MatSnackBarConfig {
  return Object.assign(new MatSnackBarConfig(), {
    horizontalPosition: 'center',
    verticalPosition: 'top',
    panelClass: ['app-snackbar'],
  });
}

export const appConfig: ApplicationConfig = {
  providers: [
    provideZonelessChangeDetection(),
    provideBrowserGlobalErrorListeners(),
    provideAnimationsAsync(),
    provideHttpClient(withInterceptors([authInterceptor])),
    provideRouter(routes),
    { provide: LOCALE_ID, useValue: 'zh-CN' },
    {
      provide: MAT_DIALOG_DEFAULT_OPTIONS,
      useFactory: dialogDefaults,
    },
    {
      provide: MAT_SNACK_BAR_DEFAULT_OPTIONS,
      useFactory: snackBarDefaults,
    },
  ],
};
