import 'globals.css';
import type { AppProps } from 'next/app';
import DashboardLayout from 'components/DashboardLayout';
import { config } from '@fortawesome/fontawesome-svg-core';
import '@fortawesome/fontawesome-svg-core/styles.css';
config.autoAddCss = false;

import { QueryClientProvider, QueryClient } from '@tanstack/react-query';

const queryClient = new QueryClient();

function MyApp({ Component, pageProps }: AppProps) {
  return (
    <QueryClientProvider client={queryClient}>
      <DashboardLayout>
        <Component {...pageProps} />
      </DashboardLayout>
    </QueryClientProvider>
  );
}

export default MyApp;
