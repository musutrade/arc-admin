export interface ApiUser {
  id: number;
  username: string;
  displayName: string;
  email: string | null;
  status: 'active' | 'inactive' | 'suspended';
  roles: string[];
  lastLoginAt: string | null;
  createdAt: string;
}

export interface ApiPage<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}

export interface LoginResponse {
  expiresAt: string;
  user: ApiUser;
}

export interface ChangePasswordRequest {
  currentPassword: string;
  newPassword: string;
}

export interface PermissionCodes {
  codes: string[];
}

export interface ApiPermission {
  id: number;
  code: string;
  name: string;
  type: 'menu' | 'button' | 'api';
  description: string | null;
}

export interface ApiPermissionGroup {
  id: number;
  code: string;
  name: string;
  icon: string | null;
  permissions: ApiPermission[];
}

export interface ApiRole {
  id: number;
  code: string;
  name: string;
  category: string;
  icon: string | null;
  color: 'primary' | 'warning' | 'success' | 'danger' | 'neutral';
  description: string | null;
  dataScope: 'all' | 'organization' | 'department_and_children' | 'department' | 'self';
  isActive: boolean;
  members: number;
  permissionGroupIds: number[];
}

export interface RolePermissions {
  permissionIds: number[];
}

export interface DashboardStats {
  totalUsers: number;
  activeUsers: number;
  totalRoles: number;
  totalPermissions: number;
  suspendedUsers: number;
}

export interface ApiAuditLog {
  id: number;
  actorUserId: number | null;
  actorUsername: string | null;
  action: string;
  targetType: string;
  targetId: number | null;
  details: Record<string, unknown>;
  traceId: string | null;
  createdAt: string;
}

export interface CreateUserRequest {
  username: string;
  password: string;
  displayName: string;
  email?: string | null;
  status?: ApiUser['status'];
  roleIds?: number[];
}

export interface UpdateUserRequest {
  displayName?: string;
  email?: string | null;
  status?: ApiUser['status'];
  password?: string;
}

export interface CreateRoleRequest {
  code: string;
  name: string;
  category?: string;
  icon?: string | null;
  color?: ApiRole['color'];
  description?: string | null;
  dataScope?: ApiRole['dataScope'];
  permissionIds?: number[];
}

export interface UpdateRoleRequest {
  name?: string;
  category?: string;
  icon?: string | null;
  color?: ApiRole['color'];
  description?: string | null;
  dataScope?: ApiRole['dataScope'];
  isActive?: boolean;
}
