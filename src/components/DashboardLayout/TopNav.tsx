import Button from 'components/Atoms/Button';
import { LodestoneContext } from 'data/LodestoneContext';
import { useUserInfo } from 'data/UserInfo';
import router from 'next/router';
import { useContext, useEffect, useState } from 'react';
import { pushKeepQuery } from 'utils/util';
import { faBell, faSpinner } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';

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
    <div className="flex h-12 w-full flex-row items-center justify-end gap-4 border-b border-gray-faded/30 bg-gray-800 px-4 py-2">
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
      <div className="flex flex-row items-center gap-2">
        <p className="font-medium text-gray-300">
          {userState === 'logged-in' && user
            ? `Hi, ${user.username}`
            : userState === 'loading'
            ? 'Loading...'
            : 'Not logged in'}
        </p>
        <Button
          label={userState === 'logged-in' ? 'Logout' : 'Login'}
          loading={userState === 'loading'}
          onClick={() => {
            // remove the current token
            setToken('');
            if (userState !== 'logged-in')
              // redirect to login page
              pushKeepQuery(router, '/auth');
          }}
        />
      </div>
      {/* <FontAwesomeIcon
        icon={faSpinner}
        className={`w-4 select-none hover:cursor-pointer ${
          true
            ? 'text-white hover:text-white/75'
            : 'text-white/50 hover:text-white/75'
        }`}
      /> */}
      <FontAwesomeIcon
        icon={faBell}
        className={`w-4 select-none hover:cursor-pointer ${
          showNotifications
            ? 'text-white hover:text-white/75'
            : 'text-white/50 hover:text-white/75'
        }`}
        onClick={() => {
          setShowNotifications(!showNotifications);
        }}
      />
    </div>
  );
}
