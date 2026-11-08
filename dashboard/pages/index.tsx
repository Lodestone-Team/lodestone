import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  CoreConnectionInfo,
  CoreConnectionStatus,
  LodestoneContext,
} from 'data/LodestoneContext';
import { NotificationContextProvider } from 'data/NotificationContext';
import {
  Suspense,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { Routes, Route } from 'react-router';
import { useLocalStorage } from 'usehooks-ts';
import {
  coreApiBaseUrl,
  DEFAULT_LOCAL_CORE,
  errorToString,
  isAxiosError,
} from 'utils/util';
import axios from 'axios';
import { jwtDecode } from 'jwt-decode';
import CoreSelectExisting from 'pages/login/CoreSelectExisting';
import CoreConnect, { CORE_QUERY_PARAMS } from 'pages/login/CoreConnect';
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
import RequireSetup from 'utils/router/RequireSetup';
import { AppShellSkeleton } from 'components/DashboardLayout/LayoutSkeletons';
import { lazyWithPreload } from 'utils/lazyWithPreload';

// The dashboard (and everything it pulls in: charts, the file editor, forms)
// is loaded on demand, so the login screens don't wait for it.
const DashboardLayout = lazyWithPreload(
  () => import('components/DashboardLayout')
);
const InstanceViewLayout = lazyWithPreload(() =>
  import('components/DashboardLayout/InstanceViewLayout').then((m) => ({
    default: m.InstanceViewLayout,
  }))
);
const SettingsLayout = lazyWithPreload(() =>
  import('components/DashboardLayout/SettingsLayout').then((m) => ({
    default: m.SettingsLayout,
  }))
);
const Home = lazyWithPreload(() => import('pages/home'));
const InstanceTabs = lazyWithPreload(
  () => import('pages/InstanceTabs/InstanceTabs')
);
const ProfilePage = lazyWithPreload(() => import('pages/settings/profile'));
const CoreSettings = lazyWithPreload(
  () => import('pages/settings/CoreSettings')
);
const UserSettings = lazyWithPreload(
  () => import('pages/settings/UserSettings')
);

/**
 * Loads every dashboard chunk at once. Without this they load one after the
 * other (each layout only asks for the next when it renders), and every step
 * shows a loading state; once loaded, they render without suspending.
 */
const preloadDashboard = () =>
  Promise.all(
    [
      DashboardLayout,
      InstanceViewLayout,
      SettingsLayout,
      Home,
      InstanceTabs,
      ProfilePage,
      CoreSettings,
      UserSettings,
    ].map((component) => component.preload())
  ).catch(() => {
    // retried when the component renders, and its error shows there
  });

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: Infinity,
      refetchOnWindowFocus: false,
    },
  },
});

const InstanceTabList = [
  'overview',
  'settings',
  'console',
  'files',
  'mods',
  'macros',
  'logs',
  'playitgg',
];

export const CoreSettingsTabList = [
  {
    title: 'General',
    path: 'general',
    content: <CoreSettings />,
  },
  {
    title: 'Users',
    path: 'users',
    content: <UserSettings />,
  },
];

export const AccountSettingsTabList = [
  {
    title: 'Profile',
    path: 'profile',
    content: <ProfilePage />,
  },
];

// setTimeout's longest delay (about 24.8 days); longer timers fire at once
const MAX_TIMEOUT_MS = 2 ** 31 - 1;

/** The claims of a JWT, or an Error if it is malformed. Not verified. */
const decodeToken = (token: string): { uid: string; exp: number } | Error => {
  let payload: { exp?: unknown; uid?: unknown };
  try {
    payload = jwtDecode(token);
  } catch {
    return new Error('Invalid token');
  }
  if (typeof payload !== 'object' || payload === null)
    return new Error('Invalid token');
  if (typeof payload.exp !== 'number') return new Error('Invalid exp in token');
  if (typeof payload.uid !== 'string') return new Error('Invalid uid in token');
  return { uid: payload.uid, exp: payload.exp };
};

const sameCore = (a: CoreConnectionInfo, b: CoreConnectionInfo) =>
  a.address === b.address &&
  a.port === b.port &&
  a.protocol === b.protocol &&
  a.apiVersion === b.apiVersion;

