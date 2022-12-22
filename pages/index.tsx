import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { CoreConnectionInfo, LodestoneContext } from 'data/LodestoneContext';
import {
  NotificationContext,
  useNotificationReducer,
  useOngoingNotificationReducer,
} from 'data/NotificationContext';
import React, { useEffect, useLayoutEffect, useMemo } from 'react';
import { Routes, Route } from 'react-router-dom';
import { useEffectOnce, useLocalStorage } from 'usehooks-ts';
import { useLocalStorageQueryParam } from 'utils/hooks';
import { errorToString, LODESTONE_PORT } from 'utils/util';
import Dashboard from 'pages/dashboard';
import Home from 'pages/home';
import { tauri } from 'utils/tauriUtil';
import axios from 'axios';
import jwt from 'jsonwebtoken';
import DashboardLayout from 'components/DashboardLayout';
import SettingsPage from 'pages/settings';
import SelectCorePage from 'pages/login/SelectCorePage';
import ConnectNewCorePage from 'pages/login/ConnectNewCorePage';
import SelectUserPage from 'pages/login/SelectUserPage';
import LoginNewUserPage from 'pages/login/LoginNewUserPage';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: Infinity,
      refetchOnWindowFocus: false,
    },
  },
});

export default function App() {
  /* Start Core */
  const [address, setAddress] = useLocalStorageQueryParam('address', 'localhost');
  const [port, setPort] = useLocalStorageQueryParam('port', LODESTONE_PORT.toString());
  const [protocol, setProtocol] = useLocalStorageQueryParam('protocol', 'http');
  const [apiVersion, setApiVersion] = useLocalStorageQueryParam('apiVersion', 'v1');
  const core: CoreConnectionInfo = useMemo(
    () => ({
      address,
      port,
      protocol,
      apiVersion,
    }),
    [address, port, protocol, apiVersion]
  );
  const socket = useMemo(() => `${address}:${port}`, [address, port]);
  const [coreList, setCoreList] = useLocalStorage<CoreConnectionInfo[]>(
    'cores',
    []
  );
  const setCore = (c: CoreConnectionInfo) => {
    queryClient.invalidateQueries();
    queryClient.clear();
    //TODO: add core to the key of each query instead of invalidating all queries
    setAddress(c.address);
    setPort(c.port.toString());
    setProtocol(c.protocol);
    setApiVersion(c.apiVersion);
  };
  useEffect(() => {
    // check if core is already in the list
    // if it's exactly the same, do nothing
    if (
      coreList.some(
        (c) =>
          c.address === address &&
          c.port === port &&
          c.protocol === protocol &&
          c.apiVersion === apiVersion
      )
    )
      return;
    const index = coreList.findIndex(
      (c) => c.address === address && c.port === port
    );
    if (index !== -1) {
      const newCoreList = [...coreList];
      newCoreList[index] = core;
      setCoreList(newCoreList);
    } else {
      setCoreList([...coreList, core]);
    }

    // core and corelist left out on purpose
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [address, apiVersion, core, port, protocol]);
  useLayoutEffect(() => {
    axios.defaults.baseURL = `${protocol}://${socket}/api/${apiVersion}`;
  }, [apiVersion, protocol, socket]);
  /* End Core */

  /* Start Token */
  const [tokens, setTokens] = useLocalStorage<Record<string, string>>(
    'tokens',
    {}
  ); //TODO: clear all outdated tokens
  const token = tokens[socket] ?? '';
  const setToken = (token: string, coreSocket: string) => {
    setTokens({ ...tokens, [coreSocket]: token });
  };
  useLayoutEffect(() => {
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

    // only token in the dependency list
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token]);
  /* End Token */

  /* Start Notifications */
  const { notifications, dispatch } = useNotificationReducer();
  const { ongoingNotifications, ongoingDispatch } =
    useOngoingNotificationReducer();
  /* End Notifications */

  /* Start Tauri */
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
  /* End Tauri */

  return (
    <QueryClientProvider client={queryClient}>
      <LodestoneContext.Provider
        value={{
          core,
          setCore,
          coreList,
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
          <Routes>
            <Route element={<DashboardLayout />}>
              <Route path="/dashboard" element={<Dashboard />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="/" element={<Home />} />
            </Route>
            <Route path="/login/core/select" element={<SelectCorePage />} />
            <Route path="/login/core/new" element={<ConnectNewCorePage />} />
            <Route path="/login/user/select" element={<SelectUserPage />} />
            <Route path="/login/user/new" element={<LoginNewUserPage />} />
          </Routes>
        </NotificationContext.Provider>
      </LodestoneContext.Provider>
    </QueryClientProvider>
  );
}
