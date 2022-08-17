import 'globals.css';
import type { AppProps } from 'next/app';
import DashboardLayout from 'components/DashboardLayout';
import { config } from '@fortawesome/fontawesome-svg-core'
import '@fortawesome/fontawesome-svg-core/styles.css'
config.autoAddCss = false

import { store } from 'data/store';
import { Provider } from 'react-redux';

function MyApp({ Component, pageProps }: AppProps) {
  return (
    <Provider store={store}>
      <DashboardLayout>
        <Component {...pageProps} />
      </DashboardLayout>
    </Provider>
  );
}

export default MyApp;