export default function App() {
  const { location, searchParams, setLocation, setSearchParam } = useContext(
    BrowserLocationContext
  );

  /* Start Core */
  // The core is only read from localStorage. A link can't switch the core
  // silently, see the linked core effect below.
  const [address, setAddress] = useLocalStorage(
    'address',
    DEFAULT_LOCAL_CORE.address
  );
  const [port, setPort] = useLocalStorage('port', DEFAULT_LOCAL_CORE.port);
  const [protocol, setProtocol] = useLocalStorage(
    'protocol',
    DEFAULT_LOCAL_CORE.protocol
  );
  const [apiVersion, setApiVersion] = useLocalStorage(
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
  const setCore = useCallback(
    (c: CoreConnectionInfo) => {
      queryClient.invalidateQueries();
      queryClient.clear();
      setSearchParam('instance', undefined);
      //TODO: add core to the key of each query instead of invalidating all queries
      setAddress(c.address);
      setPort(c.port.toString());
      setProtocol(c.protocol);
      setApiVersion(c.apiVersion);
    },
    [setSearchParam, setAddress, setPort, setProtocol, setApiVersion]
  );

  // Links with ?address=&port=&protocol=&apiVersion= (older dashboards put the
  // core in every URL) never switch the core by themselves. If they point to
  // a different core, the "add core" form opens with the values filled in and
  // the user decides; if they match the current core they are just removed.
  useEffect(() => {
    if (!CORE_QUERY_PARAMS.some((key) => searchParams.has(key))) return;
    const linkedCore: CoreConnectionInfo = {
      address: searchParams.get('address') || address,
      port: searchParams.get('port') || port,
      protocol: searchParams.get('protocol') || protocol,
      apiVersion: searchParams.get('apiVersion') || apiVersion,
    };
    if (sameCore(linkedCore, core)) {
      setLocation((loc) => {
        const search = new URLSearchParams(loc.search);
        CORE_QUERY_PARAMS.forEach((key) => search.delete(key));
        return { ...loc, search: search.toString() };
      }, true);
    } else if (location.pathname !== '/login/core/new') {
      // CoreConnect reads the values and removes them from the URL
      setLocation((loc) => ({ ...loc, pathname: '/login/core/new' }), true);
    }
  }, [
    searchParams,
    location.pathname,
    core,
    address,
    port,
    protocol,
    apiVersion,
    setLocation,
  ]);

  useEffect(() => {
    // we only want to add successful cores to the list
    if (coreConnectionStatus !== 'success') return;
    // check if core is already in the list
    // if it's exactly the same, do nothing
    if (coreList.some((c) => sameCore(c, core))) return;
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
    axios.defaults.baseURL = coreApiBaseUrl(core);
    setCoreConnectionStatus('loading');
  }, [core]);

  // Add the core to the list if it's not already there
  // Cores with the same address and port are considered the same
  const addCore = useCallback(
    (c: CoreConnectionInfo) => {
      setCoreList((list) =>
        list.some((core) => core.address === c.address && core.port === c.port)
          ? list
          : [...list, c]
      );
    },
    [setCoreList]
  );
  /* End Core */

  /* Start Token */
  const [tokens, setTokens] = useLocalStorage<Record<string, string>>(
    'tokens',
    {}
  ); //TODO: clear all outdated tokens
  const token = tokens[socket] ?? '';
  const claims = useMemo(() => (token ? decodeToken(token) : null), [token]);
  // empty when logged out, so it can't outlive the token
  const uid = claims && !(claims instanceof Error) ? claims.uid : '';
  // the token and core the query cache holds data for
  const lastSession = useRef<string | null>(null);
  const setToken = useCallback(
    (token: string, coreSocket: string) => {
      // update from the latest map, not the one from this render
      setTokens((oldTokens) => ({ ...oldTokens, [coreSocket]: token }));
    },
    [setTokens]
  );
  useLayoutEffect(() => {
    let logoutTimer: ReturnType<typeof setTimeout> | undefined;
    if (!token || !claims) {
      delete axios.defaults.headers.common['Authorization'];
    } else {
      try {
        if (claims instanceof Error) throw claims;
        const msUntilExpiry = claims.exp * 1000 - Date.now();
        if (msUntilExpiry <= 0) throw new Error('Token expired');
        axios.defaults.headers.common['Authorization'] = `Bearer ${token}`;
        // log out when the token expires, instead of failing every request
        if (msUntilExpiry < MAX_TIMEOUT_MS)
          logoutTimer = setTimeout(() => {
            toast.warn('Your session has expired, please sign in again', {
              toastId: 'sessionExpired',
            });
            setToken('', socket);
          }, msUntilExpiry);
      } catch (e) {
        const message = errorToString(e);
        toast.error(message);
        setToken('', socket);
        delete axios.defaults.headers.common['Authorization'];
      }
    }
    // Drop the data of the previous user or core. Not on the first run: the
    // cache only holds queries the first render just made for this token, and
    // clearing would orphan them, so the next component asking for the same
    // data would load it again (showing its loading state a second time).
    const session = `${token}@${socket}`;
    if (lastSession.current !== null && lastSession.current !== session) {
      queryClient.invalidateQueries();
      queryClient.clear();
    }
    lastSession.current = session;
    return () => {
      if (logoutTimer !== undefined) clearTimeout(logoutTimer);
    };
    // token and socket only: claims derives from token and setToken is stable
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, socket]);

  // Signed in: fetch the dashboard's chunks now, in parallel, so it doesn't
  // load them one by one (with a loading state for each) when it opens.
  useEffect(() => {
    if (token) void preloadDashboard();
  }, [token]);

  // Log out when the core rejects the token (expired, revoked, or the user was
  // deleted). The core also answers 401 for some requests that are not about
  // the token (e.g. a wrong old password), so unless the failed request was
  // /user/info itself, confirm with /user/info before logging out.
  useEffect(() => {
    if (!token) return;
    const bearer = `Bearer ${token}`;
    let verifying: Promise<void> | null = null;
    const logout = () => {
      toast.warn('Your session is no longer valid, please sign in again', {
        toastId: 'sessionExpired',
      });
      setToken('', socket);
    };
    const id = axios.interceptors.response.use(undefined, (error) => {
      if (
        isAxiosError(error) &&
        error.response?.status === 401 &&
        error.config?.headers?.Authorization === bearer
      ) {
        if (error.config.url === '/user/info') {
          logout();
        } else if (!verifying) {
          verifying = axios
            .get('/user/info')
            .then(
              () => undefined,
              () => undefined // a 401 here logs out through this interceptor
            )
            .finally(() => {
              verifying = null;
            });
        }
      }
      return Promise.reject(error);
    });
    return () => {
      axios.interceptors.response.eject(id);
    };
  }, [token, socket, setToken]);
  /* End Token */

  const lodestoneContextValue = useMemo(
    () => ({
      core,
      setCore,
      addCore,
      coreConnectionStatus,
      setCoreConnectionStatus,
      coreList,
      token,
      uid,
      setToken,
      tokens,
    }),
    [
      core,
      setCore,
      addCore,
      coreConnectionStatus,
      coreList,
      token,
      uid,
      setToken,
      tokens,
    ]
  );

  return (
    <QueryClientProvider client={queryClient}>
      <LodestoneContext.Provider value={lodestoneContextValue}>
        <NotificationContextProvider>
          <Suspense fallback={<AppShellSkeleton />}>
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
                  element={
                    <RequireSetup>
                      <UserSelectExisting />
                    </RequireSetup>
                  }
                />
                <Route
                  path="/login/user"
                  element={
                    <RequireSetup>
                      <UserLogin />
                    </RequireSetup>
                  }
                />
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
                <Route element={<InstanceViewLayout />}>
                  {InstanceTabList.map((path, i) => (
                    <Route
                      path={`/dashboard/${path}`}
                      element={<InstanceTabs />}
                      key={i}
                    />
                  ))}
                  <Route path="/" element={<Home />} />
                </Route>
                <Route element={<SettingsLayout />}>
                  {CoreSettingsTabList.map((tab, i) => (
                    <Route
                      path={`/settings/${tab.path}`}
                      element={tab.content}
                      key={i}
                    />
                  ))}
                  {AccountSettingsTabList.map((tab, i) => (
                    <Route
                      path={`/settings/${tab.path}`}
                      element={tab.content}
                      key={i}
                    />
                  ))}
                </Route>
              </Route>
              <Route path="*" element={<NotFound />} />
            </Routes>
          </Suspense>
        </NotificationContextProvider>
      </LodestoneContext.Provider>
    </QueryClientProvider>
  );
}
