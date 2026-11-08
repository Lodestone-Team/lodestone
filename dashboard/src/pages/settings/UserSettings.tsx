import { Dialog, Switch, Transition } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from 'components/Atoms/Modal';
import { useQueryClient } from '@tanstack/react-query';
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
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import {
  Skeleton,
  SkeletonGroup,
  SkeletonRows,
} from 'components/Atoms/Skeleton';
import { useUid, useUserInfo } from 'data/UserInfo';
import { Fragment, useContext, useRef, useState } from 'react';
import { toast } from 'react-toastify';
import { changeUserPermissions } from 'utils/apis';
import { errorToString } from 'utils/util';
import CreateUserForm from '../../components/Settings/CreateUserForm';
import {
  NormalPermissions,
  PermissionDescription,
  UnsafePermissions,
} from 'components/Settings/permissionDescriptions';
import UserApiKeys from 'components/Settings/ApiKeys/UserApiKeys';
import { useDocumentTitle } from 'usehooks-ts';
import { NO_SCRIPTING_MESSAGE, useCoreCapabilities } from 'data/SystemInfo';

// a user's permission rows (title, description, toggle) while the user loads
const PermissionsSkeleton = () => (
  <SkeletonGroup label="Loading the user" className="flex flex-col gap-10">
    {Array.from({ length: 6 }, (_, i) => (
      <div key={i} className="flex flex-row items-center justify-between gap-4">
        <div className="flex grow flex-col gap-2">
          <Skeleton className="h-4 w-48 rounded-sm" />
          <Skeleton className="h-3.5 w-72 max-w-full rounded-sm" />
        </div>
        <Skeleton className="h-6 w-11 shrink-0 rounded-full" />
      </div>
    ))}
  </SkeletonGroup>
);

