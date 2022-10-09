import Button from 'components/Atoms/Button';
import { LodestoneContext } from 'data/LodestoneContext';
import { useUserInfo } from 'data/UserInfo';
import router from 'next/router';
import { useContext, useEffect, useState } from 'react';
import { useCookies } from 'react-cookie';
import { pushKeepQuery } from 'utils/util';

export type UserState = 'loading' | 'logged-in' | 'logged-out';

export default function TopNav() {
  const { isLoading, isError, data: user } = useUserInfo();
  const [userState, setUserState] = useState<UserState>('logged-out');
  const [,, removeCookie] = useCookies(['token']);
  const {token} = useContext(LodestoneContext);

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
    <div className="flex flex-row items-center justify-end w-full h-16 gap-2 p-2 bg-gray-700 border-b border-gray-faded/30">
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
          // remove token cookie
          removeCookie('token');
          if (userState !== 'logged-in')
            // redirect to login page
            pushKeepQuery(router, '/auth');
        }}
      />
    </div>
  );
}
