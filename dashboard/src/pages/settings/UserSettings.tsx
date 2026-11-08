import { Dialog, Transition } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from 'components/Atoms/Modal';
import { useQueryClient } from '@tanstack/react-query';
import { PublicUser } from 'bindings/PublicUser';
import { UserPermission } from 'bindings/UserPermission';
import Button from 'components/Atoms/Button';
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
import { UserMinecraftLinks } from 'components/Settings/MinecraftLinks';
import { useDocumentTitle } from 'usehooks-ts';
import { noScriptingMessage, useCoreCapabilities } from 'data/SystemInfo';
import { useT } from 'i18n';
import {
  SettingsCard,
  SettingsLink,
  SettingsPage,
  SettingsRow,
  SettingsSection,
} from 'components/Settings/SettingsBlocks';
import { notifySaved } from 'components/Settings/savedToast';

// a user's permission rows (title, description, toggle) while the user loads
const PermissionsSkeleton = () => {
  const t = useT();
  return (
    <SkeletonGroup
      label={t('settings.users.loadingUser')}
      className="flex flex-col divide-y divide-fade-700 rounded-xl border border-fade-700 bg-gray-850"
    >
      {Array.from({ length: 6 }, (_, i) => (
        <div
          key={i}
          className="flex min-h-14 flex-row items-center justify-between gap-4 px-4 py-3"
        >
          <div className="flex grow flex-col gap-2">
            <Skeleton className="h-3.5 w-48 rounded-sm" />
            <Skeleton className="h-3 w-72 max-w-full rounded-sm" />
          </div>
          <Skeleton className="h-6 w-11 shrink-0 rounded-full" />
        </div>
      ))}
    </SkeletonGroup>
  );
};

export const UserSettings = () => {
  const t = useT();
  useDocumentTitle(
    t('common.pageTitle', { page: t('settings.users.documentTitle') })
  );
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
        notifySaved();
      } catch (e) {
        toast.error(errorToString(e));
      }
    });
  };

  const UserBoxes = !selectedUser && !openingUser && (
    <SettingsSection
      title={
        userListLoading
          ? t('settings.users.allMembers')
          : t('settings.users.allMembersCount', {
              count: Object.keys(userList).length,
            })
      }
      lead={t('settings.users.allMembersLead')}
      action={
        <Button
          label={t('settings.users.createUser')}
          className="whitespace-nowrap"
          onClick={() => {
            setShowCreateUser(true);
          }}
        />
      }
    >
      {userListLoading ? (
        // user rows: avatar, name and role
        <SkeletonGroup
          label={t('settings.users.loadingUsers')}
          className="w-full min-w-0"
        >
          <SkeletonRows
            rows={3}
            icon="round"
            secondLine
            className="divide-y divide-fade-700 rounded-xl border border-fade-700 bg-gray-850"
            rowClassName="h-14 px-4"
          />
        </SkeletonGroup>
      ) : (
        <SettingsCard>
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
        </SettingsCard>
      )}
    </SettingsSection>
  );

  const UserPermissions = (
    permissionList: PermissionDescription[],
    disabled: boolean
  ) =>
    selectedUser &&
    permissionList.map((permission) => {
      const currentSettings = selectedUser.permissions[permission.permission];
      const labelId = `permission-${permission.permission}`;
      return (
        <SettingsRow
          key={permission.permission}
          label={t(permission.titleKey)}
          labelId={labelId}
          disabled={disabled}
          hint={t(permission.userKey)}
          below={
            <>
              {typeof currentSettings === 'object' && !instanceList && (
                // the instance checkboxes, while the instances load
                <SkeletonGroup
                  label={t('settings.users.loadingServers')}
                  className="grid grid-cols-2 gap-4 rounded-lg border border-fade-700 bg-gray-900 px-4 py-3"
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
                  className="rounded-lg border border-fade-700 bg-gray-900 px-4 py-3"
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
            </>
          }
        >
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
              aria-labelledby={labelId}
            />
          )}
        </SettingsRow>
      );
    });

  // TODO: add non-owner view (can't manage users, can only change their own password)
  return (
    <>
      <Transition appear show={showCreateUser} as={Fragment}>
        <Dialog
          onClose={() => setShowCreateUser(false)}
          className="relative z-60"
        >
          <ModalBackdrop />
          <div className="fixed inset-0 overflow-y-auto">
            <div className="flex min-h-full items-center justify-center p-4">
              <ModalPanel className="flex w-[500px] flex-col items-stretch justify-center gap-4 rounded-3xl bg-gray-800 px-8 pt-16 pb-8">
                <h1 className="text-h1 font-bold tracking-tight text-gray-300">
                  {t('settings.users.createTitle')}
                </h1>
                <p>{t('settings.users.createLead')}</p>
                <CreateUserForm
                  onSuccess={() => setShowCreateUser(false)}
                  onCancel={() => setShowCreateUser(false)}
                />
              </ModalPanel>
            </div>
          </div>
        </Dialog>
      </Transition>
      <SettingsPage
        title={t('settings.users.title')}
        description={
          selectedUser ? (
            <span className="flex flex-row flex-wrap items-baseline gap-x-2">
              <span className="truncate">{selectedUser.username}</span>
              <SettingsLink onClick={() => selectUser(null)}>
                {t('settings.users.allUsers')}
              </SettingsLink>
            </span>
          ) : (
            t('settings.users.description')
          )
        }
      >
        {UserBoxes}
        {openingUser && <PermissionsSkeleton />}
        {selectedUser && (
          <>
            <SettingsSection
              title={t('settings.users.permissions')}
              lead={t('settings.users.permissionsLead', {
                username: selectedUser.username,
              })}
            >
              <SettingsCard>
                {UserPermissions(NormalPermissions, false)}
              </SettingsCard>
            </SettingsSection>
            <SettingsSection
              title={t('settings.users.unsafePermissions')}
              lead={t('settings.users.unsafePermissionsLead')}
            >
              <SettingsCard danger>
                {UserPermissions(
                  UnsafePermissions,
                  globalSettings?.safe_mode || false
                )}
                <SettingsRow
                  label={t('settings.users.ownerOnly')}
                  disabled
                  hint={
                    macrosAvailable
                      ? t('settings.users.ownerOnlyHint')
                      : noScriptingMessage()
                  }
                />
              </SettingsCard>
            </SettingsSection>
            <UserApiKeys user={selectedUser} />
            {canManageUsers && (
              <UserMinecraftLinks
                uid={selectedUser.uid}
                username={selectedUser.username}
              />
            )}
          </>
        )}
      </SettingsPage>
      {/* TODO: your own section */}
    </>
  );
};

export default UserSettings;
