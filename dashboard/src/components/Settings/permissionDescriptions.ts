import { UserPermission } from 'bindings/UserPermission';

/**
 * A permission shown in a permission editor: the user permission editor in
 * the user settings, and the custom scope of an API key.
 */
export type PermissionDescription = {
  permission: keyof UserPermission;
  title: string;
  /** What the permission allows, after a subject such as "The user". */
  allows: string;
};

export const NormalPermissions: PermissionDescription[] = [
  {
    permission: 'can_create_instance',
    title: 'Create Servers',
    allows: 'can create new servers.',
  },
  {
    permission: 'can_delete_instance',
    title: 'Delete Servers',
    allows: 'can delete any server.',
  },
  {
    permission: 'can_read_global_file',
    title: 'Read Global Files',
    allows: 'can read any file on the computer Lodestone core is running on.',
  },
  {
    permission: 'can_view_instance',
    title: 'View Servers',
    allows: 'can see these servers.',
  },
  {
    permission: 'can_start_instance',
    title: 'Start Servers',
    allows: 'can start these servers.',
  },
  {
    permission: 'can_stop_instance',
    title: 'Stop Servers',
    allows: 'can stop these servers.',
  },
  {
    permission: 'can_access_instance_console',
    title: 'Access Server Console',
    allows:
      'can read and send commands to the console of these servers. This essentially gives operator access to the server.',
  },
  {
    permission: 'can_access_instance_setting',
    title: 'Access Server Settings',
    allows: 'can read and change the settings of these servers.',
  },
  {
    permission: 'can_read_instance_resource',
    title: 'Read Server Resources',
    allows: 'can read the resources(worlds, mods, etc.) of these servers.',
  },
  {
    permission: 'can_read_instance_file',
    title: 'Read Server Files',
    allows: 'can read the files of these servers.',
  },
];

export const UnsafePermissions: PermissionDescription[] = [
  {
    permission: 'can_write_instance_resource',
    title: 'Modify Server Resources',
    allows: 'can modify the resources(worlds, mods, etc.) of these servers.',
  },
  {
    permission: 'can_write_instance_file',
    title: 'Modify Server Files',
    allows: 'can modify the files of these servers.',
  },
  {
    permission: 'can_write_global_file',
    title: 'Modify Global Files',
    allows: 'can modify any file on the computer Lodestone core is running on.',
  },
  {
    permission: 'can_manage_permission',
    title: 'Manage Permissions',
    allows: 'can manage the permissions of other users.',
  },
];
