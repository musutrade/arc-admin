/** 数据模型定义 —— RBAC Control Center */

export type UserStatus = 'active' | 'inactive' | 'suspended';

export interface User {
  id: string;
  username: string;
  name: string;
  email: string;
  roles: string[];
  status: UserStatus;
  lastLogin: string;
  createdAt: string;
  avatarColor: string; // 首字母头像背景色
}

export type PermissionType = 'menu' | 'button' | 'api';

export interface Permission {
  id: string;
  name: string;
  code: string;
  type: PermissionType;
  description: string;
}

export interface PermissionGroup {
  id: string;
  code: string;
  name: string;
  icon: string;
  permissions: Permission[];
}

export interface Role {
  id: string;
  code: string;
  name: string;
  category: string;
  icon: string;
  color: 'primary' | 'warning' | 'success' | 'danger' | 'neutral';
  description: string;
  members: number;
  permissionGroupIds: string[];
  isActive: boolean;
}

export interface RolePermissionRow {
  roleId: string;
  roleCode: string;
  roleName: string;
  usersAssigned: number;
  active: boolean;
  groupIds: string[];
}

export interface StatCard {
  label: string;
  value: string;
  trend?: string;
  trendTone?: 'success' | 'warning' | 'danger';
  icon?: string;
  pulse?: boolean;
}
