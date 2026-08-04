import { PermissionGroup, Role, RolePermissionRow, StatCard, User } from './models';

/** ================= 用户 ================= */
export const MOCK_USERS: User[] = [
  {
    id: 'u-001',
    name: 'Sarah Jenkins',
    email: 's.jenkins@enterprise.com',
    roles: ['Security Lead', 'Auditor'],
    status: 'active',
    lastLogin: '2 mins ago',
    avatarColor: '#165dff',
  },
  {
    id: 'u-002',
    name: 'Marcus Thorne',
    email: 'm.thorne@enterprise.com',
    roles: ['Standard User'],
    status: 'active',
    lastLogin: '1 hour ago',
    avatarColor: '#14c9c9',
  },
  {
    id: 'u-003',
    name: 'Elena Rodriguez',
    email: 'e.rodriguez@enterprise.com',
    roles: ['Global Admin'],
    status: 'inactive',
    lastLogin: '3 days ago',
    avatarColor: '#722ed1',
  },
  {
    id: 'u-004',
    name: 'Amit Patel',
    email: 'a.patel@enterprise.com',
    roles: ['Billing Mgr'],
    status: 'active',
    lastLogin: '14 mins ago',
    avatarColor: '#f77234',
  },
  {
    id: 'u-005',
    name: 'Yuki Tanaka',
    email: 'y.tanaka@enterprise.com',
    roles: ['Security Lead'],
    status: 'suspended',
    lastLogin: '5 days ago',
    avatarColor: '#00b42a',
  },
  {
    id: 'u-006',
    name: 'David Kim',
    email: 'd.kim@enterprise.com',
    roles: ['Standard User', 'Billing Mgr'],
    status: 'active',
    lastLogin: '42 mins ago',
    avatarColor: '#f53f3f',
  },
  {
    id: 'u-007',
    name: 'Priya Sharma',
    email: 'p.sharma@enterprise.com',
    roles: ['Auditor'],
    status: 'active',
    lastLogin: '3 hours ago',
    avatarColor: '#ff7d00',
  },
  {
    id: 'u-008',
    name: 'Tom Becker',
    email: 't.becker@enterprise.com',
    roles: ['Standard User'],
    status: 'inactive',
    lastLogin: '12 days ago',
    avatarColor: '#0fc6c2',
  },
  {
    id: 'u-009',
    name: 'Lena Fischer',
    email: 'l.fischer@enterprise.com',
    roles: ['Global Admin', 'Auditor'],
    status: 'active',
    lastLogin: '8 mins ago',
    avatarColor: '#7d67ff',
  },
  {
    id: 'u-010',
    name: 'James Okafor',
    email: 'j.okafor@enterprise.com',
    roles: ['Standard User'],
    status: 'active',
    lastLogin: '1 day ago',
    avatarColor: '#18a058',
  },
  {
    id: 'u-011',
    name: 'Sofia Marino',
    email: 's.marino@enterprise.com',
    roles: ['Auditor', 'Standard User'],
    status: 'active',
    lastLogin: '26 mins ago',
    avatarColor: '#4080ff',
  },
  {
    id: 'u-012',
    name: 'Lucas Weber',
    email: 'l.weber@enterprise.com',
    roles: ['Standard User'],
    status: 'suspended',
    lastLogin: '9 days ago',
    avatarColor: '#9c27b0',
  },
];

