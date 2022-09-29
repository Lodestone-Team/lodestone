import 'rc-tooltip/assets/bootstrap.css';
import 'globals.css';
import type { AppProps } from 'next/app';
import { config } from '@fortawesome/fontawesome-svg-core';
import '@fortawesome/fontawesome-svg-core/styles.css';
import { QueryClientProvider, QueryClient } from '@tanstack/react-query';
import { ReactElement, ReactNode, useLayoutEffect } from 'react';
import { NextPage } from 'next';
import { LodestoneContext } from 'data/LodestoneContext';
import axios from 'axios';
import { useRouterQuery } from 'utils/hooks';
import { useCookies } from 'react-cookie';

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
  const [cookies] = useCookies(['token']);

  const protocol = 'http';
  const apiVersion = 'v1';

  // set axios defaults
  useLayoutEffect(() => {
    if (!isReady) return;
    axios.defaults.baseURL = `${protocol}://${address}:${
      port ?? 3000
    }/api/${apiVersion}`;
  }, [address, port, isReady]);

  useLayoutEffect(() => {
    if (cookies.token)
      axios.defaults.headers.common[
        'Authorization'
      ] = `Bearer ${cookies.token}`;
    else delete axios.defaults.headers.common['Authorization'];
    queryClient.invalidateQueries();
  }, [cookies.token]);

  return (
    <QueryClientProvider client={queryClient}>
      <LodestoneContext.Provider
        value={{
          address: address as string,
          port: port ?? '3000',
          protocol,
          apiVersion,
          isReady: isReady,
          token: cookies.token,
        }}
      >
        {getLayout(<Component {...pageProps} />)}
      </LodestoneContext.Provider>
    </QueryClientProvider>
  );
}

export default MyApp;
