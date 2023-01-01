import {
  faArrowRightArrowLeft,
  faEllipsis,
  faTrashCan,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Menu } from '@headlessui/react';
import { PublicUser } from 'bindings/PublicUser';
import clsx from 'clsx';
import { useState } from 'react';
import Avatar from './Atoms/Avatar';
import Button from './Atoms/Button';

export default function UserBox({
  className,
  user,
  onClick,
}: {
  className?: string;
  user: PublicUser;
  onClick: () => void;
}) {
  return (
    <div
      className={clsx(
        'group relative flex flex-row items-center justify-between',
        'gap-4 bg-gray-800 px-4 py-3 text-base',
        'hover:bg-gray-700',
        className
      )}
    >
      <div className="flex flex-row items-center gap-4">
        <Avatar size={35} name={user.uid} />
        <div className="flex flex-col">
          <h1 className="text-medium font-medium text-gray-300">
            {user.username}
            {/* this text is bigger then the one in inputbox on purpose */}
          </h1>
          <h2 className="overflow-hidden text-ellipsis text-small font-medium tracking-medium text-white/50">
            {user.is_owner ? 'Owner' : user.uid}
          </h2>
        </div>
      </div>
      <Menu as="div" className="relative inline-block text-right">
        <Menu.Button
          as={FontAwesomeIcon}
          icon={faEllipsis}
          className="h-4 w-4 select-none text-large text-white/50 hover:cursor-pointer hover:text-white/75"
        />
        <Menu.Items className="absolute right-0 z-10 mt-0.5 origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-800 drop-shadow-md focus:outline-none">
          <div className="py-2 px-1.5">
            <Menu.Item>
              {({ active, disabled }) => (
                <Button
                  className="w-full flex-nowrap whitespace-nowrap"
                  label={'Change Password'}
                  iconRight={faArrowRightArrowLeft}
                  onClick={() => {
                    // TODO
                    alert('TODO');
                  }}
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
                  disabled={disabled}
                  active={active}
                  onClick={() => {
                    // TODO
                    alert('TODO');
                  }}
                />
              )}
            </Menu.Item>
          </div>
        </Menu.Items>
      </Menu>
    </div>
  );
}