/** ================= 权限(层级分组) ================= */
export const MOCK_PERMISSION_GROUPS: PermissionGroup[] = [
  {
    id: 'pg-dashboard',
    name: 'Dashboard Module',
    icon: 'dashboard',
    permissions: [
      {
        id: 'p-dash-1',
        name: 'View Analytics',
        code: 'dashboard:analytics:read',
        type: 'menu',
        description: 'Ability to access and view the statistical charts in dashboard.',
      },
      {
        id: 'p-dash-2',
        name: 'Export Reports',
        code: 'dashboard:analytics:export',
        type: 'button',
        description: 'Allows downloading CSV/PDF versions of analytics reports.',
      },
      {
        id: 'p-dash-3',
        name: 'Configure Widgets',
        code: 'dashboard:widgets:manage',
        type: 'api',
        description: 'Persist dashboard widget layout and data-source bindings.',
      },
    ],
  },
  {
    id: 'pg-identity',
    name: 'Identity & Access Module',
    icon: 'group',
    permissions: [
      {
        id: 'p-ia-1',
        name: 'User Write Access',
        code: 'user:write',
        type: 'api',
        description: 'Allows creating and updating user account data via REST endpoints.',
      },
      {
        id: 'p-ia-2',
        name: 'Password Reset Trigger',
        code: 'user:admin:reset_password',
        type: 'button',
        description: 'Grants authority to force a password reset on any user account.',
      },
      {
        id: 'p-ia-3',
        name: 'View Directory',
        code: 'user:directory:read',
        type: 'menu',
        description: 'Browse the full employee directory and contact details.',
      },
      {
        id: 'p-ia-4',
        name: 'Deactivate Accounts',
        code: 'user:admin:deactivate',
        type: 'button',
        description: 'Suspend or deactivate user accounts across the organization.',
      },
    ],
  },
  {
    id: 'pg-resources',
    name: 'Resources Module',
    icon: 'inventory_2',
    permissions: [
      {
        id: 'p-res-1',
        name: 'Hardware Inventory Read',
        code: 'resource:hw:read',
        type: 'menu',
        description: 'View the list of all hardware assets in the system.',
      },
      {
        id: 'p-res-2',
        name: 'Modify Infrastructure',
        code: 'resource:infra:manage',
        type: 'api',
        description: 'Critical permission for modifying cloud infrastructure components.',
      },
      {
        id: 'p-res-3',
        name: 'License Provisioning',
        code: 'resource:license:grant',
        type: 'button',
        description: 'Grant or revoke software license seats for teams.',
      },
    ],
  },
  {
    id: 'pg-audit',
    name: 'Audit & Compliance Module',
    icon: 'fact_check',
    permissions: [
      {
        id: 'p-aud-1',
        name: 'View Audit Logs',
        code: 'audit:logs:read',
        type: 'menu',
        description: 'Read immutable audit trail for all admin actions.',
      },
      {
        id: 'p-aud-2',
        name: 'Export Audit Evidence',
        code: 'audit:logs:export',
        type: 'button',
        description: 'Export signed audit evidence for external compliance reviews.',
      },
    ],
  },
  {
    id: 'pg-security',
    name: 'Security Center Module',
    icon: 'shield',
    permissions: [
      {
        id: 'p-sec-1',
        name: 'Manage Policies',
        code: 'security:policies:manage',
        type: 'api',
        description: 'Create and update global security policies and rule sets.',
      },
      {
        id: 'p-sec-2',
        name: 'MFA Enforcement',
        code: 'security:mfa:enforce',
        type: 'button',
        description: 'Force multi-factor authentication enrollment for user groups.',
      },
      {
        id: 'p-sec-3',
        name: 'View Threat Events',
        code: 'security:threats:read',
        type: 'menu',
        description: 'Access real-time threat detection and anomaly events.',
      },
    ],
  },
];

