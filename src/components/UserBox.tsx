import {
  faArrowRightArrowLeft,
  faEllipsis,
  faTrashCan,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Dialog, Menu, Transition } from '@headlessui/react';
import { useQueryClient } from '@tanstack/react-query';
import { PublicUser } from 'bindings/PublicUser';
import clsx from 'clsx';
import { useUid } from 'data/UserInfo';
import { Fragment, useState } from 'react';
import { deleteUser } from 'utils/util';
import Avatar from './Atoms/Avatar';
import Button from './Atoms/Button';
import ConfirmDialog from './Atoms/ConfirmDialog';
import ChangeSelfPasswordForm from './Settings/ChangeSelfPasswordForm';
import ChangeUserPasswordForm from './Settings/ChangeUserPasswordForm';

export default function UserBox({
  className,
  user,
  onClick,
}: {
  className?: string;
  user: PublicUser;
  onClick: () => void;
}) {
  const [showChangePassword, setShowChangePassword] = useState(false);
  const [showDeleteUser, setShowDeleteUser] = useState(false);
  const queryClient = useQueryClient();
  const uid = useUid();
  const isSelf = user.uid === uid;

  const userTags = [];
  if (isSelf) userTags.push('You');
  if (user.is_admin) userTags.push('Admin');
  if (user.is_owner) userTags.push('Owner');

  return (
    <div
      className={clsx(
        'group relative flex cursor-pointer flex-row items-center justify-between',
        'gap-4 bg-gray-800 px-4 py-3 text-h3',
        'hover:bg-gray-700',
        className
      )}
      onClick={onClick}
    >
      <Transition
        appear
        show={showChangePassword}
        as={Fragment}
        enter="ease-out duration-200"
        enterFrom="opacity-0"
        enterTo="opacity-100"
        leave="ease-in duration-150"
        leaveFrom="opacity-100"
        leaveTo="opacity-0"
      >
        <Dialog
          onClose={() => setShowChangePassword(false)}
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
                {isSelf ? (
                  <ChangeSelfPasswordForm
                    onSuccess={() => setShowChangePassword(false)}
                    onCancel={() => setShowChangePassword(false)}
                  />
                ) : (
                  <ChangeUserPasswordForm
                    uid={user.uid}
                    onSuccess={() => setShowChangePassword(false)}
                    onCancel={() => setShowChangePassword(false)}
                  />
                )}
              </Dialog.Panel>
            </div>
          </div>
        </Dialog>
      </Transition>
      <ConfirmDialog
        title={isSelf ? "You can't delete yourself" : 'Delete User'}
        onConfirm={
          !isSelf
            ? () => {
                deleteUser(user.uid);
                setShowDeleteUser(false);
                queryClient.setQueryData(
                  ['user', 'list'],
                  (users: { [uid: string]: PublicUser } | undefined) =>
                    users
                      ? Object.fromEntries(
                          Object.entries(users).filter(
                            ([uid]) => uid !== user.uid
                          )
                        )
                      : undefined
                );
              }
            : undefined
        }
        confirmButtonText={!isSelf ? 'Delete' : undefined}
        type="danger"
        isOpen={showDeleteUser}
        onClose={() => setShowDeleteUser(false)}
        closeButtonText={!isSelf ? 'Cancel' : 'Ok'}
      >
        {!isSelf
          ? `Are you sure you want to delete ${user.username}? This action cannot be
        undone.`
          : `You can't delete yourself.`}
      </ConfirmDialog>
      <div className="flex min-w-0 flex-row items-center gap-4">
        <Avatar size={35} name={user.uid} />
        <div className="flex min-w-0 flex-col">
          <h1 className="truncate text-h3 font-bold leading-tight text-gray-300">
            {user.username}
            {/* this text is bigger then the one in inputbox on purpose */}
          </h1>
          <h2 className="overflow-hidden truncate text-ellipsis text-small font-medium tracking-medium text-white/50">
            {userTags.join(', ')}
          </h2>
        </div>
      </div>
      <Menu as="div" className="relative inline-block text-right">
        <Menu.Button
          as={FontAwesomeIcon}
          icon={faEllipsis}
          className="h-4 w-4 select-none text-h2 text-white/50 hover:cursor-pointer hover:text-white/75"
        />
        <Menu.Items className="absolute right-0 z-10 mt-0.5 origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-800 drop-shadow-md focus:outline-none">
          <div className="py-2 px-1.5">
            <Menu.Item>
              {({ active, disabled }) => (
                <Button
                  className="w-full flex-nowrap whitespace-nowrap"
                  label={'Change Password'}
                  iconRight={faArrowRightArrowLeft}
                  onClick={() => setShowChangePassword(true)}
                  variant="text"
                  align="end"
                  disabled={disabled}
                  active={active}
                />
              )}
            </Menu.Item>
            <Menu.Item>
              {({ active, disabled }) => (
                <Button
                  className="w-full flex-nowrap whitespace-nowrap"
                  label="Delete User"
                  iconRight={faTrashCan}
                  variant="text"
                  color="danger"
                  align="end"
                  disabled={disabled || isSelf}
                  active={active}
                  onClick={() => setShowDeleteUser(true)}
                />
              )}
            </Menu.Item>
          </div>
        </Menu.Items>
      </Menu>
    </div>
  );
}
