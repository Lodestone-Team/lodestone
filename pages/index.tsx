import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  CoreConnectionInfo,
  CoreConnectionStatus,
  LodestoneContext,
} from 'data/LodestoneContext';
import {
  NotificationContext,
  useNotificationReducer,
  useOngoingNotificationReducer,
} from 'data/NotificationContext';
import React, {
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
} from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import { useLocalStorage } from 'usehooks-ts';
import { useLocalStorageQueryParam } from 'utils/hooks';
import { DEFAULT_LOCAL_CORE, errorToString, LODESTONE_PORT } from 'utils/util';
import Dashboard from 'pages/dashboard';
import Home from 'pages/home';
import axios from 'axios';
import jwt from 'jsonwebtoken';
import DashboardLayout from 'components/DashboardLayout';
import SettingsPage from 'pages/settings';
import CoreSelectExisting from 'pages/login/CoreSelectExisting';
import CoreConnect from 'pages/login/CoreConnect';
import UserSelectExisting from 'pages/login/UserSelectExisting';
import UserLogin from 'pages/login/UserLogin';
import CoreSetupNew from 'pages/login/CoreSetupNew';
import CoreConfigNew from 'pages/login/CoreConfigNew';
import LoginLayout from 'components/LoginLayout';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import NotFound from 'pages/notfound';
import FirstTime from 'pages/login/FirstTime';
import RequireCore from 'utils/router/RequireCore';
import RequireToken from 'utils/router/RequireToken';
import { toast } from 'react-toastify';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: Infinity,
      refetchOnWindowFocus: false,
    },
  },
});

export default function App() {
  const { location, setSearchParam } = useContext(BrowserLocationContext);

  /* Start Core */
  const [address, setAddress] = useLocalStorageQueryParam(
    'address',
    DEFAULT_LOCAL_CORE.address
  );
  const [port, setPort] = useLocalStorageQueryParam(
    'port',
    DEFAULT_LOCAL_CORE.port
  );
  const [protocol, setProtocol] = useLocalStorageQueryParam(
    'protocol',
    DEFAULT_LOCAL_CORE.protocol
  );
  const [apiVersion, setApiVersion] = useLocalStorageQueryParam(
    'apiVersion',
    DEFAULT_LOCAL_CORE.apiVersion
  );
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
  const [coreConnectionStatus, setCoreConnectionStatus] =
    useState<CoreConnectionStatus>('loading');
  const setCore = (c: CoreConnectionInfo) => {
    queryClient.invalidateQueries();
    queryClient.clear();
    setSearchParam('instance', undefined);
    //TODO: add core to the key of each query instead of invalidating all queries
    setAddress(c.address);
    setPort(c.port.toString());
    setProtocol(c.protocol);
    setApiVersion(c.apiVersion);
  };

  useEffect(() => {
    // we only want to add successful cores to the list
    if (coreConnectionStatus !== 'success') return;
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
  }, [address, apiVersion, core, port, protocol, coreConnectionStatus]);
  useLayoutEffect(() => {
    axios.defaults.baseURL = `${protocol}://${socket}/api/${apiVersion}`;
    setCoreConnectionStatus('loading');
  }, [apiVersion, protocol, socket]);

  // Add the core to the list if it's not already there
  // Cores with the same address and port are considered the same
  const addCore = (c: CoreConnectionInfo) => {
    if (
      coreList.some(
        (core) => core.address === c.address && core.port === c.port
      )
    )
      return;
    setCoreList([...coreList, c]);
  };
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
        toast.error(message);
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

  return (
    <QueryClientProvider client={queryClient}>
      <LodestoneContext.Provider
        value={{
          core,
          setCore,
          addCore,
          coreConnectionStatus,
          setCoreConnectionStatus,
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
            <Route element={<LoginLayout />}>
              <Route path="/first_setup" element={<FirstTime />} />
              <Route
                path="/login/core/select"
                element={
                  <RequireCore redirect="/login/core/new">
                    <CoreSelectExisting />
                  </RequireCore>
                }
              />
              <Route path="/login/core/new" element={<CoreConnect />} />
            </Route>
            <Route
              element={
                <RequireCore>
                  <LoginLayout />
                </RequireCore>
              }
            >
              <Route
                path="/login/core/first_setup"
                element={<CoreSetupNew />}
              />
              <Route
                path="/login/core/first_config"
                element={<CoreConfigNew />}
              />
              <Route
                path="/login/user/select"
                element={<UserSelectExisting />}
              />
              <Route path="/login/user" element={<UserLogin />} />
            </Route>
            <Route
              element={
                <RequireCore>
                  <RequireToken>
                    <DashboardLayout />
                  </RequireToken>
                </RequireCore>
              }
            >
              <Route path="/dashboard" element={<Dashboard />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="/" element={<Home />} />
            </Route>
            <Route path="*" element={<NotFound />} />
          </Routes>
        </NotificationContext.Provider>
      </LodestoneContext.Provider>
    </QueryClientProvider>
  );
}
