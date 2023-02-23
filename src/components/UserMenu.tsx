import React, { Fragment, useContext, useState, useEffect } from 'react';
import { useUid, useUserInfo } from 'data/UserInfo';
import { LodestoneContext } from 'data/LodestoneContext';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { Menu, Transition } from '@headlessui/react';
import Button from 'components/Atoms/Button';
import Avatar from 'components/Atoms/Avatar';
import {
  faCaretDown,
  faArrowRightArrowLeft,
  faRightFromBracket,
  faUser,
  faCog,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import IconButton from './Atoms/IconButton';
import { useLocation } from 'react-router-dom';

export type UserState = 'loading' | 'logged-in' | 'logged-out';

const UserMenu = () => {
  const { token, setToken, core } = useContext(LodestoneContext);
  const { setPathname, setSearchParam } = useContext(BrowserLocationContext);
  const location = useLocation();
  const uid = useUid();
  const { isLoading, isError, data: user } = useUserInfo();
  const [userState, setUserState] = useState<UserState>('logged-out');
  const { address, port } = core;
  const socket = `${address}:${port}`;

  useEffect(() => {
    if (!token) {
      setUserState('logged-out');
    } else if (isLoading) {
      setUserState('loading');
      return;
    } else if (isError) {
      setUserState('logged-out');
      return;
    } else {
      setUserState('logged-in');
    }
  }, [token, isLoading, isError, user]);

  return (
    <div className="mb-6 flex w-full gap-2 px-2">
      <Menu as="div" className="relative inline-block w-full min-w-0 text-left">
        <Menu.Button
          as={Button}
          align="start"
          labelGrow={true}
          className="w-full"
          loading={userState === 'loading'}
          label={
            userState === 'logged-in' && user ? `${user.username}` : 'Guest'
          }
          iconComponent={
            userState == 'logged-in' ? (
              <Avatar name={uid} />
            ) : (
              <FontAwesomeIcon icon={faUser} className="w-4 opacity-50" />
            )
          }
          iconRight={faCaretDown}
        ></Menu.Button>
        <Transition
          as={Fragment}
          enter="transition ease-out duration-200"
          enterFrom="opacity-0 -translate-y-1"
          enterTo="opacity-100 translate-y-0"
          leave="transition ease-in duration-150"
          leaveFrom="opacity-100 translate-y-0"
          leaveTo="opacity-0 -translate-y-1"
        >
          <Menu.Items className="absolute right-0 z-10 mt-1.5 w-full origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-800 drop-shadow-md focus:outline-none">
            <div className="py-2 px-1.5">
              <Menu.Item>
                {({ disabled }) => (
                  <Button
                    className="w-full whitespace-nowrap"
                    label={userState === 'logged-in' ? 'Sign out' : 'Sign in'}
                    loading={userState === 'loading'}
                    iconRight={faRightFromBracket}
                    onClick={() => {
                      // remove the current token
                      // a logged out user will be auto-redirected to the login page
                      setToken('', socket);
                      setSearchParam('instance', undefined);
                      setSearchParam('user', undefined);
                    }}
                    align="end"
                    disabled={disabled}
                    variant="text"
                  />
                )}
              </Menu.Item>

              <Menu.Item>
                {({ disabled }) => (
                  <Button
                    className="w-full flex-nowrap whitespace-nowrap"
                    label="Change core"
                    iconRight={faArrowRightArrowLeft}
                    align="end"
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
      <IconButton
        icon={faCog}
        onClick={() => {
          localStorage.setItem('lastVisitedRoute', location.pathname);
          setPathname('/settings');
        }}
      />
    </div>
  );
};

export default UserMenu;
