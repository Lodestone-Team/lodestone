import { Dialog, Switch, Transition } from '@headlessui/react';
import { useQueryClient } from '@tanstack/react-query';
import { j } from '@tauri-apps/api/event-2a9960e7';
import { PublicUser } from 'bindings/PublicUser';
import { UserPermission } from 'bindings/UserPermission';
import clsx from 'clsx';
import Button from 'components/Atoms/Button';
import { HorizontalLine } from 'components/Atoms/HorizontalLine';
import MultiSelectGrid from 'components/Atoms/MultiSelectGrid';
import { Toggle } from 'components/Atoms/Toggle';
import UserBox from 'components/UserBox';
import { useGlobalSettings } from 'data/GlobalSettings';
import { useInstanceList } from 'data/InstanceList';
import { SettingsContext } from 'data/SettingsContext';
import { useUid, useUserInfo } from 'data/UserInfo';
import { Fragment, useContext, useState } from 'react';
import { toast } from 'react-toastify';
import { changeUserPermissions } from 'utils/util';
import CreateUserForm from './CreateUserForm';

const NormalPermissions: {
  permission: keyof UserPermission;
  title: string;
  description: string;
}[] = [
  {
    permission: 'can_create_instance' as keyof UserPermission,
    title: 'Create Instances',
    description: 'The user can create new instances.',
  },
  {
    permission: 'can_delete_instance' as keyof UserPermission,
    title: 'Delete Instances',
    description: 'The user can delete any instance.',
  },
  {
    permission: 'can_read_global_file' as keyof UserPermission,
    title: 'Read Global Files',
    description:
      'The user can read any file on the computer Lodestone core is running on.',
  },
  {
    permission: 'can_view_instance' as keyof UserPermission,
    title: 'View Instances',
    description: 'The user can see these instances.',
  },
  {
    permission: 'can_start_instance' as keyof UserPermission,
    title: 'Start Instances',
    description: 'The user can start these instances.',
  },
  {
    permission: 'can_stop_instance' as keyof UserPermission,
    title: 'Stop Instances',
    description: 'The user can stop these instances.',
  },
  {
    permission: 'can_access_instance_console' as keyof UserPermission,
    title: 'Access Instance Console',
    description:
      'The user can read and send commands to the console of these instances. This essentially gives the user operator access to the instance.',
  },
  {
    permission: 'can_access_instance_setting' as keyof UserPermission,
    title: 'Access Instance Settings',
    description:
      'The user can read and change the settings of these instances.',
  },
  {
    permission: 'can_read_instance_resource' as keyof UserPermission,
    title: 'Read Instance Resources',
    description:
      'The user can read the resources(worlds, mods, etc.) of these instances.',
  },
  {
    permission: 'can_read_instance_file' as keyof UserPermission,
    title: 'Read Instance Files',
    description: 'The user can read the files of these instances.',
  },
];

const UnsafePermissions: {
  permission: keyof UserPermission;
  title: string;
  description: string;
}[] = [
  {
    permission: 'can_write_instance_resource' as keyof UserPermission,
    title: 'Modify Instance Resources',
    description:
      'The user can modify the resources(worlds, mods, etc.) of these instances.',
  },
  {
    permission: 'can_access_instance_macro' as keyof UserPermission,
    title: 'Access Instance Macros',
    description: 'The user can read and modify the macros of these instances.',
  },
  {
    permission: 'can_write_instance_file' as keyof UserPermission,
    title: 'Modify Instance Files',
    description: 'The user can modify the files of these instances.',
  },
  {
    permission: 'can_write_global_file' as keyof UserPermission,
    title: 'Modify Global Files',
    description:
      'The user can modify any file on the computer Lodestone core is running on.',
  },
  {
    permission: 'can_manage_permission' as keyof UserPermission,
    title: 'Manage Permissions',
    description: 'The user can manage the permissions of other users.',
  },
];

