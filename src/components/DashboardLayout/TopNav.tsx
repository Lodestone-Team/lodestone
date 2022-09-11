import axios from 'axios';
import Button from 'components/Button';
import router from 'next/router';
import { useEffect, useState } from 'react';
import { useCookies } from 'react-cookie';
import { pushKeepQuery } from 'utils/util';

export default function TopNav() {
  const [cookies, setCookie] = useCookies(['token']);
  const [loggedIn, setLoggedIn] = useState(false);

  useEffect(() => {
    if (cookies.token) {
      setLoggedIn(true);
    }
  }, [cookies.token]);
  
  return (
    <div className="flex flex-row items-center justify-end w-full h-16 p-2 bg-gray-700 border-b border-gray-500">
      <Button
        label={loggedIn ? 'Logout' : 'Login'}
        className="h-fit"
        onClick={() => {
          // remove token cookie
          setCookie('token', '');
          // remove default auth header
          delete axios.defaults.headers.common['Authorization'];
          // redirect to login page
          pushKeepQuery(router, '/auth');
        }}
      />
    </div>
  );
}
