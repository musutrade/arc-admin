import { provideZonelessChangeDetection } from '@angular/core';
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { AuditLogApiService } from '../../core/api/audit-log-api.service';

import { AuditLogs } from './audit-logs';

describe('AuditLogs', () => {
  let component: AuditLogs;
  let fixture: ComponentFixture<AuditLogs>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [AuditLogs],
      providers: [
        provideZonelessChangeDetection(),
        {
          provide: AuditLogApiService,
          useValue: {
            getAuditLogs: () =>
              Promise.resolve({
                items: [
                  {
                    id: 1,
                    actorUserId: 1,
                    actorUsername: 'admin',
                    action: 'user.roles.update',
                    targetType: 'user',
                    targetId: 7,
                    details: { roleIds: [2] },
                    createdAt: '2026-08-08T00:00:00Z',
                  },
                ],
                total: 1,
                page: 1,
                pageSize: 20,
              }),
          },
        },
      ],
    }).compileComponents();

    fixture = TestBed.createComponent(AuditLogs);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });

  it('renders localized audit event information', () => {
    expect(fixture.nativeElement.textContent).toContain('变更用户角色');
    expect(fixture.nativeElement.textContent).toContain('admin');
    expect(fixture.nativeElement.textContent).toContain('用户 #7');
  });
});