export const UserSettings = () => {
  const queryClient = useQueryClient();
  const { userList, selectUser, selectedUser } = useContext(SettingsContext);
  const { data: instanceList } = useInstanceList();
  const [showCreateUser, setShowCreateUser] = useState(false);
  const { data: userInfo } = useUserInfo();
  const canManageUsers = userInfo?.is_owner || false;
  const uid = useUid();
  const { data: globalSettings } = useGlobalSettings();

  const updateCachedPermissions = async (
    permissions: UserPermission,
    selectedUid: string
  ) => {
    if (!selectedUser) return;
    if (selectedUser.uid === uid) {
      queryClient.setQueryData(
        ['user', 'info'],
        (oldInfo: PublicUser | undefined) => {
          if (!oldInfo) return oldInfo;
          return {
            ...oldInfo,
            permissions,
          };
        }
      );
    }
    queryClient.setQueryData(
      ['user', 'list'],
      (oldList: { [uid: string]: PublicUser } | undefined) => {
        if (!oldList) return oldList;
        return {
          ...oldList,
          [selectedUser.uid]: {
            ...selectedUser,
            permissions,
          },
        };
      }
    );
  };

  const UserBoxes = !selectedUser && (
    <div className="flex w-full flex-col gap-4 @4xl:flex-row">
      <div className="flex w-full flex-row flex-nowrap items-end justify-between gap-4 @4xl:w-[28rem] @4xl:flex-col @4xl:items-start @4xl:justify-start">
        <div>
          <h2 className="text-h2 font-bold tracking-tight text-gray-300">
            All Members ({Object.keys(userList).length})
          </h2>
          <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
            A list of all users. Click into a user to manage
          </h3>
        </div>
        <Button
          label="Create New User"
          className="whitespace-nowrap"
          onClick={() => {
            setShowCreateUser(true);
          }}
        />
      </div>
      <div className="h-fit w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
        {Object.keys(userList)
          .sort((a, b) =>
            userList[a].uid === uid
              ? -1
              : userList[b].uid === uid
              ? 1
              : userList[a].username.localeCompare(userList[b].username)
          )
          .map((uid) => (
            <UserBox
              user={userList[uid]}
              key={uid}
              onClick={() => selectUser(userList[uid])}
            />
          ))}
      </div>
    </div>
  );

  const UserPermissions = (
    permissionList: {
      permission: keyof UserPermission;
      title: string;
      description: string;
    }[],
    disabled: boolean
  ) =>
    selectedUser && (
      <div className="flex w-full flex-col gap-10">
        {permissionList.map((permission) => {
          const currentSettings =
            selectedUser.permissions[permission.permission];
          const permissionType = typeof currentSettings;
          return (
            <div
              key={permission.permission}
              className={clsx(
                'flex gap-4',
                permissionType === 'boolean' &&
                  'flex-row items-center justify-between',
                permissionType === 'object' && 'flex-col'
              )}
            >
              <div>
                <div
                  className={clsx(
                    'text-h3 font-bold leading-tight tracking-medium',
                    disabled ? 'text-white/50' : 'text-gray-300'
                  )}
                >
                  {permission.title}
                </div>
                <div className="overflow-hidden text-ellipsis text-medium font-medium italic tracking-medium text-white/50">
                  {permission.description}
                </div>
              </div>
              {typeof currentSettings === 'object' && (
                <MultiSelectGrid
                  className="rounded-lg border border-gray-faded/30 bg-gray-800 px-6 py-4"
                  options={
                    Object.values(instanceList || {}).map(
                      (instance) => instance.uuid
                    ) as string[]
                  }
                  selectedOptions={currentSettings}
                  optionLabel={(uuid) => instanceList?.[uuid]?.name || uuid}
                  onChange={(newSettings) => {
                    // I hate typescript
                    const newPermissions = {
                      ...selectedUser.permissions,
                      [permission.permission]: newSettings,
                    };
                    changeUserPermissions(selectedUser.uid, newPermissions)
                      .then(() => {
                        updateCachedPermissions(
                          newPermissions,
                          selectedUser.uid
                        );
                      })
                      .catch((e) => {
                        toast.error(e.message);
                      });
                  }}
                  disabled={!canManageUsers || disabled}
                />
              )}
              {typeof currentSettings === 'boolean' && (
                <Toggle
                  value={currentSettings}
                  onChange={(value: boolean) => {
                    const newPermissions = {
                      ...selectedUser.permissions,
                      [permission.permission]: value,
                    };
                    changeUserPermissions(selectedUser.uid, newPermissions)
                      .then(() => {
                        updateCachedPermissions(
                          newPermissions,
                          selectedUser.uid
                        );
                      })
                      .catch((e) => {
                        toast.error(e.message);
                      });
                  }}
                  disabled={!canManageUsers || disabled}
                />
              )}
            </div>
          );
        })}
      </div>
    );

  // TODO: add non-owner view (can't manage users, can only change their own password)
  return (
    <>
      <Transition
        appear
        show={showCreateUser}
        as={Fragment}
        enter="ease-out duration-200"
        enterFrom="opacity-0"
        enterTo="opacity-100"
        leave="ease-in duration-150"
        leaveFrom="opacity-100"
        leaveTo="opacity-0"
      >
        <Dialog
          onClose={() => setShowCreateUser(false)}
          className="relative z-10"
        >
          <div className="fixed inset-0 bg-gray-900/60" />
          <div className="fixed inset-0 overflow-y-auto">
            <div className="flex min-h-full items-center justify-center p-4">
              <Dialog.Panel className="flex w-[500px] flex-col items-stretch justify-center gap-4 rounded-3xl bg-gray-800 px-8 pb-8 pt-16">
                <h1 className="text-h1 font-bold tracking-tight text-gray-300">
                  Create a new user account
                </h1>
                <p>
                  This user will start with no permissions. You can grant them
                  permissions later.
                </p>
                <CreateUserForm
                  onSuccess={() => setShowCreateUser(false)}
                  onCancel={() => setShowCreateUser(false)}
                />
              </Dialog.Panel>
            </div>
          </div>
        </Dialog>
      </Transition>
      {UserBoxes}
      {UserPermissions(NormalPermissions, false)}
      {selectedUser && (
        <>
          <HorizontalLine thicknessClass="h-0.5" className="-my-4" />
          <div className="flex flex-col gap-8">
            <div>
              <div className="text-h2 font-bold leading-tight tracking-tight text-red-200">
                Unsafe Settings
              </div>
              <div className="text-h3 font-medium tracking-medium text-red-200">
                Turn off safe mode in core settings to grant these permissions
                to non-owner users.
              </div>
            </div>
            {UserPermissions(
              UnsafePermissions,
              globalSettings?.safe_mode || false
            )}
          </div>
        </>
      )}
      {/* TODO: your own section */}
    </>
  );
};

export default UserSettings;
