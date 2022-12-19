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
import {
  useEffectOnce,
  useIsomorphicLayoutEffect,
  useLocalStorage,
} from 'usehooks-ts';
import jwt from 'jsonwebtoken';
import { errorToString } from 'utils/util';
import {
  NotificationContext,
  useNotificationReducer,
  useOngoingNotificationReducer,
} from 'data/NotificationContext';
import { tauri } from 'utils/tauriUtil';

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

axios.defaults.timeout = 5000;

function MyApp({ Component, pageProps }: AppPropsWithLayout) {
  const getLayout = Component.getLayout ?? ((page) => page);
  const {
    query: address,
    isReady,
    setQuery: setAddress,
  } = useRouterQuery('address', 'localhost');
  const { query: port, setQuery: setPort } = useRouterQuery('port', '16662');
  const { query: protocol, setQuery: setProtocol } = useRouterQuery(
    'protocol',
    'http',
    true,
    false //TODO: make it true when https is supported
  );
  const { query: apiVersion, setQuery: setApiVersion } = useRouterQuery(
    'apiVersion',
    'v1',
    true,
    false //TODO: make it true when there are multiple api versions
  );
  const socket = `${address}:${port}`;
  const [tokens, setTokens] = useLocalStorage<Record<string, string>>(
    'tokens',
    {}
  ); //TODO: clear all outdated tokens
  const token = tokens[socket] ?? '';
  const { notifications, dispatch } = useNotificationReducer();
  const { ongoingNotifications, ongoingDispatch } =
    useOngoingNotificationReducer();

  const setToken = (token: string, coreSocket: string) => {
    setTokens({ ...tokens, [coreSocket]: token });
  };

  useEffectOnce(() => {
    if (tauri) {
      console.log('globalTauri', tauri);
      console.log('globalTauri is defined');
      tauri
        ?.invoke('is_setup')
        .then((isSetup: unknown) => {
          console.log('globalTauri isSetup', isSetup);
        })
        .catch((err: any) => {
          console.log('globalTauri call failed');
        });
    } else {
      console.log('globalTauri is undefined');
    }
  });

  // set axios defaults
  useIsomorphicLayoutEffect(() => {
    if (!isReady) return;
    axios.defaults.baseURL = `${protocol}://${socket}/api/${apiVersion}`;
  }, [socket, isReady]);

  useIsomorphicLayoutEffect(() => {
    if (!token) {
      delete axios.defaults.headers.common['Authorization'];
      dispatch({
        type: 'clear',
      });
      // TODO: clear ongoing notifications as well
    } else {
      try {
        const decoded = jwt.decode(token, { complete: true });
        if (!decoded) throw new Error('Invalid token');
        const { exp } = decoded.payload as { exp: number };
        if (Date.now() >= exp * 1000) throw new Error('Token expired');
        axios.defaults.headers.common['Authorization'] = `Bearer ${token}`;
      } catch (e) {
        const message = errorToString(e);
        alert(message);
        setToken('', socket);
        delete axios.defaults.headers.common['Authorization'];
      }
    }
    queryClient.invalidateQueries();
    queryClient.clear();
  }, [token]);

  return (
    <QueryClientProvider client={queryClient}>
      <LodestoneContext.Provider
        value={{
          address: address as string,
          port: port ?? '16662',
          protocol,
          apiVersion,
          isReady: isReady,
          token,
          setToken,
          tokens,
          socket: socket,
          setAddress,
          setPort,
          setProtocol,
          setApiVersion,
        }}
      >
        <NotificationContext.Provider
          value={{
            notifications,
            dispatch,
            ongoingNotifications,
            ongoingDispatch,
          }}
        >
          {getLayout(<Component {...pageProps} />)}
        </NotificationContext.Provider>
      </LodestoneContext.Provider>
    </QueryClientProvider>
  );
}

export default MyApp;
