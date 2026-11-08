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
    title: 'Create Instances',
    allows: 'can create new instances.',
  },
  {
    permission: 'can_delete_instance',
    title: 'Delete Instances',
    allows: 'can delete any instance.',
  },
  {
    permission: 'can_read_global_file',
    title: 'Read Global Files',
    allows: 'can read any file on the computer Lodestone core is running on.',
  },
  {
    permission: 'can_view_instance',
    title: 'View Instances',
    allows: 'can see these instances.',
  },
  {
    permission: 'can_start_instance',
    title: 'Start Instances',
    allows: 'can start these instances.',
  },
  {
    permission: 'can_stop_instance',
    title: 'Stop Instances',
    allows: 'can stop these instances.',
  },
  {
    permission: 'can_access_instance_console',
    title: 'Access Instance Console',
    allows:
      'can read and send commands to the console of these instances. This essentially gives operator access to the instance.',
  },
  {
    permission: 'can_access_instance_setting',
    title: 'Access Instance Settings',
    allows: 'can read and change the settings of these instances.',
  },
  {
    permission: 'can_read_instance_resource',
    title: 'Read Instance Resources',
    allows: 'can read the resources(worlds, mods, etc.) of these instances.',
  },
  {
    permission: 'can_read_instance_file',
    title: 'Read Instance Files',
    allows: 'can read the files of these instances.',
  },
];

export const UnsafePermissions: PermissionDescription[] = [
  {
    permission: 'can_write_instance_resource',
    title: 'Modify Instance Resources',
    allows: 'can modify the resources(worlds, mods, etc.) of these instances.',
  },
  {
    permission: 'can_write_instance_file',
    title: 'Modify Instance Files',
    allows: 'can modify the files of these instances.',
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
