import Button from 'components/Atoms/Button';
import { LodestoneContext } from 'data/LodestoneContext';
import { useUserInfo } from 'data/UserInfo';
import { Fragment, useContext, useEffect, useState } from 'react';
import {
  faCaretDown,
  faArrowRightArrowLeft,
  faBell,
  faCog,
  faRightFromBracket,
  faUser,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Menu, Transition } from '@headlessui/react';
import { InstanceContext } from 'data/InstanceContext';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { CoreInfo, useCoreInfo } from 'data/SystemInfo';
import { AxiosError } from 'axios';
import Label, { LabelColor } from 'components/Atoms/Label';
import Avatar from 'components/Atoms/Avatar';

export type UserState = 'loading' | 'logged-in' | 'logged-out';

export default function TopNav({
  showNotifications,
  setShowNotifications,
}: {
  showNotifications: boolean;
  setShowNotifications: (show: boolean) => void;
}) {
  const { setPathname } = useContext(BrowserLocationContext);
  const { isLoading, isError, data: user } = useUserInfo();
  const [userState, setUserState] = useState<UserState>('logged-out');
  const { token, setToken, core, coreConnectionStatus } =
    useContext(LodestoneContext);
  const { address, port } = core;
  const socket = `${address}:${port}`;
  const { selectInstance } = useContext(InstanceContext);
  const { data: coreData } = useCoreInfo();

  const statusMap = {
    loading: 'Connecting',
    error: 'Error',
    success: 'Connected',
  };

  const colorMap: Record<string, LabelColor> = {
    loading: 'yellow',
    error: 'red',
    success: 'green',
  };

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
            selectInstance(undefined);
          }}
        />
      </div>
      <div className="flex flex-row flex-wrap items-baseline gap-1">
        <p className="text-center text-base font-medium text-white/50">
          {coreData?.core_name ?? '...'}:
        </p>
        <Label
          size="small"
          color={colorMap[coreConnectionStatus]}
          className="w-16 text-center"
        >
          {statusMap[coreConnectionStatus]}
        </Label>
      </div>
      <FontAwesomeIcon
        icon={faCog}
        className="w-4 select-none text-white/50 hover:cursor-pointer hover:text-white/75"
        onClick={() => {
          selectInstance(undefined);
          setPathname('/settings');
        }}
      />
      <FontAwesomeIcon
        icon={faBell}
        className={`w-4 select-none hover:cursor-pointer ${
          showNotifications
            ? 'text-gray-300 hover:text-white/75'
            : 'text-white/50 hover:text-white/75'
        }`}
        onClick={() => setShowNotifications(!showNotifications)}
      />
      <Menu as="div" className="relative inline-block text-left">
        <Menu.Button
          as={Button}
          loading={userState === 'loading'}
          label={
            userState === 'logged-in' && user ? `Hi, ${user.username}` : 'Guest'
          }
          iconComponent={
            userState == 'logged-in' ? (
              <Avatar name={user?.uid} />
            ) : (
              <FontAwesomeIcon icon={faUser} className="w-4 opacity-50" />
            )
          }
          iconRight={faCaretDown}
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
                    label={userState === 'logged-in' ? 'Sign out' : 'Sign in'}
                    loading={userState === 'loading'}
                    iconRight={faRightFromBracket}
                    onClick={() => {
                      // remove the current token
                      setToken('', socket);
                      selectInstance(undefined);
                      // if (userState !== 'logged-in') {
                      //   // redirect to login page
                      //   setPathname('/login/user');
                      // }
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
                    label="Change core"
                    iconRight={faArrowRightArrowLeft}
                    variant="text"
                    align="end"
                    disabled={disabled}
                    active={active}
                    onClick={() => {
                      selectInstance(undefined);
                      setPathname('/login/core/select');
                    }}
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
