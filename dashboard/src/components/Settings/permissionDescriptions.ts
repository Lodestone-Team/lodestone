import { UserPermission } from 'bindings/UserPermission';
import type { MessageKey } from 'i18n';

// the permissions the editors list; each has a title in the settings part
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

// the ones whose title doesn't say it all: they also have a hint
type HintedPermission = Extract<
  ListedPermission,
  | 'can_delete_instance'
  | 'can_read_global_file'
  | 'can_access_instance_console'
  | 'can_read_instance_resource'
  | 'can_write_instance_resource'
  | 'can_write_global_file'
  | 'can_manage_permission'
>;

const HINTED: readonly HintedPermission[] = [
  'can_delete_instance',
  'can_read_global_file',
  'can_access_instance_console',
  'can_read_instance_resource',
  'can_write_instance_resource',
  'can_write_global_file',
  'can_manage_permission',
];

const isHinted = (p: ListedPermission): p is HintedPermission =>
  (HINTED as readonly ListedPermission[]).includes(p);

/**
 * A permission shown in a permission editor: the user permission editor in
 * the user settings, and the custom scope of an API key. The texts are
 * message keys, translated where they are shown.
 */
export type PermissionDescription = {
  permission: keyof UserPermission;
  titleKey: `settings.permission.${ListedPermission}.title`;
  /** What the title leaves out, the same for a user and an API key; none when the title says it all */
  hintKey: `settings.permission.${HintedPermission}.hint` | null;
};

// every listed permission has its title, and the hinted ones their hint
type Check<T extends MessageKey> = T;
export type PermissionMessageKey = Check<
  | PermissionDescription['titleKey']
  | NonNullable<PermissionDescription['hintKey']>
>;

const describe = (permission: ListedPermission): PermissionDescription => ({
  permission,
  titleKey: `settings.permission.${permission}.title`,
  hintKey: isHinted(permission)
    ? `settings.permission.${permission}.hint`
    : null,
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
