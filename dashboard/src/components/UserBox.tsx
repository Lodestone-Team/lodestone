import {
  faArrowRightArrowLeft,
  faEllipsis,
  faTrashCan,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Dialog, Menu, Transition } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from './Atoms/Modal';
import { useQueryClient } from '@tanstack/react-query';
import { PublicUser } from 'bindings/PublicUser';
import clsx from 'clsx';
import { useUid } from 'data/UserInfo';
import { Fragment, useState } from 'react';
import { deleteUser } from 'utils/apis';
import Avatar from './Atoms/Avatar';
import Button from './Atoms/Button';
import ConfirmDialog from './Atoms/ConfirmDialog';
import ChangeSelfPasswordForm from './Settings/ChangeSelfPasswordForm';
import ChangeUserPasswordForm from './Settings/ChangeUserPasswordForm';
import { SettingsTag } from './Settings/SettingsBlocks';
import { useT } from 'i18n';

export default function UserBox({
  className,
  user,
  onClick,
}: {
  className?: string;
  user: PublicUser;
  onClick: () => void;
}) {
  const t = useT();
  const [showChangePassword, setShowChangePassword] = useState(false);
  const [showDeleteUser, setShowDeleteUser] = useState(false);
  const queryClient = useQueryClient();
  const uid = useUid();
  const isSelf = user.uid === uid;

  const userTags = [];
  if (isSelf) userTags.push(t('common.you'));
  if (user.is_admin) userTags.push(t('shell.userBox.admin'));
  if (user.is_owner) userTags.push(t('shell.userBox.owner'));

  return (
    <div
      className={clsx(
        'group relative flex min-h-14 flex-row items-center justify-between',
        'gap-4 px-4 py-3 text-medium',
        className
      )}
    >
      <Transition appear show={showChangePassword} as={Fragment}>
        <Dialog
          onClose={() => setShowChangePassword(false)}
          className="relative z-60"
        >
          <ModalBackdrop />
          <div className="fixed inset-0 overflow-y-auto">
            <div className="flex min-h-full items-center justify-center p-4">
              <ModalPanel className="flex w-[500px] flex-col items-stretch justify-center gap-4 rounded-3xl bg-gray-800 px-8 pt-16 pb-8">
                <h1 className="text-h1 font-bold tracking-tight text-gray-300">
                  {t('shell.userBox.changePassword')}
                </h1>
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
              </ModalPanel>
            </div>
          </div>
        </Dialog>
      </Transition>
      <ConfirmDialog
        title={
          isSelf
            ? t('shell.userBox.cantDeleteSelfTitle')
            : t('shell.userBox.deleteTitle')
        }
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
        confirmButtonText={!isSelf ? t('common.delete') : undefined}
        type="danger"
        isOpen={showDeleteUser}
        onClose={() => setShowDeleteUser(false)}
        closeButtonText={!isSelf ? t('common.cancel') : t('common.ok')}
      >
        {!isSelf
          ? t('shell.userBox.deleteConfirm', { name: user.username })
          : t('shell.userBox.cantDeleteSelf')}
      </ConfirmDialog>
      <div className="flex min-w-0 flex-row items-center gap-3">
        <Avatar size={32} name={user.uid} />
        <div className="flex min-w-0 flex-row flex-wrap items-center gap-x-2 gap-y-1">
          <button
            type="button"
            className="truncate rounded-sm text-left text-medium font-mediumbold text-gray-300 hover:underline focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden"
            onClick={onClick}
          >
            {user.username}
          </button>
          {userTags.map((tag) => (
            <SettingsTag key={tag}>{tag}</SettingsTag>
          ))}
        </div>
      </div>
      <Menu as="div" className="relative inline-block text-right">
        <Menu.Button
          as={FontAwesomeIcon}
          icon={faEllipsis}
          className="h-4 w-4 text-h2 text-white/50 select-none hover:cursor-pointer hover:text-white/75"
        />
        <Menu.Items className="absolute right-0 z-10 mt-0.5 origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-800 drop-shadow-md focus:outline-hidden">
          <div className="px-1.5 py-2">
            <Menu.Item>
              {({ disabled }) => (
                <Button
                  className="w-full flex-nowrap whitespace-nowrap"
                  label={t('shell.userBox.changePassword')}
                  iconRight={faArrowRightArrowLeft}
                  onClick={() => setShowChangePassword(true)}
                  variant="text"
                  align="end"
                  disabled={disabled}
                />
              )}
            </Menu.Item>
            <Menu.Item>
              {({ disabled }) => (
                <Button
                  className="w-full flex-nowrap whitespace-nowrap"
                  label={t('shell.userBox.deleteUser')}
                  iconRight={faTrashCan}
                  variant="text"
                  intention="danger"
                  align="end"
                  disabled={disabled || isSelf}
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
