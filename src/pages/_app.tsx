import 'rc-tooltip/assets/bootstrap.css';
import 'globals.css';
import type { AppProps } from 'next/app';
import { config } from '@fortawesome/fontawesome-svg-core';
import '@fortawesome/fontawesome-svg-core/styles.css';
import { QueryClientProvider, QueryClient } from '@tanstack/react-query';
import { ReactElement, ReactNode, useEffect, useMemo } from 'react';
import { NextPage } from 'next';
import { CoreConnectionInfo, LodestoneContext } from 'data/LodestoneContext';
import axios from 'axios';
import { useRouterQuery } from 'utils/hooks';
import {
  useEffectOnce,
  useIsomorphicLayoutEffect,
  useLocalStorage,
} from 'usehooks-ts';
import jwt from 'jsonwebtoken';
import { errorToString, LODESTONE_PORT } from 'utils/util';
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
    query: coreQuery,
    isReady,
    setQuery: setCoreQuery,
  } = useRouterQuery('core', {
    address: 'localhost',
    port: LODESTONE_PORT.toString(),
    protocol: 'http',
    apiVersion: 'v1',
  });
  const core: CoreConnectionInfo = useMemo(
    () => ({
      address: coreQuery.address ?? 'localhost',
      port: coreQuery.port ?? LODESTONE_PORT.toString(),
      protocol: coreQuery.protocol ?? 'http',
      apiVersion: coreQuery.apiVersion ?? 'v1',
    }),
    [coreQuery]
  );
  const { address, port, protocol, apiVersion } = core;

  const [coreList, setCoreList] = useLocalStorage<CoreConnectionInfo[]>(
    'cores',
    []
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

  const setCore = (core: CoreConnectionInfo, pathname?: string) => {
    queryClient.invalidateQueries();
    //TODO: add core to the key of each query instead of invalidating all queries
    setCoreQuery(
      {
        address: core.address,
        port: core.port + '',
        protocol: core.protocol,
        apiVersion: core.apiVersion,
      },
      pathname
    );
  };

  useEffect(() => {
    // check if core is already in the list
    // if it's exactly the same, do nothing
    if (
      coreList.some(
        (c) =>
          c.address === core.address &&
          c.port === core.port &&
          c.protocol === core.protocol &&
          c.apiVersion === core.apiVersion
      )
    )
      return;
    const index = coreList.findIndex(
      (c) => c.address === core.address && c.port === core.port
    );
    if (index !== -1) {
      const newCoreList = [...coreList];
      newCoreList[index] = core;
      setCoreList(newCoreList);
    } else {
      setCoreList([...coreList, core]);
    }

    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [core, setCoreList]); //setCoreList left out on purpose

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
    axios.defaults.baseURL = `${protocol}://${socket}/api/${apiVersion}`;
  }, [core, isReady]);

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
          core,
          setCore,
          coreList,
          isReady,
          token,
          setToken,
          tokens,
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
