import { Dialog, Transition } from '@headlessui/react';
import Button from 'components/Atoms/Button';
import UserBox from 'components/UserBox';
import { SettingsContext } from 'data/SettingsContext';
import { useUid, useUserInfo } from 'data/UserInfo';
import { Fragment, useContext, useState } from 'react';
import CreateUserForm from './CreateUserForm';

export const UserSettings = () => {
  const { userList, selectUser, selectedUser } = useContext(SettingsContext);
  const [showCreateUser, setShowCreateUser] = useState(false);
  const { data: userInfo } = useUserInfo();
  const canManageUsers = userInfo?.is_owner || false;
  const uid = useUid();

  // TODO: add non-owner view (can't manage users, can only change their own password)
  return (
    <div className="flex w-full flex-col gap-4 @4xl:flex-row">
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
                <h1 className="text-larger font-bold tracking-tight text-gray-300">
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
      <div className="flex w-full flex-row flex-nowrap items-end justify-between gap-4 @4xl:w-[28rem] @4xl:flex-col @4xl:items-start @4xl:justify-start">
        <div>
          <h1 className="text-large font-black">
            All Members ({Object.keys(userList).length})
          </h1>
          <h2 className="text-base font-medium italic tracking-tight text-white/50">
            A list of all users. Click into a user to manage.
          </h2>
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
};

export default UserSettings;
