import axios from 'axios';
import Button from 'components/Button';
import DashboardLayout from 'components/DashboardLayout';
import Head from 'next/head';
import Image from 'next/image';
import { useRouter } from 'next/router';
import { ReactElement, ReactNode, useEffect, useState } from 'react';
import { pushKeepQuery } from 'utils/util';
import { NextPageWithLayout } from './_app';
import { useQueryClient } from '@tanstack/react-query';
import { useCookies } from 'react-cookie';

const Auth: NextPageWithLayout = () => {
  const router = useRouter();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [buttonLoading, setButtonLoading] = useState(false);
  const queryClient = useQueryClient();
  const [cookies, setCookie] = useCookies(['token']);

  useEffect(() => {
    if (cookies.token) {
      axios.defaults.headers.common['Authorization'] = `Bearer ${cookies.token}`;
      queryClient.invalidateQueries();
      alert("Logged in!");
      pushKeepQuery(router, '/');
    }
  }, [cookies.token]);

  const onSubmit = (event: React.SyntheticEvent) => {
    event.preventDefault();
    setButtonLoading(true);

    // login using basic auth
    axios
      .get('/users/login', {
        auth: {
          username,
          password,
        },
      })
      .then((response) => {
        // set the token cookie
        setCookie('token', response.data, {
          maxAge: 60 * 60 * 24 * 7, // 1 week
          path: '/',
        });
      })
      .catch((error) => {
        alert('Login failed');
      })
      .finally(() => {
        setButtonLoading(false);
      });
  };

  return (
    <div className="px-8 py-10 bg-gray-800 grow">
      <h1 className="font-semibold tracking-tight text-gray-300 text-2xlarge font-heading">
        Login
      </h1>
      <form noValidate autoComplete="off" onSubmit={onSubmit}>
        <div className="flex flex-col gap-y-2 w-52">
          <label htmlFor="username">Username:</label>
          <input
            type="text"
            id="username"
            name="username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
          />
          <label htmlFor="password">Password:</label>
          <input
            type="password"
            id="password"
            name="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          <Button label="Login" type="submit" disabled={buttonLoading} />
        </div>
      </form>
    </div>
  );
};

export default Auth;
