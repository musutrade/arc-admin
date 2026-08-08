import { HttpClient, HttpParams } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { firstValueFrom } from 'rxjs';
import { ApiAuditLog, ApiPage } from '../api.models';
import { API_BASE_URL } from '../runtime-config';

@Injectable({
  providedIn: 'root',
})
export class AuditLogApiService {
  private readonly http = inject(HttpClient);
  private readonly apiBaseUrl = inject(API_BASE_URL);

  getAuditLogs(
    page: number,
    pageSize: number,
    keyword = '',
    action = '',
  ): Promise<ApiPage<ApiAuditLog>> {
    let params = new HttpParams().set('page', page).set('pageSize', pageSize);
    if (keyword.trim()) {
      params = params.set('keyword', keyword.trim());
    }
    if (action) {
      params = params.set('action', action);
    }
    return firstValueFrom(
      this.http.get<ApiPage<ApiAuditLog>>(`${this.apiBaseUrl}/audit-logs`, { params }),
    );
  }
}
