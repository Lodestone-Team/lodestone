import { UserPermission } from 'bindings/UserPermission';
import type { MessageKey } from 'i18n';

// the permissions the editors list; each has messages in the settings part
type ListedPermission =
  | 'can_create_instance'
  | 'can_delete_instance'
  | 'can_read_global_file'
  | 'can_view_instance'
  | 'can_start_instance'
  | 'can_stop_instance'
  | 'can_access_instance_console'
  | 'can_access_instance_setting'
  | 'can_read_instance_resource'
  | 'can_read_instance_file'
  | 'can_write_instance_resource'
  | 'can_write_instance_file'
  | 'can_write_global_file'
  | 'can_manage_permission';

/**
 * A permission shown in a permission editor: the user permission editor in
 * the user settings, and the custom scope of an API key. The texts are
 * message keys, translated where they are shown.
 */
export type PermissionDescription = {
  permission: keyof UserPermission;
  titleKey: `settings.permission.${ListedPermission}.title`;
  /** What the permission allows a user: "The user can …" */
  userKey: `settings.permission.${ListedPermission}.user`;
  /** What the permission allows an API key: "The key can …" */
  keyKey: `settings.permission.${ListedPermission}.key`;
};

// every listed permission has its three messages
type Check<T extends MessageKey> = T;
export type PermissionMessageKey = Check<
  | PermissionDescription['titleKey']
  | PermissionDescription['userKey']
  | PermissionDescription['keyKey']
>;

const describe = (permission: ListedPermission): PermissionDescription => ({
  permission,
  titleKey: `settings.permission.${permission}.title`,
  userKey: `settings.permission.${permission}.user`,
  keyKey: `settings.permission.${permission}.key`,
});

export const NormalPermissions: PermissionDescription[] = (
  [
    'can_create_instance',
    'can_delete_instance',
    'can_read_global_file',
    'can_view_instance',
    'can_start_instance',
    'can_stop_instance',
    'can_access_instance_console',
    'can_access_instance_setting',
    'can_read_instance_resource',
    'can_read_instance_file',
  ] as const
).map(describe);

export const UnsafePermissions: PermissionDescription[] = (
  [
    'can_write_instance_resource',
    'can_write_instance_file',
    'can_write_global_file',
    'can_manage_permission',
  ] as const
).map(describe);
