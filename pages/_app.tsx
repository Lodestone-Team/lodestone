import 'rc-tooltip/assets/bootstrap.css';
import 'globals.css';
import type { AppProps } from 'next/app';
import { config } from '@fortawesome/fontawesome-svg-core';
import '@fortawesome/fontawesome-svg-core/styles.css';
import axios from 'axios';
import NoSSR from 'react-no-ssr';
import { BrowserRouter } from 'react-router-dom';
import {BrowserLocationContextProvider} from 'data/BrowserLocationContext';

config.autoAddCss = false;
axios.defaults.timeout = 5000;

function MyApp({ Component, pageProps }: AppProps) {
  return (
    <NoSSR>
      <BrowserRouter>
        <BrowserLocationContextProvider>
          <Component {...pageProps} />
        </BrowserLocationContextProvider>
      </BrowserRouter>
    </NoSSR>
  );
}

export default MyApp;
