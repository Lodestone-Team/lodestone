import React, { Fragment, useContext } from 'react';
import clsx from 'clsx';
import { useUid } from 'data/UserInfo';
import { LodestoneContext } from 'data/LodestoneContext';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { Menu, Transition } from '@headlessui/react';
import Button from 'components/Atoms/Button';
import Avatar from 'components/Atoms/Avatar';
import {
  faCaretUp,
  faArrowRightArrowLeft,
  faRightFromBracket,
  faUser,
  faCog,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import IconButton from './Atoms/IconButton';
import { useLocation } from 'react-router';
import { useUserState } from './useUserState';
import { Skeleton } from 'components/Atoms/Skeleton';
import { rememberSettingsReturn } from 'utils/settingsReturn';
import { useI18n } from 'i18n';

/**
 * The signed-in user with the account menu (sign out, change core), and the settings
 * button. It sits at the bottom of the sidebar footer, under the core connection
 * status, so the menu opens upward.
 */
const UserMenu = ({ showSettings = true }: { showSettings?: boolean }) => {
  const { t, rich } = useI18n();
  const { setToken, core } = useContext(LodestoneContext);
  const { setPathname, setSearchParam } = useContext(BrowserLocationContext);
  const location = useLocation();
  const uid = useUid();
  const { userState, user } = useUserState();
  const { address, port } = core;
  const socket = `${address}:${port}`;
  const loggedIn = userState === 'logged-in' && user;
  const accountLabel = loggedIn ? user.username : t('shell.userMenu.guest');

  return (
    <div className="flex w-full flex-row items-center gap-1">
      <Menu as="div" className="relative min-w-0 flex-1">
        <Menu.Button
          title={accountLabel}
          aria-label={t('shell.userMenu.account', { name: accountLabel })}
          className={clsx(
            'flex w-full flex-row items-center gap-2 rounded-md px-2 py-1.5',
            'text-small font-medium',
            'text-white/50 hover:bg-gray-800 hover:text-white/75',
            'focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden'
          )}
        >
          {userState === 'loading' ? (
            // the avatar and the name, while the user loads
            <Skeleton className="size-5 shrink-0 rounded-full" />
          ) : loggedIn ? (
            <Avatar name={uid} />
          ) : (
            <FontAwesomeIcon icon={faUser} className="w-4 opacity-50" />
          )}
          <span className="min-w-0 flex-1 truncate text-left">
            {/* not "Guest" while the user loads, that flashed before the name */}
            {userState === 'loading' ? (
              <Skeleton className="inline-block h-3 w-20 rounded-sm align-middle" />
            ) : (
              accountLabel
            )}
          </span>
          <FontAwesomeIcon icon={faCaretUp} className="w-2.5 shrink-0" />
        </Menu.Button>
        <Transition
          as={Fragment}
          enter="transition ease-out duration-200"
          enterFrom="opacity-0 translate-y-1"
          enterTo="opacity-100 translate-y-0"
          leave="transition ease-in duration-150"
          leaveFrom="opacity-100 translate-y-0"
          leaveTo="opacity-0 translate-y-1"
        >
          <Menu.Items className="absolute bottom-full left-0 z-10 mb-1.5 w-full min-w-48 origin-bottom-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-800 drop-shadow-md focus:outline-hidden">
            <div className="truncate px-3 py-2 text-small font-medium text-white/50">
              {loggedIn
                ? rich('shell.userMenu.signedInAs', {
                    name: (
                      <span className="text-gray-300">{user.username}</span>
                    ),
                  })
                : t('shell.userMenu.notSignedIn')}
            </div>
            <div className="px-1.5 py-2">
              <Menu.Item>
                {({ disabled }) => (
                  <Button
                    className="w-full whitespace-nowrap"
                    label={
                      userState === 'logged-in'
                        ? t('shell.userMenu.signOut')
                        : t('shell.userMenu.signIn')
                    }
                    loading={userState === 'loading'}
                    iconRight={faRightFromBracket}
                    onClick={() => {
                      // remove the current token
                      // a logged out user will be auto-redirected to the login page
                      setToken('', socket);
                      setSearchParam('instance', undefined);
                      setSearchParam('user', undefined);
                    }}
                    align="between"
                    disabled={disabled}
                    variant="text"
                  />
                )}
              </Menu.Item>

              <Menu.Item>
                {({ disabled }) => (
                  <Button
                    className="w-full flex-nowrap whitespace-nowrap"
                    label={t('shell.userMenu.changeCore')}
                    iconRight={faArrowRightArrowLeft}
                    align="between"
                    disabled={disabled}
                    onClick={() => {
                      setSearchParam('instance', undefined);
                      setSearchParam('user', undefined);
                      setPathname('/login/core/select');
                    }}
                    variant="text"
                  />
                )}
              </Menu.Item>
            </div>
          </Menu.Items>
        </Transition>
      </Menu>
      {showSettings && (
        <IconButton
          icon={faCog}
          aria-label={t('shell.userMenu.settings')}
          title={t('shell.userMenu.settings')}
          onClick={() => {
            rememberSettingsReturn(location.pathname);
            setPathname('/settings/general');
          }}
        />
      )}
    </div>
  );
};

export default UserMenu;