export const UserSettings = () => {
  useDocumentTitle('User Settings - Lodestone');
  const queryClient = useQueryClient();
  const { userList, userListLoading, selectUser, selectedUser } =
    useContext(SettingsContext);
  const { searchParams } = useContext(BrowserLocationContext);
  // a link to a user: show nothing until the list (and the user) has loaded,
  // instead of the list of all users first
  const openingUser = userListLoading && !!searchParams.get('user');
  const { data: instanceList } = useInstanceList();
  const [showCreateUser, setShowCreateUser] = useState(false);
  const { data: userInfo } = useUserInfo();
  const canManageUsers = userInfo?.is_owner || false;
  const uid = useUid();
  const { data: globalSettings } = useGlobalSettings();
  const { macros: macrosAvailable } = useCoreCapabilities();

  const updateCachedPermissions = (
    permissions: UserPermission,
    selectedUid: string
  ) => {
    if (selectedUid === uid) {
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
        if (!oldList || !oldList[selectedUid]) return oldList;
        return {
          ...oldList,
          [selectedUid]: {
            ...oldList[selectedUid],
            permissions,
          },
        };
      }
    );
  };

  // Permission changes are sent one at a time, and each is built from the
  // newest cached permissions when it is sent. Building every request from
  // the render's snapshot let a second quick toggle undo the first.
  const permissionQueue = useRef<Promise<void>>(Promise.resolve());
  const changePermission = (
    user: PublicUser,
    update: (latest: UserPermission) => UserPermission
  ) => {
    permissionQueue.current = permissionQueue.current.then(async () => {
      const latest =
        queryClient.getQueryData<{ [uid: string]: PublicUser }>([
          'user',
          'list',
        ])?.[user.uid]?.permissions ?? user.permissions;
      const newPermissions = update(latest);
      try {
        await changeUserPermissions(user.uid, newPermissions);
        updateCachedPermissions(newPermissions, user.uid);
      } catch (e) {
        toast.error(errorToString(e));
      }
    });
  };

  const UserBoxes = !selectedUser && !openingUser && (
    <div className="flex w-full flex-col gap-4 @4xl:flex-row">
      <div className="flex w-full flex-row flex-nowrap items-end justify-between gap-4 @4xl:w-[28rem] @4xl:flex-col @4xl:items-start @4xl:justify-start">
        <div>
          <h2 className="text-h2 font-bold tracking-tight text-gray-300">
            All Members
            {!userListLoading && ` (${Object.keys(userList).length})`}
          </h2>
          <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
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
      {userListLoading ? (
        // user rows: avatar, name and role
        <SkeletonGroup label="Loading users" className="w-full min-w-0">
          <SkeletonRows
            rows={3}
            icon="round"
            secondLine
            className="divide-y divide-gray-faded/30 overflow-hidden rounded-lg border border-gray-faded/30"
            rowClassName="h-15 bg-gray-800 px-4"
          />
        </SkeletonGroup>
      ) : (
        <div className="h-fit w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
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
      )}
    </div>
  );

  const UserPermissions = (
    permissionList: PermissionDescription[],
    disabled: boolean
  ) =>
    selectedUser && (
      <div className="mb-10 flex w-full flex-col gap-10">
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
                    'text-h3 leading-tight font-bold tracking-medium',
                    disabled ? 'text-white/50' : 'text-gray-300'
                  )}
                >
                  {permission.title}
                </div>
                <div className="overflow-hidden text-medium font-medium tracking-medium text-ellipsis text-white/50 italic">
                  The user {permission.allows}
                </div>
              </div>
              {typeof currentSettings === 'object' && !instanceList && (
                // the instance checkboxes, while the instances load
                <SkeletonGroup
                  label="Loading instances"
                  className="grid grid-cols-2 gap-4 rounded-lg border border-gray-faded/30 bg-gray-800 px-6 py-4"
                >
                  {Array.from({ length: 4 }, (_, i) => (
                    <div key={i} className="flex flex-row items-center gap-3">
                      <Skeleton className="size-4 rounded-sm" />
                      <Skeleton className="h-3.5 w-28 rounded-sm" />
                    </div>
                  ))}
                </SkeletonGroup>
              )}
              {typeof currentSettings === 'object' && instanceList && (
                <MultiSelectGrid
                  className="rounded-lg border border-gray-faded/30 bg-gray-800 px-6 py-4"
                  options={
                    Object.values(instanceList || {}).map(
                      (instance) => instance.uuid
                    ) as string[]
                  }
                  selectedOptions={currentSettings}
                  optionLabel={(uuid) => instanceList?.[uuid]?.name || uuid}
                  optionKey={(uuid) => uuid}
                  onChange={(newSettings) => {
                    // apply only what this click changed, so it combines
                    // with changes still being sent
                    const before = new Set(currentSettings);
                    const after = new Set(newSettings);
                    changePermission(selectedUser, (latest) => {
                      const latestList = latest[
                        permission.permission
                      ] as string[];
                      const kept = latestList.filter(
                        (id) => !before.has(id) || after.has(id)
                      );
                      const added = newSettings.filter(
                        (id) => !before.has(id) && !kept.includes(id)
                      );
                      return {
                        ...latest,
                        [permission.permission]: [...kept, ...added],
                      };
                    });
                  }}
                  disabled={!canManageUsers || disabled}
                />
              )}
              {typeof currentSettings === 'boolean' && (
                <Toggle
                  value={currentSettings}
                  onChange={(value: boolean) => {
                    changePermission(selectedUser, (latest) => ({
                      ...latest,
                      [permission.permission]: value,
                    }));
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
      <Transition appear show={showCreateUser} as={Fragment}>
        <Dialog
          onClose={() => setShowCreateUser(false)}
          className="relative z-10"
        >
          <ModalBackdrop />
          <div className="fixed inset-0 overflow-y-auto">
            <div className="flex min-h-full items-center justify-center p-4">
              <ModalPanel className="flex w-[500px] flex-col items-stretch justify-center gap-4 rounded-3xl bg-gray-800 px-8 pt-16 pb-8">
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
              </ModalPanel>
            </div>
          </div>
        </Dialog>
      </Transition>
      <div className="@container relative mx-auto mb-10 flex h-full w-full max-w-2xl flex-row justify-center">
        <div className="flex w-full flex-col gap-12 overflow-y-scroll px-4 pt-8">
          <h1 className="dashboard-instance-heading">User Settings</h1>
          {UserBoxes}
          {openingUser && <PermissionsSkeleton />}
          {UserPermissions(NormalPermissions, false)}
          {selectedUser && (
            <>
              <HorizontalLine thicknessClass="h-0.5" className="-my-4" />
              <div className="flex flex-col gap-8">
                <div>
                  <div className="pt-12 text-h2 leading-tight font-bold tracking-tight text-red-200">
                    Unsafe Settings
                  </div>
                  <div className="text-h3 font-medium tracking-medium text-red-200">
                    Turn off safe mode in core settings to grant these
                    permissions to non-owner users.
                  </div>
                </div>
                {UserPermissions(
                  UnsafePermissions,
                  globalSettings?.safe_mode || false
                )}
                <div className="-mt-6 mb-10">
                  <div className="text-h3 leading-tight font-bold tracking-medium text-white/50">
                    Macros, Extensions and Atom Instances
                  </div>
                  <div className="text-medium font-medium tracking-medium text-white/50 italic">
                    {macrosAvailable ? (
                      <>
                        Only the owner can use macros, install extensions, and
                        create or control atom instances. These can&apos;t be
                        granted to other users. Docker instances use the normal
                        instance permissions.
                      </>
                    ) : (
                      NO_SCRIPTING_MESSAGE
                    )}
                  </div>
                </div>
              </div>
              <HorizontalLine thicknessClass="h-0.5" className="-my-4" />
              <div className="mb-10 pt-12">
                <UserApiKeys user={selectedUser} />
              </div>
            </>
          )}
        </div>
      </div>
      {/* TODO: your own section */}
    </>
  );
};

export default UserSettings;
