import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { firstValueFrom } from 'rxjs';
import { DashboardStats } from '../api.models';
import { StatCard } from '../models';
import { API_BASE_URL } from '../runtime-config';

@Injectable({
  providedIn: 'root',
})
export class DashboardApiService {
  private readonly http = inject(HttpClient);
  private readonly apiBaseUrl = inject(API_BASE_URL);

  async getUserStats(): Promise<StatCard[]> {
    const stats = await firstValueFrom(
      this.http.get<DashboardStats>(`${this.apiBaseUrl}/dashboard/stats`),
    );
    return [
      { label: '用户总数', value: String(stats.totalUsers), icon: 'group' },
      { label: '启用用户', value: String(stats.activeUsers), icon: 'verified_user' },
      { label: '角色总数', value: String(stats.totalRoles), icon: 'badge' },
      { label: '已暂停用户', value: String(stats.suspendedUsers), icon: 'person_off' },
    ];
  }
}
