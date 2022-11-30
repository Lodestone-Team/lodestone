import Button from 'components/Atoms/Button';
import { LodestoneContext } from 'data/LodestoneContext';
import { useUserInfo } from 'data/UserInfo';
import router from 'next/router';
import { Fragment, useContext, useEffect, useState } from 'react';
import { pushKeepQuery } from 'utils/util';
import {
  faAngleDown,
  faArrowRightArrowLeft,
  faBell,
  faCog,
  faRightFromBracket,
  faSpinner,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import Avatar from 'boring-avatars';
import { Menu, Transition } from '@headlessui/react';

export type UserState = 'loading' | 'logged-in' | 'logged-out';

export default function TopNav({
  showNotifications,
  setShowNotifications,
}: {
  showNotifications: boolean;
  setShowNotifications: (show: boolean) => void;
}) {
  const { isLoading, isError, data: user } = useUserInfo();
  const [userState, setUserState] = useState<UserState>('logged-out');
  const { token, setToken } = useContext(LodestoneContext);

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
    <div className="flex w-full shrink-0 flex-row items-center justify-end gap-4 border-b border-gray-faded/30 bg-gray-800 px-4 py-2">
      <div className="grow">
        <img
          src="/logo.svg"
          alt="logo"
          className="w-32 hover:cursor-pointer"
          onClick={() => {
            router.push(
              {
                pathname: '/',
                query: {
                  ...router.query,
                  uuid: undefined,
                },
              },
              undefined,
              { shallow: true }
            );
          }}
        />
      </div>
      <FontAwesomeIcon
        icon={faCog}
        className="w-4 select-none text-gray-faded/50 hover:cursor-pointer hover:text-white/75"
        onClick={() => {
          router.push(
            {
              pathname: '/settings/',
              query: {
                ...router.query,
                uuid: undefined,
              },
            },
            undefined,
            { shallow: true }
          );
        }}
      />
      <FontAwesomeIcon
        icon={faBell}
        className={`w-4 select-none hover:cursor-pointer ${
          showNotifications
            ? 'text-white hover:text-white/75'
            : 'text-gray-faded/50 hover:text-white/75'
        }`}
        onClick={() => {
          setShowNotifications(!showNotifications);
        }}
      />
      <Menu as="div" className="relative inline-block text-left">
        <Menu.Button
          as={Button}
          label={
            userState === 'logged-in' && user
              ? `Hi, ${user.username}`
              : userState === 'loading'
              ? 'Loading...'
              : 'Not logged in'
          }
          iconComponent={
            userState == 'logged-in' && (
              <Avatar
                size={20}
                name={user?.uid}
                variant="beam"
                colors={['#62DD76', '#1D8EB2', '#EFB440', '#DD6262', '#dd62c6']}
              />
            )
          }
          iconRight={faAngleDown}
          className="font-medium text-gray-300"
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
          <Menu.Items className="absolute right-0 z-10 mt-1.5 origin-top-left divide-y divide-gray-faded/30 rounded border border-gray-faded/30 bg-gray-800 drop-shadow-md focus:outline-none">
            <div className="py-2 px-1.5">
              <Menu.Item>
                {({ active, disabled }) => (
                  <Button
                    className="w-full flex-nowrap whitespace-nowrap"
                    label={userState === 'logged-in' ? 'Logout' : 'Login'}
                    loading={userState === 'loading'}
                    iconRight={faRightFromBracket}
                    onClick={() => {
                      // remove the current token
                      setToken('');
                      if (userState !== 'logged-in')
                        // redirect to login page
                        pushKeepQuery(router, '/auth');
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
                    label="Change node"
                    iconRight={faArrowRightArrowLeft}
                    variant="text"
                    align="end"
                    disabled={disabled}
                    active={active}
                  />
                )}
              </Menu.Item>
            </div>
          </Menu.Items>
        </Transition>
      </Menu>
    </div>
  );
}
