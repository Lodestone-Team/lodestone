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
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import Avatar from 'boring-avatars';
import { Menu, Transition } from '@headlessui/react';
import { InstanceContext } from 'data/InstanceContext';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { CoreInfo, useCoreInfo } from 'data/SystemInfo';
import { AxiosError } from 'axios';

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
  const { token, setToken, core } = useContext(LodestoneContext);
  const { address, port } = core;
  const socket = `${address}:${port}`;
  const { selectInstance } = useContext(InstanceContext);
  const { status: fetchingStatus } = useCoreInfo();

  const statusMap = {
    "loading": "Connecting",
    "error": "Timeout error",
    "success": "Connected"
  }  
  
  const colorMap = {
    "loading": "rgba(174, 139, 50, 0.25)",
    "error": "rgba(174, 50, 50, 0.25)",
    "success": "rgba(97, 174, 50, 0.25)"
  }

  const textMap = {
    "loading": "rgba(239, 180, 64, 1)",
    "error": "rgba(255, 92, 92, 1)",
    "success": "rgba(98, 221, 118, 1)"
  }

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
      <div className="flex flex-row flex-wrap items-center gap-3">
        <p className="font-medium text-base text-center text-gray-400">Ubuntu's Lodestone Core:</p>
        <span className="select-none rounded-full font-bold text-small py-1 px-2 pl-3 pr-3 w-[101px] text-center" style={{backgroundColor: colorMap[fetchingStatus], color: textMap[fetchingStatus]}}>
          {statusMap[fetchingStatus]}
          {/* Timeout error */}
        </span>
      </div>
      <FontAwesomeIcon
        icon={faCog}
        className="w-4 select-none text-white/50 hover:cursor-pointer hover:text-white/75"
        onClick={() => setPathname('/settings')}
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
                    label={userState === 'logged-in' ? 'Logout' : 'Login'}
                    loading={userState === 'loading'}
                    iconRight={faRightFromBracket}
                    onClick={() => {
                      // remove the current token
                      setToken('', socket);
                      if (userState !== 'logged-in')
                        // redirect to login page
                        setPathname('/login/user/select');
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
                    onClick={() => setPathname('/login/core/select')}
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
