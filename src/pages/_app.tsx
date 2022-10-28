import 'rc-tooltip/assets/bootstrap.css';
import 'globals.css';
import type { AppProps } from 'next/app';
import { config } from '@fortawesome/fontawesome-svg-core';
import '@fortawesome/fontawesome-svg-core/styles.css';
import { QueryClientProvider, QueryClient } from '@tanstack/react-query';
import { ReactElement, ReactNode, useState } from 'react';
import { NextPage } from 'next';
import { LodestoneContext } from 'data/LodestoneContext';
import axios from 'axios';
import { useRouterQuery } from 'utils/hooks';
import { useIsomorphicLayoutEffect, useLocalStorage } from 'usehooks-ts';
import jwt from 'jsonwebtoken';
import { errorToMessage } from 'utils/util';
import { useClientInfo } from 'data/SystemInfo';

config.autoAddCss = false;

// eslint-disable-next-line @typescript-eslint/ban-types
export type NextPageWithLayout<P = {}, IP = P> = NextPage<P, IP> & {
  getLayout?: (page: ReactElement) => ReactNode;
};

type AppPropsWithLayout = AppProps & {
  Component: NextPageWithLayout;
};

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: Infinity,
      refetchOnWindowFocus: false,
    },
  },
});

axios.defaults.timeout = 2000;

function MyApp({ Component, pageProps }: AppPropsWithLayout) {
  const getLayout = Component.getLayout ?? ((page) => page);
  const { query: address, isReady } = useRouterQuery('address');
  const { query: port } = useRouterQuery('port');
  const [token, setToken] = useLocalStorage<string>('token', '');

  const protocol = 'http';
  const apiVersion = 'v1';
  const apiAddress = address ?? 'localhost';

  // set axios defaults
  useIsomorphicLayoutEffect(() => {
    if (!isReady) return;
    axios.defaults.baseURL = `${protocol}://${apiAddress}:${
      port ?? 3000
    }/api/${apiVersion}`;
  }, [apiAddress, port, isReady]);

  useIsomorphicLayoutEffect(() => {
    if (!token) {
      delete axios.defaults.headers.common['Authorization'];
      return;
    }
    try {
      const decoded = jwt.decode(token, {complete: true});
      if (!decoded) throw new Error('Invalid token');
      const { exp } = decoded.payload as { exp: number };
      if (Date.now() >= exp * 1000) throw new Error('Token expired');
      axios.defaults.headers.common['Authorization'] = `Bearer ${token}`;
    } catch (e) {
      const message = errorToMessage(e);
      alert(message);
      setToken('');
      delete axios.defaults.headers.common['Authorization'];
    }
    queryClient.invalidateQueries();
  }, [token]);

  return (
    <QueryClientProvider client={queryClient}>
      <LodestoneContext.Provider
        value={{
          address: apiAddress as string,
          port: port ?? '3000',
          protocol,
          apiVersion,
          isReady: isReady,
          token,
          setToken,
        }}
      >
        {getLayout(<Component {...pageProps} />)}
      </LodestoneContext.Provider>
    </QueryClientProvider>
  );
}

export default MyApp;
