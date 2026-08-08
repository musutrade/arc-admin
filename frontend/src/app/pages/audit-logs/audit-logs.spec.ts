import { Clipboard } from '@angular/cdk/clipboard';
import { provideZonelessChangeDetection } from '@angular/core';
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { AuditLogApiService } from '../../core/api/audit-log-api.service';

import { AuditLogs } from './audit-logs';

describe('AuditLogs', () => {
  let component: AuditLogs;
  let fixture: ComponentFixture<AuditLogs>;
  let clipboard: { copy: ReturnType<typeof vi.fn> };

  beforeEach(async () => {
    clipboard = { copy: vi.fn(() => true) };
    await TestBed.configureTestingModule({
      imports: [AuditLogs],
      providers: [
        provideZonelessChangeDetection(),
        { provide: Clipboard, useValue: clipboard },
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
                    traceId: 'audit-trace-123',
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
    expect(fixture.nativeElement.textContent).toContain('audit-trace-123');
  });

  it('copies the trace id for incident lookup', async () => {
    const button: HTMLButtonElement = fixture.nativeElement.querySelector('.copy-trace');
    button.click();
    await fixture.whenStable();

    expect(clipboard.copy).toHaveBeenCalledWith('audit-trace-123');
    expect(button.title).toBe('已复制');
  });
});