/** ================= 角色 ================= */
export const MOCK_ROLES: Role[] = [
  {
    id: 'r-001',
    name: 'Super Admin',
    category: 'System Core',
    icon: 'rocket_launch',
    color: 'primary',
    description:
      'Full access to all modules, including billing, user management, and global security settings.',
    members: 4,
    permissionGroupIds: ['pg-dashboard', 'pg-identity', 'pg-resources', 'pg-audit', 'pg-security'],
  },
  {
    id: 'r-002',
    name: 'Editor',
    category: 'Content',
    icon: 'edit_note',
    color: 'warning',
    description:
      'Can create, edit, and publish content. No access to financial or security settings.',
    members: 12,
    permissionGroupIds: ['pg-dashboard'],
  },
  {
    id: 'r-003',
    name: 'Viewer',
    category: 'Read Only',
    icon: 'visibility',
    color: 'success',
    description:
      'Can view data, reports, and dashboards. No modification rights across the system.',
    members: 84,
    permissionGroupIds: ['pg-dashboard', 'pg-resources'],
  },
  {
    id: 'r-004',
    name: 'Compliance Auditor',
    category: 'Compliance',
    icon: 'fact_check',
    color: 'danger',
    description: 'Special access to audit logs and security reporting for quarterly reviews.',
    members: 2,
    permissionGroupIds: ['pg-audit', 'pg-security'],
  },
  {
    id: 'r-005',
    name: 'Support Tier 2',
    category: 'Service',
    icon: 'support_agent',
    color: 'neutral',
    description:
      'Helpdesk troubleshooting rights including user password resets and ticket escalation.',
    members: 9,
    permissionGroupIds: ['pg-identity'],
  },
  {
    id: 'r-006',
    name: 'Billing Manager',
    category: 'Finance',
    icon: 'account_balance_wallet',
    color: 'warning',
    description: 'Invoice management, payment processing, and subscription plan adjustments.',
    members: 3,
    permissionGroupIds: ['pg-resources', 'pg-dashboard'],
  },
];

/** ================= 分配权限(角色行) ================= */
export const MOCK_ROLE_PERMISSION_ROWS: RolePermissionRow[] = [
  {
    roleId: 'r-001',
    roleName: 'Super Administrator',
    usersAssigned: 12,
    active: true,
    groupIds: ['pg-dashboard', 'pg-identity', 'pg-resources', 'pg-audit', 'pg-security'],
  },
  {
    roleId: 'r-004',
    roleName: 'Compliance Officer',
    usersAssigned: 5,
    active: true,
    groupIds: ['pg-audit', 'pg-security'],
  },
  {
    roleId: 'r-003',
    roleName: 'Read Only Viewer',
    usersAssigned: 84,
    active: true,
    groupIds: ['pg-dashboard', 'pg-resources'],
  },
  {
    roleId: 'r-002',
    roleName: 'Content Editor',
    usersAssigned: 12,
    active: false,
    groupIds: ['pg-dashboard'],
  },
  {
    roleId: 'r-005',
    roleName: 'Support Agent',
    usersAssigned: 9,
    active: true,
    groupIds: ['pg-identity'],
  },
];

/** 分配权限模态框内,每个权限的勾选状态(以 roleId 为键) */
export const MOCK_ASSIGNED_MAP: Record<string, string[]> = {
  'r-001': [
    'p-dash-1',
    'p-dash-2',
    'p-dash-3',
    'p-ia-1',
    'p-ia-2',
    'p-ia-3',
    'p-ia-4',
    'p-res-1',
    'p-res-2',
    'p-res-3',
    'p-aud-1',
    'p-aud-2',
    'p-sec-1',
    'p-sec-2',
    'p-sec-3',
  ],
  'r-004': ['p-aud-1', 'p-aud-2', 'p-sec-1', 'p-sec-3'],
  'r-003': ['p-dash-1', 'p-res-1'],
  'r-002': ['p-dash-1', 'p-dash-2'],
  'r-005': ['p-ia-1', 'p-ia-2', 'p-ia-3'],
};

/** ================= 统计卡片 ================= */
export const MOCK_USER_STATS: StatCard[] = [
  { label: 'Total Users', value: '1,284', trend: '+4.2%', trendTone: 'success' },
  { label: 'Active Now', value: '342', pulse: true },
  { label: 'Pending Requests', value: '12', trend: 'Review', trendTone: 'warning' },
  { label: 'Login Failures', value: '5', trend: '-12%', trendTone: 'danger' },
];

/** ================= 登录提示 ================= */
export const MOCK_CREDENTIALS = {
  username: 'admin',
  password: 'admin123',
};
